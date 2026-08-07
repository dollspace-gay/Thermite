//! `forge/src/exec_tv.rs` — the exec-position (body) translation-validation check
//! phase (`.design/verified/exec-tv.md` REQ-5; epic crosslink #151, blockers #154 +
//! #156).
//!
//! Step-1 contract-TV (`forge/src/contract_tv.rs`) certifies that the emitted Verus
//! contract (`req`/`ens`/`inv`/`dec`) means the same as the source contract. It does
//! not cover the exec body, where the #122 (`(n - 1) as nat` paren) and #146
//! (`x as u32 < 33` cast-`<` mis-parse) infidelity classes generally live. This
//! phase closes that gap for pure exec-position expressions (step 2.1): for each
//! such expr it computes
//!
//!   `P_production = thermite_lower::lower_exec_expr(expr)`             (the artifact under test)
//!   `reference    = thermite_tv::exec_ref_value(expr, …)`             (the independent bounded reference)
//!
//! wraps them as the exec-fn obligation `fn tv_exec_wrap(..) ensures result ==
//! <reference> { <P_production> }` (`thermite_tv::exec_equivalence_obligation`), and
//! discharges it through `verus`. Verified ⟺ the exec lowering of that expr is
//! faithful (it computes the bounded reference value for all inputs); a
//! `postcondition not satisfied` / type / parse error ⟺ a exec-lowering
//! infidelity (the off-corpus #122/#146/overflow/off-by-one classes). It is exposed
//! as `forge exec-tv <file>`, a separate opt-in deeper audit (like `forge tv`), not
//! folded into `forge check`.
//!
//! `thermite-tv` stays independent of `thermite-lower` (the N-version boundary,
//! AC-6): this forge module is the one place the two exec encoders meet.
//!
//! ## Two runs (both surfaced; the generated run is primary)
//!
//! - The generated run ([`run_generated`], primary): over
//!   `thermite_tv::gen_exec_exprs`, the off-corpus #122/#146 regression guard
//!   (REQ-3). Each generated `ExecClause` carries an adequate frame (every base
//!   scalar `<= 1000`, an index `< xs.len()`), so the faithful lowerer makes every
//!   clause `Faithful`; a `Divergent`/`Unverifiable` is an off-corpus
//!   exec-lowering infidelity (surfaced, with a blocker). Reports the construct
//!   coverage (cast-`<` / arith / cast / index).
//! - The corpus body-expr check ([`exec_tv_file`], best-effort): over the pure
//!   exec expressions of each corpus fn whose var-frame is derivable (a `let`-RHS, a
//!   tail/`return` expr; the in-scope vars and their exec types known from the
//!   surrounding params/lets). Statements / loops / mutation are out of scope (step
//!   2.2) and skipped. Corpus coverage is partial: the var-frame
//!   derivation is hard for arbitrary bodies (an arithmetic expr's adequate overflow
//!   frame is not always derivable from the source `req`/`inv` text), and the
//!   generated run is the primary value.
//!
//! ## REQ status
//!
//! <!-- generated:reqs view=forge-exec-tv-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-TV-EXEC-FORGE-PLUGIN | shipped | `forge/src/exec_tv.rs` | Exec-TV forge plug-in point |  |
//! <!-- /generated:reqs -->

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

use thermite_syntax::ast::{Clause, Expr, FnItem, IndexArg, Item, PrimType, Stmt, Type};

use thermite_tv::exec_encode::{exec_ref_value, ExecRefCtx};
use thermite_tv::gen_exec_exprs;
use thermite_tv::obligation::{exec_equivalence_obligation, ExecObligationFrame, ExecParamDecl};

use crate::check::{unique_scratch_dir, ScratchDir, DEFAULT_RLIMIT, DEFAULT_SOLVER_SEED};
use crate::cli::ForgeError;

/// One exec expr's TV verdict (REQ-5; the four-way classification, reported
/// distinctly so Unverifiable / Skipped does not mask an infidelity). `Faithful` ⟺
/// the obligation verified (the exec lowering of this expr means the bounded
/// reference value); `Divergent` ⟺ verus found a counterexample or the production
/// text did not compile/parse (an exec-lowering infidelity); `Unverifiable` ⟺ the
/// obligation did not discharge for a non-infidelity reason (verus absent, or an
/// inadequate frame so verus could not prove the postcondition without a
/// wrap/overflow bound), reported distinctly, not as Faithful; `Skipped` ⟺ the
/// expr / statement is out of the pure-exec step-2.1 subset (a statement, a loop, a
/// non-derivable frame, an `exec_ref_value`/`lower_exec_expr` Unsupported).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecVerdict {
    Faithful,
    Divergent { detail: String },
    Unverifiable { reason: String },
    Skipped { reason: String },
}

/// One exec expr's TV result: a human label + the verdict (REQ-5).
#[derive(Debug, Clone)]
pub struct ExecResult {
    /// A human label (`gen#3`, `sum.let#1`, `sum.tail`, …).
    pub label: String,
    /// The verdict.
    pub verdict: ExecVerdict,
}

/// The aggregate exec-TV report for one run (REQ-5). `divergent` is the headline: 0
/// divergent over the generated run is the off-corpus faithfulness AC; a
/// divergence is an exec-lowering finding.
#[derive(Debug, Clone, Default)]
pub struct ExecTvReport {
    pub results: Vec<ExecResult>,
}

impl ExecTvReport {
    /// The per-verdict integer tally (the reported counts).
    pub fn counts(&self) -> ExecCounts {
        let mut c = ExecCounts::default();
        for r in &self.results {
            match &r.verdict {
                ExecVerdict::Faithful => c.faithful += 1,
                ExecVerdict::Divergent { .. } => c.divergent += 1,
                ExecVerdict::Unverifiable { .. } => c.unverifiable += 1,
                ExecVerdict::Skipped { .. } => c.skipped += 1,
            }
        }
        c
    }
}

/// The per-verdict integer tally (REQ-5 — the reported "N checked, M divergent").
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ExecCounts {
    pub faithful: usize,
    pub divergent: usize,
    pub unverifiable: usize,
    pub skipped: usize,
}

impl ExecCounts {
    /// The exprs that reached verus and produced a faithfulness verdict
    /// (faithful + divergent). Unverifiable / Skipped did not.
    pub fn checked(&self) -> usize {
        self.faithful + self.divergent
    }
}

/// The off-corpus construct-coverage breakdown the generated run reports (REQ-3 /
/// AC-7 — the #122/#146 regression-guard surface; reported so the guard is
/// non-vacuous). A clause contributes to multiple buckets (an indexed expr also
/// casts).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ExecConstructCoverage {
    pub arith: usize,
    pub casts: usize,
    /// A `Cast` left operand of a `<`-leading op (`<`/`<=`/`<<`), the #146 class.
    pub cast_lt: usize,
    pub index: usize,
    pub shifts: usize,
    pub bitops: usize,
}

/// Tally the construct coverage of a slice of generated exec exprs (REQ-3 / AC-7).
fn construct_coverage(exprs: &[&Expr]) -> ExecConstructCoverage {
    let mut c = ExecConstructCoverage::default();
    for e in exprs {
        tally_construct(e, &mut c);
    }
    c
}

fn tally_construct(e: &Expr, c: &mut ExecConstructCoverage) {
    use thermite_syntax::ast::BinOp;
    match e {
        Expr::Binary { op, lhs, rhs } => {
            if matches!(op, BinOp::Add | BinOp::Sub | BinOp::Mul) {
                c.arith += 1;
            } else if matches!(op, BinOp::Shl | BinOp::Shr) {
                c.shifts += 1;
            } else if matches!(op, BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor) {
                c.bitops += 1;
            }
            if matches!(op, BinOp::Lt | BinOp::Le | BinOp::Shl)
                && matches!(lhs.as_ref(), Expr::Cast { .. })
            {
                c.cast_lt += 1;
            }
            tally_construct(lhs, c);
            tally_construct(rhs, c);
        }
        Expr::Cast { expr, .. } => {
            c.casts += 1;
            tally_construct(expr, c);
        }
        Expr::Unary { expr, .. } => tally_construct(expr, c),
        Expr::Index { base, index } => {
            c.index += 1;
            tally_construct(base, c);
            if let IndexArg::Single(i) = index {
                tally_construct(i, c);
            }
        }
        _ => {}
    }
}

// ---- the generated run (primary) -------------------------------------------

/// Run the off-corpus generated exec-TV run (REQ-3 / REQ-5; the primary value, the
/// #122/#146 off-corpus regression guard). Generates `n` well-framed exec exprs
/// deterministically from `seed` (`thermite_tv::gen_exec_exprs`), lowers each via
/// `thermite_lower::lower_exec_expr`, builds and discharges the exec-fn obligation
/// against the carried adequate frame, and reports. The lowerer is faithful and the
/// frames are adequate, so all should verify; a `Divergent`/`Unverifiable` is an
/// off-corpus exec-lowering infidelity / framing hole (surfaced).
pub fn run_generated(
    seed: u64,
    n: usize,
    rlimit: f64,
) -> Result<(ExecTvReport, ExecConstructCoverage), ForgeError> {
    let clauses = gen_exec_exprs(seed, n);
    let coverage = construct_coverage(&clauses.iter().map(|c| &c.expr).collect::<Vec<_>>());
    let mut report = ExecTvReport::default();
    for (i, clause) in clauses.iter().enumerate() {
        let label = format!("gen#{i}");
        // P_production — the exec lowering of the generated expr (the artifact
        // under test, the eventual non-test consumer of `lower_exec_expr`).
        let p_production = match thermite_lower::lower_exec_expr(&clause.expr) {
            Ok(p) => p,
            Err(e) => {
                // A generated expr the exec lowering does not compile is an
                // infidelity (the generator only emits the in-scope exec subset):
                // Divergent, not Skipped (a non-compiling production is the
                // #122/#146 catch shape).
                report.results.push(ExecResult {
                    label,
                    verdict: ExecVerdict::Divergent {
                        detail: format!(
                            "production exec lowering FAILED to compile the generated \
                             expr — a real off-corpus exec-lowering infidelity: {e}"
                        ),
                    },
                });
                continue;
            }
        };
        let frame = clause_frame(clause);
        let program = match exec_equivalence_obligation(&clause.expr, &p_production, &frame) {
            Ok(prog) => prog,
            Err(e) => {
                // The independent reference does not cover the generated expr:
                // Skipped (the generator stays within the encoder's subset, so this
                // is not expected; reported, not a false faithful).
                report.results.push(ExecResult {
                    label,
                    verdict: ExecVerdict::Skipped {
                        reason: format!("exec reference encoder does not cover: {e}"),
                    },
                });
                continue;
            }
        };
        let verdict = discharge(&program, &label, seed, rlimit);
        report.results.push(ExecResult { label, verdict });
    }
    Ok((report, coverage))
}

/// Build the [`ExecObligationFrame`] for a generated [`thermite_tv::ExecClause`] —
/// the params (at their exec types), the return type, the adequate overflow/index
/// `req`, and the slice-param set, all carried by the clause (REQ-3 — the frame is
/// part of the generated unit).
fn clause_frame(clause: &thermite_tv::ExecClause) -> ExecObligationFrame {
    ExecObligationFrame {
        spec_defs: Vec::new(),
        params: clause
            .params
            .iter()
            .map(|(name, ty)| ExecParamDecl::new(name.clone(), ty.clone()))
            .collect(),
        ret_type: clause.ret_type.clone(),
        req: clause.req.clone(),
        slice_params: clause.slice_params.clone(),
        fixed_array_params: Vec::new(),
        fixed_array_fields: Vec::new(),
        mutable_records: Vec::new(),
        result_is_fixed_array: false,
        result_record: None,
    }
}

// ---- the corpus body-expr check (best-effort) ------------------------------

/// Run the corpus body-expr exec-TV check over a `.th` file (REQ-5; best-effort).
/// For each `fn` item, extract the pure exec expressions whose var-frame is
/// derivable (the RHS of a typed `let`, the body tail / a `return` expr) and
/// TV-check each. Statements / loops / mutation are out of scope (step 2.2) and
/// skipped. Coverage is partial: an arithmetic expr's adequate overflow frame is
/// not always derivable from the source `req`/`inv` text. Such an expr is
/// Unverifiable, not a false Faithful.
pub fn exec_tv_file(path: &Path, seed: u64, rlimit: f64) -> Result<ExecTvReport, ForgeError> {
    // A single `.th` file or a canonical `.thpkg.json` manifest; the package
    // closure parses module by module, keeping each diagnostic module-local.
    let resolved = crate::thermite_package::load_source(path)?;
    let program = resolved.program();

    let mut report = ExecTvReport::default();
    for item in &program.items {
        match item {
            Item::Fn(f) => exec_tv_fn(program, f, seed, rlimit, &mut report),
            // A `spec fn` body lowers in spec context (not exec), out of scope for
            // exec-TV; a struct/enum has no exec body.
            Item::Const(_) | Item::SpecFn(_) | Item::Struct(_) | Item::Enum(_) => {}
            // Forge-tier item (stage1-forge-tier.md REQ-3): no v1 exec-TV consumer
            // yet (increments 2b-3); inert here, mirroring the spec/ADT no-op arm.
            Item::Forge(_) => {}
        }
    }
    Ok(report)
}

/// Whether `expr` is one direct call to an ordinary in-language function with
/// at least one exclusive-reference formal. Such a call is stateful: its value
/// and post-state are validated together by body TV, never by placing an exec
/// call inside the pure per-expression specification obligation.
pub(crate) fn is_direct_mutable_call(program: &thermite_syntax::Program, expr: &Expr) -> bool {
    let Expr::Call { callee, .. } = expr else {
        return false;
    };
    let Expr::Path(path) = callee.as_ref() else {
        return false;
    };
    let [name] = path.as_slice() else {
        return false;
    };
    program.items.iter().any(|item| match item {
        Item::Fn(function)
            if function.name == *name
                && function.body.is_some()
                && function.boundary.is_none()
                && function.slag.is_none() =>
        {
            function
                .params
                .iter()
                .any(|parameter| matches!(parameter.ty, Type::Ref { mutable: true, .. }))
        }
        _ => false,
    })
}

/// Whether `expr` is one direct call that consumes an admitted finite named
/// record by value. An isolated exec-expression obligation does not carry the
/// caller's leafwise lifecycle overlays, so body TV owns this call together
/// with the surrounding record state. The callee's ordinary native-record body
/// remains independently expression/body validated in its own frame.
pub(crate) fn is_direct_record_value_call(program: &thermite_syntax::Program, expr: &Expr) -> bool {
    let Expr::Call { callee, .. } = expr else {
        return false;
    };
    let Expr::Path(path) = callee.as_ref() else {
        return false;
    };
    let [name] = path.as_slice() else {
        return false;
    };
    let admitted: BTreeSet<String> = crate::body_tv::named_record_frames(program)
        .into_iter()
        .map(|record| record.type_name)
        .collect();
    program.items.iter().any(|item| {
        match item {
        Item::Fn(function)
            if function.name == *name
                && function.body.is_some()
                && function.boundary.is_none()
                && function.slag.is_none() =>
        {
            function.params.iter().any(|parameter| {
                matches!(&parameter.ty, Type::Named(type_name) if admitted.contains(type_name))
            })
        }
        _ => false,
    }
    })
}

/// Calls whose value cannot be isolated from the complete caller lifecycle are
/// validated by body TV instead of the pure leaf-expression TV pass.
pub(crate) fn is_direct_body_state_call(program: &thermite_syntax::Program, expr: &Expr) -> bool {
    is_direct_mutable_call(program, expr) || is_direct_record_value_call(program, expr)
}

/// Translation-validate the exact executable guard used by an L3 total export
/// wrapper. The guard is checked over the full input domain: the synthetic
/// frame's `req true` intentionally does not assume the guard being validated.
/// This is the wrapper-specific bridge between contract-position syntax and
/// its executable boundary use.
pub fn exec_tv_export_guard(
    program: &thermite_syntax::Program,
    f: &FnItem,
    seed: u64,
    rlimit: f64,
) -> ExecResult {
    let label = format!("{}.export_guard", f.name);
    let mut env = ExecEnv {
        constant_names: program
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Const(capacity) => Some(capacity.name.clone()),
                _ => None,
            })
            .collect(),
        ..Default::default()
    };
    env.fixed_array_fields = crate::body_tv::fixed_array_field_bindings(program, f);
    env.mutable_records = match crate::body_tv::mutable_record_frames(program, f) {
        Ok(records) => records,
        Err(reason) => {
            return ExecResult {
                label,
                verdict: ExecVerdict::Skipped { reason },
            }
        }
    };
    for param in &f.params {
        let Some((ty, slice)) = exec_type_spelling(&param.ty) else {
            return ExecResult {
                label,
                verdict: ExecVerdict::Skipped {
                    reason: format!(
                        "export guard parameter `{}` has an unframable type {:?}",
                        param.name, param.ty
                    ),
                },
            };
        };
        env.bind(&param.name, ty, slice);
    }
    let mut frame_fn = f.clone();
    frame_fn.contract.req = Clause {
        expr: Expr::BoolLit(true),
        text: "true".to_string(),
        span: f.contract.req.span,
        bv: None,
    };
    let support_defs = match crate::body_tv::body_tv_support(program, f) {
        Ok((defs, _, _)) => defs,
        Err(reason) => {
            return ExecResult {
                label,
                verdict: ExecVerdict::Skipped { reason },
            }
        }
    };
    let mut report = ExecTvReport::default();
    check_corpus_expr(
        program,
        &f.contract.req.expr,
        &label,
        "bool",
        &env,
        &frame_fn,
        &support_defs,
        false,
        seed,
        rlimit,
        &mut report,
    );
    report.results.pop().unwrap_or(ExecResult {
        label,
        verdict: ExecVerdict::Unverifiable {
            reason: "wrapper guard TV produced no result".to_string(),
        },
    })
}

/// The exec-type environment for a corpus fn body: each in-scope var's
/// [`ExecParamDecl`] (its exec value-type spelling) + the slice-bound name set. Built
/// from the params and extended by each typed `let` as the walk descends.
#[derive(Debug, Clone, Default)]
struct ExecEnv {
    params: Vec<ExecParamDecl>,
    slice_params: Vec<String>,
    fixed_array_params: Vec<String>,
    fixed_array_fields: Vec<String>,
    mutable_records: Vec<thermite_tv::MutableRecordFrame>,
    constant_names: Vec<String>,
    local_relations: Vec<LocalRelation>,
}

#[derive(Debug, Clone)]
struct LocalRelation {
    name: String,
    predicate: String,
    dependencies: Vec<String>,
}

impl ExecEnv {
    fn bind(&mut self, name: &str, ty_str: String, is_slice: bool) {
        // A re-`let` of the same name keeps the first binding (v0.1 corpus locals are
        // not re-typed); deduplicate so the obligation signature is well-formed.
        if self.params.iter().any(|p| p.name == name) {
            return;
        }
        let is_fixed_array = ty_str.starts_with('[') && ty_str.contains(';');
        self.params
            .push(ExecParamDecl::new(name.to_string(), ty_str));
        if is_slice {
            self.slice_params.push(name.to_string());
        }
        if is_fixed_array {
            self.fixed_array_params.push(name.to_string());
        }
    }

    /// The names this env declares (the obligation can reference these).
    fn declares(&self, name: &str) -> bool {
        self.params.iter().any(|p| p.name == name)
            || self.constant_names.iter().any(|constant| constant == name)
    }

    /// Record the exact independently encoded value of a scalar typed local.
    /// Later per-expression obligations use these equations to recover bounds
    /// such as `changed == slot % CAPACITY` rather than treating every local as
    /// an unrelated arbitrary parameter.
    fn bind_local_relation(&mut self, name: &str, ty: &str, init: &Expr) {
        let bounded = matches!(ty, "u8" | "u16" | "u32" | "u64" | "usize");
        if !bounded && ty != "bool" {
            return;
        }
        let context = ExecRefCtx::with_slice_bound(self.slice_params.iter().cloned())
            .with_fixed_array_bound(self.fixed_array_params.iter().cloned())
            .with_fixed_array_fields(self.fixed_array_fields.iter().cloned());
        let Ok(reference) = exec_ref_value(init, &context) else {
            return;
        };
        let value = if bounded {
            format!("({reference}) as {ty}")
        } else {
            reference
        };
        let mut dependencies = Vec::new();
        collect_free_paths(init, &mut dependencies);
        dependencies.retain(|dependency| self.declares(dependency));
        dependencies.sort();
        dependencies.dedup();
        self.local_relations.push(LocalRelation {
            name: name.to_string(),
            predicate: format!("{name} == {value}"),
            dependencies,
        });
    }
}

/// TV the derivable pure exec exprs of one fn body (REQ-5, best-effort). Builds the
/// param env from the signature, then walks the top-level block: a typed `let x: T =
/// rhs` checks `rhs` at ret_type `T` and binds `x`; the body tail / a top-level
/// `return e` checks `e` at the fn return type. Nested-block statements (loop / if
/// bodies), assignments, and untyped lets are skipped.
fn exec_tv_fn(
    program: &thermite_syntax::Program,
    f: &FnItem,
    seed: u64,
    rlimit: f64,
    report: &mut ExecTvReport,
) {
    let Some(body) = &f.body else {
        return; // a boundary fn has no in-language body.
    };

    // #193/#195 open-hole gate (`.design/forge/goal-repl.md` REQ-5; the four-way's
    // out-of-subset class): a fn carrying any open body hole (`?N`) is incomplete.
    // A hole is recorded on `FnItem.holes`, not in the `Stmt` stream, so checking the
    // body's exprs here would lower a hole-stripped body and report `Faithful` for
    // the tail expr of an unfinished body. An incomplete body is not checkable, so it
    // is Skipped with the OpenHole reason (not Faithful, R-HONEST-3) before any
    // expr lowers, mirroring `check`'s `OpenHole` reject (the shared
    // `goal_repl::open_hole_reason`, the #192 single-copy lesson).
    if let Some(reason) = crate::goal_repl::open_hole_reason(f) {
        report.results.push(ExecResult {
            label: f.name.clone(),
            verdict: ExecVerdict::Skipped { reason },
        });
        return;
    }

    let support_defs = match crate::body_tv::body_tv_support(program, f) {
        Ok((defs, _, _)) => defs,
        Err(reason) => {
            report.results.push(ExecResult {
                label: f.name.clone(),
                verdict: ExecVerdict::Skipped { reason },
            });
            return;
        }
    };

    // The signature env: each param at its exec value type. A param of a type the
    // exec frame cannot spell (Map/Option/String/heap wrappers) is recorded as un-spellable;
    // an expr that references it is then Skipped (non-derivable frame).
    let mut env = ExecEnv {
        constant_names: program
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Const(capacity) => Some(capacity.name.clone()),
                _ => None,
            })
            .collect(),
        ..Default::default()
    };
    env.fixed_array_fields = crate::body_tv::fixed_array_field_bindings(program, f);
    env.mutable_records = match crate::body_tv::mutable_record_frames(program, f) {
        Ok(records) => records,
        Err(reason) => {
            report.results.push(ExecResult {
                label: f.name.clone(),
                verdict: ExecVerdict::Skipped { reason },
            });
            return;
        }
    };
    for p in &f.params {
        if let Some((ty_str, is_slice)) = exec_type_spelling(&p.ty) {
            env.bind(&p.name, ty_str, is_slice);
        }
    }

    let mut let_no = 0usize;
    for stmt in &body.stmts {
        match stmt {
            Stmt::Let {
                name,
                ty: Some(ty),
                init,
                ..
            } => {
                let_no += 1;
                let label = format!("{}.let#{}", f.name, let_no);
                let body_owned_call = is_direct_body_state_call(program, init);
                if body_owned_call {
                    // The exact callee source result and any complete mutable
                    // post-state are interpreted by body TV. A pure exec-TV
                    // obligation cannot call an exec-mode `&mut` function from
                    // its `ensures` clause or reconstruct a logical record
                    // overlay in an isolated expression frame.
                } else if let Some((ret_ty, _is_slice)) = exec_type_spelling(ty) {
                    check_corpus_expr(
                        program,
                        init,
                        &label,
                        &ret_ty,
                        &env,
                        f,
                        &support_defs,
                        true,
                        seed,
                        rlimit,
                        report,
                    );
                } else {
                    report.results.push(ExecResult {
                        label,
                        verdict: ExecVerdict::Skipped {
                            reason: format!(
                                "the `let` type is outside the exec frame sublanguage \
                                 (not a bounded u8/u16/u32/u64/usize/bool/&[u32]) — \
                                 non-derivable exec ret type: {ty:?}"
                            ),
                        },
                    });
                }
                // Bind the local so a later expr referencing it frames.
                if let Some((ty_str, is_slice)) = exec_type_spelling(ty) {
                    if !body_owned_call {
                        env.bind_local_relation(name, &ty_str, init);
                    }
                    env.bind(name, ty_str, is_slice);
                }
            }
            Stmt::Let { name, ty: None, .. } => {
                // An untyped let: the ret type is not derivable from source, so a
                // Skip (never an inferred guess).
                let_no += 1;
                report.results.push(ExecResult {
                    label: format!("{}.let#{}", f.name, let_no),
                    verdict: ExecVerdict::Skipped {
                        reason: format!(
                            "untyped `let {name}` — the exec value type is not derivable \
                             from source (step-2.1 frames only typed lets)"
                        ),
                    },
                });
            }
            Stmt::Return(Some(e)) => {
                let label = format!("{}.return", f.name);
                if !is_direct_body_state_call(program, e) {
                    check_return_like(
                        program,
                        e,
                        &label,
                        &f.ret,
                        &env,
                        f,
                        &support_defs,
                        seed,
                        rlimit,
                        report,
                    );
                }
            }
            // A loop / if / assignment / break / continue / bare-expr statement is
            // out of scope for step 2.1 (statements/loops/mutation are step 2.2)
            // and skipped.
            Stmt::Loop(_) => report.results.push(ExecResult {
                label: format!("{}.loop", f.name),
                verdict: ExecVerdict::Skipped {
                    reason: "a LOOP statement — statements/loops/mutation are step 2.2 \
                             (kernel-gated), out of scope for exec-expr TV"
                        .to_string(),
                },
            }),
            Stmt::If { .. } => report.results.push(ExecResult {
                label: format!("{}.if", f.name),
                verdict: ExecVerdict::Skipped {
                    reason: "an IF statement — control flow is step 2.2, out of scope".to_string(),
                },
            }),
            Stmt::Assign { .. } => report.results.push(ExecResult {
                label: format!("{}.assign", f.name),
                verdict: ExecVerdict::Skipped {
                    reason: "an ASSIGNMENT statement — mutation is step 2.2, out of scope"
                        .to_string(),
                },
            }),
            Stmt::Expr(_) | Stmt::Return(None) | Stmt::Break | Stmt::Continue => {}
        }
    }

    // The body tail expr (the fn's value — `sum`'s final `acc`).
    if let Some(tail) = &body.tail {
        let label = format!("{}.tail", f.name);
        if !is_direct_body_state_call(program, tail) {
            check_return_like(
                program,
                tail,
                &label,
                &f.ret,
                &env,
                f,
                &support_defs,
                seed,
                rlimit,
                report,
            );
        }
    }
}

/// Check a tail / `return` expr at the fn return type, or Skip if the return type
/// is not exec-frame-spellable.
#[allow(
    clippy::too_many_arguments,
    reason = "a corpus expr's TV genuinely needs the expr + its label + the return \
        type + the env + the fn (for the req frame) + the verus config; grouping \
        them would obscure the per-expr data flow"
)]
fn check_return_like(
    program: &thermite_syntax::Program,
    e: &Expr,
    label: &str,
    ret: &Type,
    env: &ExecEnv,
    f: &FnItem,
    support_defs: &[String],
    seed: u64,
    rlimit: f64,
    report: &mut ExecTvReport,
) {
    match exec_type_spelling(ret) {
        Some((ret_ty, _)) => check_corpus_expr(
            program,
            e,
            label,
            &ret_ty,
            env,
            f,
            support_defs,
            true,
            seed,
            rlimit,
            report,
        ),
        None => report.results.push(ExecResult {
            label: label.to_string(),
            verdict: ExecVerdict::Skipped {
                reason: format!(
                    "the fn return type is outside the exec frame sublanguage — \
                     non-derivable exec ret type: {ret:?}"
                ),
            },
        }),
    }
}

/// Whether an expression contains control-flow value semantics owned by the
/// complete body-TV state transformer rather than the leaf exec-value encoder.
pub(crate) fn expr_contains_body_control(expr: &Expr) -> bool {
    let any = |values: &[Expr]| values.iter().any(expr_contains_body_control);
    match expr {
        Expr::Match { .. } | Expr::If { .. } => true,
        Expr::Array(values) | Expr::Tuple(values) => any(values),
        Expr::ArrayRepeat { value, .. }
        | Expr::Unary { expr: value, .. }
        | Expr::Cast { expr: value, .. }
        | Expr::Ref { expr: value, .. }
        | Expr::Deref(value)
        | Expr::TupleProj {
            receiver: value, ..
        }
        | Expr::Closure { body: value, .. } => expr_contains_body_control(value),
        Expr::Binary { lhs, rhs, .. } => {
            expr_contains_body_control(lhs) || expr_contains_body_control(rhs)
        }
        Expr::Call { callee, args } => expr_contains_body_control(callee) || any(args),
        Expr::MethodCall { receiver, args, .. } => {
            expr_contains_body_control(receiver) || any(args)
        }
        Expr::Field { receiver, .. }
        | Expr::Is {
            scrutinee: receiver,
            ..
        } => expr_contains_body_control(receiver),
        Expr::Index { base, index } => {
            expr_contains_body_control(base)
                || match index {
                    IndexArg::Single(index)
                    | IndexArg::RangeTo(index)
                    | IndexArg::RangeFrom(index) => expr_contains_body_control(index),
                    IndexArg::Range(start, end) => {
                        expr_contains_body_control(start) || expr_contains_body_control(end)
                    }
                }
        }
        Expr::StructLit { fields, .. } => fields
            .iter()
            .any(|(_, value)| expr_contains_body_control(value)),
        Expr::Quantifier { domain, body, .. } => {
            expr_contains_body_control(domain) || expr_contains_body_control(body)
        }
        Expr::IntLit { .. } | Expr::BoolLit(_) | Expr::StrLit(_) | Expr::Path(_) => false,
    }
}

/// TV one corpus exec expr at a derived `ret_ty` (REQ-5, best-effort). Checks the
/// expr is in the pure-exec subset, frames it from the env (referenced vars must all
/// be declared; else Skip), lowers it via `lower_exec_expr`, builds and (when the
/// frame is adequate) discharges the obligation, and classifies.
#[allow(
    clippy::too_many_arguments,
    reason = "see `check_return_like` — the genuine per-expr fan-in"
)]
fn check_corpus_expr(
    program: &thermite_syntax::Program,
    e: &Expr,
    label: &str,
    ret_ty: &str,
    env: &ExecEnv,
    f: &FnItem,
    support_defs: &[String],
    body_control_handoff: bool,
    seed: u64,
    rlimit: f64,
    report: &mut ExecTvReport,
) {
    // Match/if values are control-flow state transformers.  When this expression
    // came from an in-language body, the complete body-TV obligation owns it and
    // checks the production lowering against the independent state denotation.
    // Do not also force it through the deliberately leaf-only exec-value encoder;
    // strict verified builds require the corresponding body row to be faithful.
    // Export guards have no such body owner and therefore keep the exec-TV path.
    if body_control_handoff && expr_contains_body_control(e) {
        return;
    }
    // The expr's free vars must all be declared in the env (a derivable frame). An
    // undeclared free var (a local bound by an out-of-scope construct, a richer-typed
    // param) → Skip (non-derivable frame), not a guessed binding.
    let mut referenced = Vec::new();
    collect_free_paths(e, &mut referenced);
    for name in &referenced {
        if !env.declares(name) {
            report.results.push(ExecResult {
                label: label.to_string(),
                verdict: ExecVerdict::Skipped {
                    reason: format!(
                        "the expr references `{name}` whose exec type is not derivable \
                         (a richer-typed param / a local bound by an out-of-scope \
                         construct) — non-derivable frame"
                    ),
                },
            });
            return;
        }
    }

    // P_production — the exec lowering. A construct the exec lowering does not
    // cover (a method call, a spec-only form) → Skip (out of the pure-exec subset),
    // not a faithfulness verdict.
    let p_production = match thermite_lower::lower_exec_expr(e) {
        Ok(p) => p,
        Err(err) => {
            report.results.push(ExecResult {
                label: label.to_string(),
                verdict: ExecVerdict::Skipped {
                    reason: format!(
                        "production exec lowering does not cover this body expr \
                         (out of the pure-exec step-2.1 subset): {err}"
                    ),
                },
            });
            return;
        }
    };

    // Independently encode the function precondition. Raw Thermite contract text
    // is not a Verus specification once it contains a slice/fixed-array view or a
    // bounded-integer coercion, so reusing it here would let the production syntax
    // define its own TV frame. The parsed source expression is used only to derive
    // the parameters that the independently encoded predicate needs.
    let source_req = match crate::body_tv::reference_corpus_req(program, f) {
        Ok(Some(predicate)) => {
            let mut variables = Vec::new();
            collect_free_paths(&f.contract.req.expr, &mut variables);
            if variables.iter().all(|variable| env.declares(variable)) {
                Some((predicate, variables))
            } else {
                None
            }
        }
        Ok(None) => None,
        Err(reason) => {
            report.results.push(ExecResult {
                label: label.to_string(),
                verdict: ExecVerdict::Skipped { reason },
            });
            return;
        }
    };

    // The obligation params: every var the expr references, plus every var the
    // (included) `req` references — declared from the env at its exec type.
    let mut needed: Vec<String> = referenced.clone();
    if let Some((_, req_vars)) = &source_req {
        for v in req_vars {
            if !needed.contains(v) {
                needed.push(v.clone());
            }
        }
    }

    // A typed scalar local is not an unconstrained fresh parameter. Pull in its
    // independently encoded defining equation, then recursively pull in equations
    // for predecessor locals. This restores source facts such as
    // `changed == slot % CAPACITY` when proving a later fixed-array access or
    // bounded arithmetic expression.
    let mut active_relations = Vec::new();
    loop {
        let mut grew = false;
        for (index, relation) in env.local_relations.iter().enumerate() {
            if active_relations.contains(&index)
                || !needed.iter().any(|name| name == &relation.name)
            {
                continue;
            }
            active_relations.push(index);
            for dependency in &relation.dependencies {
                if !needed.contains(dependency) {
                    needed.push(dependency.clone());
                }
            }
            grew = true;
        }
        if !grew {
            break;
        }
    }

    let mut req_clauses = Vec::new();
    if let Some((predicate, _)) = source_req {
        req_clauses.push(predicate);
    }
    req_clauses.extend(
        active_relations
            .iter()
            .map(|index| env.local_relations[*index].predicate.clone()),
    );
    let req = (!req_clauses.is_empty()).then(|| {
        req_clauses
            .iter()
            .map(|clause| format!("({clause})"))
            .collect::<Vec<_>>()
            .join(" && ")
    });
    let mut spec_defs = support_defs.to_vec();
    if thermite_lower::expr_uses_fixed_array_equality(e) {
        if let Err(reason) =
            crate::body_tv::extend_fixed_array_equality_defs(program, &mut spec_defs)
        {
            report.results.push(ExecResult {
                label: label.to_string(),
                verdict: ExecVerdict::Skipped { reason },
            });
            return;
        }
    }
    if thermite_lower::expr_uses_u64_bit_methods(e) {
        spec_defs.push(thermite_lower::u64_bit_defs());
    }
    let frame = ExecObligationFrame {
        spec_defs,
        params: env
            .params
            .iter()
            .filter(|p| needed.iter().any(|r| r == &p.name))
            .cloned()
            .collect(),
        ret_type: ret_ty.to_string(),
        req,
        slice_params: env
            .slice_params
            .iter()
            .filter(|n| needed.iter().any(|r| r == *n))
            .cloned()
            .collect(),
        fixed_array_params: env
            .fixed_array_params
            .iter()
            .filter(|n| needed.iter().any(|r| r == *n))
            .cloned()
            .collect(),
        fixed_array_fields: env.fixed_array_fields.clone(),
        mutable_records: env
            .mutable_records
            .iter()
            .filter(|record| needed.iter().any(|name| name == &record.name))
            .cloned()
            .collect(),
        result_is_fixed_array: ret_ty.starts_with('[') && ret_ty.contains(';'),
        result_record: crate::body_tv::named_record_frames(program)
            .into_iter()
            .find(|record| record.type_name == ret_ty),
    };

    let program = match exec_equivalence_obligation(e, &p_production, &frame) {
        Ok(prog) => prog,
        Err(err) => {
            // The independent reference does not cover the expr → Skip (out of the
            // pure-exec subset; e.g. a method call / Vec-String accessor), not a
            // faithfulness verdict.
            report.results.push(ExecResult {
                label: label.to_string(),
                verdict: ExecVerdict::Skipped {
                    reason: format!("exec reference encoder does not cover this body expr: {err}"),
                },
            });
            return;
        }
    };

    let verdict = discharge(&program, label, seed, rlimit);
    report.results.push(ExecResult {
        label: label.to_string(),
        verdict,
    });
}

/// Collect the single-segment free-var path names an exec expr references (the
/// frame's referenced set). Multi-segment paths (`u32::MAX`) and bound names are not
/// collected as free vars.
fn collect_free_paths(e: &Expr, out: &mut Vec<String>) {
    match e {
        Expr::Path(segs) if segs.len() == 1 && !out.contains(&segs[0]) => {
            out.push(segs[0].clone());
        }
        Expr::Path(_) => {}
        Expr::Binary { lhs, rhs, .. } => {
            collect_free_paths(lhs, out);
            collect_free_paths(rhs, out);
        }
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } => collect_free_paths(expr, out),
        Expr::Index { base, index } => {
            collect_free_paths(base, out);
            if let IndexArg::Single(i) = index {
                collect_free_paths(i, out);
            }
        }
        Expr::Call { args, .. } => {
            for a in args {
                collect_free_paths(a, out);
            }
        }
        Expr::MethodCall { receiver, args, .. } => {
            collect_free_paths(receiver, out);
            for arg in args {
                collect_free_paths(arg, out);
            }
        }
        Expr::Field { receiver, .. } | Expr::TupleProj { receiver, .. } => {
            collect_free_paths(receiver, out)
        }
        Expr::Array(values) | Expr::Tuple(values) => {
            for value in values {
                collect_free_paths(value, out);
            }
        }
        Expr::ArrayRepeat { value, .. } | Expr::Ref { expr: value, .. } | Expr::Deref(value) => {
            collect_free_paths(value, out)
        }
        Expr::StructLit { fields, .. } => {
            for (_, value) in fields {
                collect_free_paths(value, out);
            }
        }
        Expr::If { cond, then, else_ } => {
            collect_free_paths(cond, out);
            if let Some(tail) = &then.tail {
                collect_free_paths(tail, out);
            }
            if let Some(tail) = &else_.tail {
                collect_free_paths(tail, out);
            }
        }
        _ => {}
    }
}

/// The exec value-type spelling for a body var/return type, plus whether it is a
/// slice (`&[u32]` → indexed element-wise). `None` for a type outside the exec frame
/// sublanguage (Map/Option/String/heap wrappers) — an expr over such a var is Skipped
/// (non-derivable frame).
fn exec_type_spelling(ty: &Type) -> Option<(String, bool)> {
    match ty {
        Type::Prim(PrimType::U8) => Some(("u8".to_string(), false)),
        Type::Prim(PrimType::U16) => Some(("u16".to_string(), false)),
        Type::Prim(PrimType::U32) => Some(("u32".to_string(), false)),
        Type::Prim(PrimType::U64) => Some(("u64".to_string(), false)),
        Type::Prim(PrimType::Usize) => Some(("usize".to_string(), false)),
        Type::Prim(PrimType::Bool) => Some(("bool".to_string(), false)),
        Type::Unit => Some(("()".to_string(), false)),
        Type::Array { elem, len } => {
            let (elem, _) = exec_type_spelling(elem)?;
            let len = match len {
                thermite_syntax::ArrayLen::Literal { value, .. } => value.to_string(),
                thermite_syntax::ArrayLen::Const(name) => name.clone(),
            };
            Some((format!("[{elem}; {len}]"), false))
        }
        Type::Ref { mutable, inner } => match inner.as_ref() {
            // `&[u32]` → the exec slice binding (indexed element-wise as `xs[i as
            // int]` in the reference, AC-5). Only a `u32` element slice is framed.
            Type::Slice(elem) => {
                exec_type_spelling(elem).map(|(spelling, _)| (format!("&[{spelling}]"), true))
            }
            // Keep named-record borrows as borrows. In particular, Verus only
            // permits `final(record)` in a postcondition when the obligation
            // parameter retains its exact `&mut Record` type.
            Type::Named(name) => Some((
                if *mutable {
                    format!("&mut {name}")
                } else {
                    format!("&{name}")
                },
                false,
            )),
            // A `&u64`/`&usize` borrow frames as the inner scalar.
            other => exec_type_spelling(other),
        },
        Type::Slice(elem) => {
            exec_type_spelling(elem).map(|(spelling, _)| (format!("&[{spelling}]"), true))
        }
        Type::Tuple(types) => Some((
            format!(
                "({})",
                types
                    .iter()
                    .map(|ty| exec_type_spelling(ty).map(|(spelling, _)| spelling))
                    .collect::<Option<Vec<_>>>()?
                    .join(", ")
            ),
            false,
        )),
        Type::Named(name) => Some((name.clone(), false)),
        _ => None,
    }
}

// ---- verus discharge (mirrors contract_tv::discharge) -----------------------

/// Discharge one exec obligation program through `verus`, classifying the verdict
/// (REQ-5 — verified ⟺ Faithful; a counterexample (errors, no rlimit signal) /
/// compile-parse abort ⟺ Divergent; a Verus/Z3 rlimit timeout / verus-absent /
/// inadequate-frame non-discharge ⟺ Unverifiable). An `errors >= 1` run carrying a
/// rlimit signal degrades to Unverifiable ahead of the Divergent arm (the #189-class
/// gate via the shared `crate::tv_signal::is_rlimit_signal`; #192), so a solver-budget
/// timeout is not mapped to an exec-lowering infidelity (R-HONEST-3 / R-CODE-4).
/// Runs in a per-run scratch dir removed wholesale on every exit path (blocker #53,
/// reusing `crate::check::ScratchDir`).
fn discharge(program: &str, label: &str, seed: u64, rlimit: f64) -> ExecVerdict {
    let stem = sanitize_stem(label);
    let scratch = ScratchDir {
        path: unique_scratch_dir(&stem),
    };
    if std::fs::create_dir_all(&scratch.path).is_err() {
        return ExecVerdict::Unverifiable {
            reason: "could not create the scratch dir for the verus discharge".to_string(),
        };
    }
    let file = scratch.path.join(format!("{stem}.rs"));
    if std::fs::write(&file, program).is_err() {
        return ExecVerdict::Unverifiable {
            reason: "could not write the obligation program to the scratch dir".to_string(),
        };
    }

    // The pinned `--rlimit` + `smt.random_seed` keep the discharge deterministic
    // (R-CODE-5), matching `forge check`'s / `forge tv`'s verus config.
    let output = Command::new("verus")
        .arg("--rlimit")
        .arg(format!("{rlimit}"))
        .arg("--smt-option")
        .arg(format!("smt.random_seed={seed}"))
        .arg(&file)
        .current_dir(&scratch.path)
        .output();

    let output = match output {
        Ok(o) => o,
        Err(_) => {
            // verus absent / spawn failure → Unverifiable (surfaced, R-CODE-4). Not
            // Divergent (that is reserved for an infidelity that reached verus).
            return ExecVerdict::Unverifiable {
                reason: "verus could not be spawned (absent on PATH or spawn failure)".to_string(),
            };
        }
    };
    let mut combined = String::from_utf8_lossy(&output.stdout).to_string();
    combined.push_str(&String::from_utf8_lossy(&output.stderr));

    // A Verus/Z3 resource-limit (rlimit) exhaustion / timeout: verus prints an rlimit
    // diagnostic (`rlimit exceeded` / `Resource limit (rlimit) exceeded` / z3's own
    // `max. resource limit exceeded`) and a results line counting the exhausted
    // obligation as an error. That is a discharge failure (the solver ran out of
    // budget), not a value mismatch: the #189-class hardening (the #192 root-cause fix:
    // the shared `crate::tv_signal::is_rlimit_signal` discriminator, mirroring body_tv /
    // contract_tv). An rlimit-hit error run is routed to Unverifiable, not the
    // `errors >= 1` Divergent arm, so a solver-budget timeout is not mapped to
    // an exec-lowering infidelity (R-HONEST-3 / R-CODE-4: a timeout degrades and is
    // reported).
    let rlimit_hit = crate::tv_signal::is_rlimit_signal(&combined);

    match parse_results(&combined) {
        // A clean verification (a results line, 0 errors, exit success) ⟺ Faithful.
        Some((verified, errors)) if errors == 0 && verified >= 1 && output.status.success() => {
            ExecVerdict::Faithful
        }
        // An error run that is an rlimit exhaustion → Unverifiable, not Divergent
        // (the #189-class mapping fix; this arm precedes the Divergent arm, mirroring
        // body_tv::run_obligation / contract_tv::discharge).
        Some((_verified, errors)) if errors >= 1 && rlimit_hit => {
            let _ = errors;
            ExecVerdict::Unverifiable {
                reason: format!(
                    "verus exhausted its SMT resource budget (rlimit) on `{label}` before \
                     proving the obligation — a Verus/Z3 timeout, not a counterexample \
                     (routed to Unverifiable, never Divergent)"
                ),
            }
        }
        // A results line with errors (no rlimit signal) ⟺ a postcondition
        // counterexample: the production exec value differs from the bounded
        // reference, an infidelity.
        Some((_verified, errors)) if errors >= 1 => ExecVerdict::Divergent {
            detail: format!(
                "verus found {errors} error(s) on the exec equivalence obligation — \
                 the production exec lowering of `{label}` computes a value that differs \
                 from the independent bounded reference (a postcondition counterexample: \
                 the off-corpus exec-lowering infidelity finding)"
            ),
        },
        // No parseable results line. If verus exited non-success, the production text
        // failed to compile/parse (the #122 `E0308` / #146 `expected ','` catch shapes
        // abort before verification) → an infidelity (Divergent). If verus exited
        // success with no results line, the obligation did not discharge →
        // Unverifiable, not Faithful.
        _ => {
            if !output.status.success() {
                let diagnostic = combined.lines().take(12).collect::<Vec<_>>().join(" | ");
                ExecVerdict::Divergent {
                    detail: format!(
                        "verus ABORTED (compile/parse) on the exec obligation for `{label}` \
                         — the production exec text did not compile/parse (the #122 `E0308` / \
                         #146 cast-`<` mis-parse catch shapes): a real exec-lowering infidelity; \
                         tool diagnostic: {diagnostic}"
                    ),
                }
            } else {
                ExecVerdict::Unverifiable {
                    reason: format!(
                        "verus produced no parseable results line for `{label}` (the obligation \
                         did not discharge — likely an INADEQUATE overflow/index frame for the \
                         body expr, not derivable from the source `req`; reported distinctly, \
                         never as Faithful)"
                    ),
                }
            }
        }
    }
}

/// Parse the `N verified, M errors` summary line from verus output (mirrors
/// `contract_tv`'s parser and the negative test). `None` if no summary line is present.
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

/// A crate-name-safe scratch stem from an expr label (no `.`/`#` — verus rejects a
/// `.` in the derived crate name; mirrors `contract_tv::sanitize_stem`).
fn sanitize_stem(label: &str) -> String {
    let mut s = String::with_capacity(label.len() + 7);
    for ch in label.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            s.push(ch);
        } else {
            s.push('_');
        }
    }
    if s.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(true) {
        s.insert(0, 'c');
    }
    s.push_str("_exectv");
    s
}

/// Render an [`ExecTvReport`] as a human summary (REQ-5; `forge exec-tv` text
/// output). One line per expr plus the headline counts (the reported integers, the
/// four-way classification surfaced distinctly).
pub fn render_report(report: &ExecTvReport, header: &str) -> String {
    let mut out = String::new();
    let counts = report.counts();
    out.push_str(&format!(
        "{header}: {} expr(s) checked, {} faithful, {} DIVERGENT, {} unverifiable, {} skipped\n",
        counts.checked(),
        counts.faithful,
        counts.divergent,
        counts.unverifiable,
        counts.skipped,
    ));
    for r in &report.results {
        match &r.verdict {
            ExecVerdict::Faithful => out.push_str(&format!("  {} — faithful\n", r.label)),
            ExecVerdict::Divergent { detail } => {
                out.push_str(&format!("  {} — DIVERGENT: {detail}\n", r.label))
            }
            ExecVerdict::Unverifiable { reason } => {
                out.push_str(&format!("  {} — unverifiable ({reason})\n", r.label))
            }
            ExecVerdict::Skipped { reason } => {
                out.push_str(&format!("  {} — skipped ({reason})\n", r.label))
            }
        }
    }
    out
}

/// Render the off-corpus construct coverage line (REQ-3 / AC-7 — the #122/#146
/// regression-guard surface, reported so the guard is non-vacuous).
pub fn render_coverage(cov: &ExecConstructCoverage) -> String {
    format!(
        "  construct coverage: cast-`<`={}, arith={}, casts={}, index={}, shifts={}, bitops={}\n",
        cov.cast_lt, cov.arith, cov.casts, cov.index, cov.shifts, cov.bitops
    )
}

/// The pinned default seed + rlimit for `forge exec-tv` (mirrors `forge check` /
/// `forge tv` — the deterministic config, §5.3 / R-CODE-5).
pub const EXEC_TV_DEFAULT_SEED: u64 = DEFAULT_SOLVER_SEED;
pub const EXEC_TV_DEFAULT_RLIMIT: f64 = DEFAULT_RLIMIT;
/// The default generated-exec-expr count for `forge exec-tv --generated` (REQ-3 /
/// AC-7).
pub const EXEC_TV_GENERATED_DEFAULT_N: usize = 200;

// ---- forge-level Divergent regression tests (REQ-5; blocker #157) ----------
//
// The obligation-layer tests (`thermite-tv/tests/exec_teeth.rs` E1-E4) prove a
// wrong `P_production` -> a verus error. They do not exercise the forge-level
// step that maps that verus error to `ExecVerdict::Divergent`: `discharge`'s
// four-way classification. Over the generated/corpus space the faithful lowerer
// never produces a Divergent, so the Divergent arm had no direct test coverage.
//
// This module tests the forge classification end to end. It builds a
// real exec obligation with a wrong production (the same E1/E3 infidelity shapes
// the obligation layer pins), discharges it through the actual `discharge` fn, and
// asserts the verdict. It covers both Divergent triggers (postcondition-
// counterexample and non-compile) plus the positive control (faithful -> Faithful)
// and the degenerate boundary (no obligation -> Unverifiable, not Divergent -- the
// masking path the four-way classification must keep distinct).
//
// Test-only: no production-logic change. `discharge` is a private sibling fn,
// reachable here via `super::` (a child mod sees the parent's private items), so no
// visibility tweak is needed either. The tests run a wrong production through a
// verus error -> the real `discharge` mapping, not a mocked verdict. Mirrors
// `thermite-tv/tests/exec_teeth.rs`'s verus gate -- `discharge` spawns a
// bare `verus`, so the test gates on the same PATH-resolvable binary and reports a
// skip when it is absent.
#[cfg(test)]
mod divergent_teeth {
    use super::*;
    use thermite_syntax::ast::BinOp;

    /// `true` iff a bare `verus` is spawnable (the same resolution `discharge`
    /// uses -- `Command::new("verus")`, i.e. PATH). Skip otherwise so the
    /// tests do not pass when the discharge cannot reach a solver.
    fn verus_on_path() -> bool {
        Command::new("verus").arg("--version").output().is_ok()
    }

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

    // Pinned deterministic discharge config (mirrors `forge exec-tv`'s defaults).
    const SEED: u64 = EXEC_TV_DEFAULT_SEED;
    const RLIMIT: f64 = EXEC_TV_DEFAULT_RLIMIT;

    /// The E3 source `a + b` with the no-overflow frame `a + b <= 0xFFFF` (the
    /// faithful checked add is total, so a counterexample on a wrong production is
    /// a value difference). Reused for both the positive control and the
    /// postcondition-counterexample Divergent trigger.
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
            req: Some("a + b <= 0xFFFF".to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn named_record_borrows_keep_their_exact_exec_frame_type() {
        let named = Type::Named("State".to_string());
        assert_eq!(
            exec_type_spelling(&Type::Ref {
                mutable: false,
                inner: Box::new(named.clone()),
            }),
            Some(("&State".to_string(), false))
        );
        assert_eq!(
            exec_type_spelling(&Type::Ref {
                mutable: true,
                inner: Box::new(named),
            }),
            Some(("&mut State".to_string(), false))
        );
    }

    #[test]
    fn direct_typed_mutable_call_is_owned_by_body_tv_not_pure_exec_tv() {
        let parsed = thermite_syntax::parse(
            r#"
struct State { value: u64 }
fn set(state: &mut State, value: u64) -> u64
  req true
  ens result == value
  fx pure
{
  state.value = value;
  value
}
fn caller(state: &mut State, value: u64) -> u64
  req true
  ens result == value
  fx pure
{
  let observed: u64 = set(state, value);
  observed
}
"#,
        );
        assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
        let init = parsed
            .program
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fn(function) if function.name == "caller" => function
                    .body
                    .as_ref()
                    .and_then(|body| body.stmts.first())
                    .and_then(|statement| match statement {
                        Stmt::Let { init, .. } => Some(init),
                        _ => None,
                    }),
                _ => None,
            })
            .expect("caller let initializer");
        assert!(is_direct_mutable_call(&parsed.program, init));

        let nested = Expr::Binary {
            op: BinOp::Add,
            lhs: Box::new(init.clone()),
            rhs: Box::new(int(1)),
        };
        assert!(
            !is_direct_mutable_call(&parsed.program, &nested),
            "nested effectful expressions must not be removed from pure exec TV as if they were the admitted direct initializer"
        );
    }

    #[test]
    fn direct_finite_record_value_call_is_owned_by_body_tv_not_frameless_exec_tv() {
        let parsed = thermite_syntax::parse(
            r#"
struct State { value: u64 }
fn observe(state: State) -> u64
  req true
  ens result == state.value
  fx pure
{
  state.value
}
fn caller(state: State) -> u64
  req true
  ens result == state.value
  fx pure
{
  let observed: u64 = observe(state);
  observed
}
"#,
        );
        assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
        let init = parsed
            .program
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fn(function) if function.name == "caller" => function
                    .body
                    .as_ref()
                    .and_then(|body| body.stmts.first())
                    .and_then(|statement| match statement {
                        Stmt::Let { init, .. } => Some(init),
                        _ => None,
                    }),
                _ => None,
            })
            .expect("caller let initializer");
        assert!(is_direct_record_value_call(&parsed.program, init));
        assert!(is_direct_body_state_call(&parsed.program, init));

        let nested = Expr::Binary {
            op: BinOp::Add,
            lhs: Box::new(init.clone()),
            rhs: Box::new(int(1)),
        };
        assert!(
            !is_direct_record_value_call(&parsed.program, &nested),
            "nested aggregate calls must remain visible to the fail-closed expression/body paths"
        );
    }

    /// Positive control: a faithful production (`a + b`, the lowering of the source)
    /// -> the forge classification is `ExecVerdict::Faithful`. Without this, a
    /// `discharge` that returned Divergent unconditionally would pass the Divergent
    /// assertions vacuously.
    #[test]
    fn faithful_production_classifies_faithful() {
        if !verus_on_path() {
            eprintln!(
                "SKIP: verus not on PATH -- the forge-level Faithful control not discharged."
            );
            return;
        }
        let prog = exec_equivalence_obligation(&e3_source(), "a + b", &e3_frame())
            .expect("faithful exec obligation builds");
        let verdict = discharge(&prog, "teeth.faithful", SEED, RLIMIT);
        assert_eq!(
            verdict,
            ExecVerdict::Faithful,
            "a FAITHFUL production exec lowering must classify Faithful (a forge-level \
             false positive otherwise)"
        );
    }

    /// Divergent trigger #1 (postcondition counterexample): a production that
    /// typechecks but computes the wrong value (`a.wrapping_sub(b)` for source
    /// `a + b`) -> verus finds a counterexample on `ensures result == (a + b)` ->
    /// `discharge` maps `errors >= 1` to `ExecVerdict::Divergent`. This is the arm
    /// `Some((_v, errors)) if errors >= 1` of the four-way classification.
    #[test]
    fn wrong_value_production_classifies_divergent() {
        if !verus_on_path() {
            eprintln!(
                "SKIP: verus not on PATH -- the forge-level Divergent (counterexample) \
                 teeth not discharged."
            );
            return;
        }
        let prog = exec_equivalence_obligation(&e3_source(), "a.wrapping_sub(b)", &e3_frame())
            .expect("wrong-value exec obligation builds");
        let verdict = discharge(&prog, "teeth.counterexample", SEED, RLIMIT);
        assert!(
            matches!(verdict, ExecVerdict::Divergent { .. }),
            "a WRONG-VALUE production (a.wrapping_sub(b) for a + b) must classify \
             Divergent via a postcondition counterexample; got {verdict:?}"
        );
    }

    /// Divergent trigger #2 (non-compile): the #122 paren-drop production
    /// `n - 1 as u8` (= `n - (1 as u8)`, a `u64 - u8` type mix) for source
    /// `(n - 1) as u8` -> verus aborts with `E0308`/`mismatched types` before
    /// verification (no parseable results line, non-success exit) -> `discharge`
    /// maps the `!status.success()` no-results branch to `ExecVerdict::Divergent`.
    /// This is the `_ => if !output.status.success()` arm of the classification.
    #[test]
    fn non_compiling_production_classifies_divergent() {
        if !verus_on_path() {
            eprintln!(
                "SKIP: verus not on PATH -- the forge-level Divergent (non-compile) \
                 teeth not discharged."
            );
            return;
        }
        let source = Expr::Cast {
            expr: Box::new(bin(BinOp::Sub, path("n"), int(1))),
            ty: Type::Prim(PrimType::U8),
        };
        let frame = ExecObligationFrame {
            params: vec![ExecParamDecl::new("n", "u64")],
            ret_type: "u8".to_string(),
            req: Some("n >= 1, n - 1 <= 255".to_string()),
            ..Default::default()
        };
        // The #122 paren-drop: `n - 1 as u8` parses as `n - (1 as u8)`, a u64 - u8
        // mix -> an E0308 type error that aborts verus before verification.
        let prog = exec_equivalence_obligation(&source, "n - 1 as u8", &frame)
            .expect("non-compiling exec obligation builds");
        let verdict = discharge(&prog, "teeth.noncompile", SEED, RLIMIT);
        match verdict {
            ExecVerdict::Divergent { detail } => {
                assert!(
                    detail.contains("tool diagnostic:")
                        && (detail.contains("E0308") || detail.contains("mismatched types")),
                    "a compile-abort verdict must preserve an actionable Verus diagnostic: \
                     {detail}"
                );
            }
            other => panic!(
                "a NON-COMPILING production (the #122 paren-drop -> E0308) must classify \
                 Divergent via the compile/parse-abort branch; got {other:?}"
            ),
        }
    }

    /// The Divergent-vs-Unverifiable boundary (the critic's masking-path concern):
    /// a degenerate program with zero exec obligations verifies as `0 verified,
    /// 0 errors` (verus succeeds, but no obligation reached a faithfulness verdict)
    /// -> `discharge` maps it to `ExecVerdict::Unverifiable`, not `Divergent` and
    /// not `Faithful`. This pins the `_ => if status.success()` arm so the
    /// non-infidelity no-discharge case stays distinct from a Divergent
    /// (errors >= 1) / a Faithful (verified >= 1).
    #[test]
    fn degenerate_no_obligation_classifies_unverifiable() {
        if !verus_on_path() {
            eprintln!(
                "SKIP: verus not on PATH -- the forge-level Unverifiable boundary not \
                 discharged."
            );
            return;
        }
        // A well-formed verus program with no proof/exec obligation: verus reports
        // `0 verified, 0 errors` and exits success -- neither verified >= 1
        // (Faithful) nor errors >= 1 (Divergent), so the four-way classification
        // reports Unverifiable (distinct).
        let degenerate = "use vstd::prelude::*;\nverus! {\n}\nfn main() {}\n";
        let verdict = discharge(degenerate, "teeth.degenerate", SEED, RLIMIT);
        assert!(
            matches!(verdict, ExecVerdict::Unverifiable { .. }),
            "a degenerate zero-obligation program must classify Unverifiable (the \
             Divergent-vs-Unverifiable boundary), never Divergent/Faithful; got {verdict:?}"
        );
    }

    /// The #192/#189-class gate (the missing-gate fix): `discharge` adds an
    /// `errors >= 1 && rlimit_hit -> Unverifiable` arm ahead of the Divergent arm, so
    /// a Verus/Z3 solver-budget timeout (an error run carrying a rlimit signal) is
    /// not mapped to an exec-lowering infidelity. exec_tv previously had no
    /// rlimit gate at all (every `errors >= 1` run -> Divergent unconditionally, the
    /// same #189 class body_tv/contract_tv fixed); #192 centralizes the discriminator
    /// in `crate::tv_signal::is_rlimit_signal` and consumes it here.
    ///
    /// Hand-derived (R-CHAR-3): a Z3 rlimit exhaustion is not deterministically
    /// forcible, so this pins the discriminator that drives the gate (the same seam
    /// body_tv / contract_tv pin). The full phrase set is detected (verus's two
    /// phrasings + z3's own `max. resource limit exceeded`); a `postcondition not
    /// satisfied` counterexample is not (it stays in the Divergent class). The
    /// `discharge` source routes `errors >= 1 && rlimit_hit` to Unverifiable ahead of
    /// the `errors >= 1` Divergent arm, so the discriminator firing is the gate
    /// firing.
    #[test]
    fn rlimit_signal_is_detected_counterexample_is_not() {
        use crate::tv_signal::is_rlimit_signal;
        assert!(
            is_rlimit_signal("error: Resource limit (rlimit) exceeded\n0 verified, 1 errors"),
            "a `Resource limit (rlimit) exceeded` output MUST be detected as a timeout \
             signal (routed to Unverifiable, never Divergent — the #192 exec_tv gate)"
        );
        assert!(
            is_rlimit_signal("error: rlimit exceeded; consider raising the budget"),
            "a bare `rlimit exceeded` output MUST be detected as a timeout signal"
        );
        // The distributed z3 binary's own resourceout literal (#192 — the shared
        // discriminator): `resource limit exceeded` with no `rlimit` token.
        assert!(
            is_rlimit_signal("unknown: max. resource limit exceeded\n0 verified, 1 errors"),
            "z3's own `max. resource limit exceeded` resourceout literal MUST be detected"
        );
        assert!(
            !is_rlimit_signal(
                "error: postcondition not satisfied\n --> x.rs:5:12\n0 verified, 1 errors"
            ),
            "a genuine `postcondition not satisfied` counterexample MUST NOT be detected as \
             a timeout (it stays in the Divergent class — the exec gate must not over-fire)"
        );
    }
}
