//! THE R-CHAR-3 TEETH-TEST (`.design/verified/contract-tv.md` REQ-4; epic
//! crosslink #143). The proof that contract-faithfulness TV DISCRIMINATES a
//! faithful lowering from an injected infidelity — the bug class the five
//! existing layers (verus-on-emitted, the cert oracle, the vacuity/mutation
//! battery, the critic) structurally cannot see.
//!
//! For each ORCHESTRATOR-authored fixture (F1–F4) the test builds the per-clause
//! equivalence obligation TWICE — once with the FAITHFUL production predicate
//! (TV must PASS: `verified, 0 errors`), once with the INFIDEL one (TV must
//! CATCH it: `errors >= 1`, a verus counterexample). The reference encoder's
//! output for the source MUST mean the same as the FAITHFUL column — so this also
//! tests `ref_contract_pred` is correct.
//!
//! Expected values trace to the fixtures + `thermite-design.md` §4.2 + the
//! frozen `thermite_spec::REGISTRY.verus_l3` — NEVER copied from the lowerer's
//! output (R-CHAR-3). Verus is resolved via `VERUS_BIN`/PATH/`~/.local/bin` and
//! the test SKIPS LOUDLY if it is genuinely absent (mirroring
//! `thermite-lower/tests/lower_conformance.rs`); `unwrap`/`expect` are fine here
//! (`tests/` is not anti-pattern-gated).

use std::path::{Path, PathBuf};
use std::process::Command;

use thermite_syntax::ast::{BinOp, Expr, IndexArg, PrimType, Type};
use thermite_tv::obligation::{equivalence_obligation, ObligationFrame, ParamDecl};

// ---- AST construction helpers (the source clauses, as Thermite AST) --------

fn path(name: &str) -> Expr {
    Expr::Path(vec![name.to_string()])
}

fn int(value: u128) -> Expr {
    Expr::IntLit {
        value,
        raw: value.to_string(),
    }
}

fn bin(op: BinOp, lhs: Expr, rhs: Expr) -> Expr {
    Expr::Binary {
        op,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    }
}

fn call(callee: &str, args: Vec<Expr>) -> Expr {
    Expr::Call {
        callee: Box::new(path(callee)),
        args,
    }
}

fn closure(param: &str, body: Expr) -> Expr {
    Expr::Closure {
        params: vec![param.to_string()],
        body: Box::new(body),
    }
}

fn method(receiver: Expr, name: &str, args: Vec<Expr>) -> Expr {
    Expr::MethodCall {
        receiver: Box::new(receiver),
        name: name.to_string(),
        args,
    }
}

// ---- verus resolution + discharge (mirrors lower_conformance.rs) -----------

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

/// Run `verus <file>` in the temp dir (no scratch pollution — #53). Returns the
/// combined stdout+stderr and exit success, or `None` if verus is unavailable.
fn run_verus(file: &Path) -> Option<(bool, String)> {
    let bin = verus_bin()?;
    let out = Command::new(bin)
        .arg(file)
        .current_dir(std::env::temp_dir())
        .output()
        .ok()?;
    let mut combined = String::from_utf8_lossy(&out.stdout).to_string();
    combined.push_str(&String::from_utf8_lossy(&out.stderr));
    Some((out.status.success(), combined))
}

/// Parse the `N verified, M errors` summary line from verus output.
fn parse_results(output: &str) -> Option<(u32, u32)> {
    let line = output
        .lines()
        .find(|l| l.contains("verified,") && l.contains("errors"))?;
    let verified = line
        .split("verified,")
        .next()?
        .split_whitespace()
        .last()?
        .parse::<u32>()
        .ok()?;
    let errors = line
        .split("verified,")
        .nth(1)?
        .split_whitespace()
        .next()?
        .parse::<u32>()
        .ok()?;
    Some((verified, errors))
}

/// Discharge a faithful obligation: it MUST verify (`verified >= 1, errors ==
/// 0`). SKIPS LOUDLY if verus is absent.
fn assert_faithful_verifies(fixture: &str, program: &str) {
    let tmp = std::env::temp_dir().join(format!("tv_teeth_{fixture}_faithful.rs"));
    std::fs::write(&tmp, program).unwrap_or_else(|e| panic!("write {fixture} faithful: {e}"));
    match run_verus(&tmp) {
        Some((ok, output)) => {
            let (verified, errors) = parse_results(&output).unwrap_or_else(|| {
                panic!("{fixture} faithful: no verus results line:\n{output}\n--- program ---\n{program}")
            });
            assert!(
                ok && errors == 0 && verified >= 1,
                "{fixture} FAITHFUL obligation did NOT verify (a TV false positive — likely a \
                 coercion mismatch, the #1 flagged risk). exit_success={ok} verified={verified} \
                 errors={errors}\n--- verus output ---\n{output}\n--- program ---\n{program}"
            );
            eprintln!("FAITHFUL {fixture}: verus = {verified} verified, {errors} errors (PASS)");
        }
        None => eprintln!(
            "SKIP: verus not available — {fixture} faithful obligation not discharged \
             (set VERUS_BIN or install verus on PATH)."
        ),
    }
}

/// Discharge an infidel obligation: TV MUST catch it (`errors >= 1`, a
/// counterexample at the `assert(P_production <==> P_reference)` site). SKIPS
/// LOUDLY if verus is absent.
fn assert_infidel_caught(fixture: &str, program: &str) {
    let tmp = std::env::temp_dir().join(format!("tv_teeth_{fixture}_infidel.rs"));
    std::fs::write(&tmp, program).unwrap_or_else(|e| panic!("write {fixture} infidel: {e}"));
    match run_verus(&tmp) {
        Some((_ok, output)) => {
            let (_verified, errors) = parse_results(&output).unwrap_or_else(|| {
                panic!("{fixture} infidel: no verus results line:\n{output}\n--- program ---\n{program}")
            });
            assert!(
                errors >= 1,
                "{fixture} INFIDEL obligation VERIFIED — TV FAILED TO CATCH the infidelity \
                 (the teeth did not bite). errors={errors}\n--- verus output ---\n{output}\n\
                 --- program ---\n{program}"
            );
            assert!(
                output.contains("assertion failed"),
                "{fixture} infidel failed but not at the equivalence assertion:\n{output}"
            );
            eprintln!("INFIDEL {fixture}: verus = {errors} errors — TV CAUGHT it (PASS)");
        }
        None => {
            eprintln!("SKIP: verus not available — {fixture} infidel obligation not discharged.")
        }
    }
}

// ============================================================================
// F1 — comparison: the ==-vs-<= teeth (the canonical infidelity case)
// source clause:  result == spec_sum(xs)
// faithful P_prod: result as nat == spec_sum(xs)   (xs bound as Seq, so the
//                  slice→@ view is the identity — the doc AC-1 grounded form
//                  binds `xs: Seq<u32>` and emits `spec_sum(xs)` on BOTH sides;
//                  the fixture's `xs@` is the slice-param spelling, which reduces
//                  to `xs` once the obligation binds the param directly as a Seq).
// infidel  P_prod: result as nat <= spec_sum(xs)
// frame: result: u64, xs: Seq<u32>; spec_sum in scope
// ============================================================================

/// `spec fn spec_sum` shape copied from `tests/golden/lower/sum.verus.rs` (the
/// external golden — NOT regenerated). Used as the frame spec-fn dep.
const SPEC_SUM_DEF: &str = "spec fn spec_sum(s: Seq<u32>) -> nat\n    decreases s.len()\n{\n    if s.len() == 0 { 0 } else { s[0] as nat + spec_sum(s.drop_first()) }\n}";

fn f1_frame() -> ObligationFrame {
    ObligationFrame {
        spec_defs: vec![SPEC_SUM_DEF.to_string()],
        params: vec![
            ParamDecl::new("result", "u64"),
            ParamDecl::new("xs", "Seq<u32>"),
        ],
        req: None,
        seq_params: vec!["xs".to_string()],
        nat_coerce_params: vec!["result".to_string()],
        // #150 mechanical ripple: the additive `string_params`/`map_params` fields
        // (the String/Map byte-view receiver dispatch) are empty for F1 (no
        // String/Map receiver in the comparison teeth).
        ..Default::default()
    }
}

fn f1_source() -> Expr {
    // result == spec_sum(xs)
    bin(
        BinOp::Eq,
        path("result"),
        call("spec_sum", vec![path("xs")]),
    )
}

#[test]
fn f1_comparison_faithful_verifies() {
    let prog = equivalence_obligation(&f1_source(), "result as nat == spec_sum(xs)", &f1_frame())
        .expect("F1 faithful obligation builds");
    assert_faithful_verifies("f1", &prog);
}

#[test]
fn f1_comparison_infidel_caught() {
    let prog = equivalence_obligation(&f1_source(), "result as nat <= spec_sum(xs)", &f1_frame())
        .expect("F1 infidel obligation builds");
    assert_infidel_caught("f1", &prog);
}

// ============================================================================
// F2 — combinator: the wrong-predicate teeth
// source clause:  forall_in(xs, |x| x < 10)
// faithful P_prod: the forall_in verus_l3 body applied with |x| x < 10
// infidel  P_prod: same but |x| x <= 10
// frame: xs: Seq<u32>; forall_in verus_l3 in scope (the shared frozen ground truth)
// ============================================================================

fn forall_in_def() -> String {
    // REUSED: the frozen combinator ground truth (NOT re-implemented here).
    thermite_spec::lookup("forall_in")
        .expect("forall_in is a frozen combinator")
        .verus_l3
        .to_string()
}

fn f2_frame() -> ObligationFrame {
    ObligationFrame {
        spec_defs: vec![forall_in_def()],
        params: vec![ParamDecl::new("xs", "Seq<u32>")],
        req: None,
        seq_params: vec!["xs".to_string()],
        nat_coerce_params: vec![],
        ..Default::default()
    }
}

fn f2_source() -> Expr {
    // forall_in(xs, |x| x < 10)
    call(
        "forall_in",
        vec![path("xs"), closure("x", bin(BinOp::Lt, path("x"), int(10)))],
    )
}

#[test]
fn f2_combinator_faithful_verifies() {
    let prog = equivalence_obligation(&f2_source(), "forall_in(xs, |x: u32| x < 10)", &f2_frame())
        .expect("F2 faithful obligation builds");
    assert_faithful_verifies("f2", &prog);
}

#[test]
fn f2_combinator_infidel_caught() {
    let prog = equivalence_obligation(&f2_source(), "forall_in(xs, |x: u32| x <= 10)", &f2_frame())
        .expect("F2 infidel obligation builds");
    assert_infidel_caught("f2", &prog);
}

// ============================================================================
// F3 — byteview (#127): the wrong-index teeth (receiver-shape dispatch)
// source clause:  s.byte_at(0) == 65
// faithful P_prod: s@[0] == 65   (s bound as Seq<u8> → s@ == s, index 0 correct)
// infidel  P_prod: s@[1] == 65   (wrong index — the #127 misdispatch class)
// frame: s: Seq<u8>
// ============================================================================

fn f3_frame() -> ObligationFrame {
    ObligationFrame {
        spec_defs: vec![],
        params: vec![ParamDecl::new("s", "Seq<u8>")],
        // The faithful index 0 needs the receiver non-empty; the infidel index 1
        // needs len >= 2 to be well-defined. We bind len >= 2 (covers both): a
        // counterexample on the infidel side is a MEANING difference, not an OOB.
        req: Some("s.len() >= 2".to_string()),
        seq_params: vec!["s".to_string()],
        nat_coerce_params: vec![],
        ..Default::default()
    }
}

fn f3_source() -> Expr {
    // s.byte_at(0) == 65
    bin(
        BinOp::Eq,
        method(path("s"), "byte_at", vec![int(0)]),
        int(65),
    )
}

#[test]
fn f3_byteview_faithful_verifies() {
    let prog = equivalence_obligation(&f3_source(), "s[0] == 65", &f3_frame())
        .expect("F3 faithful obligation builds");
    assert_faithful_verifies("f3", &prog);
}

#[test]
fn f3_byteview_infidel_caught() {
    // The #127-class wrong byte-view rewrite: production mis-dispatched index 0
    // to index 1.
    let prog = equivalence_obligation(&f3_source(), "s[1] == 65", &f3_frame())
        .expect("F3 infidel obligation builds");
    assert_infidel_caught("f3", &prog);
}

// ============================================================================
// F4 — structural-drop: the dropped-conjunct teeth
// source clause:  a == b && c == d
// faithful P_prod: a == b && c == d
// infidel  P_prod: a == b    (a conjunct silently dropped)
// frame: a,b,c,d: int
// ============================================================================

fn f4_frame() -> ObligationFrame {
    ObligationFrame {
        spec_defs: vec![],
        params: vec![
            ParamDecl::new("a", "int"),
            ParamDecl::new("b", "int"),
            ParamDecl::new("c", "int"),
            ParamDecl::new("d", "int"),
        ],
        req: None,
        seq_params: vec![],
        nat_coerce_params: vec![],
        ..Default::default()
    }
}

fn f4_source() -> Expr {
    // a == b && c == d
    bin(
        BinOp::And,
        bin(BinOp::Eq, path("a"), path("b")),
        bin(BinOp::Eq, path("c"), path("d")),
    )
}

#[test]
fn f4_structural_drop_faithful_verifies() {
    let prog = equivalence_obligation(&f4_source(), "a == b && c == d", &f4_frame())
        .expect("F4 faithful obligation builds");
    assert_faithful_verifies("f4", &prog);
}

#[test]
fn f4_structural_drop_infidel_caught() {
    // A dropped conjunct — the production side silently lost `&& c == d`.
    let prog = equivalence_obligation(&f4_source(), "a == b", &f4_frame())
        .expect("F4 infidel obligation builds");
    assert_infidel_caught("f4", &prog);
}

// ---- ref_encode unit checks (the reference output MEANS the faithful column) -

/// The reference encoding of each source must be a string that MEANS the
/// faithful production column (the obligation tests prove semantic equivalence
/// under verus; these pin the exact independent encoding for auditability).
#[test]
fn ref_encode_matches_faithful_meaning() {
    use thermite_tv::ref_encode::ref_contract_pred;
    use thermite_tv::RefCtx;

    // F1: nat-coercion inferred on `result`, xs seq-bound (identity @-view).
    let f1_ctx = RefCtx::with_seq_bound(["xs"]).with_nat_coerce(["result"]);
    assert_eq!(
        ref_contract_pred(&f1_source(), &f1_ctx).unwrap(),
        "(result as nat == spec_sum(xs))"
    );
    // F2: closure predicate re-encoded at u32, xs seq-bound.
    let f2_ctx = RefCtx::with_seq_bound(["xs"]);
    assert_eq!(
        ref_contract_pred(&f2_source(), &f2_ctx).unwrap(),
        "forall_in(xs, |x: u32| (x < 10))"
    );
    // F3: byte_at(0) on a seq-bound receiver → s[0].
    let f3_ctx = RefCtx::with_seq_bound(["s"]);
    assert_eq!(
        ref_contract_pred(&f3_source(), &f3_ctx).unwrap(),
        "(s[0] == 65)"
    );
    // F4: structural conjunction, no coercion.
    let f4_ctx = RefCtx::default();
    assert_eq!(
        ref_contract_pred(&f4_source(), &f4_ctx).unwrap(),
        "((a == b) && (c == d))"
    );
}

/// An unsupported construct is an honest `Err`, NEVER a panic / silent wrong
/// encoding (REQ-1 / R-CODE-2). A `match` in spec position is outside the frozen
/// sublanguage.
#[test]
fn unsupported_construct_is_err_not_panic() {
    use thermite_tv::ref_encode::{ref_contract_pred, RefEncodeError};
    use thermite_tv::RefCtx;

    let bad = Expr::Cast {
        expr: Box::new(int(3)),
        ty: Type::Prim(PrimType::Bool),
    };
    let r = ref_contract_pred(&bad, &RefCtx::default());
    assert!(matches!(r, Err(RefEncodeError::Unsupported(_))));

    // An index-arg range over a seq-bound base is supported (sanity for the
    // subrange path used by `&xs[..i]`).
    let slice = Expr::Index {
        base: Box::new(path("xs")),
        index: IndexArg::RangeTo(Box::new(int(3))),
    };
    let ok = ref_contract_pred(&slice, &RefCtx::with_seq_bound(["xs"]));
    assert_eq!(ok.unwrap(), "xs.subrange(0, 3)");
}
