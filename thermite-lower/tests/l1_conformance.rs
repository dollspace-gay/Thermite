//! Conformance test for `thermite-lower`'s L1 runtime-check emission against the
//! EXTERNAL truth: the real `rustc` compiler (the emitted L1 file must COMPILE
//! and RUN; the always-active contract checks must FIRE on violation) and the
//! hand-authored golden oracle `tests/golden/l1/sum.l1.rs`
//! (`.design/lower/l1-runtime-checks.md`). Verification is by EXECUTION, not a
//! strict byte-match (the design AC-6/AC-1: "compiles + runs + checks fire").
//!
//! `rustc` is always installed (no skip). The `.` in a `*.l1.rs` filename breaks
//! rustc's crate-name derivation, so we always pass `--crate-name`. `unwrap`/
//! `expect`/`panic!` are fine here — `tests/` is not anti-pattern-gated.

use std::path::PathBuf;
use std::process::Command;

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("conformance")
}

fn lower_corpus_l1(name: &str) -> String {
    let src = std::fs::read_to_string(corpus_dir().join(format!("{name}.th")))
        .unwrap_or_else(|e| panic!("cannot read corpus {name}.th: {e}"));
    let parsed = thermite_syntax::parse(&src);
    assert!(
        parsed.errors.is_empty(),
        "corpus {name}.th must parse clean (thermite-syntax bug otherwise): {:?}",
        parsed.errors
    );
    thermite_lower::lower_l1(&parsed.program)
        .unwrap_or_else(|e| panic!("L1 lowering {name}.th failed: {e}"))
}

/// Compile `src` (a self-contained Rust program incl. a `main`) with `rustc`
/// into `crate_name` under the temp dir, then RUN it. Returns
/// `(compiled_ok, ran_ok, combined_output)`. Always passes `--crate-name`
/// (the `.l1.rs` dotted filename gotcha) and `--edition 2021`.
fn compile_and_run(src: &str, crate_name: &str) -> (bool, bool, String) {
    let dir = std::env::temp_dir();
    let rs = dir.join(format!("{crate_name}.l1.rs"));
    let bin = dir.join(crate_name);
    std::fs::write(&rs, src).unwrap_or_else(|e| panic!("write temp {crate_name}: {e}"));

    let comp = Command::new("rustc")
        .arg("--crate-name")
        .arg(crate_name)
        .arg("--edition")
        .arg("2021")
        .arg(&rs)
        .arg("-o")
        .arg(&bin)
        .current_dir(&dir)
        .output()
        .unwrap_or_else(|e| panic!("rustc invocation failed for {crate_name}: {e}"));
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

/// Append a positive test harness (`main`) to the emitted L1 source. Expected
/// values are hand-derived from the L3 `Seq` denotation (`sum(&[1,2,3]) == 6`,
/// AC-1/AC-4 — R-CHAR-3), NOT copied from the toolchain.
fn with_positive_main(emitted: &str) -> String {
    format!(
        "{emitted}\nfn main() {{\n    assert_eq!(sum(&[1u32, 2, 3]), 6);\n    assert_eq!(sum(&[]), 0);\n    assert_eq!(sum(&[7u32]), 7);\n    assert_eq!(spec_sum(&[1u32, 2, 3]), 6);\n    println!(\"ok\");\n}}\n"
    )
}

// ---- AC-1 / REQ-1..REQ-4 / REQ-6: sum L1 compiles, runs, checks pass --------

#[test]
fn sum_l1_compiles_and_runs() {
    let emitted = lower_corpus_l1("sum");
    let program = with_positive_main(&emitted);
    let (compiled, ran, out) = compile_and_run(&program, "sum_l1");
    assert!(
        compiled,
        "emitted sum L1 did NOT compile under rustc:\n{out}\n--- emitted ---\n{program}"
    );
    assert!(
        ran && out.contains("ok"),
        "emitted sum L1 compiled but did NOT run clean (positive: sum(&[1,2,3])==6, checks pass):\n{out}"
    );
}

// ---- AC-1 (negative) / REQ-2: a violating body fires the violation handler --

#[test]
fn negative_fixture_fires_violation() {
    // Corrupt the body so the accumulator over-counts: `ens result ==
    // spec_sum(xs)` is then VIOLATED at runtime and the always-active handler
    // must fire (a non-zero exit / panic), observable — not silent.
    let emitted = lower_corpus_l1("sum");
    assert!(
        emitted.contains("acc = acc + xs[i] as u64;"),
        "expected the accumulator assignment in the emitted body:\n{emitted}"
    );
    let corrupted = emitted.replace("acc = acc + xs[i] as u64;", "acc = acc + xs[i] as u64 + 1;");
    // main calls sum on a non-empty slice so the corrupted fold diverges from
    // spec_sum and the `ens` check fires.
    let program = format!("{corrupted}\nfn main() {{\n    let _ = sum(&[1u32, 2, 3]);\n    println!(\"should-not-reach\");\n}}\n");
    let (compiled, ran, out) = compile_and_run(&program, "sum_l1_neg");
    assert!(
        compiled,
        "corrupted sum L1 must still COMPILE (only the runtime check fires):\n{out}"
    );
    assert!(
        !ran,
        "corrupted sum L1 must ABORT at the violated `ens` check (non-zero exit), not run clean:\n{out}"
    );
    // The over-counting body first violates the loop invariant `acc ==
    // spec_sum(&xs[..i])` (iteration 2), then would violate `ens`; either way
    // the always-active handler fires with its structured diagnostic — the
    // contract failure is OBSERVABLE, not silent (AC-1 negative).
    assert!(
        out.contains("thermite L1 contract violation [inv]")
            || out.contains("thermite L1 contract violation [ens]"),
        "the violation handler must fire with a structured [inv]/[ens] diagnostic:\n{out}"
    );
    assert!(
        !out.contains("should-not-reach"),
        "execution must abort before main's tail (the check is observable):\n{out}"
    );
}

// ---- AC-2: checks are always-active, NOT debug_assert -----------------------

#[test]
fn no_debug_assert_in_emission() {
    let emitted = lower_corpus_l1("sum");
    assert!(
        !emitted.contains("debug_assert"),
        "L1 emission must NOT use debug_assert (stripped in release; §6 demands every profile):\n{emitted}"
    );
    // The always-active form is the plain `if !(cond)` macro body.
    assert!(
        emitted.contains("macro_rules! thermite_check"),
        "L1 emission must define the always-active thermite_check macro:\n{emitted}"
    );
    assert!(
        emitted.contains("if !($cond)"),
        "the thermite_check macro must be the always-active `if !(cond)` form:\n{emitted}"
    );
}

// ---- AC-5: no fx syscall sandbox, no dec termination guarantee --------------

#[test]
fn no_syscall_sandbox_and_no_dec_guarantee() {
    let emitted = lower_corpus_l1("sum");
    // REQ-7: no runtime effect sandbox in v0.1.
    for forbidden in ["syscall", "sandbox", "seccomp", "fx pure"] {
        assert!(
            !emitted.contains(forbidden),
            "L1 emission must emit NO syscall-sandbox scaffolding (`{forbidden}`; REQ-7):\n{emitted}"
        );
    }
    // REQ-5/OQ-3: `inv` checks present, but NO `dec` runtime check emitted.
    assert!(
        emitted.contains("thermite_check!(\"inv\""),
        "L1 emission must assert loop invariants (REQ-1/REQ-5):\n{emitted}"
    );
    assert!(
        !emitted.contains("thermite_check!(\"dec\""),
        "L1 emission must NOT emit a `dec` runtime check (termination is proof-time; REQ-5/OQ-3):\n{emitted}"
    );
}

// ---- AC-6: lower_l1 returns Result, never panics in the toolchain -----------

#[test]
fn corpus_lowers_ok_no_panic() {
    // Over the corpus, lower_l1 returns Ok (no toolchain panic; the emitted
    // program's violation handler is the separate, intended runtime behavior).
    let src = std::fs::read_to_string(corpus_dir().join("sum.th")).unwrap();
    let parsed = thermite_syntax::parse(&src);
    let r = thermite_lower::lower_l1(&parsed.program);
    assert!(r.is_ok(), "lower_l1 over the corpus must be Ok: {r:?}");
}

#[test]
fn unsupported_construct_is_err_not_panic() {
    // A spec-fn body shaped as a slice match WITHOUT a recursive tail call hits
    // the head-fold detector's `rec_name.is_empty()` guard and must return
    // `Err(LowerError::Unsupported)`, never panic (AC-6, R-CODE-2). We build
    // such a program: a head-fold-looking spec fn whose cons arm adds two
    // literals instead of recursing.
    let src = "spec fn bad(xs: &[u32]) -> u64\n  dec xs.len()\n{\n  match xs {\n    [] => 0,\n    [head, ..t] => head as u64 + 1,\n  }\n}\n";
    let parsed = thermite_syntax::parse(src);
    assert!(
        parsed.errors.is_empty(),
        "probe program must parse clean: {:?}",
        parsed.errors
    );
    let r = thermite_lower::lower_l1(&parsed.program);
    assert!(
        matches!(r, Err(thermite_lower::LowerError::Unsupported { .. })),
        "a head-fold spec fn without a recursive tail call must be Err(Unsupported), got: {r:?}"
    );
}

// ---- AC-3: combinator L1 forms run over concrete slices ---------------------
//
// The registry `l1` field is the single source of truth; we materialize all 8
// L1 fns into ONE program, append a `main` exercising each over concrete slices
// with HAND-DERIVED expected values (R-CHAR-3, from l1-runtime-checks.md AC-3),
// compile + run it, and require exit 0.

#[test]
fn combinator_l1_forms_run() {
    let mut src = String::new();
    for sig in thermite_spec::all() {
        src.push_str(sig.l1);
        src.push_str("\n\n");
    }
    src.push_str(COMBINATOR_MAIN);
    let (compiled, ran, out) = compile_and_run(&src, "combinators_l1");
    assert!(
        compiled,
        "the registry L1 combinator forms did NOT compile under rustc:\n{out}\n--- src ---\n{src}"
    );
    assert!(
        ran && out.contains("combinators-ok"),
        "the registry L1 combinator forms did NOT run clean over the hand-derived cases:\n{out}"
    );
}

/// Hand-derived combinator cases (R-CHAR-3 — from l1-runtime-checks.md AC-3 +
/// the executable semantics, NEVER from toolchain output). `#[allow(dead_code)]`
/// per-combinator is unnecessary: `main` references every fn.
const COMBINATOR_MAIN: &str = r#"
fn main() {
    // sorted
    assert!(sorted(&[1u32, 2, 2, 3]));
    assert!(!sorted(&[3u32, 1]));
    assert!(sorted(&[]));
    assert!(sorted(&[5u32]));
    // forall_in
    assert!(forall_in(&[2u32, 4, 6], |x| x % 2 == 0));
    assert!(!forall_in(&[2u32, 3], |x| x % 2 == 0));
    assert!(forall_in(&[], |x| x > 100));
    // exists_in
    assert!(exists_in(&[1u32, 2, 3], |x| x == 2));
    assert!(!exists_in(&[1u32, 3, 5], |x| x % 2 == 0));
    assert!(!exists_in(&[], |x| x == 0));
    // count_where
    assert_eq!(count_where(&[1u32, 2, 3, 4], |x| x % 2 == 0), 2);
    assert_eq!(count_where(&[], |x| x == 0), 0);
    // permutation_of
    assert!(permutation_of(&[1u32, 2, 3], &[3u32, 1, 2]));
    assert!(!permutation_of(&[1u32, 2, 3], &[1u32, 2, 2]));
    assert!(!permutation_of(&[1u32], &[1u32, 1]));
    assert!(permutation_of(&[], &[]));
    // disjoint
    assert!(disjoint(&[1u32, 2], &[3u32, 4]));
    assert!(!disjoint(&[1u32, 2], &[2u32, 5]));
    assert!(disjoint(&[], &[1u32]));
    // forall_below: only the first `n` (and < len) elements checked
    assert!(forall_below(&[1u32, 2, 9], 2, |x| x < 5));
    assert!(!forall_below(&[1u32, 9, 2], 2, |x| x < 5));
    assert!(forall_below(&[9u32], 0, |x| x < 5)); // n == 0: vacuously true
    // forall_from: only elements from index `n` checked
    assert!(forall_from(&[9u32, 1, 2], 1, |x| x < 5));
    assert!(!forall_from(&[1u32, 9, 2], 1, |x| x < 5));
    assert!(forall_from(&[9u32], 1, |x| x < 5)); // n == len: vacuously true
    println!("combinators-ok");
}
"#;
