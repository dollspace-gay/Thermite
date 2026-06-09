//! THE R-CHAR-3 BODY-TEETH-TEST (`.design/verified/exec-stmt-tv.md` REQ-3 /
//! AC-1..AC-4; epic crosslink #158, blockers #159/#160/#161). The proof that
//! straight-line exec-BODY state-refinement TV (step 2.2.1) DISCRIMINATES a faithful
//! body lowering from an injected STATE-TRANSFORMATION infidelity — the class the
//! per-EXPRESSION step-2.1 TV structurally CANNOT see: a dropped statement, a
//! REORDERED mutation, a swapped `if`-branch, a multi-cell projection error. Each is
//! a final-STATE difference while every individual sub-expression stays
//! value-faithful (the orthogonal axis 2.2.1 adds on top of 2.1).
//!
//! For each ORCHESTRATOR-authored fixture (B1-B4) the test builds the body
//! state-refinement obligation TWICE — once with the FAITHFUL `P_production` (the
//! exact string `thermite_lower::lower_exec_body` emits, PINNED by
//! `thermite-lower/src/lower.rs::exec_body_tests` — the cross-crate bridge, since
//! this INDEPENDENT crate has NO `thermite-lower` dep), once with the INFIDEL one
//! (the hand-injected state bug from the fixture). TV must:
//!   - faithful -> VERIFY (`verified >= 1, errors == 0`);
//!   - infidel  -> be CAUGHT — a `postcondition not satisfied` counterexample (the
//!     final state differs from the reference state-denotation).
//!
//! THE STATE-THREADING IS REAL (the load-bearing point): B2's reorder is caught
//! BECAUSE the FINAL STATE differs — each RHS (`s + 1`, `s * 2`) is value-faithful in
//! ISOLATION (step 2.1 would pass each), the ORDER is the bug. The reference
//! `body_ref_state` threads the substitution chain in order, so the reordered
//! production's final state `((x * 2) + 1)` is provably != the reference
//! `((x + 1) * 2)`.
//!
//! Expected values trace to the fixtures + `exec-stmt-tv.md` REQ-2 state-transformer
//! semantics — NEVER copied from the lowerer's output (R-CHAR-3); the FAITHFUL
//! `P_production` is the production lowering, pinned independently in
//! `thermite-lower`'s own `exec_body_tests`. Verus is resolved via `VERUS_BIN`/PATH/
//! `~/.local/bin` and the test SKIPS LOUDLY if it is genuinely absent (mirroring
//! `thermite-tv/tests/exec_teeth.rs`); `unwrap`/`expect` are fine here (`tests/` is
//! not anti-pattern-gated).

use std::path::{Path, PathBuf};
use std::process::Command;

use thermite_syntax::ast::{BinOp, Block, Expr, Stmt};
use thermite_tv::obligation::{body_equivalence_obligation, BodyObligationFrame, BodyParamDecl};

// ---- AST construction helpers (the source straight-line bodies) -------------

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

fn let_(mutable: bool, name: &str, init: Expr) -> Stmt {
    Stmt::Let {
        mutable,
        name: name.to_string(),
        ty: None,
        init,
    }
}

fn assign(target: &str, value: Expr) -> Stmt {
    Stmt::Assign {
        target: path(target),
        value,
    }
}

// ---- verus resolution + discharge (mirrors exec_teeth.rs) ------------------

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

/// Discharge a faithful body obligation: it MUST verify. SKIPS LOUDLY if verus is
/// absent.
fn assert_faithful_verifies(fixture: &str, program: &str) {
    let tmp = std::env::temp_dir().join(format!("tv_body_teeth_{fixture}_faithful.rs"));
    std::fs::write(&tmp, program).unwrap_or_else(|e| panic!("write {fixture} faithful: {e}"));
    match run_verus(&tmp) {
        Some((ok, output)) => {
            let (verified, errors) = parse_results(&output).unwrap_or_else(|| {
                panic!("{fixture} faithful: no verus results line:\n{output}\n--- program ---\n{program}")
            });
            assert!(
                ok && errors == 0 && verified >= 1,
                "{fixture} FAITHFUL body obligation did NOT verify (a TV false positive — the \
                 faithful production body lowering should produce the reference FINAL STATE). \
                 exit_success={ok} verified={verified} errors={errors}\n--- verus output ---\n\
                 {output}\n--- program ---\n{program}"
            );
            eprintln!("FAITHFUL {fixture}: verus = {verified} verified, {errors} errors (PASS)");
        }
        None => eprintln!(
            "SKIP: verus not available — {fixture} faithful body obligation not discharged \
             (set VERUS_BIN or install verus on PATH)."
        ),
    }
}

/// Discharge an infidel body obligation: TV MUST catch it as a `postcondition not
/// satisfied` counterexample (the final STATE differs from the reference
/// state-denotation). SKIPS LOUDLY if verus is absent. The catch shape is asserted
/// precisely so the teeth bite for the RIGHT reason (R-CHAR-3 — a spurious unrelated
/// failure is NOT a pass).
fn assert_infidel_caught(fixture: &str, program: &str) {
    let tmp = std::env::temp_dir().join(format!("tv_body_teeth_{fixture}_infidel.rs"));
    std::fs::write(&tmp, program).unwrap_or_else(|e| panic!("write {fixture} infidel: {e}"));
    match run_verus(&tmp) {
        Some((_ok, output)) => {
            let (_verified, errors) = parse_results(&output).unwrap_or_else(|| {
                panic!(
                    "{fixture} infidel: expected a postcondition counterexample but no verus \
                     results line:\n{output}\n--- program ---\n{program}"
                )
            });
            assert!(
                errors >= 1,
                "{fixture} INFIDEL body obligation VERIFIED — TV FAILED TO CATCH the \
                 state-transformation infidelity (the teeth did not bite). errors={errors}\n\
                 --- verus output ---\n{output}\n--- program ---\n{program}"
            );
            assert!(
                output.contains("postcondition not satisfied"),
                "{fixture} infidel failed but NOT at the `ensures result == <ref state>` \
                 postcondition (the catch is the wrong one — not a final-state difference):\n\
                 {output}"
            );
            eprintln!(
                "INFIDEL {fixture}: verus = {errors} errors (postcondition — final state differs) \
                 — TV CAUGHT it (PASS)"
            );
        }
        None => {
            eprintln!(
                "SKIP: verus not available — {fixture} infidel body obligation not discharged."
            )
        }
    }
}

// ============================================================================
// B1 straight-line — the let-chain teeth (a DROPPED statement)
// source body: { let a = x + 1; let b = a * 2; b }   frame: x: u64, x <= 1000
// faithful P_prod: the real lower_exec_body (pinned exec_body_tests::b1)
// infidel  P_prod: drops `let b = a * 2`, returns `a` (final state x+1 != (x+1)*2)
// reference (body_ref_state): ((x + 1) * 2)
// ============================================================================

fn b1_body() -> Block {
    Block {
        stmts: vec![
            let_(false, "a", bin(BinOp::Add, path("x"), int(1))),
            let_(false, "b", bin(BinOp::Mul, path("a"), int(2))),
        ],
        tail: Some(Box::new(path("b"))),
    }
}

fn b1_frame() -> BodyObligationFrame {
    BodyObligationFrame {
        params: vec![BodyParamDecl::new("x", "u64")],
        ret_type: "u64".to_string(),
        req: Some("x <= 1000".to_string()),
        ..Default::default()
    }
}

#[test]
fn b1_straight_line_faithful_verifies() {
    let prog = body_equivalence_obligation(
        &b1_body(),
        "    let a = x + 1;\n    let b = a * 2;\n    b\n",
        &b1_frame(),
    )
    .expect("B1 faithful body obligation builds");
    assert_faithful_verifies("b1", &prog);
}

#[test]
fn b1_dropped_statement_infidel_caught() {
    // The dropped-statement infidelity: production drops `let b = a * 2` and returns
    // `a` -> final state `x + 1`, != the reference `((x + 1) * 2)`.
    let prog = body_equivalence_obligation(&b1_body(), "    let a = x + 1;\n    a\n", &b1_frame())
        .expect("B1 infidel body obligation builds");
    assert_infidel_caught("b1", &prog);
}

// ============================================================================
// B2 mutation-order — the STATE-SEQUENCING teeth (the load-bearing fixture)
// source body: { let mut s = x; s = s + 1; s = s * 2; s }  frame: x: u64, x <= 1000
// faithful P_prod: the real lower_exec_body (pinned exec_body_tests::b2)
// infidel  P_prod: REORDERED `s = s * 2; s = s + 1` (final state (x*2)+1 != (x+1)*2)
// reference (body_ref_state): ((x + 1) * 2)
// Each RHS (s + 1, s * 2) is value-faithful in isolation — the ORDER is the bug.
// ============================================================================

fn b2_body() -> Block {
    Block {
        stmts: vec![
            let_(true, "s", path("x")),
            assign("s", bin(BinOp::Add, path("s"), int(1))),
            assign("s", bin(BinOp::Mul, path("s"), int(2))),
        ],
        tail: Some(Box::new(path("s"))),
    }
}

fn b2_frame() -> BodyObligationFrame {
    BodyObligationFrame {
        params: vec![BodyParamDecl::new("x", "u64")],
        ret_type: "u64".to_string(),
        req: Some("x <= 1000".to_string()),
        ..Default::default()
    }
}

#[test]
fn b2_mutation_order_faithful_verifies() {
    let prog = body_equivalence_obligation(
        &b2_body(),
        "    let mut s = x;\n    s = s + 1;\n    s = s * 2;\n    s\n",
        &b2_frame(),
    )
    .expect("B2 faithful body obligation builds");
    assert_faithful_verifies("b2", &prog);
}

#[test]
fn b2_reordered_mutation_infidel_caught() {
    // The REORDERED-mutation infidelity: production threads `s = s * 2; s = s + 1`
    // -> final state `(x * 2) + 1`, != the reference `((x + 1) * 2)`. Each RHS is
    // value-faithful in isolation (step 2.1 passes each); the ORDER changes the
    // final state — exactly what 2.2.1 adds on top of 2.1.
    let prog = body_equivalence_obligation(
        &b2_body(),
        "    let mut s = x;\n    s = s * 2;\n    s = s + 1;\n    s\n",
        &b2_frame(),
    )
    .expect("B2 infidel body obligation builds");
    assert_infidel_caught("b2", &prog);
}

// ============================================================================
// B3 if-branch — the branch state-transformer teeth (a SWAPPED branch)
// source body: { if c { x + 1 } else { x - 1 } }  frame: c: bool, x: u64, 1<=x<=1000
// faithful P_prod: the real lower_exec_body (pinned exec_body_tests::b3)
// infidel  P_prod: SWAPPED branches `if c { x - 1 } else { x + 1 }`
// reference (body_ref_state): if c { (x + 1) } else { (x - 1) }
// ============================================================================

fn b3_body() -> Block {
    let then = Block {
        stmts: vec![],
        tail: Some(Box::new(bin(BinOp::Add, path("x"), int(1)))),
    };
    let els = Block {
        stmts: vec![],
        tail: Some(Box::new(bin(BinOp::Sub, path("x"), int(1)))),
    };
    Block {
        stmts: vec![],
        tail: Some(Box::new(Expr::If {
            cond: Box::new(path("c")),
            then,
            else_: els,
        })),
    }
}

fn b3_frame() -> BodyObligationFrame {
    BodyObligationFrame {
        params: vec![
            BodyParamDecl::new("c", "bool"),
            BodyParamDecl::new("x", "u64"),
        ],
        ret_type: "u64".to_string(),
        // 1 <= x (so the `else` `x - 1` is total — no underflow) and x <= 1000 (so
        // the `then` `x + 1` is total — no overflow). The B3 bug is the SWAP, not a
        // wrap.
        req: Some("1 <= x, x <= 1000".to_string()),
        ..Default::default()
    }
}

#[test]
fn b3_if_branch_faithful_verifies() {
    let prog = body_equivalence_obligation(
        &b3_body(),
        "    if c { x + 1 } else { x - 1 }\n",
        &b3_frame(),
    )
    .expect("B3 faithful body obligation builds");
    assert_faithful_verifies("b3", &prog);
}

#[test]
fn b3_swapped_branch_infidel_caught() {
    // The swapped-branch infidelity: production swaps the arms -> for a given `c`
    // the final state is the OTHER branch's value, != the reference.
    let prog = body_equivalence_obligation(
        &b3_body(),
        "    if c { x - 1 } else { x + 1 }\n",
        &b3_frame(),
    )
    .expect("B3 infidel body obligation builds");
    assert_infidel_caught("b3", &prog);
}

// ============================================================================
// B4 multi-cell tuple — the multi-cell projection teeth (the design's #1)
// source body: { let mut a = x; let mut b = y; a = a + 1; b = b + a; (a, b) }
//   frame: x: u64, y: u64, x <= 1000, y <= 1000
// faithful P_prod: the real lower_exec_body (pinned exec_body_tests::b4)
// infidel  P_prod: `b = b + x` (uses the OLD x, not the UPDATED a) -> b cell wrong
// reference (body_ref_state): ((x + 1), (y + (x + 1)))
// ============================================================================

fn b4_body() -> Block {
    Block {
        stmts: vec![
            let_(true, "a", path("x")),
            let_(true, "b", path("y")),
            assign("a", bin(BinOp::Add, path("a"), int(1))),
            assign("b", bin(BinOp::Add, path("b"), path("a"))),
        ],
        tail: Some(Box::new(Expr::Tuple(vec![path("a"), path("b")]))),
    }
}

fn b4_frame() -> BodyObligationFrame {
    BodyObligationFrame {
        params: vec![
            BodyParamDecl::new("x", "u64"),
            BodyParamDecl::new("y", "u64"),
        ],
        ret_type: "(u64, u64)".to_string(),
        // x, y <= 1000 so a + 1 and b + a are total (no overflow); the B4 bug is the
        // WRONG CELL (b uses old x not updated a), not a wrap.
        req: Some("x <= 1000, y <= 1000".to_string()),
        ..Default::default()
    }
}

#[test]
fn b4_multi_cell_tuple_faithful_verifies() {
    let prog = body_equivalence_obligation(
        &b4_body(),
        "    let mut a = x;\n    let mut b = y;\n    a = a + 1;\n    b = b + a;\n    (a, b)\n",
        &b4_frame(),
    )
    .expect("B4 faithful body obligation builds");
    assert_faithful_verifies("b4", &prog);
}

#[test]
fn b4_wrong_cell_infidel_caught() {
    // The multi-cell projection infidelity: production threads `b = b + x` (the OLD
    // x) instead of `b = b + a` (the UPDATED a) -> the b cell's final state is
    // `y + x`, != the reference `(y + (x + 1))`. The a cell is unchanged — only the
    // b PROJECTION of the tuple is wrong, so the teeth bite on the tuple comparison.
    let prog = body_equivalence_obligation(
        &b4_body(),
        "    let mut a = x;\n    let mut b = y;\n    a = a + 1;\n    b = b + x;\n    (a, b)\n",
        &b4_frame(),
    )
    .expect("B4 infidel body obligation builds");
    assert_infidel_caught("b4", &prog);
}

// ---- body_ref_state unit checks (the reference output MEANS the final state) -

/// The reference state-denotation of each source body must be the closed-form FINAL
/// STATE in the inputs (the obligation tests prove semantic equivalence under verus;
/// these pin the exact independent encoding for auditability — and that the state is
/// threaded in ORDER, not just value-faithful per RHS).
#[test]
fn body_ref_state_matches_final_state() {
    use thermite_tv::body_ref_state;
    use thermite_tv::BodyRefCtx;

    // B1: the let-chain threads to ((x + 1) * 2).
    assert_eq!(
        body_ref_state(&b1_body(), &BodyRefCtx::default()).unwrap(),
        "((x + 1) * 2)"
    );
    // B2: the ordered mutation threads to ((x + 1) * 2) (NOT (x*2)+1 — the order).
    assert_eq!(
        body_ref_state(&b2_body(), &BodyRefCtx::default()).unwrap(),
        "((x + 1) * 2)"
    );
    // B3: the if-tail composes the two branch transformers.
    assert_eq!(
        body_ref_state(&b3_body(), &BodyRefCtx::default()).unwrap(),
        "if c { (x + 1) } else { (x - 1) }"
    );
    // B4: the multi-cell tuple projects a |-> (x+1), b |-> (y + (x+1)) (b uses the
    // UPDATED a — the order-sensitive threading).
    assert_eq!(
        body_ref_state(&b4_body(), &BodyRefCtx::default()).unwrap(),
        "((x + 1), (y + (x + 1)))"
    );
}
