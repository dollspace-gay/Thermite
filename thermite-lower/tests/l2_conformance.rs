//! Conformance test for `thermite-lower`'s L2 Kani-harness emission against the
//! EXTERNAL truth: the real `cargo kani 0.67.0` binary
//! (`.design/lower/l2-kani.md` AC-1/AC-2/AC-3/AC-5). For each corpus program:
//! parse it, `lower_l2` it, write the emitted harness into a throwaway cargo
//! crate, run `cargo kani --output-format terse`, and assert
//! `VERIFICATION:- SUCCESSFUL` (the contract holds for ALL inputs up to the
//! bound) — at the design-pinned bound `N = 4` (`unwind(5)` for `sum`'s
//! `while`, `unwind(6)` for `binary_search`'s `loop`).
//!
//! `cargo kani` is a HEAVY external toolchain (its own nightly + CBMC), so the
//! kani-spawning tests SKIP LOUDLY (a diagnostic + early return, NOT `#[ignore]`)
//! when kani is absent, mirroring the verus-absent skip in `lower_conformance.rs`
//! (`.design/lower/l2-kani.md` REQ-8). The PURE emitter shape assertions (no kani
//! spawn) run unconditionally. Expected `VERIFICATION:- SUCCESSFUL` / the
//! counterexample markers trace to the grounded real-kani runs (R-CHAR-3 — Kani's
//! own format, not forge's output). `unwrap`/`expect` are fine here (`tests/` is
//! not anti-pattern-gated).

use std::path::PathBuf;
use std::process::Command;

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("conformance")
}

fn lower_corpus_l2(name: &str) -> String {
    let src = std::fs::read_to_string(corpus_dir().join(format!("{name}.th")))
        .unwrap_or_else(|e| panic!("cannot read corpus {name}.th: {e}"));
    let parsed = thermite_syntax::parse(&src);
    assert!(
        parsed.errors.is_empty(),
        "corpus {name}.th must parse clean: {:?}",
        parsed.errors
    );
    thermite_lower::lower_l2(&parsed.program)
        .unwrap_or_else(|e| panic!("L2 lowering {name}.th failed: {e}"))
}

/// Locate the kani plugin binary: `KANI_BIN` override, then PATH (`which
/// cargo-kani`), then `~/.cargo/bin/cargo-kani`. Returns `None` if kani is
/// genuinely absent so the kani-dependent assertions SKIP LOUDLY rather than
/// panic (the suite must run without kani; REQ-8). kani IS present in the build
/// environment here, so the live paths run.
fn kani_bin() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("KANI_BIN") {
        let pb = PathBuf::from(p);
        if pb.exists() {
            return Some(pb);
        }
    }
    if let Ok(out) = Command::new("which").arg("cargo-kani").output() {
        if out.status.success() {
            let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !p.is_empty() {
                return Some(PathBuf::from(p));
            }
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        let cand = PathBuf::from(home).join(".cargo/bin/cargo-kani");
        if cand.exists() {
            return Some(cand);
        }
    }
    None
}

/// Write `harness` into a throwaway cargo crate under the temp dir and run
/// `cargo kani --output-format terse` in it. Returns the combined stdout+stderr,
/// or `None` if kani is unavailable (caller SKIPs). The crate is removed after.
fn run_kani_on(harness: &str, stem: &str) -> Option<String> {
    let bin = kani_bin()?;
    let crate_dir = std::env::temp_dir().join(format!("l2_conf_{stem}_{}", std::process::id()));
    let src_dir = crate_dir.join("src");
    std::fs::create_dir_all(&src_dir).expect("create temp crate");
    std::fs::write(
        crate_dir.join("Cargo.toml"),
        format!(
            "[package]\nname = \"{stem}\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n\
             [lib]\npath = \"src/lib.rs\"\n\n[workspace]\n"
        ),
    )
    .expect("write Cargo.toml");
    std::fs::write(src_dir.join("lib.rs"), harness).expect("write lib.rs");

    let out = Command::new(bin)
        .arg("--output-format")
        .arg("terse")
        .current_dir(&crate_dir)
        .output()
        .ok();
    let combined = out.map(|o| {
        let mut s = String::from_utf8_lossy(&o.stdout).to_string();
        s.push_str(&String::from_utf8_lossy(&o.stderr));
        s
    });
    let _ = std::fs::remove_dir_all(&crate_dir);
    combined
}

// ---- AC-1: sum harness verifies UP TO BOUND ------------------------------

#[test]
fn sum_harness_verifies_to_bound() {
    let harness = lower_corpus_l2("sum");
    // Pure emitter shape (always runs): the design-pinned harness.
    assert!(harness.contains("#[kani::proof]"));
    assert!(
        harness.contains("#[kani::unwind(5)]"),
        "sum's while-over-slice → unwind N+1 = 5:\n{harness}"
    );
    assert!(harness.contains("assert!(result == spec_sum(xs));"));

    match run_kani_on(&harness, "l2sum") {
        Some(output) => assert!(
            output.contains("VERIFICATION:- SUCCESSFUL"),
            "kani on emitted sum harness did NOT verify to bound (AC-1):\n{output}"
        ),
        None => eprintln!(
            "SKIP: kani not available — L2 verification of emitted `sum` not run \
             (set KANI_BIN or install cargo kani); emitter-shape asserts still ran."
        ),
    }
}

// ---- AC-2: binary_search harness verifies UP TO BOUND --------------------

#[test]
fn binary_search_harness_verifies_to_bound() {
    let harness = lower_corpus_l2("binary_search");
    assert!(
        harness.contains("#[kani::unwind(6)]"),
        "binary_search's unconditional loop → unwind N+2 = 6:\n{harness}"
    );
    assert!(
        harness.contains("kani::assume(sorted(haystack));"),
        "the req (sorted) is assumed:\n{harness}"
    );

    match run_kani_on(&harness, "l2bs") {
        Some(output) => assert!(
            output.contains("VERIFICATION:- SUCCESSFUL"),
            "kani on emitted binary_search harness did NOT verify to bound (AC-2):\n{output}"
        ),
        None => eprintln!(
            "SKIP: kani not available — L2 verification of emitted `binary_search` not run."
        ),
    }
}

// ---- AC-3: a broken contract → counterexample → NOT L2 -------------------

#[test]
fn broken_contract_yields_counterexample_not_l2() {
    // Mutate the emitted `sum` body's `i = i + 1;` to `i = i + 2;` (the grounded
    // off-by mutation, AC-3). The `ens` `result == spec_sum(xs)` then fails for a
    // concrete symbolic input — a Kani counterexample, never a false L2 pass.
    let harness = lower_corpus_l2("sum");
    let mutated = harness.replacen("i = i + 1;", "i = i + 2;", 1);
    assert_ne!(mutated, harness, "the mutation must have applied");

    match run_kani_on(&mutated, "l2sumbroken") {
        Some(output) => {
            assert!(
                output.contains("VERIFICATION:- FAILED"),
                "the broken-contract harness must FAIL (not a false pass):\n{output}"
            );
            assert!(
                !output.contains("VERIFICATION:- SUCCESSFUL"),
                "a broken contract must NOT verify:\n{output}"
            );
            assert!(
                output.contains("Failed Checks: assertion failed: result == spec_sum(xs)"),
                "the counterexample names the failed `ens` (AC-3):\n{output}"
            );
        }
        None => eprintln!("SKIP: kani not available — broken-contract counterexample not run."),
    }
}

// ---- AC-5: an under-bound unwind is a REPORTED failure, never a false pass ---

#[test]
fn under_bound_is_reported_failure_not_false_pass() {
    // Force an under-bound (unwind 2) on binary_search's loop: CBMC reports
    // `unwinding assertion loop 0` (a reported non-L2 failure), never a spurious
    // L2 (AC-5). The emitter's K = N+2 rule (REQ-3) avoids this for the corpus.
    let harness = lower_corpus_l2("binary_search");
    let under = harness.replacen("#[kani::unwind(6)]", "#[kani::unwind(2)]", 1);
    assert_ne!(under, harness, "the unwind under-bound must have applied");

    match run_kani_on(&under, "l2bsunder") {
        Some(output) => {
            assert!(
                output.contains("VERIFICATION:- FAILED"),
                "an under-bound run must FAIL, never a spurious L2 (AC-5):\n{output}"
            );
            assert!(
                output.contains("unwinding assertion"),
                "the under-bound failure is an unwinding assertion (AC-5):\n{output}"
            );
        }
        None => eprintln!("SKIP: kani not available — under-bound assertion not run."),
    }
}

// ---- AC-4: the bound is TYPE-derived, not name-derived (pure, no kani) ----

#[test]
fn bound_is_type_derived_not_name_derived() {
    // A synthetic `fn f(xs: &[u32], k: u32)`: the SAME slice scaffolding the
    // corpus uses + an unbounded `kani::any()` for the scalar `k` — no name check.
    let src = "fn f(xs: &[u32], k: u32) -> u64\n  req k < 10\n  ens result == k as u64\n  fx pure\n{\n  k as u64\n}\n";
    let parsed = thermite_syntax::parse(src);
    assert!(
        parsed.errors.is_empty(),
        "synthetic parses: {:?}",
        parsed.errors
    );
    let harness = thermite_lower::lower_l2(&parsed.program).expect("lower_l2");
    assert!(harness.contains("const N: usize = 4;"));
    assert!(harness.contains("kani::assume(xs_len <= N);"));
    assert!(harness.contains("let xs_data: [u32; N] = kani::any();"));
    assert!(
        harness.contains("let k: u32 = kani::any();"),
        "the scalar `k` is a type-driven unbounded symbolic value:\n{harness}"
    );
}

// ---- AC-9: an un-lowerable construct → Err(LowerError), never a panic -----

#[test]
fn unlowerable_is_err_not_panic() {
    // `&u32` (reference-to-scalar) has no L2 symbolic-input inference → Unsupported.
    let src = "fn f(p: &u32) -> u32\n  req true\n  ens result == 0\n  fx pure\n{\n  0\n}\n";
    let parsed = thermite_syntax::parse(src);
    assert!(parsed.errors.is_empty());
    let r = thermite_lower::lower_l2(&parsed.program);
    assert!(
        r.is_err(),
        "un-lowerable param is an Err, not a panic: {r:?}"
    );
}

// ---- determinism: two lowerings of the same program are byte-identical ----

#[test]
fn lowering_is_deterministic() {
    let a = lower_corpus_l2("sum");
    let b = lower_corpus_l2("sum");
    assert_eq!(
        a, b,
        "L2 lowering is a pure function of the program (R-CODE-5)"
    );
}
