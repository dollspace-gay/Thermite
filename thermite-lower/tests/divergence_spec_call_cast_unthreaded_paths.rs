//! Divergence pins for crosslink #227 + #228 — the #225 fix (commit `c02b9a6b`,
//! the type-directed spec-call narrowing cast `Ctx::spec_call_param_cast` fed by
//! `spec_fn_param_type_map`) threaded the param-type map through
//! `lower_spec_fn`/`lower_fn_signature`/`lower_external_body_fn` ONLY. Three
//! spec-context lowering entry points still build their `Ctx` WITHOUT
//! `.with_spec_fn_param_types(..)`, so a user-spec-fn call with an arithmetic
//! argument on those paths falls back to the hardcoded `as u64` — the EXACT #225
//! bug class the fix was supposed to close:
//!
//!   1. `lower_loop` (`Ctx::spec(..)` for the loop `inv` clauses AND the loop
//!      `dec` measure) — a `u32`-param spec fn named with an arithmetic arg in a
//!      loop invariant emits `s_dec((i + 0) as u64)` → ill-typed Verus
//!      (`expected u32, found u64`) → the whole item dies at L0 though the
//!      Thermite source is correct. LIVE-CONFIRMED via `forge check` (E0308 in
//!      the emitted invariant). Tracking #227.
//!   2. `spec_dec` (the fn-level `decreases` measure, `Ctx::spec_seq()`) — a
//!      `dec s_dec(n + 0)` measure emits `decreases s_dec((n + 0) as u64)`,
//!      the same E0308 class. Tracking #227.
//!   3. `lower_contract_expr` (the contract-TV production column) — its signature
//!      takes no `spec_fn_param_types`, so the TV's "production" lowering of
//!      `result == s_dec(n - 1)` is `s_dec((n - 1) as u64)` while the REAL
//!      production lowering (`lower_fn_signature`, post-#225) emits
//!      `s_dec((n - 1) as u32)`. `.design/verified/contract-tv.md` REQ-2: "The
//!      production side reuses `lower_fn_signature in lower.rs`'s clause output
//!      verbatim (the artifact under test)" — violated; the TV phase no longer
//!      checks the real artifact for this clause class. Tracking #228.
//!
//! THE AUTHORITY (R-CHAR-3): the expected cast target is the callee's DECLARED
//! param type — the same authority the #225 fix commit cites
//! (`.design/lower/verus-lowering.md` REQ-5: spec position is unbounded `int`,
//! the narrowing target is the surface integer set `u32`/`u64`/`usize`) — and,
//! for case 3, `.design/verified/contract-tv.md` REQ-2 (verbatim reuse of the
//! signature path's clause output). Expected substrings are HAND-DERIVED from
//! the fixtures' declared param types, never copied from the lowerer's output.

fn lower(src: &str) -> String {
    let parsed = thermite_syntax::parse(src);
    assert!(parsed.is_clean(), "fixture must parse clean: {parsed:?}");
    thermite_lower::lower(&parsed.program).expect("lowering must succeed")
}

/// A `u32`-param recursive spec fn + an exec fn whose LOOP INVARIANT names it
/// with an arithmetic argument (`s_dec(i + 0)`) — the `lower_loop` inv path.
const LOOP_INV_PROGRAM: &str = "\
spec fn s_dec(n: u32) -> u32
  dec n
{
  if n == 0 {
    0
  } else {
    s_dec(n - 1)
  }
}

fn count_up(n: u32) -> u32
  req true
  ens result == s_dec(n)
  fx  pure
{
  let mut i: u32 = 0;
  while i < n
    inv i <= n
    inv s_dec(i + 0) == 0
    dec n - i
  {
    i = i + 1;
  }
  0
}
";

#[test]
fn loop_invariant_spec_call_arith_arg_casts_to_declared_param_type() {
    let out = lower(LOOP_INV_PROGRAM);
    // The callee `s_dec` declares a `u32` param, so the invariant's arithmetic
    // arg must narrow to the DECLARED `u32` (verus-lowering.md REQ-5 / the #225
    // rule), exactly as the signature path does for `ens result == s_dec(n)`.
    assert!(
        out.contains("s_dec((i + 0) as u32)"),
        "a u32-param spec fn's loop-INVARIANT arith arg must cast `as u32`:\n{out}"
    );
    // The divergence: the hardcoded fallback emits ill-typed Verus (`expected
    // u32, found u64`) and the whole item dies at L0 (live-confirmed E0308).
    assert!(
        !out.contains("s_dec((i + 0) as u64)"),
        "the loop-inv path still hardcodes the #225 `as u64` fallback:\n{out}"
    );
}

/// A `u32`-param spec fn whose fn-level `dec` MEASURE names another u32-param
/// spec fn with an arithmetic argument — the `spec_dec` path.
const SPEC_DEC_PROGRAM: &str = "\
spec fn s_dec(n: u32) -> u32
  dec n
{
  if n == 0 {
    0
  } else {
    s_dec(n - 1)
  }
}

spec fn s_two(n: u32) -> u32
  dec s_dec(n + 0)
{
  0
}
";

#[test]
fn fn_level_dec_measure_spec_call_arith_arg_casts_to_declared_param_type() {
    let out = lower(SPEC_DEC_PROGRAM);
    assert!(
        out.contains("decreases s_dec((n + 0) as u32)"),
        "a u32-param spec fn named in a `dec` measure with an arith arg must \
         cast `as u32`:\n{out}"
    );
    assert!(
        !out.contains("decreases s_dec((n + 0) as u64)"),
        "the spec_dec path still hardcodes the #225 `as u64` fallback:\n{out}"
    );
}

#[test]
#[ignore = "divergence: lower_contract_expr omits spec_fn_param_types — the \
            contract-TV production column is NOT the real production lowering \
            (contract-tv.md REQ-2 verbatim); tracking #228"]
fn contract_tv_production_column_matches_real_signature_lowering() {
    use thermite_syntax::ast::{BinOp, Expr};
    // The clause `result == s_dec(n - 1)` — the contract-TV per-clause re-entry.
    // contract-tv.md REQ-2: the production side "reuses lower_fn_signature's
    // clause output verbatim (the artifact under test)". Post-#225 the real
    // signature lowering for a u32-param callee emits `s_dec((n - 1) as u32)`
    // (the declared-param-type rule, pinned by the fix commit's own
    // `divergence_spec_call_param_cast.rs`); `lower_contract_expr` has no way to
    // receive the param-type map and emits the `as u64` fallback instead — so
    // `forge tv` discharges a predicate that is NOT the production artifact.
    //
    // NOTE for the generator: closing this requires extending the
    // `lower_contract_expr` API (a `spec_fn_param_types` input mirroring the
    // other threaded ctx inputs) AND threading the program-derived map from
    // `forge::contract_tv::tv_clause`; this assertion pins the MEANING, the
    // call shape below changes with the API.
    let clause = Expr::Binary {
        op: BinOp::Eq,
        lhs: Box::new(Expr::Path(vec!["result".into()])),
        rhs: Box::new(Expr::Call {
            callee: Box::new(Expr::Path(vec!["s_dec".into()])),
            args: vec![Expr::Binary {
                op: BinOp::Sub,
                lhs: Box::new(Expr::Path(vec!["n".into()])),
                rhs: Box::new(Expr::IntLit {
                    value: 1,
                    raw: "1".into(),
                }),
            }],
        }),
    };
    let out = thermite_lower::lower_contract_expr(&clause, &[], &[], &[], &[], &[])
        .expect("contract lowering must succeed");
    // HAND-DERIVED expectation (verus-lowering.md REQ-5 + the #225 declared-
    // param-type rule for a `spec fn s_dec(n: u32)` callee):
    assert_eq!(
        out, "result == s_dec((n - 1) as u32)",
        "the TV production column must be the REAL production lowering \
         (contract-tv.md REQ-2 'verbatim') — got the u64 fallback instead"
    );
}
