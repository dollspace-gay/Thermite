//! Conformance test for `thermite-lower`'s Basis Stage 2c recursion-scheme
//! lowering (`.design/basis/02-recursion-schemes.md` REQ-6/REQ-7) against the
//! EXTERNAL truths: the real `verus` binary run under `--no-cheating` (the
//! emitted L3 must VERIFY with `0 errors`, and the multiplier must hold WITHOUT a
//! cheat token), and the hand-derived oracle `conformance/adt-schemes/cases.json`
//! (R-CHAR-3 — NEVER edited).
//!
//! The corpus `conformance/list_fold.th` carries three scheme-call `spec fn`s
//! (`len_list`/`sum_list` = `fold(l, 0, …)`, `all_positive` = `for_all(l, …)`),
//! each lowering to a CALL of the GENERATED scheme `spec fn` (REQ-6) plus the
//! materialized `list_len`/`fold_list`/`for_all_list`/`fold_bound_list`.
//!
//! Three checks:
//!   - `certify`  — the emitted `list_fold.th` lowering VERIFIES (`verified, 0
//!     errors`) under `--no-cheating` (len_list/sum_list/all_positive → L3).
//!   - `multiplier` — the GENERATED `fold_bound_list` + an instance bound proven
//!     by CITING it (the design's GROUNDED `sum_list_bounded`, NO fresh
//!     induction) verifies; the NEGATIVE CONTROL (per-node premise dropped) FAILS
//!     verus. The induction is real, not vacuous (R-DEFER-9).
//!   - structural — the emitted lowering materializes `fold_bound_list` + the
//!     generated `fold_list`/`for_all_list` with `decreases l`; a scheme-call
//!     INSTANCE body carries NO `decreases` (the recursion lives in the
//!     generated fold — the multiplier is observable).
//!
//! `unwrap`/`expect`/`panic!` are fine here — `tests/` is not anti-pattern-gated.

use std::path::{Path, PathBuf};
use std::process::Command;

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("conformance")
}

fn parse_corpus(name: &str) -> thermite_syntax::ast::Program {
    let src = std::fs::read_to_string(corpus_dir().join(format!("{name}.th")))
        .unwrap_or_else(|e| panic!("cannot read corpus {name}.th: {e}"));
    let parsed = thermite_syntax::parse(&src);
    assert!(
        parsed.errors.is_empty(),
        "corpus {name}.th must parse clean: {:?}",
        parsed.errors
    );
    parsed.program
}

fn lower_l3(name: &str) -> String {
    thermite_lower::lower(&parse_corpus(name))
        .unwrap_or_else(|e| panic!("L3 lowering {name}.th failed: {e}"))
}

// ---- verus driver (shared shape with adt_lower_conformance.rs) -------------

fn verus_bin() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("VERUS_BIN") {
        let pb = PathBuf::from(p);
        if pb.exists() {
            return Some(pb);
        }
    }
    if let Ok(out) = Command::new("which").arg("verus").output() {
        if out.status.success() {
            let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !p.is_empty() {
                return Some(PathBuf::from(p));
            }
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        let cand = PathBuf::from(home).join(".local/bin/verus");
        if cand.exists() {
            return Some(cand);
        }
    }
    None
}

/// Run `verus --no-cheating <file>` (the cheat-token battery enabled — a
/// generated fold/law that relied on `assume`/`external_body` would be REJECTED).
/// `None` if verus is unavailable (caller SKIPs LOUDLY). R-CODE-4: status checked.
fn run_verus_no_cheating(file: &Path) -> Option<(bool, String)> {
    let bin = verus_bin()?;
    let out = Command::new(bin)
        .arg("--no-cheating")
        .arg(file)
        .current_dir(std::env::temp_dir())
        .output()
        .ok()?;
    let mut combined = String::from_utf8_lossy(&out.stdout).to_string();
    combined.push_str(&String::from_utf8_lossy(&out.stderr));
    Some((out.status.success(), combined))
}

fn write_temp(name: &str, src: &str) -> PathBuf {
    let tmp = std::env::temp_dir().join(format!("{name}_adt_scheme.rs"));
    std::fs::write(&tmp, src).unwrap_or_else(|e| panic!("write temp for {name}: {e}"));
    tmp
}

// ---- AC-1: the certify source lowers + verifies L3 -------------------------

/// `conformance/list_fold.th` lowers to the GENERATED `fold_list`/`for_all_list`/
/// `list_len`/`fold_bound_list` + the 3 scheme-call instances, and the real
/// `verus --no-cheating` binary VERIFIES it (`verified, 0 errors`) —
/// len_list/sum_list/all_positive each → L3 (REQ-6).
#[test]
fn list_fold_lowers_to_generated_schemes_and_verifies_l3() {
    let emitted = lower_l3("list_fold");

    // Structural (REQ-6): the materialized generated fns are present, each
    // recursive with `decreases l`.
    assert!(
        emitted
            .contains("spec fn fold_list(l: List, init: nat, f: spec_fn(u64, nat) -> nat) -> nat"),
        "generated fold_list spec fn (REQ-6):\n{emitted}"
    );
    assert!(
        emitted.contains("spec fn for_all_list(l: List, p: spec_fn(u64) -> bool) -> bool"),
        "generated for_all_list spec fn (REQ-6):\n{emitted}"
    );
    assert!(
        emitted.contains("spec fn list_len(l: List) -> nat"),
        "generated structural measure list_len (REQ-6/REQ-7):\n{emitted}"
    );
    // The instances lower to CALLS of the generated fns (REQ-6 — step as spec_fn).
    assert!(
        emitted.contains("fold_list(l, 0, |x: u64, acc: nat| (x + acc) as nat)"),
        "sum_list lowers to a fold_list call with the step as a spec_fn (REQ-6):\n{emitted}"
    );
    assert!(
        emitted.contains("for_all_list(l, |x: u64| x > 0)"),
        "all_positive lowers to a for_all_list call (REQ-6):\n{emitted}"
    );

    assert_no_cheats(&emitted, "list_fold");

    let tmp = write_temp("list_fold", &emitted);
    match run_verus_no_cheating(&tmp) {
        Some((ok, output)) => {
            assert!(
                ok && output.contains("verified, 0 errors"),
                "verus --no-cheating on emitted list_fold did NOT verify (R-CODE-4). \
                 exit_success={ok}\n--- verus output ---\n{output}\n--- emitted ---\n{emitted}"
            );
        }
        None => eprintln!(
            "SKIP: verus not available — L3 verification of emitted `list_fold` not run \
             (set VERUS_BIN or install verus on PATH); structural asserts still run."
        ),
    }
}

// ---- AC-2: the multiplier — instance bound proven by CITING the law --------

/// The induction-discharged-once MULTIPLIER (REQ-7). The lowerer GENERATES
/// `fold_bound_list` (the single-induction generic law). An instance bound
/// (`sum_list(l) <= list_len(l) * MAX`) is proven by CITING `fold_bound_list`
/// with NO fresh induction — only a FLAT per-node `assert`, then the cite. We
/// reproduce the design's GROUNDED `sum_list_bounded` (hand-derived, R-CHAR-3)
/// APPENDED to the lowerer's emitted output, and assert real `verus
/// --no-cheating` reports `verified, 0 errors` (the oracle `multiplier`
/// expectation). The instance proof contains `fold_bound_` and NO `decreases`.
#[test]
fn multiplier_instance_cites_the_generated_law_no_fresh_induction() {
    let emitted = lower_l3("list_fold");
    // The generated law is materialized (REQ-7).
    assert!(
        emitted.contains(
            "proof fn fold_bound_list(l: List, init: nat, f: spec_fn(u64, nat) -> nat, b: nat)"
        ),
        "generated fold_bound_list law (REQ-7):\n{emitted}"
    );
    assert!(
        emitted.contains("decreases l,"),
        "the generated law carries the single structural induction `decreases l` (REQ-7):\n{emitted}"
    );

    // The instance proof (the design's GROUNDED `sum_list_bounded`). It is
    // hand-derived from the design (R-CHAR-3), NOT read from toolchain output: it
    // CITES `fold_bound_list` and discharges only the FLAT per-node premise — NO
    // `decreases`, NO `match`, NO recursive proof call.
    let instance_proof = "\nproof fn sum_list_bounded(l: List)\n    \
        ensures sum_list(l) <= list_len(l) * (u64::MAX as nat),\n{\n    \
        let f = |x: u64, acc: nat| (x + acc) as nat;\n    \
        assert(forall|x: u64, acc: nat| #[trigger] f(x, acc) <= acc + (u64::MAX as nat));\n    \
        fold_bound_list(l, 0, f, u64::MAX as nat);\n}\n";

    // The multiplier is OBSERVABLE: the instance proof cites the law and has no
    // fresh induction (AC-2 mechanical assertion).
    assert!(
        instance_proof.contains("fold_bound_list("),
        "the instance proof must CITE the generated law"
    );
    assert!(
        !instance_proof.contains("decreases"),
        "the instance proof must contain NO fresh `decreases` (the induction is \
         discharged once in the law)"
    );

    // Splice the instance proof in before the closing `}` of the `verus! { … }`.
    let spliced = splice_into_verus(&emitted, instance_proof);
    assert_no_cheats(&spliced, "multiplier");

    let tmp = write_temp("multiplier", &spliced);
    match run_verus_no_cheating(&tmp) {
        Some((ok, output)) => {
            assert!(
                ok && output.contains("verified, 0 errors"),
                "the multiplier (instance citing fold_bound_list, NO fresh induction) \
                 must verify under --no-cheating (REQ-7). exit_success={ok}\n\
                 --- verus output ---\n{output}\n--- spliced ---\n{spliced}"
            );
        }
        None => eprintln!(
            "SKIP: verus not available — the multiplier grounding not run \
             (set VERUS_BIN or install verus on PATH); structural asserts still run."
        ),
    }
}

/// NEGATIVE CONTROL (AC-2 / R-DEFER-9, §7): the GENERATED law minus its per-node
/// premise FAILS verus — the per-node premise is LOAD-BEARING, the induction is
/// not vacuous. We take the emitted lowering, STRIP the `forall|…| f(x, acc) <=
/// acc + b` premise line from `fold_bound_list`, and assert verus REPORTS an
/// error (the oracle `multiplier` expectation: dropping the premise → 1 error).
#[test]
fn negative_control_premise_removed_fails_verus() {
    let emitted = lower_l3("list_fold");
    let premise_line = "        forall|x: u64, acc: nat| #[trigger] f(x, acc) <= acc + b,\n";
    assert!(
        emitted.contains(premise_line),
        "the generated law must carry the per-node premise to remove:\n{emitted}"
    );
    let weakened = emitted.replace(premise_line, "");

    let tmp = write_temp("scheme_neg_control", &weakened);
    match run_verus_no_cheating(&tmp) {
        Some((ok, output)) => {
            assert!(
                !ok && output.contains("error"),
                "negative control: with the per-node premise REMOVED the generated law \
                 must FAIL verus (the premise is load-bearing; the induction is real). \
                 exit_success={ok}\n--- verus output ---\n{output}"
            );
        }
        None => eprintln!(
            "SKIP: verus not available — the negative control not run (set VERUS_BIN or \
             install verus on PATH)."
        ),
    }
}

// ---- AC-5: no regression — sum/list_sum still lower + verify ---------------

/// The scheme additions are PURELY ADDITIVE (new generated fns; the hand-written
/// `is_adt_fold_sum` path is unchanged). The SHIPPED Stage-1 hand-written ADT
/// fold `list_sum.th` (lowered via `is_adt_fold_sum`, NOT the scheme path) still
/// lowers to its recursive `spec fn` and VERIFIES — Stage 2 does NOT reshape it.
#[test]
fn list_sum_handwritten_fold_unchanged_and_verifies() {
    let emitted = lower_l3("list_sum");
    // The hand-written fold stays the `is_adt_fold_sum` recursive `spec fn` with
    // `decreases l` (NOT a generated `fold_list` call).
    assert!(
        emitted.contains("spec fn sum_list(l: List) -> nat")
            && emitted.contains("List::Cons(h, t) => h as nat + sum_list(*t),"),
        "list_sum's hand-written recursive fold is unchanged (AC-5):\n{emitted}"
    );
    assert!(
        !emitted.contains("fold_list("),
        "list_sum must NOT be reshaped into the generated-fold_list path (AC-5):\n{emitted}"
    );
    assert_no_cheats(&emitted, "list_sum");
    let tmp = write_temp("list_sum_scheme_regression", &emitted);
    match run_verus_no_cheating(&tmp) {
        Some((ok, output)) => assert!(
            ok && output.contains("verified, 0 errors"),
            "list_sum hand-written fold must still verify (AC-5). exit_success={ok}\n{output}"
        ),
        None => eprintln!("SKIP: verus not available — list_sum regression check not run."),
    }
}

// ---- helpers ---------------------------------------------------------------

/// Splice `addition` into the emitted lowering just before the final `}` that
/// closes the `verus! { … }` block (the emitter ends with `\n}\nfn main() {}\n`).
fn splice_into_verus(emitted: &str, addition: &str) -> String {
    let marker = "\n}\nfn main() {}\n";
    let idx = emitted
        .rfind(marker)
        .unwrap_or_else(|| panic!("emitted lowering missing the verus close marker:\n{emitted}"));
    let mut out = String::with_capacity(emitted.len() + addition.len());
    out.push_str(&emitted[..idx]);
    out.push('\n');
    out.push_str(addition);
    out.push_str(marker);
    out
}

/// The lowering must never emit a proof cheat (R-DEFER-9): no `assume`,
/// `external_body`, `external`, or `#[slag]` on the generated folds / law.
fn assert_no_cheats(emitted: &str, name: &str) {
    for forbidden in [
        "assume(false)",
        "assume(",
        "#[verifier::external]",
        "#[verifier::external_body]",
        "admit()",
        "#[slag]",
    ] {
        assert!(
            !emitted.contains(forbidden),
            "{name} emission contains forbidden cheat token `{forbidden}` (R-DEFER-9):\n{emitted}"
        );
    }
}
