//! CRITIC divergence pin (blocker #145): `ref_encode` is OVERFIT to the
//! slice+predicate combinator shape of F2 (`forall_in`/`exists_in`/`count_where`)
//! and mis-encodes the THREE-ARG combinators that take an `int` INDEX argument
//! (`forall_below`, `forall_from` — `arg_kinds = [Slice, Index, Pred]`,
//! `combinators.rs`).
//!
//! Authority:
//!   - `.design/verified/contract-tv.md` REQ-1: the reference encoder covers
//!     "the 8 frozen combinators (resolved via `thermite_spec::lookup(name).verus_l3`)".
//!     `forall_below`/`forall_from` ARE two of the 8 frozen combinators
//!     (`thermite-spec::combinators` REGISTRY). REQ-1 says it covers them.
//!   - `thermite-design.md` §4.2 (the frozen combinator cage) — the registry IS
//!     the ground truth; `forall_below`'s 2nd arg is `n: int` (an index BOUND),
//!     NOT a slice.
//!
//! THE DIVERGENCE: `ref_encode::encode_combinator_call` routes EVERY argument
//! through `encode_call_arg` → `encode_slice_arg`, which appends `@` to any bare
//! non-seq-bound `Path`. The `int` index arg `n` therefore becomes `n@`, which is
//! a Verus type error (`no method named view found for int`). A FAITHFUL clause
//! `forall_below(xs, n as int, |x| x <= 7)` cannot even be validated by the
//! reference side — so TV is BLIND on these two combinators (an `Unsupported`
//! would at least be honest; this is a SILENT WRONG ENCODING, worse — R-CHAR-3 /
//! contract-tv.md REQ-1 "NEVER a silent wrong encoding").
//!
//! Expected value source: the registry `verus_l3` body of `forall_below`
//! (`thermite_spec::lookup("forall_below").verus_l3`) — NOT the lowerer's output
//! (R-CHAR-3). The reference encoding of the INDEX arg must be `n as int` (a
//! scalar coercion), never `n@` (a slice view).
//!
//! Un-ignore when blocker #145 is fixed (R-DEFER-3).

use thermite_syntax::ast::{BinOp, Expr};
use thermite_tv::{ref_contract_pred, RefCtx};

fn path(name: &str) -> Expr {
    Expr::Path(vec![name.to_string()])
}
fn int(value: u128) -> Expr {
    Expr::IntLit {
        value,
        raw: value.to_string(),
    }
}
fn closure(p: &str, body: Expr) -> Expr {
    Expr::Closure {
        params: vec![p.to_string()],
        body: Box::new(body),
    }
}
fn call(callee: &str, args: Vec<Expr>) -> Expr {
    Expr::Call {
        callee: Box::new(path(callee)),
        args,
    }
}

/// `forall_below(xs, n, |x| x <= 7)` — `xs` is a Slice, `n` is an `int` INDEX
/// bound (`arg_kinds[1] == ArgKind::Index`), `|x| x <= 7` is the Pred.
fn forall_below_source() -> Expr {
    call(
        "forall_below",
        vec![
            path("xs"),
            path("n"),
            closure(
                "x",
                Expr::Binary {
                    op: BinOp::Le,
                    lhs: Box::new(path("x")),
                    rhs: Box::new(int(7)),
                },
            ),
        ],
    )
}

/// DIVERGENCE PIN: the reference encoder MUST NOT slice-`@`-view the `int` index
/// argument `n`. The frozen `forall_below` registry signature is
/// `forall_below(s: Seq<u32>, n: int, p: ...)` — the second arg is a scalar
/// `int`. A faithful encoding is `forall_below(xs, n as int, |x: u32| (x <= 7))`
/// (or any non-`@` index spelling). The current encoder emits `n@`, a type error.
///
/// This asserts the encoder does NOT produce the spurious `@`-view on the index
/// arg. The expected NEGATIVE comes from the registry arg-kind (Index, not
/// Slice), traceable to `thermite-design.md` §4.2 / the frozen REGISTRY — NOT the
/// lowerer's output (R-CHAR-3).
#[test]
fn ref_encode_index_arg_not_slice_viewed() {
    let ctx = RefCtx::with_seq_bound(["xs"]);
    let encoded = ref_contract_pred(&forall_below_source(), &ctx)
        .expect("forall_below is a frozen combinator REQ-1 says is covered");
    // The index arg `n` must be a scalar `int`, NEVER the slice `@`-view `n@`.
    assert!(
        !encoded.contains("n@"),
        "ref_encode mis-encoded the `int` INDEX arg of forall_below as a slice \
         `@`-view (`n@`), which is a Verus type error (no `view` on `int`). \
         The frozen registry signature is `forall_below(s: Seq<u32>, n: int, p)` \
         (thermite-design.md §4.2). Faithful index spelling is `n as int`. \
         Got: {encoded}"
    );
}
