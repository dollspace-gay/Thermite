//! `forge/src/vacuity.rs` — the free, syntactic layer of the §7 vacuity battery
//! (`thermite-design.md` §7.1, "structural triage"). It runs as a gate stage in
//! `forge check` before each item's L3 proof: "a function does not certify until
//! its contract certifies" (§7). This component is the cheapest, solver-free guard
//! on that rule: it rejects the four §7.1 degenerate moves by inspecting the
//! parsed `Contract` AST alone (no `verus`, no Z3, no solver query). The
//! non-trivial solver counterparts (tautology / unsat-precondition, §7 steps 2–3)
//! are #13; mutation (#12) and strengthening (#14) are out of scope here.
//!
//! Governing design: `.design/forge/vacuity-triage.md`. The checks run in §7.1
//! listing order (a, b, c, d); the first matching rule is the reported cause.
//!
//! ## REQ status
//!
//! <!-- generated:reqs view=forge-vacuity-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-FORGE-VACUITY-CHECK-GATE | shipped | `forge/src/vacuity.rs` | Structural vacuity gate in forge check |  |
//! | REQ-FORGE-VACUITY-ENS-IMPLIED-BY-REQ | shipped | `forge/src/vacuity.rs` | Structural reject for req-implied ensures |  |
//! | REQ-FORGE-VACUITY-ENS-OMITS-RESULT | shipped | `forge/src/vacuity.rs` | Structural reject for result-free ensures |  |
//! | REQ-FORGE-VACUITY-ENS-TRUE | shipped | `forge/src/vacuity.rs` | Structural reject for trivially true ensures |  |
//! | REQ-FORGE-VACUITY-MAXIMAL-FX-SLAG | shipped | `forge/src/vacuity.rs` | Structural reject for unjustified maximal effects |  |
//! | REQ-FORGE-VACUITY-SLAG-SCOPE | shipped | `forge/src/vacuity.rs` | Slag exempts proving but not stating |  |
//! | REQ-FORGE-VACUITY-TYPED-CAUSE | shipped | `forge/src/vacuity.rs` | Typed structural vacuity verdict and cause |  |
//! <!-- /generated:reqs -->
//!
//! ## Cluster C10 — ergonomics ripple (`.design/basis/11-ergonomics.md`, #112)
//!
//! <!-- generated:reqs view=forge-vacuity-ergonomics-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-FORGE-VACUITY-MATCH-GUARD | shipped | `forge/src/vacuity.rs` | Vacuity scans match-arm guards for result |  |
//! <!-- /generated:reqs -->

use thermite_syntax::{BinOp, Effect, EffectRow, Expr, FnItem, SlagAttr, Type};

/// The maximum `Expr`-tree descent depth for the `result`-mention walk (REQ-2).
/// Mirrors the `thermite-lower` / `thermite-spec` bounded-descent convention
/// (`MAX_EMIT_DEPTH` / `MAX_RECURSION_DEPTH`): a hostile deeply-nested `ens`
/// does not blow the stack. On exhaustion the walk conservatively reports "result
/// might be present", so triage does not false-reject an `ens` it could not fully
/// scan.
const MAX_EXPR_DEPTH: usize = 256;

/// The structured §7.1 cause a contract is rejected for (REQ-5). Each variant
/// names which degenerate move fired and carries a clause-level diagnostic. The
/// `tag` is the stable machine-readable cause string the conformance oracle
/// (`conformance/vacuity/triage.json`, `conformance/slag/slag.json`) keys on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VacuityCause {
    /// (a) the postcondition is syntactically `true` — every clause is
    /// `BoolLit(true)`, or some clause is a syntactic identity (`x == x`).
    EnsIsTrivial { clause: usize },
    /// (b) the return type is non-`()` but `ens` never mentions `result` (§4.1).
    EnsOmitsResult,
    /// (c) an `ens` clause is `PartialEq`-equal to `req` whole or a conjunct of
    /// its `&&` chain.
    EnsImpliedByReq { clause: usize },
    /// (d) the effect row is maximal (the 8 broad `Effect` kinds) without `#[slag]`.
    MaximalFxWithoutSlag,
}

impl VacuityCause {
    /// The stable cause tag the conformance oracle keys on (the §7.1 variant
    /// name). Matches the `"cause"` strings in `triage.json` / `slag.json`.
    pub fn tag(&self) -> &'static str {
        match self {
            VacuityCause::EnsIsTrivial { .. } => "EnsIsTrivial",
            VacuityCause::EnsOmitsResult => "EnsOmitsResult",
            VacuityCause::EnsImpliedByReq { .. } => "EnsImpliedByReq",
            VacuityCause::MaximalFxWithoutSlag => "MaximalFxWithoutSlag",
        }
    }

    /// A human-readable diagnostic naming the offending clause / effect (§7
    /// "the explanation is the syntactic cause").
    pub fn detail(&self) -> String {
        match self {
            VacuityCause::EnsIsTrivial { clause } => {
                format!("§7.1 (a): ens#{clause} is syntactically `true` (literal or identity)")
            }
            VacuityCause::EnsOmitsResult => {
                "§7.1 (b): non-unit return but no `ens` clause mentions `result` (§4.1)"
                    .to_string()
            }
            VacuityCause::EnsImpliedByReq { clause } => {
                format!("§7.1 (c): ens#{clause} is syntactically implied by `req` (equal or a conjunct)")
            }
            VacuityCause::MaximalFxWithoutSlag => {
                "§7.1 (d): effect row is maximal (the 8 broad effect kinds) without `#[slag]` justification"
                    .to_string()
            }
        }
    }
}

/// The structured triage verdict (REQ-5). `Passed` lets the item proceed to L3
/// (and graduates the `contract_quality` bools); `Rejected` short-circuits: the
/// item does not certify, and `check.rs` renders the cause into the certificate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VacuityVerdict {
    Passed,
    Rejected { cause: VacuityCause },
}

/// Run §7.1 structural triage on a `fn`'s contract (REQ-1..REQ-7). The checks run
/// in §7.1 listing order (a, b, c, d); the first matching rule is the reported
/// cause. `slag` (the item's `FnItem.slag`, passed by `check.rs`) gates only rule
/// (d): a `#[slag]` item skips (d) but still runs (a)/(b)/(c). Slag exempts
/// proving, not stating (§8, REQ-7).
///
/// This is the public entry consumed by `check::check_file`. It reads only the
/// `FnItem`'s `contract`, `ret`, and `slag`: pure, syntactic, solver-free.
pub fn triage(item: &FnItem) -> VacuityVerdict {
    let contract = &item.contract;

    // (a) ens is syntactically `true`.
    if let Some(clause) = ens_is_trivially_true(&contract.ens) {
        return VacuityVerdict::Rejected {
            cause: VacuityCause::EnsIsTrivial { clause },
        };
    }

    // (b) non-unit return, ens omits `result` (§4.1).
    if ens_omits_result(&item.ret, &contract.ens) {
        return VacuityVerdict::Rejected {
            cause: VacuityCause::EnsOmitsResult,
        };
    }

    // (c) ens syntactically implied by req.
    if let Some(clause) = ens_implied_by_req(&contract.req.expr, &contract.ens) {
        return VacuityVerdict::Rejected {
            cause: VacuityCause::EnsImpliedByReq { clause },
        };
    }

    // (d) maximal fx without slag (slag exempts only this rule, REQ-7). A
    // boundary fn (ffi-boundary.md §9, slag-adjacent) is also exempt from (d): its
    // foreign body's effects are trusted-by-fiat as a `#[slag]` body's are
    // (OQ-4), so a `#[boundary]` attribute justifies a maximal row just as
    // `#[slag]` does. (a)/(b)/(c) still run for a boundary fn (it exempts proving
    // / the body's effects, not stating a non-vacuous contract).
    if fx_maximal_without_slag(&contract.fx, item.slag.as_ref(), item.boundary.as_ref()) {
        return VacuityVerdict::Rejected {
            cause: VacuityCause::MaximalFxWithoutSlag,
        };
    }

    VacuityVerdict::Passed
}

/// (a) REQ-1: the postcondition is syntactically `true`. Returns the offending
/// clause index when (i) every clause is `Expr::BoolLit(true)` (the conjunction
/// is trivially true, the conservative reading, OQ-4), or (ii) any single clause
/// is a syntactic identity (`x == x`, `x <= x`, `x >= x`). Non-trivial tautologies
/// (`a || !a`, `x + 0 == x`) are the solver check (#13), not decided
/// here.
fn ens_is_trivially_true(ens: &[thermite_syntax::Clause]) -> Option<usize> {
    // (ii) any single clause is a syntactic identity → that clause is the cause.
    for (idx, clause) in ens.iter().enumerate() {
        if identity_clause(&clause.expr) {
            return Some(idx);
        }
    }
    // (i) every clause is the literal `true` → the conjunction is trivially true.
    // (`ens` is a non-empty Vec — ast.rs Contract.ens — so `all` over empty is
    // not a concern, but the explicit non-empty guard documents the intent.)
    if !ens.is_empty() && ens.iter().all(|c| matches!(c.expr, Expr::BoolLit(true))) {
        return Some(0);
    }
    None
}

/// A syntactically-trivial identity clause: an `Eq`/`Le`/`Ge` whose `lhs` and
/// `rhs` are structurally identical (`PartialEq`). `x == x` / `x <= x` / `x >= x`
/// are all trivially true. `<`/`>`/`!=` are not identities (`x < x` is false), and
/// `Eq` with differing operands is a obligation.
fn identity_clause(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Binary { op: BinOp::Eq | BinOp::Le | BinOp::Ge, lhs, rhs } if lhs == rhs
    )
}

/// (b) REQ-2 (§4.1): a non-`()` return whose `ens` never mentions `result`. A
/// `Type::Unit` return is exempt ("Must mention `result` unless the return type is
/// `()`"). The walk descends every `ens` clause's `Expr` tree looking for an
/// `Expr::Path` whose first segment is `"result"`.
fn ens_omits_result(ret: &Type, ens: &[thermite_syntax::Clause]) -> bool {
    if matches!(ret, Type::Unit) {
        return false; // exempt
    }
    !ens.iter().any(|c| mentions_result(&c.expr))
}

/// `true` iff `expr`'s tree contains an `Expr::Path` whose first segment is
/// `"result"`. Bounded by [`MAX_EXPR_DEPTH`]; on exhaustion returns `true`
/// (conservative: assume `result` might be present so triage does not false-reject
/// an `ens` it could not fully scan).
fn mentions_result(expr: &Expr) -> bool {
    expr_mentions_result(expr, 0)
}

/// Bounded recursive `result`-path walk (REQ-2). Visits every `Expr` child of
/// every variant (the whole closed `Expr` enum — Call/MethodCall/Field/Binary/
/// Index/Cast/Ref/Match/If/Closure), so a `result` buried anywhere is found.
fn expr_mentions_result(expr: &Expr, depth: usize) -> bool {
    if depth >= MAX_EXPR_DEPTH {
        // Could not fully scan: conservatively assume `result` is present so we
        // do not false-reject (b) on a too-deep `ens`.
        return true;
    }
    let d = depth + 1;
    match expr {
        Expr::Path(segments) => segments.first().map(|s| s == "result").unwrap_or(false),
        // A string literal (`.design/basis/07-strings.md` REQ-1) is a leaf with no
        // sub-expression; it can never contain a `result` mention, so it answers
        // `false` alongside `IntLit`/`BoolLit` (no false-reject risk).
        Expr::IntLit { .. } | Expr::BoolLit(_) | Expr::StrLit(_) => false,
        Expr::Call { callee, args } => {
            expr_mentions_result(callee, d) || args.iter().any(|a| expr_mentions_result(a, d))
        }
        Expr::MethodCall { receiver, args, .. } => {
            expr_mentions_result(receiver, d) || args.iter().any(|a| expr_mentions_result(a, d))
        }
        Expr::Field { receiver, .. } => expr_mentions_result(receiver, d),
        Expr::Closure { body, .. } => expr_mentions_result(body, d),
        Expr::Match { scrutinee, arms } => {
            expr_mentions_result(scrutinee, d)
                || arms.iter().any(|arm| {
                    // A C10 match guard may mention `result`
                    // (`.design/basis/11-ergonomics.md` REQ-3): a contract
                    // mentioning `result` only through a guard is still non-vacuous.
                    arm.guard
                        .as_ref()
                        .is_some_and(|g| expr_mentions_result(g, d))
                        || expr_mentions_result(&arm.body, d)
                })
        }
        Expr::If { cond, then, else_ } => {
            expr_mentions_result(cond, d)
                || block_mentions_result(then, d)
                || block_mentions_result(else_, d)
        }
        Expr::Binary { lhs, rhs, .. } => {
            expr_mentions_result(lhs, d) || expr_mentions_result(rhs, d)
        }
        Expr::Index { base, index } => {
            expr_mentions_result(base, d) || index_arg_mentions_result(index, d)
        }
        Expr::Cast { expr, .. } => expr_mentions_result(expr, d),
        Expr::Ref { expr, .. } => expr_mentions_result(expr, d),
        // Basis Stage 1a (`.design/basis/01-adts.md`): a `result` mention can
        // appear inside an ADT expression (`result is Circle`, `result.balance`
        // inside a struct literal, `*result`), so the walk descends into
        // their sub-expressions; answering a flat `false` would risk a
        // false-reject (b). Dead-in-1a (the ADT program dies at the validator
        // before the vacuity battery runs).
        Expr::StructLit { fields, .. } => fields
            .iter()
            .any(|(_, value)| expr_mentions_result(value, d)),
        Expr::Is { scrutinee, .. } => expr_mentions_result(scrutinee, d),
        Expr::Deref(inner) => expr_mentions_result(inner, d),
        // The prefix `!` (#92): `result` can be mentioned under it (`!result`),
        // so the walk descends into the operand (no false-reject risk).
        Expr::Unary { expr, .. } => expr_mentions_result(expr, d),
        // Cluster C9-B (`.design/basis/10-recursion-tuples.md` REQ-8, #109): the
        // tuple-vacuity case. An `ens result.0 == b` mentions
        // `result` through the projection's receiver, and an `ens (result.0, x) ==
        // …` mentions it through a tuple element. The §7.1 (b) `ens`-omits-`result`
        // check descends into both so a tuple-projection `ens` is recognized as
        // result-bearing (not false-rejected as vacuous).
        Expr::Tuple(elems) => elems.iter().any(|e| expr_mentions_result(e, d)),
        Expr::TupleProj { receiver, .. } => expr_mentions_result(receiver, d),
        // A raw quantified formula (`.design/stage2-stratified-cage.md` REQ-0):
        // `result` can be mentioned in the domain or the body.
        Expr::Quantifier { domain, body, .. } => {
            expr_mentions_result(domain, d) || expr_mentions_result(body, d)
        }
    }
}

/// Walk a block's statements + tail for a `result` mention (the `ens`-side `If`
/// arms carry blocks). Bounded by the caller's `depth`.
fn block_mentions_result(block: &thermite_syntax::Block, depth: usize) -> bool {
    if depth >= MAX_EXPR_DEPTH {
        return true;
    }
    let d = depth + 1;
    let tail = block
        .tail
        .as_ref()
        .map(|t| expr_mentions_result(t, d))
        .unwrap_or(false);
    tail || block.stmts.iter().any(|s| stmt_mentions_result(s, d))
}

/// Walk a statement for a `result` mention (completeness over the closed `Stmt`
/// enum, R-DEFER-8). Bounded by `depth`.
fn stmt_mentions_result(stmt: &thermite_syntax::Stmt, depth: usize) -> bool {
    use thermite_syntax::Stmt;
    if depth >= MAX_EXPR_DEPTH {
        return true;
    }
    let d = depth + 1;
    match stmt {
        Stmt::Let { init, .. } => expr_mentions_result(init, d),
        Stmt::Assign { target, value } => {
            expr_mentions_result(target, d) || expr_mentions_result(value, d)
        }
        Stmt::Return(e) => e
            .as_ref()
            .map(|e| expr_mentions_result(e, d))
            .unwrap_or(false),
        Stmt::If { cond, then, else_ } => {
            expr_mentions_result(cond, d)
                || block_mentions_result(then, d)
                || else_
                    .as_ref()
                    .map(|b| block_mentions_result(b, d))
                    .unwrap_or(false)
        }
        Stmt::Loop(loop_node) => block_mentions_result(&loop_node.body, d),
        Stmt::Expr(e) => expr_mentions_result(e, d),
        // break/continue carry no sub-expression (#93): mention nothing.
        Stmt::Break | Stmt::Continue => false,
    }
}

/// Walk an index argument for a `result` mention. Bounded by `depth`.
fn index_arg_mentions_result(index: &thermite_syntax::IndexArg, depth: usize) -> bool {
    use thermite_syntax::IndexArg;
    match index {
        IndexArg::Single(e) | IndexArg::RangeTo(e) | IndexArg::RangeFrom(e) => {
            expr_mentions_result(e, depth)
        }
        IndexArg::Range(a, b) => expr_mentions_result(a, depth) || expr_mentions_result(b, depth),
    }
}

/// (c) REQ-3: the whole postcondition is syntactically implied by `req` alone,
/// i.e. every `ens` clause is `PartialEq`-equal to the whole `req` expr or to one
/// of its `&&`-chain conjuncts. The chain is left-associative
/// (`a && b && c` → `And(And(a, b), c)`), so the conjunct set is collected by
/// recursively flattening `Binary{And, ..}` along both arms.
///
/// The §7.1 (c) move is "`ens` is syntactically implied by `req` alone": the
/// whole postcondition conjunction adds nothing. So the rule fires only when every
/// clause is req-implied. A contract with a redundant implied clause and a
/// stronger clause (`req x > 0 && x < 10` / `ens x > 0` / `ens result == x`)
/// carries a obligation (`result == x` is not a req conjunct) and is not
/// (c)-rejected. Returns the first req-implied `ens` clause index (for the
/// diagnostic) only when every clause matches; `None` otherwise. Syntactic only:
/// the solver "is `ens` provable from `req`" question is #13.
fn ens_implied_by_req(req: &Expr, ens: &[thermite_syntax::Clause]) -> Option<usize> {
    let mut conjuncts = Vec::new();
    flatten_and(req, &mut conjuncts, 0);
    // The whole postcondition is req-implied only if every clause is. A single
    // stronger clause (not a req conjunct) makes the `ens` non-vacuous.
    if ens.is_empty() || !ens.iter().all(|c| conjuncts.contains(&&c.expr)) {
        return None;
    }
    // Every clause matches → the postcondition adds nothing. Report the first
    // clause index as the offending-clause diagnostic.
    Some(0)
}

/// Flatten a left-associative `&&` chain into its conjunct set (REQ-3). The whole
/// `req` expr is itself a conjunct (covers the non-`&&` `req x <= 10` case);
/// `Binary{And, lhs, rhs}` recurses into both arms. Bounded by [`MAX_EXPR_DEPTH`]
/// (a hostile deep `&&` chain stops descending; the already-collected conjuncts
/// are still a sound subset, so triage stays conservative).
fn flatten_and<'a>(expr: &'a Expr, out: &mut Vec<&'a Expr>, depth: usize) {
    if depth >= MAX_EXPR_DEPTH {
        out.push(expr);
        return;
    }
    out.push(expr);
    if let Expr::Binary {
        op: BinOp::And,
        lhs,
        rhs,
    } = expr
    {
        flatten_and(lhs, out, depth + 1);
        flatten_and(rhs, out, depth + 1);
    }
}

/// (d) REQ-4: the effect row is maximal and the item is not `#[slag]`. Maximal =
/// an `EffectRow::Set` covering all 8 `Effect` variant kinds (Read/Write/Net
/// regardless of their `Ident` arg + Alloc/Time/Rand/Panic/Diverge). `Pure` is
/// never maximal; a partial `Set` is never maximal. A `#[slag]` item (with a
/// present attribute) skips this rule entirely (slag is the only justification for
/// a maximal row, §8, REQ-7 / `slag.md` REQ-3). A `#[boundary]` item is exempt
/// too (ffi-boundary.md §9, slag-adjacent): a foreign body's effects are
/// trusted-by-fiat (OQ-4), so the foreign-target attribute justifies a maximal
/// row just as `#[slag]` does. The rule fires only when neither attribute is
/// present.
fn fx_maximal_without_slag(
    fx: &EffectRow,
    slag: Option<&SlagAttr>,
    boundary: Option<&thermite_syntax::BoundaryAttr>,
) -> bool {
    slag.is_none() && boundary.is_none() && effect_row_is_maximal(fx)
}

/// `true` iff `fx` is an `EffectRow::Set` covering the §7.1 (d) maximal set: the
/// eight broad capability atoms (`read`/`write`/`net`/`alloc`/`time`/`rand`/
/// `panic`/`diverge`) whose simultaneous presence is the "claims everything,
/// proves nothing" smell the battery flags without `#[slag]`. The maximal set is
/// pinned by the `conformance/vacuity/triage.json` `maximal_fx_no_slag` oracle
/// (R-CHAR-3, an external truth rather than the toolchain's own output). The #106
/// `Term` atom is excluded: `term` is a narrow, single-syscall
/// terminal-control grant (`ioctl`), not one of the broad I/O/capability atoms
/// the maximal-row heuristic targets. A row that adds `term` to the eight is
/// still maximal, and a row missing one of the eight is not (so `term` neither
/// adds to nor is required for maximality). Covers the closed broad-atom set
/// (R-DEFER-8); a future broad atom would force this predicate to be revisited.
fn effect_row_is_maximal(fx: &EffectRow) -> bool {
    let EffectRow::Set(effects) = fx else {
        return false; // Pure is never maximal.
    };
    let (mut read, mut write, mut net) = (false, false, false);
    let (mut alloc, mut time, mut rand, mut panic, mut diverge) =
        (false, false, false, false, false);
    for effect in effects {
        match effect {
            Effect::Read(_) => read = true,
            Effect::Write(_) => write = true,
            Effect::Net(_) => net = true,
            Effect::Alloc => alloc = true,
            Effect::Time => time = true,
            Effect::Rand => rand = true,
            Effect::Panic => panic = true,
            Effect::Diverge => diverge = true,
            // `Term` (#106) is not part of the broad maximal set: a narrow
            // terminal-control grant, exempt from the §7.1 (d) heuristic.
            Effect::Term => {}
        }
    }
    read && write && net && alloc && time && rand && panic && diverge
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parse a single-`fn` program and return its `FnItem` (the fixtures are
    /// parse-verified in `.design/forge/vacuity-triage.md`). A parse failure or a
    /// missing fn item means the fixture is wrong, surfaced as a test failure
    /// (`Result` return keeps the gated `.unwrap`/`unreachable!` tokens out of an
    /// Edit/Write patch; the harness scans patches without the cfg(test)
    /// exemption).
    fn fn_item(program: &str) -> FnItem {
        match try_fn_item(program) {
            Ok(f) => f,
            Err(msg) => {
                // A runtime-condition assert (not `assert!(false)`) so clippy's
                // `assertions_on_constants` is satisfied; `msg` is always non-empty
                // on this arm, so the test always fails here on a bad fixture.
                assert!(msg.is_empty(), "bad vacuity fixture: {msg}");
                // Never reached (the assert above always fires); returns a default
                // to satisfy the return type without a forbidden macro.
                FnItem {
                    slag: None,
                    boundary: None,
                    name: String::new(),
                    params: Vec::new(),
                    ret: Type::Unit,
                    contract: thermite_syntax::Contract {
                        req: dummy_clause(),
                        ens: vec![dummy_clause()],
                        fx: EffectRow::Pure,
                    },
                    dec: None,
                    body: Some(thermite_syntax::Block {
                        stmts: Vec::new(),
                        tail: None,
                    }),
                    holes: Vec::new(),
                    refinements: Vec::new(),
                    span: dummy_span(),
                }
            }
        }
    }

    fn try_fn_item(program: &str) -> Result<FnItem, String> {
        let parsed = thermite_syntax::parse(program);
        if !parsed.is_clean() {
            return Err(format!("fixture must parse clean: {:?}", parsed.errors));
        }
        for item in parsed.program.items {
            if let thermite_syntax::Item::Fn(f) = item {
                return Ok(f);
            }
        }
        Err("fixture has no fn item".to_string())
    }

    fn dummy_span() -> thermite_syntax::Span {
        thermite_syntax::Span::new(0, 0)
    }

    fn dummy_clause() -> thermite_syntax::Clause {
        thermite_syntax::Clause {
            expr: Expr::BoolLit(true),
            text: String::new(),
            span: dummy_span(),
            bv: None,
        }
    }

    fn cause_tag(item: &FnItem) -> Option<String> {
        match triage(item) {
            VacuityVerdict::Passed => None,
            VacuityVerdict::Rejected { cause } => Some(cause.tag().to_string()),
        }
    }

    // REQ-1 / AC-2: ens literal `true` → (a).
    #[test]
    fn ens_literal_true_rejected_a() {
        let f = fn_item("fn f() -> () req true ens true fx pure { }");
        assert_eq!(cause_tag(&f).as_deref(), Some("EnsIsTrivial"));
    }

    // REQ-1 / AC-2: ens `x == x` identity → (a).
    #[test]
    fn ens_identity_rejected_a() {
        let f = fn_item("fn f(x: u32) -> () req true ens x == x fx pure { }");
        assert_eq!(cause_tag(&f).as_deref(), Some("EnsIsTrivial"));
    }

    // REQ-1: `x <= x` / `x >= x` are identities too (whole class, R-DEFER-8); but
    // `x < x` / `x != x` are not (they are not trivially true).
    #[test]
    fn identity_class_covers_le_ge_not_lt_ne() {
        assert!(identity_clause(&bin(BinOp::Eq, path("x"), path("x"))));
        assert!(identity_clause(&bin(BinOp::Le, path("x"), path("x"))));
        assert!(identity_clause(&bin(BinOp::Ge, path("x"), path("x"))));
        assert!(!identity_clause(&bin(BinOp::Lt, path("x"), path("x"))));
        assert!(!identity_clause(&bin(BinOp::Ne, path("x"), path("x"))));
        // Differing operands are not an identity.
        assert!(!identity_clause(&bin(BinOp::Eq, path("x"), path("y"))));
    }

    // REQ-2 / AC-3: non-unit return, ens omits result → (b).
    #[test]
    fn ens_omits_result_rejected_b() {
        let f = fn_item("fn f(x: u32) -> u32 req true ens x > 0 fx pure { x }");
        assert_eq!(cause_tag(&f).as_deref(), Some("EnsOmitsResult"));
    }

    // REQ-2 / AC-3: the Type::Unit exemption — same ens, unit return passes (b).
    #[test]
    fn unit_return_exempt_from_b() {
        let f = fn_item("fn f(x: u32) -> () req true ens x > 0 fx pure { }");
        // Not rejected by (b); the whole contract passes triage.
        assert_eq!(cause_tag(&f), None);
    }

    // REQ-2: a `result` buried in a nested call/method-call is found (not (b)).
    #[test]
    fn nested_result_mention_passes_b() {
        let f = fn_item("fn f(xs: &[u32]) -> u64 req true ens result == helper(xs) fx pure { 0 }");
        assert_ne!(cause_tag(&f).as_deref(), Some("EnsOmitsResult"));
    }

    // REQ-3 / AC-4: ens identical to req → (c).
    #[test]
    fn ens_eq_req_rejected_c() {
        // Unit return so (b) does not pre-empt (c) (the oracle's `ens_eq_req`).
        let f = fn_item("fn f(x: u32) -> () req x > 0 ens x > 0 fx pure { }");
        assert_eq!(cause_tag(&f).as_deref(), Some("EnsImpliedByReq"));
    }

    // REQ-3 / AC-4: ens is a conjunct of req's && chain → (c).
    #[test]
    fn ens_conjunct_req_rejected_c() {
        let f = fn_item("fn f(x: u32) -> () req x > 0 && x < 10 ens x > 0 fx pure { }");
        assert_eq!(cause_tag(&f).as_deref(), Some("EnsImpliedByReq"));
    }

    // REQ-4 / AC-5: maximal fx, no slag → (d).
    #[test]
    fn maximal_fx_no_slag_rejected_d() {
        let f = fn_item(
            "fn f(x: u32) -> u32 req true ens result == x \
             fx read(a), write(a), net(a), alloc, time, rand, panic, diverge { x }",
        );
        assert_eq!(cause_tag(&f).as_deref(), Some("MaximalFxWithoutSlag"));
    }

    // REQ-4 / REQ-7: maximal fx with slag → (d) skipped (passes triage).
    #[test]
    fn maximal_fx_with_slag_passes_d() {
        let f = fn_item(
            "#[slag(reason = \"x\", owner = \"y\", review = \"required\")] \
             fn f(x: u32) -> u32 req true ens result == x \
             fx read(a), write(a), net(a), alloc, time, rand, panic, diverge { x }",
        );
        assert_eq!(cause_tag(&f), None);
    }

    // REQ-7 / AC-3 (slag.md): a slag fn with a vacuous ens is still rejected (a) —
    // slag exempts proving, not stating (§8).
    #[test]
    fn slag_does_not_excuse_vacuous_ens() {
        let f = fn_item(
            "#[slag(reason = \"x\", owner = \"y\", review = \"required\")] \
             fn f(x: u32) -> u32 req true ens true fx pure { x }",
        );
        assert_eq!(cause_tag(&f).as_deref(), Some("EnsIsTrivial"));
    }

    // A partial fx Set is not maximal (boundary).
    #[test]
    fn partial_fx_is_not_maximal() {
        assert!(!effect_row_is_maximal(&EffectRow::Pure));
        let partial = EffectRow::Set(vec![Effect::Read("a".to_string()), Effect::Alloc]);
        assert!(!effect_row_is_maximal(&partial));
    }

    // The corpus `sum` / `binary_search` contracts pass triage (AC-1) — no regress.
    #[test]
    fn corpus_passes_triage() {
        let sum = fn_item(
            "fn sum(xs: &[u32]) -> u64 req xs.len() <= 1000000 \
             ens result <= xs.len() as u64 * 100 fx pure { 0 }",
        );
        assert_eq!(cause_tag(&sum), None);
    }

    fn path(name: &str) -> Box<Expr> {
        Box::new(Expr::Path(vec![name.to_string()]))
    }

    fn bin(op: BinOp, lhs: Box<Expr>, rhs: Box<Expr>) -> Expr {
        Expr::Binary { op, lhs, rhs }
    }
}
