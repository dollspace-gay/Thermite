//! DIVERGENCE PIN (blocker #165) — the GROUNDED AC-4 `if`-statement-mutation body
//! is rejected as `Unsupported` by the reference state-denotation.
//!
//! Authority: `.design/verified/exec-stmt-tv.md`
//!   - REQ-1 frozen subset IN table: row "`if` / `if-else` statement" |
//!     `Stmt::If { cond, then, else_ }` | "branch on a `bool` exec expr" — an
//!     `if`-as-STATEMENT is IN the frozen 2.2.1 straight-line subset.
//!   - AC-4 (GROUNDED): for `{ let mut r = x; if x < 10 { r = r + 1; } else
//!     { r = r + 2; } r }` (reference `if x < 10 { x+1 } else { x+2 }`), "the
//!     faithful production VERIFIES" — the doc's Verification section shows
//!     `"verification-results": { "success": true, "verified": 1, "errors": 0 }`.
//!     Independently re-confirmed against verus 0.2026.05.24 during this audit
//!     (`1 verified, 0 errors`).
//!
//! Toolchain: `thermite_tv::exec_stmt_encode::thread_stmt` (the `Stmt::If { .. }`
//! arm) returns `RefEncodeError::Unsupported("`if` as a non-tail STATEMENT ...")`,
//! so `body_ref_state` / `body_equivalence_obligation` CANNOT encode the AC-4 body
//! — the obligation builder returns `Err`. The production side
//! (`thermite_lower::lower_exec_body` → `lower_block_inner`'s `Stmt::If` arm) lowers
//! this body fine, so the GROUNDED-as-VERIFIED AC-4 body can never be discharged
//! through 2.2.1 TV. This is a Design-AC miss: REQ-1 lists the `if`-statement as IN
//! the frozen subset and AC-4 grounds it as `verified: 1`, but the reference rejects
//! it.
//!
//! Expected (authority): `body_equivalence_obligation` for the AC-4 body builds an
//! obligation `Ok(..)` whose `ensures` compares `result` to the branch-composed
//! reference `if x < 10 { (x + 1) } else { (x + 2) }`. CURRENT: it is `Err`.

use thermite_syntax::ast::{BinOp, Block, Expr, Stmt};
use thermite_tv::obligation::{body_equivalence_obligation, BodyObligationFrame, BodyParamDecl};

fn path(name: &str) -> Expr {
    Expr::Path(vec![name.to_string()])
}
fn int(v: u128) -> Expr {
    Expr::IntLit {
        value: v,
        raw: v.to_string(),
    }
}
fn bin(op: BinOp, l: Expr, r: Expr) -> Expr {
    Expr::Binary {
        op,
        lhs: Box::new(l),
        rhs: Box::new(r),
    }
}

/// The GROUNDED AC-4 source body: `{ let mut r = x; if x < 10 { r = r + 1; }
/// else { r = r + 2; } r }` — the `if` is a STATEMENT mutating the outer cell `r`,
/// with `r` as the body tail. (`.design/verified/exec-stmt-tv.md` AC-4.)
fn ac4_body() -> Block {
    let then = Block {
        stmts: vec![Stmt::Assign {
            target: path("r"),
            value: bin(BinOp::Add, path("r"), int(1)),
        }],
        tail: None,
    };
    let els = Block {
        stmts: vec![Stmt::Assign {
            target: path("r"),
            value: bin(BinOp::Add, path("r"), int(2)),
        }],
        tail: None,
    };
    Block {
        stmts: vec![
            Stmt::Let {
                mutable: true,
                name: "r".to_string(),
                ty: None,
                init: path("x"),
            },
            Stmt::If {
                cond: bin(BinOp::Lt, path("x"), int(10)),
                then,
                else_: Some(els),
            },
        ],
        tail: Some(Box::new(path("r"))),
    }
}

fn ac4_frame() -> BodyObligationFrame {
    BodyObligationFrame {
        params: vec![BodyParamDecl::new("x", "u64")],
        ret_type: "u64".to_string(),
        // x <= 1000 so both arms (x+1, x+2) are total — the AC-4 frame.
        req: Some("x <= 1000".to_string()),
        ..Default::default()
    }
}

/// The body-refinement obligation for the GROUNDED AC-4 body MUST build (the design
/// lists the `if`-statement as IN the frozen subset, REQ-1, and grounds it as
/// `verified: 1`, AC-4). It currently returns `Err(Unsupported)` from `thread_stmt`.
#[test]
fn divergence_ac4_if_stmt_mutation_obligation_builds() {
    let faithful = "    let mut r = x;\n    if x < 10 { r = r + 1; } else { r = r + 2; }\n    r\n";
    let result = body_equivalence_obligation(&ac4_body(), faithful, &ac4_frame());
    // Authority: REQ-1 admits the `if`-statement; AC-4 grounds it verified. The
    // obligation MUST build (Ok), and its `ensures` MUST compare `result` to the
    // branch-composed reference (NOT be an `Err`). This FAILS today: the builder
    // returns `Err(Unsupported(..))` because `thread_stmt` rejects every `Stmt::If`.
    let prog = result.unwrap_or_else(|e| {
        panic!(
            "AC-4 (GROUNDED, verified:1) if-statement-mutation body must build a body \
             obligation, but the reference rejected it: {e:?}"
        )
    });
    // The reference final state of the AC-4 body is the branch-composed
    // `if x < 10 { (x + 1) } else { (x + 2) }` (REQ-2 state-transformer + AC-4).
    assert!(
        prog.contains("if x < 10 { (x + 1) } else { (x + 2) }"),
        "AC-4 obligation `ensures` must compare `result` to the branch-composed \
         reference `if x < 10 {{ (x + 1) }} else {{ (x + 2) }}` (REQ-2 / AC-4); got:\n{prog}"
    );
}
