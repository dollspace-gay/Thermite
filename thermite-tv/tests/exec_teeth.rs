//! THE R-CHAR-3 EXEC-TEETH-TEST (`.design/verified/exec-tv.md` REQ-4; epic
//! crosslink #151, blocker #155). The proof that EXEC-position TV DISCRIMINATES a
//! faithful body-expression lowering from an injected infidelity — the #122/#146
//! infidelity class in its GENERAL home (the exec body), plus the wrong-op/overflow
//! and off-by-one classes the contract-position teeth cannot reach.
//!
//! For each ORCHESTRATOR-authored fixture (E1–E4) the test builds the exec-fn
//! equivalence obligation TWICE — once with the FAITHFUL `P_production` (the exact
//! string `thermite_lower::lower_exec_expr` emits, PINNED by
//! `thermite-lower/src/lower.rs::exec_expr_tests` — the cross-crate bridge, since
//! this INDEPENDENT crate has NO `thermite-lower` dep), once with the INFIDEL one
//! (the hand-injected bug from the fixture). TV must:
//!   - faithful → VERIFY (`verified >= 1, errors == 0`);
//!   - infidel  → be CAUGHT — a verus FAILURE: an `E0308`/type error (E1 paren-drop
//!     makes the production ill-typed), a parse `error: expected ','` (E2 cast-`<`
//!     mis-parse), or a `postcondition not satisfied` counterexample (E3 wrong-op/
//!     overflow value differs, E4 off-by-one value differs / OOB).
//!
//! The exec reference encoder's output for the source MUST mean the FAITHFUL
//! column at the BOUNDED production type — so this also tests `exec_ref_value` is
//! correct AND that the bounded reference (NOT `nat`-coerced) catches the E3
//! overflow/wrap rather than masking it.
//!
//! Expected values trace to the fixtures + `thermite-design.md` §4.1/§6 + the
//! #122/#146 fixes — NEVER copied from the lowerer's output (R-CHAR-3); the
//! FAITHFUL `P_production` is the production lowering, pinned independently in
//! `thermite-lower`'s own test. Verus is resolved via `VERUS_BIN`/PATH/`~/.local/bin`
//! and the test SKIPS LOUDLY if it is genuinely absent (mirroring
//! `thermite-tv/tests/teeth.rs`); `unwrap`/`expect` are fine here (`tests/` is not
//! anti-pattern-gated).

use std::path::{Path, PathBuf};
use std::process::Command;

use thermite_syntax::ast::{BinOp, Expr, IndexArg, PrimType, Type};
use thermite_tv::obligation::{exec_equivalence_obligation, ExecObligationFrame, ExecParamDecl};

// ---- AST construction helpers (the source exec exprs, as Thermite AST) ------

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

fn cast(inner: Expr, ty: Type) -> Expr {
    Expr::Cast {
        expr: Box::new(inner),
        ty,
    }
}

// ---- verus resolution + discharge (mirrors teeth.rs) -----------------------

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

/// Parse the `N verified, M errors` summary line from verus output. Returns `None`
/// when there is NO results line — which is itself a signal for an infidel that
/// failed to COMPILE/PARSE (the E1 `E0308` / E2 `expected ','` cases abort before
/// verification, so there is no results line; the caller treats a no-results-but-
/// failed run as CAUGHT).
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

/// Discharge a faithful obligation: it MUST verify (`verified >= 1, errors == 0`).
/// SKIPS LOUDLY if verus is absent.
fn assert_faithful_verifies(fixture: &str, program: &str) {
    let tmp = std::env::temp_dir().join(format!("tv_exec_teeth_{fixture}_faithful.rs"));
    std::fs::write(&tmp, program).unwrap_or_else(|e| panic!("write {fixture} faithful: {e}"));
    match run_verus(&tmp) {
        Some((ok, output)) => {
            let (verified, errors) = parse_results(&output).unwrap_or_else(|| {
                panic!("{fixture} faithful: no verus results line:\n{output}\n--- program ---\n{program}")
            });
            assert!(
                ok && errors == 0 && verified >= 1,
                "{fixture} FAITHFUL exec obligation did NOT verify (a TV false positive — the \
                 faithful production exec lowering should equal the bounded exec reference). \
                 exit_success={ok} verified={verified} errors={errors}\n--- verus output ---\n\
                 {output}\n--- program ---\n{program}"
            );
            eprintln!("FAITHFUL {fixture}: verus = {verified} verified, {errors} errors (PASS)");
        }
        None => eprintln!(
            "SKIP: verus not available — {fixture} faithful exec obligation not discharged \
             (set VERUS_BIN or install verus on PATH)."
        ),
    }
}

/// How an infidel is expected to be CAUGHT by verus.
enum CatchShape {
    /// A `postcondition not satisfied` counterexample (the production typechecks
    /// but computes the wrong VALUE — E3 wrong-op/overflow, E4 off-by-one). There
    /// IS a results line with `errors >= 1`.
    Postcondition,
    /// A COMPILE/PARSE failure before verification (E1 `E0308 mismatched types`
    /// from the #122 paren-drop, E2 `error: expected ','` from the #146 cast-`<`
    /// mis-parse). verus ABORTS — exit failure, often NO results line. Carries a
    /// substring the output must contain so the catch is the RIGHT one (not an
    /// unrelated error).
    Compile(&'static str),
}

/// Discharge an infidel obligation: TV MUST catch it. SKIPS LOUDLY if verus is
/// absent. The catch shape is asserted precisely so the teeth bite for the RIGHT
/// reason (R-CHAR-3 — a spurious unrelated failure is NOT a pass).
fn assert_infidel_caught(fixture: &str, program: &str, expect: CatchShape) {
    let tmp = std::env::temp_dir().join(format!("tv_exec_teeth_{fixture}_infidel.rs"));
    std::fs::write(&tmp, program).unwrap_or_else(|e| panic!("write {fixture} infidel: {e}"));
    match run_verus(&tmp) {
        Some((ok, output)) => match expect {
            CatchShape::Postcondition => {
                let (_verified, errors) = parse_results(&output).unwrap_or_else(|| {
                    panic!(
                        "{fixture} infidel: expected a postcondition counterexample but no \
                            verus results line:\n{output}\n--- program ---\n{program}"
                    )
                });
                assert!(
                    errors >= 1,
                    "{fixture} INFIDEL exec obligation VERIFIED — TV FAILED TO CATCH the \
                     infidelity (the teeth did not bite). errors={errors}\n--- verus output ---\n\
                     {output}\n--- program ---\n{program}"
                );
                assert!(
                    output.contains("postcondition not satisfied"),
                    "{fixture} infidel failed but NOT at the `ensures result == <ref>` \
                     postcondition (the catch is the wrong one):\n{output}"
                );
                eprintln!(
                    "INFIDEL {fixture}: verus = {errors} errors (postcondition) — TV CAUGHT it (PASS)"
                );
            }
            CatchShape::Compile(needle) => {
                assert!(
                    !ok,
                    "{fixture} INFIDEL exec obligation COMPILED+VERIFIED — TV FAILED TO CATCH \
                     the infidelity (expected a compile/parse abort containing {needle:?}).\n\
                     --- verus output ---\n{output}\n--- program ---\n{program}"
                );
                assert!(
                    output.contains(needle),
                    "{fixture} infidel failed but NOT with the expected compile/parse error \
                     {needle:?} (the catch is the wrong one):\n{output}"
                );
                eprintln!(
                    "INFIDEL {fixture}: verus ABORTED (compile/parse, contains {needle:?}) — \
                     TV CAUGHT it (PASS)"
                );
            }
        },
        None => {
            eprintln!(
                "SKIP: verus not available — {fixture} infidel exec obligation not discharged."
            )
        }
    }
}

// ============================================================================
// E1 — cast-paren (#122): the inner-paren teeth
// source exec expr: (n - 1) as u8     req frame: n: u64, n >= 1 (+ n - 1 <= 255
//   so the narrowing cast is well-defined — a bound on the EXPR, not the bug)
// faithful P_prod:  (n - 1) as u8     (pinned in thermite-lower exec_expr_tests)
// infidel  P_prod:  n - 1 as u8       (= n - (1 as u8), a u64 - u8 mix → E0308)
// reference (exec_ref_value): (n - 1) as u8   — bounded u8, #122 inner-paren
// ============================================================================

fn e1_source() -> Expr {
    cast(
        bin(BinOp::Sub, path("n"), int(1)),
        Type::Named("u8".to_string()),
    )
}

fn e1_frame() -> ExecObligationFrame {
    ExecObligationFrame {
        params: vec![ExecParamDecl::new("n", "u64")],
        ret_type: "u8".to_string(),
        // n >= 1 (the source `req`) + n - 1 <= 255 (the narrowing-cast
        // well-formedness, so the faithful cast is total — the E1 bug is the
        // PAREN, not the wrap).
        req: Some("n >= 1, n - 1 <= 255".to_string()),
        ..Default::default()
    }
}

#[test]
fn e1_cast_paren_faithful_verifies() {
    let prog = exec_equivalence_obligation(&e1_source(), "(n - 1) as u8", &e1_frame())
        .expect("E1 faithful exec obligation builds");
    assert_faithful_verifies("e1", &prog);
}

#[test]
fn e1_cast_paren_infidel_caught() {
    // The #122 paren-drop: production drops the inner paren → `n - 1 as u8` ==
    // `n - (1 as u8)`, a `u64 - u8` type mix → verus aborts with E0308/E0277.
    let prog = exec_equivalence_obligation(&e1_source(), "n - 1 as u8", &e1_frame())
        .expect("E1 infidel exec obligation builds");
    assert_infidel_caught("e1", &prog, CatchShape::Compile("mismatched types"));
}

// ============================================================================
// E2 — cast-`<` (#146): the outer-paren teeth
// source exec expr: x as u32 < 33     req frame: x: u64
// faithful P_prod:  (x as u32) < 33   (pinned in thermite-lower exec_expr_tests)
// infidel  P_prod:  x as u32 < 33     (the `u32 <` mis-parses as a generic list)
// reference (exec_ref_value): ((x as u32) < 33)  — #146 cast-`<` outer-paren
// ============================================================================

fn e2_source() -> Expr {
    bin(
        BinOp::Lt,
        cast(path("x"), Type::Prim(PrimType::U32)),
        int(33),
    )
}

fn e2_frame() -> ExecObligationFrame {
    ExecObligationFrame {
        params: vec![ExecParamDecl::new("x", "u64")],
        ret_type: "bool".to_string(),
        req: None,
        ..Default::default()
    }
}

#[test]
fn e2_cast_lt_faithful_verifies() {
    let prog = exec_equivalence_obligation(&e2_source(), "(x as u32) < 33", &e2_frame())
        .expect("E2 faithful exec obligation builds");
    assert_faithful_verifies("e2", &prog);
}

#[test]
fn e2_cast_lt_infidel_caught() {
    // The #146 cast-`<` paren-drop: production drops the outer paren →
    // `x as u32 < 33`, where `u32 <` mis-parses as the start of a generic-argument
    // list → a HARD parse error (`error: expected ','`).
    let prog = exec_equivalence_obligation(&e2_source(), "x as u32 < 33", &e2_frame())
        .expect("E2 infidel exec obligation builds");
    assert_infidel_caught("e2", &prog, CatchShape::Compile("expected `,`"));
}

// ============================================================================
// E3 — wrong-op / overflow: the bounded-value teeth (the EXEC-value semantics)
// source exec expr: a + b   req frame: a: u64, b: u64, a + b <= 0xFFFF (no overflow)
// faithful P_prod:  a + b               (pinned in thermite-lower exec_expr_tests)
// infidel  P_prod:  a.wrapping_sub(b)   (wrong op → value differs)
// reference (exec_ref_value): (a + b)   — BOUNDED u64 (NOT a as nat + b as nat),
//   so the value-distinguishing wrong op fails `postcondition not satisfied`. A
//   nat-coerced reference would mask the wrap; the bounded reference catches it.
// ============================================================================

fn e3_source() -> Expr {
    bin(BinOp::Add, path("a"), path("b"))
}

fn e3_frame() -> ExecObligationFrame {
    ExecObligationFrame {
        params: vec![
            ExecParamDecl::new("a", "u64"),
            ExecParamDecl::new("b", "u64"),
        ],
        ret_type: "u64".to_string(),
        // a + b <= 0xFFFF: the faithful checked add is total (no overflow), so a
        // counterexample on the infidel is a VALUE difference, not an overflow of
        // the reference itself.
        req: Some("a + b <= 0xFFFF".to_string()),
        ..Default::default()
    }
}

#[test]
fn e3_wrong_op_faithful_verifies() {
    let prog = exec_equivalence_obligation(&e3_source(), "a + b", &e3_frame())
        .expect("E3 faithful exec obligation builds");
    assert_faithful_verifies("e3", &prog);
}

#[test]
fn e3_wrong_op_infidel_caught() {
    // The wrong-op infidelity: production lowers `+` to `wrapping_sub` → the value
    // differs from the bounded reference `a + b` for nearly all inputs.
    let prog = exec_equivalence_obligation(&e3_source(), "a.wrapping_sub(b)", &e3_frame())
        .expect("E3 infidel exec obligation builds");
    assert_infidel_caught("e3", &prog, CatchShape::Postcondition);
}

// ============================================================================
// E4 — off-by-one index: the bounded-access teeth
// source exec expr: xs[i]   req frame: xs: &[u32], i: usize, i < xs.len()
// faithful P_prod:  xs[i]                 (pinned in thermite-lower exec_expr_tests)
// infidel  P_prod:  xs[i + 1]             (off-by-one → value differs / OOB)
// reference (exec_ref_value): xs[i as int]  — the bounded element VALUE (the spec
//   view of the element the production xs[i] computes).
// ============================================================================

fn e4_source() -> Expr {
    Expr::Index {
        base: Box::new(path("xs")),
        index: IndexArg::Single(Box::new(path("i"))),
    }
}

fn e4_frame() -> ExecObligationFrame {
    ExecObligationFrame {
        params: vec![
            ExecParamDecl::new("xs", "&[u32]"),
            ExecParamDecl::new("i", "usize"),
        ],
        ret_type: "u32".to_string(),
        req: Some("i < xs.len()".to_string()),
        slice_params: vec!["xs".to_string()],
        ..Default::default()
    }
}

#[test]
fn e4_index_faithful_verifies() {
    let prog = exec_equivalence_obligation(&e4_source(), "xs[i]", &e4_frame())
        .expect("E4 faithful exec obligation builds");
    assert_faithful_verifies("e4", &prog);
}

#[test]
fn e4_index_infidel_caught() {
    // The off-by-one infidelity: production indexes `xs[i + 1]` for source `xs[i]`
    // → the element value differs (and `i + 1` may exceed `xs.len()`), so verus
    // fails the `ensures result == xs[i as int]` postcondition (and the index
    // bound). Both are a hard verus failure surfacing the infidelity.
    let prog = exec_equivalence_obligation(&e4_source(), "xs[i + 1]", &e4_frame())
        .expect("E4 infidel exec obligation builds");
    assert_infidel_caught("e4", &prog, CatchShape::Postcondition);
}

// ---- exec_ref_value unit checks (the reference output MEANS the faithful col) -

/// The exec reference encoding of each source must be a string that MEANS the
/// FAITHFUL production column at the BOUNDED type (the obligation tests prove
/// semantic equivalence under verus; these pin the exact independent encoding for
/// auditability — and that it is NOT nat-coerced).
#[test]
fn exec_ref_value_matches_faithful_meaning() {
    use thermite_tv::exec_encode::exec_ref_value;
    use thermite_tv::ExecRefCtx;

    // E1: #122 inner-paren, bounded u8 (NOT nat).
    assert_eq!(
        exec_ref_value(&e1_source(), &ExecRefCtx::default()).unwrap(),
        "(n - 1) as u8"
    );
    // E2: #146 cast-`<` outer-paren.
    assert_eq!(
        exec_ref_value(&e2_source(), &ExecRefCtx::default()).unwrap(),
        "((x as u32) < 33)"
    );
    // E3: bounded u64 add (NOT `a as nat + b as nat` — the overflow obligation is
    // carried, the wrap point not masked).
    assert_eq!(
        exec_ref_value(&e3_source(), &ExecRefCtx::default()).unwrap(),
        "(a + b)"
    );
    // E4: the bounded element value (the spec view of the exec element).
    assert_eq!(
        exec_ref_value(&e4_source(), &ExecRefCtx::with_slice_bound(["xs"])).unwrap(),
        "xs[i as int]"
    );
}

/// An out-of-scope construct (a method call / Vec-String accessor) is an honest
/// `Err`, NEVER a panic / silent wrong encoding (REQ-1 / R-CODE-2). This is the
/// #154/#156 territory the step-2.1 encoder honestly refuses.
#[test]
fn out_of_scope_construct_is_err_not_panic() {
    use thermite_tv::exec_encode::{exec_ref_value, RefEncodeError};
    use thermite_tv::ExecRefCtx;

    let method = Expr::MethodCall {
        receiver: Box::new(path("v")),
        name: "len".to_string(),
        args: vec![],
    };
    assert!(matches!(
        exec_ref_value(&method, &ExecRefCtx::default()),
        Err(RefEncodeError::Unsupported(_))
    ));
}
