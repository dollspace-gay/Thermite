//! THE R-CHAR-3 LOOP-TEETH-TEST (`.design/verified/loop-tv.md` REQ-2 / AC-1..AC-4;
//! epic crosslink #169, blocker #163). The proof that the v1 frozen-subset `while`
//! loop TV (step 2.2.2-i) DISCRIMINATES a faithful loop lowering from an injected
//! per-iteration / after-loop infidelity, via the THREE per-run obligations:
//!   - ENTRY     — the invariant holds on the pre-loop entry state;
//!   - PRESERVATION — one straight-line iteration carries `inv ∧ cond` to `inv`
//!     (REUSING the SHIPPED `body_ref_state` single-iteration step);
//!   - EXIT      — the after-loop state is `inv ∧ ¬cond`-constrained.
//!
//! The four conformance pins (`loop-tv.md` AC-1..AC-4):
//!   - L1 (faithful `while` loop) — all THREE obligations VERIFY (`verified >= 1,
//!     errors == 0`). The HAND-DERIVED verdicts are in the L1 comment.
//!   - L2 (broken-preservation mutant) — a production loop body that mutates a cell
//!     the invariant constrains in a way that breaks the per-iteration step (`lo + 2`
//!     for source `lo + 1`) fails the PRESERVATION obligation with `postcondition not
//!     satisfied` (the `body_ref_sound` per-iteration teeth, AC-2/AC-5).
//!   - L3 (wrong-after-loop-state mutant) — a production after-loop characterization
//!     that OVER-CLAIMS (stronger than `inv ∧ ¬cond` — claims `lo > hi` when only
//!     `lo == hi` follows) fails the EXIT obligation (a counterexample, AC-3).
//!   - L4 (loop-without-usable-inv / out-of-v1) — a `loop`-kind, a `break` body, a
//!     mid-body `return`, and a trivially-weak `inv true` are each an honest
//!     `RefEncodeError::Unsupported` (Skipped, NEVER silently Faithful — AC-4 /
//!     R-HONEST-3). NO verus needed (the obligation builder refuses to emit).
//!
//! THE TEETH ARE REAL (the load-bearing point, R-CHAR-3): the L2 mutant's per-
//! iteration state DIFFERS from the reference `body_ref_state` step, so the
//! preservation `ensures result.i == <step_i>` is provably violated; the L3 mutant's
//! over-claim is provably NOT implied by `inv ∧ ¬cond`. Expected verdicts are derived
//! BY HAND in the fixture comments from `loop-tv.md` REQ-2's obligation forms — NEVER
//! copied from the toolchain's own output. The FAITHFUL `p_production` is the Verus-
//! native loop-body lowering shape (`lower_loop` in `thermite-lower`), authored here
//! as the cross-crate faithful bridge (this INDEPENDENT crate has NO `thermite-lower`
//! dep). Verus is resolved via `VERUS_BIN`/PATH/`~/.local/bin` and the test SKIPS
//! LOUDLY if it is genuinely absent (mirroring `body_teeth.rs`); `unwrap`/`expect` are
//! fine here (`tests/` is not anti-pattern-gated).

use std::path::{Path, PathBuf};
use std::process::Command;

use thermite_syntax::ast::{BinOp, Block, Clause, Expr, LoopKind, LoopNode, Stmt};
use thermite_syntax::lexer::Span;
use thermite_tv::exec_encode::RefEncodeError;
use thermite_tv::obligation::{
    loop_entry_obligation, loop_exit_obligation, loop_preservation_obligation, LoopObligationFrame,
    LoopParamDecl,
};
use thermite_tv::{loop_ref_obligations, BodyRefCtx};

// ---- AST construction helpers (the source loops) ---------------------------

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

fn clause(expr: Expr) -> Clause {
    Clause {
        expr,
        text: String::new(),
        span: Span { start: 0, len: 0 },
    }
}

// ---- verus resolution + discharge (mirrors body_teeth.rs) ------------------

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

/// Discharge a faithful loop obligation: it MUST verify. SKIPS LOUDLY if verus absent.
fn assert_obligation_verifies(fixture: &str, program: &str) {
    let tmp = std::env::temp_dir().join(format!("tv_loop_teeth_{fixture}.rs"));
    std::fs::write(&tmp, program).unwrap_or_else(|e| panic!("write {fixture}: {e}"));
    match run_verus(&tmp) {
        Some((ok, output)) => {
            let (verified, errors) = parse_results(&output).unwrap_or_else(|| {
                panic!("{fixture}: no verus results line:\n{output}\n--- program ---\n{program}")
            });
            assert!(
                ok && errors == 0 && verified >= 1,
                "{fixture} FAITHFUL loop obligation did NOT verify (a TV false positive). \
                 exit_success={ok} verified={verified} errors={errors}\n--- verus output ---\n\
                 {output}\n--- program ---\n{program}"
            );
            eprintln!("FAITHFUL {fixture}: verus = {verified} verified, {errors} errors (PASS)");
        }
        None => eprintln!(
            "SKIP: verus not available — {fixture} faithful loop obligation not discharged \
             (set VERUS_BIN or install verus on PATH)."
        ),
    }
}

/// Discharge an infidel loop obligation: TV MUST catch it. `expect_msg` is the precise
/// catch shape (so the teeth bite for the RIGHT reason, R-CHAR-3). SKIPS LOUDLY if
/// verus absent.
fn assert_obligation_caught(fixture: &str, program: &str, expect_msg: &str) {
    let tmp = std::env::temp_dir().join(format!("tv_loop_teeth_{fixture}.rs"));
    std::fs::write(&tmp, program).unwrap_or_else(|e| panic!("write {fixture}: {e}"));
    match run_verus(&tmp) {
        Some((_ok, output)) => {
            let (_verified, errors) = parse_results(&output).unwrap_or_else(|| {
                panic!(
                    "{fixture}: expected a counterexample but no verus results line:\n{output}\n\
                     --- program ---\n{program}"
                )
            });
            assert!(
                errors >= 1,
                "{fixture} INFIDEL loop obligation VERIFIED — TV FAILED TO CATCH the infidelity \
                 (the teeth did not bite). errors={errors}\n--- verus output ---\n{output}\n\
                 --- program ---\n{program}"
            );
            assert!(
                output.contains(expect_msg),
                "{fixture} infidel failed but NOT at the expected `{expect_msg}` site (the catch is \
                 the wrong one):\n{output}"
            );
            eprintln!(
                "INFIDEL {fixture}: verus = {errors} errors ({expect_msg}) — TV CAUGHT it (PASS)"
            );
        }
        None => {
            eprintln!(
                "SKIP: verus not available — {fixture} infidel loop obligation not discharged."
            )
        }
    }
}

// ============================================================================
// The L1 FAITHFUL v1-frozen-subset loop fixture (shared by L1/L2/L3).
//
// source loop (the v1 subset — single `while`, declared inv/dec, straight-line
// scalar body, the loop the LAST statement before the tail):
//
//   {
//     let mut lo: usize = 0;
//     while lo < n
//       inv lo <= n
//       dec n - lo
//     { lo = lo + 1; }
//     lo
//   }
//
// cells (sorted): [lo].   inputs: [n: usize].   (`n` is a read-only input — the body
// mutates only `lo`, so `lo` is the SOLE per-iteration state cell.)
//
// HAND-DERIVED reference pieces (`loop-tv.md` REQ-2; NOT copied from the toolchain):
//   - entry_pred  = (0 <= n)                         [entry env lo->0]
//   - cond        = (lo < n)
//   - inv         = (lo <= n)
//   - step (body `lo = lo + 1`): lo->(lo + 1)
//       step_cells (order [lo]) = ["(lo + 1)"]
//       inv_at_step = ((lo + 1) <= n)
//   - !cond       = (!((lo < n)))
// ============================================================================

fn l1_block() -> Block {
    let loop_node = LoopNode {
        kind: LoopKind::While(Box::new(bin(BinOp::Lt, path("lo"), path("n")))),
        invs: vec![clause(bin(BinOp::Le, path("lo"), path("n")))],
        dec: clause(bin(BinOp::Sub, path("n"), path("lo"))),
        body: Block {
            stmts: vec![assign("lo", bin(BinOp::Add, path("lo"), int(1)))],
            tail: None,
        },
        span: Span { start: 0, len: 0 },
    };
    Block {
        stmts: vec![let_(true, "lo", int(0)), Stmt::Loop(loop_node)],
        tail: Some(Box::new(path("lo"))),
    }
}

fn l1_frame() -> LoopObligationFrame {
    LoopObligationFrame {
        inputs: vec![LoopParamDecl::new("n", "usize")],
        // cells: the SOLE mutated cell `lo`.
        cells: vec![LoopParamDecl::new("lo", "usize")],
        // The enclosing fn frame (n bounded so the entry/step arithmetic is total).
        req: Some("n <= 1000".to_string()),
        ..Default::default()
    }
}

// ---- L0: the reference pieces match the HAND-DERIVED encoding (auditability) ---

#[test]
fn l0_loop_ref_obligations_match_hand_derived() {
    let obs = loop_ref_obligations(&l1_block(), &BodyRefCtx::default())
        .expect("L1 loop is in the v1 frozen subset");
    assert_eq!(obs.cells, vec!["lo".to_string()]);
    assert_eq!(obs.entry_pred, "(0 <= n)");
    assert_eq!(obs.cond, "(lo < n)");
    assert_eq!(obs.inv, "(lo <= n)");
    assert_eq!(obs.step_cells, vec!["(lo + 1)".to_string()]);
    assert_eq!(obs.inv_at_step, "((lo + 1) <= n)");
}

// ---- L1: the FAITHFUL loop → all THREE obligations VERIFY (AC-1) ------------

#[test]
fn l1_entry_obligation_verifies() {
    let prog = loop_entry_obligation(&l1_block(), &l1_frame()).expect("L1 entry obligation builds");
    assert_obligation_verifies("l1_entry", &prog);
}

#[test]
fn l1_preservation_obligation_verifies() {
    // FAITHFUL p_production: the Verus-native loop-body lowering shape (lower_loop) —
    // the SOLE cell `lo` is shadowed `let mut`, the body runs, the stepped cell is the
    // tail value (single-cell — the bare `lo`, not a tuple).
    let prog = loop_preservation_obligation(
        &l1_block(),
        "    let mut lo = lo;\n    lo = lo + 1;\n    lo\n",
        &l1_frame(),
    )
    .expect("L1 preservation obligation builds");
    assert_obligation_verifies("l1_preservation", &prog);
}

#[test]
fn l1_exit_obligation_verifies() {
    // FAITHFUL after-loop characterization: `lo == n` genuinely FOLLOWS from
    // `inv ∧ ¬cond` (lo <= n && !(lo < n) ==> lo == n). Non-trivial (it is the exit
    // fact the continuation reads), not a vacuous tautology.
    let prog = loop_exit_obligation(&l1_block(), "lo == n", &l1_frame())
        .expect("L1 exit obligation builds");
    assert_obligation_verifies("l1_exit", &prog);
}

// ---- L2: broken-preservation mutant → CAUGHT (AC-2 / AC-5) -----------------

#[test]
fn l2_broken_preservation_caught() {
    // The per-iteration infidelity: production steps `lo = lo + 2` (the source step is
    // `lo + 1`). The reference step_cells.1 = `(lo + 1)`, so production's returned
    // `lo + 2 != lo + 1` → the body-TV `ensures result.1 == (lo + 1)` is provably
    // violated (a `postcondition not satisfied` — the SAME teeth `body_ref_sound`'s
    // per-iteration negative lemmas bite). This is the AC-5 REUSE: the single-iteration
    // step is the SHIPPED body_ref_state, and a wrong per-iteration mutation breaks the
    // preservation obligation, NOT a silent pass.
    let prog = loop_preservation_obligation(
        &l1_block(),
        "    let mut lo = lo;\n    lo = lo + 2;\n    lo\n",
        &l1_frame(),
    )
    .expect("L2 mutant preservation obligation builds");
    assert_obligation_caught(
        "l2_broken_preservation",
        &prog,
        "postcondition not satisfied",
    );
}

// ---- L3: wrong-after-loop-state mutant → CAUGHT (AC-3) ---------------------

#[test]
fn l3_wrong_exit_characterization_caught() {
    // The after-loop OVER-CLAIM: production characterizes the exit state as `lo > n`,
    // STRONGER than the genuine `inv ∧ ¬cond` (which gives only `lo == n`). From
    // `lo <= n && !(lo < n)` the claim `lo > n` is FALSE (we have `lo <= n`), so the
    // exit assertion fails with a counterexample. A wrong after-loop characterization
    // is CAUGHT, never silently accepted.
    let prog = loop_exit_obligation(&l1_block(), "lo > n", &l1_frame())
        .expect("L3 mutant exit obligation builds");
    assert_obligation_caught("l3_wrong_exit", &prog, "assertion failed");
}

// ---- L4: out-of-v1 loops → honest Skipped (Unsupported), NEVER Faithful ----
//
// Each OUT-of-v1 form makes the obligation builder REFUSE to emit (an honest
// `RefEncodeError::Unsupported`), NOT a silent wrong encoding. No verus needed —
// the refusal IS the honest Skip (`loop-tv.md` AC-4 / R-HONEST-3).

fn skipped_block(loop_node: LoopNode) -> Block {
    Block {
        stmts: vec![
            let_(true, "lo", int(0)),
            let_(true, "hi", path("n")),
            Stmt::Loop(loop_node),
        ],
        tail: Some(Box::new(path("lo"))),
    }
}

#[test]
fn l4_loop_kind_is_skipped() {
    // A `loop`-kind (the infinite-loop / multi-exit form — the corpus binary_search
    // shape) is OUT of the v1 single-`while` subset.
    let loop_node = LoopNode {
        kind: LoopKind::Loop,
        invs: vec![clause(bin(BinOp::Le, path("lo"), path("hi")))],
        dec: clause(bin(BinOp::Sub, path("hi"), path("lo"))),
        body: Block {
            stmts: vec![assign("lo", bin(BinOp::Add, path("lo"), int(1)))],
            tail: None,
        },
        span: Span { start: 0, len: 0 },
    };
    let block = skipped_block(loop_node);
    assert!(matches!(
        loop_ref_obligations(&block, &BodyRefCtx::default()),
        Err(RefEncodeError::Unsupported(_))
    ));
    // The obligation EMITTERS propagate the honest Skip too (never silently Faithful).
    assert!(matches!(
        loop_entry_obligation(&block, &l1_frame()),
        Err(RefEncodeError::Unsupported(_))
    ));
    assert!(matches!(
        loop_exit_obligation(&block, "lo == hi", &l1_frame()),
        Err(RefEncodeError::Unsupported(_))
    ));
}

#[test]
fn l4_break_body_is_skipped() {
    // A `break` in the body is a multi-exit control form — OUT of v1.
    let loop_node = LoopNode {
        kind: LoopKind::While(Box::new(bin(BinOp::Lt, path("lo"), path("hi")))),
        invs: vec![clause(bin(BinOp::Le, path("lo"), path("hi")))],
        dec: clause(bin(BinOp::Sub, path("hi"), path("lo"))),
        body: Block {
            stmts: vec![
                assign("lo", bin(BinOp::Add, path("lo"), int(1))),
                Stmt::Break,
            ],
            tail: None,
        },
        span: Span { start: 0, len: 0 },
    };
    assert!(matches!(
        loop_ref_obligations(&skipped_block(loop_node), &BodyRefCtx::default()),
        Err(RefEncodeError::Unsupported(_))
    ));
}

#[test]
fn l4_mid_body_return_is_skipped() {
    // A mid-body `return` (the binary_search `return None` shape) is a multi-exit CPS
    // form — OUT of v1.
    let loop_node = LoopNode {
        kind: LoopKind::While(Box::new(bin(BinOp::Lt, path("lo"), path("hi")))),
        invs: vec![clause(bin(BinOp::Le, path("lo"), path("hi")))],
        dec: clause(bin(BinOp::Sub, path("hi"), path("lo"))),
        body: Block {
            stmts: vec![
                Stmt::Return(Some(path("lo"))),
                assign("lo", bin(BinOp::Add, path("lo"), int(1))),
            ],
            tail: None,
        },
        span: Span { start: 0, len: 0 },
    };
    assert!(matches!(
        loop_ref_obligations(&skipped_block(loop_node), &BodyRefCtx::default()),
        Err(RefEncodeError::Unsupported(_))
    ));
}

#[test]
fn l4_trivially_weak_inv_is_skipped() {
    // A trivially-weak `inv true` makes the after-loop `true ∧ ¬cond` vacuous — the
    // loop cannot enter the (a) rule. Skipped HONESTLY (NOT Faithful — the after-loop
    // characterization would be meaningless).
    let loop_node = LoopNode {
        kind: LoopKind::While(Box::new(bin(BinOp::Lt, path("lo"), path("hi")))),
        invs: vec![clause(Expr::BoolLit(true))],
        dec: clause(bin(BinOp::Sub, path("hi"), path("lo"))),
        body: Block {
            stmts: vec![assign("lo", bin(BinOp::Add, path("lo"), int(1)))],
            tail: None,
        },
        span: Span { start: 0, len: 0 },
    };
    assert!(matches!(
        loop_ref_obligations(&skipped_block(loop_node), &BodyRefCtx::default()),
        Err(RefEncodeError::Unsupported(_))
    ));
}

#[test]
fn l4_nested_loop_is_skipped() {
    // A NESTED loop in the body — the inner loop's after-state is itself a fixpoint
    // inside the outer body-step. OUT of v1.
    let inner = LoopNode {
        kind: LoopKind::While(Box::new(bin(BinOp::Lt, path("lo"), path("hi")))),
        invs: vec![clause(bin(BinOp::Le, path("lo"), path("hi")))],
        dec: clause(bin(BinOp::Sub, path("hi"), path("lo"))),
        body: Block {
            stmts: vec![assign("lo", bin(BinOp::Add, path("lo"), int(1)))],
            tail: None,
        },
        span: Span { start: 0, len: 0 },
    };
    let outer = LoopNode {
        kind: LoopKind::While(Box::new(bin(BinOp::Lt, path("lo"), path("hi")))),
        invs: vec![clause(bin(BinOp::Le, path("lo"), path("hi")))],
        dec: clause(bin(BinOp::Sub, path("hi"), path("lo"))),
        body: Block {
            stmts: vec![Stmt::Loop(inner)],
            tail: None,
        },
        span: Span { start: 0, len: 0 },
    };
    assert!(matches!(
        loop_ref_obligations(&skipped_block(outer), &BodyRefCtx::default()),
        Err(RefEncodeError::Unsupported(_))
    ));
}
