//! Adversarial L1 critic probes for `thermite-lower::l1` (issue #4, commit
//! `c0b1d8a`). Each test pins an authority-derived expectation from
//! `.design/lower/l1-runtime-checks.md` / `thermite-design.md` §6 / `goal.md`
//! R-DEFER-9 (R-CHAR-3 — expected values hand-derived or from the corpus, never
//! copied from toolchain output). These are NOT in the builder's
//! `l1_conformance.rs`; they probe corners that file may have missed:
//! a REAL runtime violation on a fresh program (not the corpus corrupted body),
//! release-profile (`-O`) check survival, generality on a renamed program, and
//! combinator edge cases (`exists_in` short-circuit, `n > len`, `permutation_of`
//! duplicates).
//!
//! `rustc` is always installed (L1 is rustc-only). The `.` in a `*.l1.rs`
//! filename breaks crate-name derivation, so we always pass `--crate-name`.
//! `unwrap`/`expect`/`panic!` are fine here — `tests/` is not anti-pattern-gated.
//!
//! NOTE: `thermite-syntax` enforces clause order `req` before `ens` before `fx`
//! (`ClauseOrder` error otherwise), so every fresh probe fn carries a leading
//! `req` (a trivially-true one) before its `ens`.

use std::path::PathBuf;
use std::process::Command;

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("conformance")
}

/// Compile `src` with `rustc` at the given opt level, then RUN. Returns
/// `(compiled_ok, ran_ok, combined_output)`. `opt` is e.g. `&[]` (debug) or
/// `&["-O"]` (release-equivalent optimization).
fn compile_and_run_opt(src: &str, crate_name: &str, opt: &[&str]) -> (bool, bool, String) {
    let dir = std::env::temp_dir();
    let rs = dir.join(format!("{crate_name}.l1.rs"));
    let bin = dir.join(crate_name);
    std::fs::write(&rs, src).unwrap_or_else(|e| panic!("write temp {crate_name}: {e}"));
    let mut cmd = Command::new("rustc");
    cmd.arg("--crate-name")
        .arg(crate_name)
        .arg("--edition")
        .arg("2021");
    for o in opt {
        cmd.arg(o);
    }
    let comp = cmd
        .arg(&rs)
        .arg("-o")
        .arg(&bin)
        .current_dir(&dir)
        .output()
        .unwrap_or_else(|e| panic!("rustc failed for {crate_name}: {e}"));
    let mut combined = String::from_utf8_lossy(&comp.stdout).to_string();
    combined.push_str(&String::from_utf8_lossy(&comp.stderr));
    if !comp.status.success() {
        return (false, false, combined);
    }
    let run = Command::new(&bin)
        .current_dir(&dir)
        .output()
        .unwrap_or_else(|e| panic!("running {crate_name} failed: {e}"));
    combined.push_str(&String::from_utf8_lossy(&run.stdout));
    combined.push_str(&String::from_utf8_lossy(&run.stderr));
    (true, run.status.success(), combined)
}

fn lower_str(src: &str) -> String {
    let parsed = thermite_syntax::parse(src);
    assert!(
        parsed.errors.is_empty(),
        "probe must parse clean: {:?}",
        parsed.errors
    );
    thermite_lower::lower_l1(&parsed.program).unwrap_or_else(|e| panic!("lower_l1 failed: {e}"))
}

// ---------------------------------------------------------------------------
// Divergence probe 1 — the L1 check fires on a REAL violation of a FRESH
// program (not the corpus corrupted-body). Authority: l1-runtime-checks.md
// REQ-2 / AC-1 ("a violating body fires the violation handler … observable, not
// silent"); §6 ("Violations detected at the call site"); R-DEFER-9 (the check
// must be a real obligation, not a no-op).
//
// Program: a fn whose `ens result == 0` is VIOLATED by a body returning its
// input. (Clause order req-before-ens satisfied with a trivially-true `req`.)
// For input 7 the body returns 7, so the `ens` check must abort.
// ---------------------------------------------------------------------------
#[test]
fn fresh_program_ens_violation_aborts() {
    let src = r#"
fn identity_should_be_zero(n: u32) -> u32
  req n <= 1_000_000
  ens result == 0
  fx  pure
{
  n
}
"#;
    let emitted = lower_str(src);
    // The `ens` must lower to an always-active check on `result == 0`.
    assert!(
        emitted.contains("thermite_check!(\"ens\", \"result == 0\", result == 0)"),
        "ens clause must lower to an always-active check on `result == 0`:\n{emitted}"
    );
    // main calls with a value that VIOLATES the ens (7 != 0) -> must abort.
    let program = format!(
        "{emitted}\nfn main() {{\n    let _ = identity_should_be_zero(7u32);\n    println!(\"should-not-reach\");\n}}\n"
    );
    let (compiled, ran, out) = compile_and_run_opt(&program, "fresh_ens_neg", &[]);
    assert!(
        compiled,
        "fresh L1 program must COMPILE (only the runtime check fires):\n{out}"
    );
    assert!(
        !ran,
        "the violated `ens result == 0` (body returns 7) must ABORT at the L1 check, not run clean:\n{out}"
    );
    assert!(
        out.contains("thermite L1 contract violation [ens]"),
        "the violation handler must fire with a structured [ens] diagnostic:\n{out}"
    );
    assert!(
        !out.contains("should-not-reach"),
        "execution must abort before main's tail — the L1 check genuinely detects the violation:\n{out}"
    );
}

// ---------------------------------------------------------------------------
// Divergence probe 2 — the always-active check survives a RELEASE / optimized
// build (`-O`). Authority: l1-runtime-checks.md AC-2 / REQ-2 ("present in a
// --release build"); §6 ("in every build profile, not just debug"). A
// `debug_assert!` would be stripped under `-O`; the check must still fire.
// ---------------------------------------------------------------------------
#[test]
fn check_fires_under_release_optimization() {
    let src = r#"
fn must_be_zero(n: u32) -> u32
  req n <= 1_000_000
  ens result == 0
  fx  pure
{
  n
}
"#;
    let emitted = lower_str(src);
    let program = format!(
        "{emitted}\nfn main() {{\n    let _ = must_be_zero(5u32);\n    println!(\"should-not-reach\");\n}}\n"
    );
    let (compiled, ran, out) = compile_and_run_opt(&program, "rel_ens_neg", &["-O"]);
    assert!(compiled, "optimized L1 program must COMPILE:\n{out}");
    assert!(
        !ran && out.contains("thermite L1 contract violation [ens]"),
        "the always-active check MUST still fire under -O (a debug_assert would be stripped; §6):\n{out}"
    );
}

// ---------------------------------------------------------------------------
// Divergence probe 3 — generality: a DIFFERENT program than `sum` lowers to L1
// referencing the NEW names + its OWN clauses (no over-fit to sum/spec_sum/xs).
// Authority: l1-runtime-checks.md REQ-1/REQ-4 (general emitter over the AST);
// goal.md R-CHAR-3 (the emitter must not be a hardcoded `sum` blob). The
// emitted file must compile+run and the renamed clauses must be present verbatim.
// ---------------------------------------------------------------------------
#[test]
fn renamed_accumulator_lowers_to_its_own_names() {
    let src = r#"
spec fn spec_tally(zs: &[u32]) -> u64
  dec zs.len()
{
  match zs {
    []          => 0,
    [head, ..t] => head as u64 + spec_tally(t),
  }
}

fn tally(zs: &[u32]) -> u64
  req zs.len() <= 50
  ens result == spec_tally(zs)
  fx  pure
{
  let mut total: u64 = 0;
  let mut k: usize = 0;
  while k < zs.len()
    inv k <= zs.len()
    inv total == spec_tally(&zs[..k])
    dec zs.len() - k
  {
    total = total + zs[k] as u64;
    k = k + 1;
  }
  total
}
"#;
    let emitted = lower_str(src);
    // The NEW identifiers/clauses must appear; the `sum` corpus identifiers must NOT.
    for needle in [
        "fn tally(zs: &[u32]) -> u64",
        "fn spec_tally(zs: &[u32]) -> u64",
        "thermite_check!(\"req\", \"zs.len() <= 50\", zs.len() <= 50)",
        "thermite_check!(\"inv\", \"total == spec_tally(&zs[..k])\", total == spec_tally(&zs[..k]))",
        "thermite_check!(\"ens\", \"result == spec_tally(zs)\", result == spec_tally(zs))",
    ] {
        assert!(
            emitted.contains(needle),
            "expected renamed L1 fragment `{needle}`:\n{emitted}"
        );
    }
    assert!(
        !emitted.contains("fn sum(") && !emitted.contains("spec_sum"),
        "L1 emitter must not emit a hardcoded `sum`/`spec_sum` blob for a different program:\n{emitted}"
    );
    // And it must actually compile + run: tally(&[1,2,3]) == 6 (hand-derived).
    let program = format!(
        "{emitted}\nfn main() {{\n    assert_eq!(tally(&[1u32, 2, 3]), 6);\n    assert_eq!(tally(&[]), 0);\n    assert_eq!(spec_tally(&[4u32, 5]), 9);\n    println!(\"tally-ok\");\n}}\n"
    );
    let (compiled, ran, out) = compile_and_run_opt(&program, "tally_l1", &[]);
    assert!(
        compiled,
        "renamed L1 program must COMPILE:\n{out}\n--- {program}"
    );
    assert!(
        ran && out.contains("tally-ok"),
        "renamed L1 program must run clean:\n{out}"
    );
}

// ---------------------------------------------------------------------------
// Divergence probe 4 — combinator L1 forms agree with the L3 quantifier on
// edge cases the builder's unit test may have under-covered. Authority:
// l1-runtime-checks.md REQ-3 Architecture (the frozen `l1` bodies) + the
// verus_l3 quantifier each mirrors. Expected bools hand-derived (R-CHAR-3).
//
// Edges: forall_below with n > len (clamped by `i < n && i < s.len()`),
// forall_from with n >= len (vacuous), exists_in short-circuit, permutation_of
// with duplicates and length mismatch, disjoint with an empty side.
// ---------------------------------------------------------------------------
#[test]
fn combinator_l1_edge_cases_match_l3() {
    let mut src = String::new();
    for sig in thermite_spec::all() {
        src.push_str(sig.l1);
        src.push_str("\n\n");
    }
    // Hand-derived from the L3 quantifier semantics (verus_l3), NOT toolchain output.
    src.push_str(
        r#"
fn main() {
    // forall_below clamps to min(n, len): n > len checks only the existing elems.
    assert!(forall_below(&[1u32, 2], 9, |x| x < 5));        // both < 5, n clamped
    assert!(!forall_below(&[1u32, 9], 9, |x| x < 5));       // 9 fails
    // forall_from with n >= len is vacuously true (no elements from n onward).
    assert!(forall_from(&[1u32, 2, 3], 3, |_x| false));     // n == len: vacuous
    assert!(forall_from(&[1u32, 2, 3], 9, |_x| false));     // n > len: vacuous
    // exists_in short-circuits at the first match.
    assert!(exists_in(&[5u32, 6, 7], |x| x == 5));
    assert!(!exists_in(&[5u32, 6, 7], |x| x == 99));
    // permutation_of: duplicates must be multiset-counted, not set-counted.
    assert!(permutation_of(&[1u32, 1, 2], &[2u32, 1, 1]));  // same multiset
    assert!(!permutation_of(&[1u32, 1, 2], &[1u32, 2, 2])); // diff multiset, same set
    assert!(!permutation_of(&[1u32, 2], &[1u32, 2, 3]));    // length mismatch
    // disjoint with empty side is trivially disjoint.
    assert!(disjoint(&[], &[1u32, 2]));
    assert!(disjoint(&[1u32, 2], &[]));
    // count_where over all-matching and none-matching.
    assert_eq!(count_where(&[2u32, 4, 6, 8], |x| x % 2 == 0), 4);
    assert_eq!(count_where(&[1u32, 3, 5], |x| x % 2 == 0), 0);
    // sorted with equal adjacent (non-strict) is sorted.
    assert!(sorted(&[1u32, 1, 1]));
    assert!(!sorted(&[2u32, 1]));
    println!("edges-ok");
}
"#,
    );
    let (compiled, ran, out) = compile_and_run_opt(&src, "combinator_edges", &[]);
    assert!(
        compiled,
        "registry L1 combinator forms must COMPILE:\n{out}\n--- {src}"
    );
    assert!(
        ran && out.contains("edges-ok"),
        "a registry L1 combinator form diverges from its L3 quantifier on an edge case:\n{out}"
    );
}

// ---------------------------------------------------------------------------
// Divergence probe 5 — REQ-4: the L1 spec fn is REAL recursion that agrees with
// the L3 `Seq` denotation on a multi-element input, not a faked constant.
// Authority: l1-runtime-checks.md AC-4 / REQ-4 (§4.2 "spec functions are
// executable"). Expected 1+2+3+4 == 10 hand-derived (R-CHAR-3).
// ---------------------------------------------------------------------------
#[test]
fn spec_sum_is_real_recursion() {
    let src = std::fs::read_to_string(corpus_dir().join("sum.th")).unwrap();
    let parsed = thermite_syntax::parse(&src);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let emitted = thermite_lower::lower_l1(&parsed.program).unwrap();
    let program = format!(
        "{emitted}\nfn main() {{\n    assert_eq!(spec_sum(&[1u32, 2, 3, 4]), 10);\n    assert_eq!(spec_sum(&[]), 0);\n    assert_eq!(spec_sum(&[9u32]), 9);\n    println!(\"specsum-ok\");\n}}\n"
    );
    let (compiled, ran, out) = compile_and_run_opt(&program, "specsum_real", &[]);
    assert!(compiled, "spec_sum L1 must compile:\n{out}");
    assert!(
        ran && out.contains("specsum-ok"),
        "L1 spec_sum must be real recursion agreeing with the L3 Seq denotation (1+2+3+4==10):\n{out}"
    );
}
