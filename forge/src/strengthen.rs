//! `forge/src/strengthen.rs` — §7 step 5 of the vacuity battery: STRENGTHENING
//! PROBES (`thermite-design.md` §7 "template-based tightenings of `ens` … if a
//! strictly stronger contract proves with no body change, Forge suggests it").
//! Given a `fn` whose REAL body already proved **L3** but whose contract is WEAK
//! (#12 mutation scoring found one or more SURVIVORS — behavior the `ens` does
//! not pin), this module generates a FROZEN, DETERMINISTIC, BOUNDED set of
//! CANDIDATE stronger `ens` clauses, VERIFIES each against the real body by
//! reusing the existing verus driver (`check::run_verus`, threaded as a verify
//! closure), and SURFACES the candidates that (a) VERIFY against the body AND (b)
//! are strictly STRONGER than the current `ens` as adoptable [`Suggestion`]s.
//!
//! Governing design: `.design/forge/strengthening-probes.md`.
//!
//! ## Advisory, never a gate
//!
//! A probe NEVER changes the certification verdict (level/reject/the
//! `contract_quality` oracle subset). It only ADDS suggestions to the additive,
//! oracle-EXCLUDED `manifest::Certificate::strengthening` field + populates the
//! reserved `suggested_move` slot with the headline. A `fn` that certified L3
//! still certifies L3 with the same oracle subset, now carrying suggestions
//! (REQ-4). This is the anti-Goodhart escape hatch (`goal.md` R-DEFER-9): the
//! probe helps the agent climb OUT of a weak-but-true contract toward one that
//! pins behavior — it never lets it certify a weaker one.
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (frozen deterministic bounded candidate template) | SHIPPED | `pub fn generate_candidates` is a pure function of the `FnItem` + the file's `spec fn`s + the #12 survivor, producing an ORDERED list in the fixed family order (spec-fn equality, result-equals-input-expression, survivor-derived tighter bound), capped by `pub const CANDIDATE_CAP`. Consumer: `pub fn probe` (this file) + `check::strengthen_certificate` in `check.rs`. |
//! | REQ-2 (verify each candidate vs the REAL body, reuse `run_verus`) | SHIPPED | `pub fn probe` weaves each candidate `ens` into a COPY of `f` (`candidate_fn`, body UNCHANGED) and calls the threaded `verify` closure (which is `check::run_verus` of the `item_subprogram` shape, content-addressed via the #8 cache). A candidate verus PROVES → HOLDS; a non-`Proved` candidate is DISCARDED. A `ForgeError` from `verify` propagates (R-CODE-4). Consumer: `check::strengthen_certificate`. |
//! | REQ-3 (strictly-stronger filter) | SHIPPED | `pub fn is_strictly_stronger` keeps a verifying candidate only if it KILLS the survivor (the `verify`-against-survivor-body witness — `CandidateClause::kills_survivor` set, the survivor body does NOT verify the candidate) OR adds an equality on `result` the current `ens` lacks (`current_ens_pins_result` is `false`). No extra implication solver query (OQ-5). Consumer: `pub fn probe`. |
//! | REQ-4 (advisory placement — additive cert field, not a gate) | SHIPPED | `pub fn probe` returns `Vec<Suggestion>`; `check::strengthen_certificate` attaches them via `Certificate::with_strengthening` (the additive `strengthening` field + the `suggested_move` headline). The `level`/`reject`/oracle subset are untouched (REQ-4). |
//! | REQ-5 (consumes #12 survivors; runs on a settled L3 + scored item) | SHIPPED | `check::check_file_with_options` calls `check::strengthen_certificate` AFTER mutation scoring, ONLY when the item is `Level::L3` with `reject.is_none()` and a `MutationScore` was produced; the `MutationScore.survivor` feeds family-3 candidate generation. |
//! | REQ-6 (determinism — same fn ⇒ same suggestions) | SHIPPED | `generate_candidates` is a pure function of the AST + spec fns + survivor + frozen template, so the candidate list is byte-stable; each candidate's verus verdict is the same deterministic pinned-seed/rlimit run + #8 cache the L3 path uses. The suggestions are DIAGNOSTIC + verus-version-sensitive, so oracle-EXCLUDED. |

use thermite_syntax::{BinOp, Clause, Expr, FnItem, Item, PrimType, Span, Type};

use crate::cli::ForgeError;
use crate::mutation::MutationScore;

/// The FIXED budget on the number of candidate `ens` clauses generated per `fn`
/// (REQ-1, OQ-2). §7 says "budgeted" without a number; this is a documented
/// `const` (R-CODE-5 — a fixed input, not wall-clock). Each candidate is a verus
/// run (cheap on a #8 cache hit), so the cap bounds the probe's cost. The corpus
/// `fn`s produce a handful of candidates (one spec-fn-equality per matching spec
/// fn + a depth-1 grammar over ≤ 4 params + the survivor-derived bound),
/// comfortably under it; selection when the count exceeds the cap is the first
/// `CANDIDATE_CAP` candidates in the fixed family order (parallel to
/// `mutation::MUTANT_CAP`).
pub const CANDIDATE_CAP: usize = 16;

/// One generated candidate stronger `ens` clause (REQ-1). The `ens` is a real
/// SpecTherm `Clause` (built from `thermite_syntax::{Expr, Clause}` — the same
/// nodes the parser produces, so it round-trips through the lowerer unchanged).
/// `kills_survivor` carries the #12 survivor description this candidate would
/// kill, when the candidate is the survivor-derived family-3 tightening; it is
/// the strictly-stronger witness (REQ-3) and the cert's `Suggestion.kills_survivor`.
#[derive(Debug, Clone)]
pub struct CandidateClause {
    /// The candidate `ens` clause (a real adoptable SpecTherm postcondition).
    pub ens: Clause,
    /// The rendered surface form of the clause (`result == a + b`), the
    /// `Suggestion.clause` payload.
    pub rendered: String,
    /// The #12 survivor this candidate would kill, when derivable (the family-3
    /// link / the strictly-stronger witness). `None` for a family-1/2 candidate,
    /// whose strictly-stronger witness is the structural equality test instead.
    pub kills_survivor: Option<String>,
}

/// One adoptable strengthening suggestion surfaced on a certificate (REQ-4). It
/// is the §7 step-5 "consider strengthening `ens` with `<clause>` — it holds for
/// your body and would kill survivor `<M>`" prompt, made concrete: the `clause`
/// VERIFIES against the real body (so it is adoptable with NO body change) and is
/// strictly STRONGER than the current `ens` (so it narrows the allowed outputs).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Suggestion {
    /// The adoptable `ens` clause's surface form (e.g. `result == a + b`). Pasting
    /// `ens <clause>` into the `fn` keeps it certifying L3 (the probe proved it).
    pub clause: String,
    /// The #12 survivor this suggestion would kill, when the suggestion is the
    /// survivor-derived tightening (the strictly-stronger witness). `None` when
    /// the strictly-stronger witness is the structural equality test instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kills_survivor: Option<String>,
}

/// A synthetic span for a generated clause. Candidate clauses are not parsed from
/// source, so they carry a zero span (the lowerer reads only `Clause.expr`, never
/// the span, for an `ens`; `lower_fn` emits the `ens` from `Clause.expr`).
fn synth_span() -> Span {
    Span::new(0, 0)
}

/// Build a candidate `ens` clause `result == <rhs>` from an `Expr` for `<rhs>`
/// (REQ-1). The clause's `expr` is the equality the lowerer emits as `ensures
/// result == <rhs>`; `text` mirrors the rendered surface form so the clause is
/// self-consistent.
fn result_equals(rhs: Expr) -> Clause {
    let rendered = format!("result == {}", render_expr(&rhs));
    Clause {
        expr: Expr::Binary {
            op: BinOp::Eq,
            lhs: Box::new(Expr::Path(vec!["result".to_string()])),
            rhs: Box::new(rhs),
        },
        text: rendered,
        span: synth_span(),
    }
}

/// Render a candidate `Expr` to its surface form (REQ-1/REQ-4). The grammar is
/// FROZEN — candidates come ONLY from `generate_candidates`'s template, so this
/// handles exactly those shapes (a path, an int literal, a depth-1 binary, a
/// `len()` method call, a spec-fn call). A shape outside the frozen grammar
/// renders to a parenthesised debug-free fallback that never panics (R-CODE-2);
/// it is never produced by the template.
pub fn render_expr(e: &Expr) -> String {
    match e {
        Expr::Path(segs) => segs.join("::"),
        Expr::IntLit { value, .. } => value.to_string(),
        Expr::BoolLit(b) => b.to_string(),
        Expr::Binary { op, lhs, rhs } => {
            format!(
                "{} {} {}",
                render_expr(lhs),
                binop_token(*op),
                render_expr(rhs)
            )
        }
        Expr::MethodCall {
            receiver,
            name,
            args,
        } => {
            let rendered_args: Vec<String> = args.iter().map(render_expr).collect();
            format!(
                "{}.{}({})",
                render_expr(receiver),
                name,
                rendered_args.join(", ")
            )
        }
        Expr::Call { callee, args } => {
            let rendered_args: Vec<String> = args.iter().map(render_expr).collect();
            format!("{}({})", render_expr(callee), rendered_args.join(", "))
        }
        // Cluster C9-B (`.design/basis/10-recursion-tuples.md` REQ-5/REQ-8, #109):
        // a tuple projection `result.0` / a tuple construction `(a, b)` can appear
        // in a strengthenable `ens` (the GROUNDED `ens result.0 == b`), so render
        // both FAITHFULLY (the surface text a strengthening suggestion echoes)
        // rather than the `<unsupported>` placeholder.
        Expr::TupleProj { receiver, index } => format!("{}.{index}", render_expr(receiver)),
        Expr::Tuple(elems) => {
            let parts: Vec<String> = elems.iter().map(render_expr).collect();
            format!("({})", parts.join(", "))
        }
        // The frozen template never emits another shape; render a safe, non-panic
        // placeholder so the function is total (R-CODE-2). Discarded downstream.
        _ => "<unsupported>".to_string(),
    }
}

/// The surface token of a `BinOp` (matches the lowerer / parser surface).
fn binop_token(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Rem => "%",
        BinOp::Shl => "<<",
        BinOp::Shr => ">>",
        BinOp::BitAnd => "&",
        BinOp::BitOr => "|",
        BinOp::BitXor => "^",
        BinOp::Eq => "==",
        BinOp::Ne => "!=",
        BinOp::Lt => "<",
        BinOp::Le => "<=",
        BinOp::Gt => ">",
        BinOp::Ge => ">=",
        BinOp::And => "&&",
        BinOp::Or => "||",
    }
}

/// Generate the FROZEN, DETERMINISTIC, BOUNDED candidate stronger-`ens` set of
/// `f` (REQ-1/REQ-6). A pure function of `f` + the file's `spec fn`s + the #12
/// `survivor`, producing an ORDERED, CAPPED list in this FIXED family order:
///
/// 1. **spec-fn equality** — for each `spec fn` `s` in `spec_items` whose
///    parameter types match `f`'s parameters (in order) and whose return type
///    matches `f`'s return type, the clause `result == s(<f's params>)` (the §4.2
///    "spec functions are executable" pinning form).
/// 2. **result-equals-input-expression** — `result == <e>` for `e` in a depth-1
///    frozen grammar over `f`'s parameters of the return type: a bare scalar
///    parameter `p`; a binary combination `a OP b` of two scalar parameters under
///    the frozen `{+, -, *}` set; a `len()` of a slice parameter for an integer
///    return (`result == xs.len()`).
/// 3. **survivor-derived tighter bound** — when the #12 `survivor` is the early
///    `return 0` mutant and family 2 produced a binary `result == a OP b`
///    candidate, that candidate carries the `kills_survivor` link (the `0`
///    early-return value violates `result == a + b`). The link is attached to the
///    matching family-2 candidate rather than duplicating the clause (the same
///    clause is one candidate, tagged with the survivor it kills).
///
/// The list is bounded by [`CANDIDATE_CAP`] (the first `CANDIDATE_CAP` in family
/// order). A pure function ⇒ the same ordered list every run (REQ-6).
pub fn generate_candidates(
    f: &FnItem,
    spec_items: &[Item],
    score: &MutationScore,
) -> Vec<CandidateClause> {
    let mut candidates: Vec<CandidateClause> = Vec::new();

    // Family 1: spec-fn equality. For each in-scope `spec fn` with a matching
    // signature, `result == s(<params>)`.
    for item in spec_items {
        if let Item::SpecFn(s) = item {
            if spec_fn_signature_matches(f, &s.params, &s.ret) {
                let call = Expr::Call {
                    callee: Box::new(Expr::Path(vec![s.name.clone()])),
                    args: f
                        .params
                        .iter()
                        .map(|p| Expr::Path(vec![p.name.clone()]))
                        .collect(),
                };
                let clause = result_equals(call);
                let rendered = clause.text.clone();
                candidates.push(CandidateClause {
                    ens: clause,
                    rendered,
                    kills_survivor: None,
                });
            }
        }
    }

    // Family 2: result-equals-input-expression over a depth-1 frozen grammar.
    // Only meaningful for a scalar (integer) return — the candidates pin the
    // result to an in-scope expression of the return type.
    let survivor_is_early_zero = survivor_is_early_return_zero(score);
    if is_integer_return(&f.ret) {
        let scalar_params: Vec<&str> = f
            .params
            .iter()
            .filter(|p| param_matches_return(&p.ty, &f.ret))
            .map(|p| p.name.as_str())
            .collect();

        // 2a: a bare scalar parameter `p` of the return type — `result == p`.
        for name in &scalar_params {
            let clause = result_equals(Expr::Path(vec![name.to_string()]));
            let rendered = clause.text.clone();
            candidates.push(CandidateClause {
                ens: clause,
                rendered,
                kills_survivor: None,
            });
        }

        // 2b: a binary combination `a OP b` of two scalar parameters under the
        // frozen `{+, -, *}` set — `result == a + b`. Pairs in source order; the
        // operator set in a fixed order. Family-3 link: when the survivor is the
        // early `return 0` mutant, the `+` candidate (and any binary equality)
        // KILLS it (`0 == a + b` is unprovable), so it carries the survivor link.
        for i in 0..scalar_params.len() {
            for j in (i + 1)..scalar_params.len() {
                for op in [BinOp::Add, BinOp::Sub, BinOp::Mul] {
                    let bin = Expr::Binary {
                        op,
                        lhs: Box::new(Expr::Path(vec![scalar_params[i].to_string()])),
                        rhs: Box::new(Expr::Path(vec![scalar_params[j].to_string()])),
                    };
                    let clause = result_equals(bin);
                    let rendered = clause.text.clone();
                    candidates.push(CandidateClause {
                        ens: clause,
                        rendered,
                        // Family 3: a binary equality on `result` rejects the
                        // early-return-0 body (the §7 survivor-kill witness).
                        kills_survivor: if survivor_is_early_zero {
                            score.survivor.clone()
                        } else {
                            None
                        },
                    });
                }
            }
        }

        // 2c: `result == xs.len()` for each slice parameter (an integer-typed
        // length pinning). A weak length-bounded `ens` is strengthened to the
        // exact length where the body returns one.
        for p in &f.params {
            if is_slice_param(&p.ty) {
                let len_call = Expr::MethodCall {
                    receiver: Box::new(Expr::Path(vec![p.name.clone()])),
                    name: "len".to_string(),
                    args: Vec::new(),
                };
                let clause = result_equals(len_call);
                let rendered = clause.text.clone();
                candidates.push(CandidateClause {
                    ens: clause,
                    rendered,
                    kills_survivor: None,
                });
            }
        }
    }

    candidates.truncate(CANDIDATE_CAP);
    candidates
}

/// Build a COPY of `f` whose `ens` is REPLACED by the single candidate clause
/// (REQ-2). The body, `req`, and `fx` are UNCHANGED — only the postcondition is
/// the candidate, so the verify step proves the candidate against the REAL body
/// (the §7 step-5 "proves with no body change"). The candidate REPLACES the `ens`
/// (rather than conjoining) so the verify run is a clean test of the candidate
/// alone; a candidate strictly stronger than the current `ens` that PROVES on its
/// own is adoptable as the strengthened `ens` (the grounding harness verified the
/// candidate as the sole `ens`).
pub fn candidate_fn(f: &FnItem, candidate: &CandidateClause) -> FnItem {
    let mut copy = f.clone();
    copy.contract.ens = vec![candidate.ens.clone()];
    copy
}

/// `true` iff the candidate is STRICTLY STRONGER than the current `ens` (REQ-3,
/// OQ-5). Two witnesses, NO extra implication solver query:
///
/// 1. **survivor-kill** — the candidate carries a `kills_survivor` link and the
///    survivor body does NOT verify the candidate (`survivor_killed == true`): the
///    candidate REJECTS a body the current `ens` ACCEPTED, so it is provably
///    stronger on that witness.
/// 2. **structural equality** — the candidate is an `result == <e>` equality while
///    the current `ens` does NOT already pin `result` with an equality
///    (`current_pins_result == false`): an equality strictly narrows a
///    satisfiable range (an inequality / non-`result` `ens`).
///
/// A candidate that is logically EQUAL to or WEAKER than the current `ens` (no
/// survivor kill AND the current `ens` already pins `result`) is DISCARDED.
pub fn is_strictly_stronger(
    candidate: &CandidateClause,
    current_pins_result: bool,
    survivor_killed: bool,
) -> bool {
    // Witness 1: it kills a survivor the current `ens` accepted.
    if candidate.kills_survivor.is_some() && survivor_killed {
        return true;
    }
    // Witness 2: it adds a `result ==` equality the current `ens` lacked.
    !current_pins_result
}

/// `true` iff the current `ens` already PINS `result` with an equality (REQ-3
/// witness 2). When it does, a family-1/2 equality candidate is NOT strictly
/// stronger (the output is already pinned — `sum`'s `ens result == spec_sum(xs)`).
/// A SHAPE test over the clause set: any `ens` of the form `result == <e>` (a
/// top-level `BinOp::Eq` with `result` on either side). DETERMINISTIC, no solver.
pub fn current_ens_pins_result(ens: &[Clause]) -> bool {
    ens.iter().any(|c| clause_is_result_equality(&c.expr))
}

/// `true` iff `e` is a top-level `result == <e>` (or `<e> == result`) equality.
fn clause_is_result_equality(e: &Expr) -> bool {
    if let Expr::Binary {
        op: BinOp::Eq,
        lhs,
        rhs,
    } = e
    {
        return expr_is_result(lhs) || expr_is_result(rhs);
    }
    false
}

/// `true` iff `e` is the `result` path.
fn expr_is_result(e: &Expr) -> bool {
    matches!(e, Expr::Path(segs) if segs.len() == 1 && segs[0] == "result")
}

/// `true` iff the #12 survivor is the early-`return 0` mutant (the family-3
/// driver). The mutation scorer's description for the scalar-zero early return is
/// `"insert early `return 0` at body head"` (`mutation::early_return_value` /
/// `mutation::zero_desc`).
fn survivor_is_early_return_zero(score: &MutationScore) -> bool {
    score
        .survivor
        .as_deref()
        .map(|d| d.contains("return 0"))
        .unwrap_or(false)
}

/// `true` iff `ret` is an integer primitive return type (`u32`/`u64`/`usize`) —
/// the family-2 grammar's domain.
fn is_integer_return(ret: &Type) -> bool {
    matches!(
        ret,
        Type::Prim(PrimType::U32 | PrimType::U64 | PrimType::Usize)
    )
}

/// `true` iff a parameter type matches the return type exactly (for the bare-`p`
/// and binary `a OP b` family-2 candidates — the operands must be the same scalar
/// type as the result).
fn param_matches_return(param: &Type, ret: &Type) -> bool {
    param == ret
}

/// `true` iff a parameter is a reference-to-slice (`&[T]`) — the `len()` family-2c
/// candidate's domain.
fn is_slice_param(ty: &Type) -> bool {
    matches!(ty, Type::Ref { inner, .. } if matches!(inner.as_ref(), Type::Slice(_)))
}

/// `true` iff `f`'s parameter types match `params` in order AND `f`'s return type
/// matches `ret` (the family-1 spec-fn-equality signature check). A `spec fn`
/// whose signature matches `f`'s can be applied to `f`'s parameters to pin the
/// result (`result == s(<f's params>)`).
fn spec_fn_signature_matches(f: &FnItem, params: &[thermite_syntax::Param], ret: &Type) -> bool {
    if &f.ret != ret {
        return false;
    }
    if f.params.len() != params.len() {
        return false;
    }
    f.params
        .iter()
        .zip(params.iter())
        .all(|(a, b)| a.ty == b.ty)
}

/// Run the strengthening probe over `f` (REQ-2/REQ-3/REQ-4). For each candidate
/// (in the deterministic family order):
///
/// 1. VERIFY the candidate against the REAL body via `verify_body` (the threaded
///    `check::run_verus` of the `item_subprogram` shape, content-addressed via the
///    #8 cache). A candidate that does NOT verify is DISCARDED (no unadoptable
///    suggestion, R-DEFER-1). A `ForgeError` propagates (R-CODE-4).
/// 2. For a candidate carrying a `kills_survivor` link, confirm the survivor-kill
///    witness: VERIFY the candidate against the SURVIVOR body via `verify_survivor`
///    — if it does NOT verify (`Proved == false`) the candidate KILLS the survivor
///    (the strictly-stronger witness 1). If it DOES verify, the survivor is not
///    actually killed, so the kill link is dropped.
/// 3. KEEP the candidate iff it is [`is_strictly_stronger`] (witness 1 or 2).
///
/// `verify_body` / `verify_survivor` return `Ok(true)` when verus PROVED the woven
/// `fn`, `Ok(false)` on a non-`Proved` outcome (counterexample / timeout) or an
/// un-lowerable fn (parallel to #12's drop), and `Err` on an environment failure.
/// The result is the ordered list of adoptable [`Suggestion`]s (DETERMINISTIC,
/// REQ-6).
pub fn probe(
    f: &FnItem,
    spec_items: &[Item],
    score: &MutationScore,
    mut verify_body: impl FnMut(&FnItem) -> Result<bool, ForgeError>,
    mut verify_survivor: impl FnMut(&CandidateClause) -> Result<bool, ForgeError>,
) -> Result<Vec<Suggestion>, ForgeError> {
    let candidates = generate_candidates(f, spec_items, score);
    let current_pins_result = current_ens_pins_result(&f.contract.ens);
    let mut suggestions = Vec::new();

    for candidate in &candidates {
        // REQ-2: the candidate must VERIFY against the real body to be adoptable.
        let woven = candidate_fn(f, candidate);
        if !verify_body(&woven)? {
            continue;
        }

        // REQ-3 witness 1: a survivor-linked candidate must actually KILL the
        // survivor (the survivor body must NOT verify the candidate).
        let survivor_killed = if candidate.kills_survivor.is_some() {
            !verify_survivor(candidate)?
        } else {
            false
        };

        if is_strictly_stronger(candidate, current_pins_result, survivor_killed) {
            suggestions.push(Suggestion {
                clause: candidate.rendered.clone(),
                // Carry the kill link ONLY when the kill was confirmed.
                kills_survivor: if survivor_killed {
                    candidate.kills_survivor.clone()
                } else {
                    None
                },
            });
        }
    }

    Ok(suggestions)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_fn(src: &str) -> FnItem {
        let parsed = thermite_syntax::parse(src);
        assert!(parsed.is_clean(), "fixture must parse: {:?}", parsed.errors);
        parsed
            .program
            .items
            .into_iter()
            .find_map(|i| match i {
                Item::Fn(f) => Some(f),
                _ => None,
            })
            .expect("fixture has a fn")
    }

    fn parse_program_items(src: &str) -> Vec<Item> {
        let parsed = thermite_syntax::parse(src);
        assert!(parsed.is_clean(), "fixture must parse: {:?}", parsed.errors);
        parsed.program.items
    }

    fn weak_fixture() -> FnItem {
        parse_fn("fn f(a: u32, b: u32) -> u32 req a <= 10 && b <= 10 ens result <= 1000000 fx pure { a + b }")
    }

    fn early_zero_survivor() -> MutationScore {
        MutationScore {
            killed: 0,
            scored: 1,
            survivor: Some("insert early `return 0` at body head".to_string()),
        }
    }

    // REQ-1 / AC-1: the weak fixture's family-2 candidates include `result == a +
    // b` (the expected adoptable suggestion). Expected clause traces to the oracle
    // `cases.json` `weak_loose_bound.expect_suggestion` (R-CHAR-3), not to forge's
    // own output.
    #[test]
    fn weak_fixture_generates_result_eq_a_plus_b() {
        let f = weak_fixture();
        let candidates = generate_candidates(&f, &[], &early_zero_survivor());
        let rendered: Vec<&str> = candidates.iter().map(|c| c.rendered.as_str()).collect();
        assert!(
            rendered.contains(&"result == a + b"),
            "the frozen family-2 grammar yields `result == a + b`: {rendered:?}"
        );
    }

    // REQ-1 (family 3 link): the `result == a + b` candidate over the early-zero
    // survivor carries the kill link (the survivor it would kill). Traces to REQ-1
    // family-3 (R-CHAR-3).
    #[test]
    fn binary_candidate_carries_survivor_kill_link() {
        let f = weak_fixture();
        let candidates = generate_candidates(&f, &[], &early_zero_survivor());
        let plus = candidates
            .iter()
            .find(|c| c.rendered == "result == a + b")
            .expect("a + b candidate present");
        assert_eq!(
            plus.kills_survivor.as_deref(),
            Some("insert early `return 0` at body head"),
            "a binary equality on result kills the early-return-0 survivor"
        );
    }

    // REQ-6 / AC-5: generate_candidates is a pure function — the same fn yields the
    // byte-identical ordered candidate list every call.
    #[test]
    fn generate_candidates_is_deterministic() {
        let f = weak_fixture();
        let a: Vec<String> = generate_candidates(&f, &[], &early_zero_survivor())
            .into_iter()
            .map(|c| c.rendered)
            .collect();
        let b: Vec<String> = generate_candidates(&f, &[], &early_zero_survivor())
            .into_iter()
            .map(|c| c.rendered)
            .collect();
        assert_eq!(a, b, "generate_candidates is deterministic");
    }

    // REQ-1 / AC-6: the candidate list is bounded by CANDIDATE_CAP.
    #[test]
    fn candidates_bounded_by_cap() {
        let f = weak_fixture();
        assert!(generate_candidates(&f, &[], &early_zero_survivor()).len() <= CANDIDATE_CAP);
    }

    // REQ-1 family 1: a matching `spec fn` produces a `result == s(<params>)`
    // candidate. Fixture: a `spec fn s(a: u32, b: u32) -> u32` matches `f`'s
    // signature.
    #[test]
    fn spec_fn_equality_candidate_for_matching_signature() {
        let items = parse_program_items(
            "spec fn s(a: u32, b: u32) -> u32 dec a { a + b } \
             fn f(a: u32, b: u32) -> u32 req a <= 10 && b <= 10 ens result <= 1000000 fx pure { a + b }",
        );
        let f = match items.iter().find(|i| i.name() == "f") {
            Some(Item::Fn(f)) => f.clone(),
            _ => panic!("f present"),
        };
        let spec_items: Vec<Item> = items
            .iter()
            .filter(|i| matches!(i, Item::SpecFn(_)))
            .cloned()
            .collect();
        let candidates = generate_candidates(&f, &spec_items, &early_zero_survivor());
        let rendered: Vec<&str> = candidates.iter().map(|c| c.rendered.as_str()).collect();
        assert!(
            rendered.contains(&"result == s(a, b)"),
            "family-1 spec-fn equality present: {rendered:?}"
        );
        // Family 1 is FIRST in the fixed order.
        assert_eq!(
            rendered.first().copied(),
            Some("result == s(a, b)"),
            "spec-fn equality is the first family"
        );
    }

    // REQ-3 witness 2: an equality candidate is strictly stronger than an `ens`
    // that does NOT pin result. Traces to REQ-3 (R-CHAR-3).
    #[test]
    fn equality_is_stronger_than_non_pinning_ens() {
        let f = weak_fixture();
        let c = &generate_candidates(&f, &[], &early_zero_survivor())[0];
        // The weak fixture's `ens result <= 1000000` does NOT pin result.
        assert!(!current_ens_pins_result(&f.contract.ens));
        assert!(
            is_strictly_stronger(c, false, false),
            "an equality candidate strengthens a non-pinning inequality ens"
        );
    }

    // REQ-3: a candidate is NOT strictly stronger when the current `ens` already
    // pins result and the candidate kills no survivor (the `sum` shape — already
    // exact-pinned).
    #[test]
    fn not_stronger_when_ens_already_pins_result() {
        let pinning = parse_fn(
            "fn g(a: u32, b: u32) -> u32 req a <= 10 && b <= 10 ens result == a + b fx pure { a + b }",
        );
        assert!(current_ens_pins_result(&pinning.contract.ens));
        // A family-2 equality candidate over this fn carries no kill link (the
        // survivor is killed by the existing exact ens, so no survivor remains).
        let no_survivor = MutationScore {
            killed: 1,
            scored: 1,
            survivor: None,
        };
        let candidates = generate_candidates(&pinning, &[], &no_survivor);
        for c in &candidates {
            assert!(
                !is_strictly_stronger(c, true, false),
                "no candidate strengthens an already-pinning ens with no survivor: {}",
                c.rendered
            );
        }
    }

    // REQ-2/REQ-3 (probe end-to-end, no verus): with a verify closure that PROVES
    // only `result == a + b` and a survivor-verify that REJECTS it (the kill), the
    // probe surfaces exactly the strictly-stronger adoptable suggestion. The verify
    // closures stand in for `check::run_verus` (the real driver is exercised by the
    // conformance test); here the FILTER logic is the unit under test (R-CHAR-3 —
    // the expected suggestion traces to the oracle, the verify polarity to REQ-2).
    #[test]
    fn probe_surfaces_only_verifying_strictly_stronger_candidate() {
        let f = weak_fixture();
        let score = early_zero_survivor();
        // Body verify: PROVE only the exact `result == a + b`; reject everything
        // else (the over-strong / wrong candidates do not hold for `{ a + b }`).
        let verify_body = |fk: &FnItem| -> Result<bool, ForgeError> {
            let ens = &fk.contract.ens[0].text;
            Ok(ens == "result == a + b")
        };
        // Survivor verify: the early-return-0 body does NOT prove `result == a + b`
        // (0 == a + b is unprovable) → killed.
        let verify_survivor = |_c: &CandidateClause| -> Result<bool, ForgeError> { Ok(false) };
        let suggestions = probe(&f, &[], &score, verify_body, verify_survivor).expect("probe ok");
        assert_eq!(
            suggestions.len(),
            1,
            "exactly the verifying strictly-stronger candidate is surfaced: {suggestions:?}"
        );
        assert_eq!(suggestions[0].clause, "result == a + b");
        assert_eq!(
            suggestions[0].kills_survivor.as_deref(),
            Some("insert early `return 0` at body head"),
            "the suggestion records the survivor it kills"
        );
    }

    // AC-3: a candidate that does NOT verify against the body is NOT suggested (no
    // unadoptable suggestions). With a verify closure that proves NOTHING, the
    // probe surfaces no suggestion.
    #[test]
    fn non_verifying_candidate_is_not_suggested() {
        let f = weak_fixture();
        let score = early_zero_survivor();
        let verify_body = |_fk: &FnItem| -> Result<bool, ForgeError> { Ok(false) };
        let verify_survivor = |_c: &CandidateClause| -> Result<bool, ForgeError> { Ok(false) };
        let suggestions = probe(&f, &[], &score, verify_body, verify_survivor).expect("probe ok");
        assert!(
            suggestions.is_empty(),
            "no candidate verifies → no suggestion: {suggestions:?}"
        );
    }

    // REQ-4 (sum / AC-2 shape): an already-pinning `ens` with every candidate that
    // verifies BUT is not strictly stronger yields NO suggestion. The verify
    // closure proves the spec-fn-equality (which equals the current ens) but it is
    // not strictly stronger.
    #[test]
    fn already_pinned_yields_no_suggestion() {
        let items = parse_program_items(
            "spec fn s(a: u32, b: u32) -> u32 dec a { a + b } \
             fn g(a: u32, b: u32) -> u32 req a <= 10 && b <= 10 ens result == s(a, b) fx pure { a + b }",
        );
        let g = match items.iter().find(|i| i.name() == "g") {
            Some(Item::Fn(f)) => f.clone(),
            _ => panic!("g present"),
        };
        let spec_items: Vec<Item> = items
            .iter()
            .filter(|i| matches!(i, Item::SpecFn(_)))
            .cloned()
            .collect();
        // No ens-killable survivor remains (the exact ens already kills the
        // early-return-0 mutant).
        let no_survivor = MutationScore {
            killed: 1,
            scored: 1,
            survivor: None,
        };
        // Verify EVERYTHING (the candidates all hold for the body), so the ONLY
        // filter that can reject them is strict-strength.
        let verify_body = |_fk: &FnItem| -> Result<bool, ForgeError> { Ok(true) };
        let verify_survivor = |_c: &CandidateClause| -> Result<bool, ForgeError> { Ok(true) };
        let suggestions =
            probe(&g, &spec_items, &no_survivor, verify_body, verify_survivor).expect("probe ok");
        assert!(
            suggestions.is_empty(),
            "an already-pinning ens has no strictly-stronger candidate: {suggestions:?}"
        );
    }
}
