//! `forge/src/exec_tv.rs` — the EXEC-POSITION (BODY) TRANSLATION-VALIDATION check
//! phase (`.design/verified/exec-tv.md` REQ-5; epic crosslink #151, blockers #154 +
//! #156).
//!
//! Step-1 contract-TV (`forge/src/contract_tv.rs`) certifies that the emitted Verus
//! CONTRACT (`req`/`ens`/`inv`/`dec`) MEANS the same as the source contract. It does
//! NOT cover the EXEC BODY — where the #122 (`(n - 1) as nat` paren) and #146
//! (`x as u32 < 33` cast-`<` mis-parse) infidelity classes GENERALLY live. This
//! phase closes that gap for PURE EXEC-POSITION EXPRESSIONS (step 2.1): for each
//! such expr it computes
//!
//!   `P_production = thermite_lower::lower_exec_expr(expr)`             (the artifact under test)
//!   `reference    = thermite_tv::exec_ref_value(expr, …)`             (the INDEPENDENT bounded reference)
//!
//! wraps them as the EXEC-FN obligation `fn tv_exec_wrap(..) ensures result ==
//! <reference> { <P_production> }` (`thermite_tv::exec_equivalence_obligation`), and
//! discharges it through `verus`. VERIFIED ⟺ the exec lowering of that expr is
//! FAITHFUL (it computes the bounded reference VALUE for all inputs); a
//! `postcondition not satisfied` / type / parse error ⟺ a real exec-lowering
//! INFIDELITY (the off-corpus #122/#146/overflow/off-by-one classes). It is exposed
//! as `forge exec-tv <file>` — a SEPARATE opt-in deeper audit (like `forge tv`), NOT
//! folded into `forge check`.
//!
//! `thermite-tv` stays INDEPENDENT of `thermite-lower` (the N-version boundary,
//! AC-6): this forge module is the ONLY place the two exec encoders meet.
//!
//! ## Two runs (both surfaced; the GENERATED run is primary)
//!
//! - **The generated run ([`run_generated`], PRIMARY):** over
//!   `thermite_tv::gen_exec_exprs` — the off-corpus #122/#146 regression guard
//!   (REQ-3). Each generated `ExecClause` carries an ADEQUATE FRAME (every base
//!   scalar `<= 1000`, an index `< xs.len()`), so the FAITHFUL lowerer makes ALL
//!   `Faithful`; ANY `Divergent`/`Unverifiable` is a REAL off-corpus exec-lowering
//!   infidelity (surfaced loudly + a blocker). Reports the construct coverage
//!   (cast-`<` / arith / cast / index).
//! - **The corpus body-expr check ([`exec_tv_file`], BEST-EFFORT):** over the pure
//!   exec EXPRESSIONS of each corpus fn whose var-frame is DERIVABLE (a `let`-RHS, a
//!   tail/`return` expr — the in-scope vars + their exec types known from the
//!   surrounding params/lets). Statements / loops / mutation are OUT OF SCOPE (step
//!   2.2) and SKIPPED honestly (loudly, never silent-pass). It is ACCEPTABLE that
//!   corpus coverage is partial — the var-frame derivation is hard for arbitrary
//!   bodies (an arithmetic expr's adequate overflow frame is not always derivable
//!   from the source `req`/`inv` text); the generated run is the primary value.
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-5 (forge plug-in point) | SHIPPED | `pub fn run_generated` (the off-corpus exec run — PRIMARY) + `pub fn exec_tv_file` (the corpus body-expr check — best-effort) here; both compute `P_production` via `thermite_lower::lower_exec_expr`, build the obligation via `thermite_tv::exec_equivalence_obligation`, and discharge it through `verus` (the `discharge` helper, reusing `crate::check::ScratchDir`/#53 cleanup). Non-test consumer: `cli::run_exec_tv` (the `forge exec-tv <file>` subcommand). The four-way classification (Faithful / Divergent / Unverifiable / Skipped) is REPORTED DISTINCTLY (never masking infidelity, R-HONEST-3). **#192 (ref #189):** `discharge` now gates an `errors >= 1` rlimit-hit run to Unverifiable AHEAD of the Divergent arm — via the SHARED `crate::tv_signal::is_rlimit_signal` discriminator (the prior copy-drift root cause: exec_tv had NO rlimit gate, mapping every error run to Divergent unconditionally) — so a Verus/Z3 solver-budget timeout is never fabricated into an exec infidelity. Verified by `forge/tests/exec_tv_conformance.rs` (the 200-clause generated all-faithful + the corpus honest coverage) under real verus + the `divergent_teeth` rlimit gate. This closes the `lower_exec_expr` consumer loop (R-DEFER-1). |

use std::path::Path;
use std::process::Command;

use thermite_syntax::ast::{Expr, FnItem, IndexArg, Item, PrimType, Stmt, Type};

use thermite_tv::gen_exec_exprs;
use thermite_tv::obligation::{exec_equivalence_obligation, ExecObligationFrame, ExecParamDecl};

use crate::check::{unique_scratch_dir, ScratchDir, DEFAULT_RLIMIT, DEFAULT_SOLVER_SEED};
use crate::cli::ForgeError;

/// One exec expr's TV verdict (REQ-5; the four-way classification, reported
/// DISTINCTLY so Unverifiable / Skipped never masks an infidelity). `Faithful` ⟺
/// the obligation VERIFIED (the exec lowering of this expr means the bounded
/// reference VALUE); `Divergent` ⟺ verus found a counterexample / the production
/// text did not compile/parse (a REAL exec-lowering infidelity — the payoff,
/// surfaced loudly); `Unverifiable` ⟺ the obligation did NOT discharge for a
/// non-infidelity reason (verus absent, or an inadequate frame so verus could not
/// prove the postcondition without a wrap/overflow bound) — reported DISTINCTLY,
/// NEVER as Faithful; `Skipped` ⟺ the expr / statement is out of the pure-exec
/// step-2.1 subset (a statement, a loop, a non-derivable frame, an
/// `exec_ref_value`/`lower_exec_expr` Unsupported) — honest, never masking
/// infidelity.
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
/// divergent over the generated run is the off-corpus faithfulness AC; any
/// divergence is a real exec-lowering finding.
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
/// AC-7 — the #122/#146 regression-guard surface; reported so the guard is provably
/// non-vacuous). A clause contributes to MULTIPLE buckets (an indexed expr also
/// casts).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ExecConstructCoverage {
    pub arith: usize,
    pub casts: usize,
    /// A `Cast` LEFT operand of a `<`-leading op (`<`/`<=`/`<<`) — the #146 class.
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

// ---- the generated run (PRIMARY) -------------------------------------------

/// Run the OFF-CORPUS generated exec-TV run (REQ-3 / REQ-5; the PRIMARY value, the
/// #122/#146 off-corpus regression guard). Generates `n` well-framed exec exprs
/// deterministically from `seed` (`thermite_tv::gen_exec_exprs`), lowers each via
/// `thermite_lower::lower_exec_expr`, builds + discharges the exec-fn obligation
/// against the carried ADEQUATE frame, and reports. The lowerer is faithful + the
/// frames are adequate, so ALL should VERIFY; ANY `Divergent`/`Unverifiable` is a
/// real off-corpus exec-lowering infidelity / framing hole (surfaced loudly).
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
        // P_production — the REAL exec lowering of the generated expr (the artifact
        // under test, the eventual non-test consumer of `lower_exec_expr`).
        let p_production = match thermite_lower::lower_exec_expr(&clause.expr) {
            Ok(p) => p,
            Err(e) => {
                // A generated expr the EXEC lowering does not compile is a real
                // infidelity (the generator only emits the in-scope exec subset) —
                // Divergent, NOT Skipped (a non-compiling production is exactly the
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
                // The independent reference does not cover the generated expr —
                // honest Skipped (the generator stays within the encoder's subset,
                // so this is not expected; reported, never a false faithful).
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
    }
}

// ---- the corpus body-expr check (BEST-EFFORT) ------------------------------

/// Run the corpus body-expr exec-TV check over a `.th` file (REQ-5; BEST-EFFORT,
/// honest coverage). For each `fn` item, extract the pure exec EXPRESSIONS whose
/// var-frame is DERIVABLE (the RHS of a typed `let`, the body tail / a `return`
/// expr) and TV-check each. Statements / loops / mutation are OUT OF SCOPE (step
/// 2.2) and SKIPPED honestly. It is ACCEPTABLE that coverage is partial (an
/// arithmetic expr's adequate overflow frame is not always derivable from the
/// source `req`/`inv` text — such an expr is honestly Unverifiable, NEVER a false
/// Faithful).
pub fn exec_tv_file(path: &Path, seed: u64, rlimit: f64) -> Result<ExecTvReport, ForgeError> {
    let src = std::fs::read_to_string(path).map_err(|e| ForgeError::Io {
        path: path.display().to_string(),
        source: e,
    })?;
    let parsed = thermite_syntax::parse(&src);
    if !parsed.is_clean() {
        return Err(ForgeError::Parse(parsed.errors));
    }

    let mut report = ExecTvReport::default();
    for item in &parsed.program.items {
        match item {
            Item::Fn(f) => exec_tv_fn(f, seed, rlimit, &mut report),
            // A `spec fn` body lowers in SPEC context (not exec) — out of scope for
            // exec-TV; a struct/enum has no exec body.
            Item::SpecFn(_) | Item::Struct(_) | Item::Enum(_) => {}
        }
    }
    Ok(report)
}

/// The exec-type environment for a corpus fn body: each in-scope var's
/// [`ExecParamDecl`] (its exec value-type spelling) + the slice-bound name set. Built
/// from the params and extended by each typed `let` as the walk descends.
#[derive(Debug, Clone, Default)]
struct ExecEnv {
    params: Vec<ExecParamDecl>,
    slice_params: Vec<String>,
}

impl ExecEnv {
    fn bind(&mut self, name: &str, ty_str: String, is_slice: bool) {
        // A re-`let` of the same name keeps the first binding (v0.1 corpus locals are
        // not re-typed); dedup so the obligation signature is well-formed.
        if self.params.iter().any(|p| p.name == name) {
            return;
        }
        self.params
            .push(ExecParamDecl::new(name.to_string(), ty_str));
        if is_slice {
            self.slice_params.push(name.to_string());
        }
    }

    /// The names this env declares (the obligation can reference exactly these).
    fn declares(&self, name: &str) -> bool {
        self.params.iter().any(|p| p.name == name)
    }
}

/// TV the derivable pure exec exprs of one fn body (REQ-5, best-effort). Builds the
/// param env from the signature, then walks the TOP-LEVEL block: a typed `let x: T =
/// rhs` checks `rhs` at ret_type `T` and binds `x`; the body tail / a top-level
/// `return e` checks `e` at the fn return type. Nested-block statements (loop / if
/// bodies) + assignments + untyped lets are SKIPPED honestly (loudly).
fn exec_tv_fn(f: &FnItem, seed: u64, rlimit: f64, report: &mut ExecTvReport) {
    let Some(body) = &f.body else {
        return; // a boundary fn has no in-language body.
    };

    // #193/#195 OPEN-HOLE gate (`.design/forge/goal-repl.md` REQ-5; the four-way's
    // out-of-subset class): a fn carrying ANY open body hole (`?N`) is INCOMPLETE —
    // a hole is recorded on `FnItem.holes`, NOT in the `Stmt` stream, so checking the
    // body's exprs here would lower a hole-stripped body and fabricate `Faithful` for
    // the tail expr of an unfinished body. An incomplete body is not checkable, so it
    // is Skipped HONESTLY with the OpenHole reason (NEVER Faithful — R-HONEST-3) BEFORE
    // any expr lowers, mirroring `check`'s `OpenHole` reject (the SHARED
    // `goal_repl::open_hole_reason`, the #192 single-copy lesson).
    if let Some(reason) = crate::goal_repl::open_hole_reason(f) {
        report.results.push(ExecResult {
            label: f.name.clone(),
            verdict: ExecVerdict::Skipped { reason },
        });
        return;
    }

    // The signature env: each param at its exec value type. A param of a type the
    // exec frame cannot spell (Map/Option/struct/…) is recorded as un-spellable;
    // an expr that references it is then Skipped (non-derivable frame).
    let mut env = ExecEnv::default();
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
                if let Some((ret_ty, _is_slice)) = exec_type_spelling(ty) {
                    check_corpus_expr(init, &label, &ret_ty, &env, f, seed, rlimit, report);
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
                    env.bind(name, ty_str, is_slice);
                }
            }
            Stmt::Let { name, ty: None, .. } => {
                // An UNTYPED let — the ret type is not derivable from source; honest
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
                check_return_like(e, &label, &f.ret, &env, f, seed, rlimit, report);
            }
            // A loop / if / assignment / break / continue / bare-expr statement is
            // OUT OF SCOPE for step 2.1 (statements/loops/mutation are step 2.2) —
            // SKIPPED honestly (loudly), never silent-pass.
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

    // The body TAIL expr (the fn's value — `sum`'s final `acc`).
    if let Some(tail) = &body.tail {
        let label = format!("{}.tail", f.name);
        check_return_like(tail, &label, &f.ret, &env, f, seed, rlimit, report);
    }
}

/// Check a tail / `return` expr at the fn return type, or honestly Skip if the
/// return type is not exec-frame-spellable.
#[allow(
    clippy::too_many_arguments,
    reason = "a corpus expr's TV genuinely needs the expr + its label + the return \
        type + the env + the fn (for the req frame) + the verus config; grouping \
        them would obscure the per-expr data flow"
)]
fn check_return_like(
    e: &Expr,
    label: &str,
    ret: &Type,
    env: &ExecEnv,
    f: &FnItem,
    seed: u64,
    rlimit: f64,
    report: &mut ExecTvReport,
) {
    match exec_type_spelling(ret) {
        Some((ret_ty, _)) => check_corpus_expr(e, label, &ret_ty, env, f, seed, rlimit, report),
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

/// TV one corpus exec expr at a derived `ret_ty` (REQ-5, best-effort). Checks the
/// expr is in the pure-exec subset, frames it from the env (referenced vars must all
/// be declared; else honest Skip), lowers it via `lower_exec_expr`, builds + (when
/// the frame is adequate) discharges the obligation, and classifies.
#[allow(
    clippy::too_many_arguments,
    reason = "see `check_return_like` — the genuine per-expr fan-in"
)]
fn check_corpus_expr(
    e: &Expr,
    label: &str,
    ret_ty: &str,
    env: &ExecEnv,
    f: &FnItem,
    seed: u64,
    rlimit: f64,
    report: &mut ExecTvReport,
) {
    // The expr's free vars must all be declared in the env (a derivable frame). An
    // undeclared free var (a local bound by an out-of-scope construct, a richer-typed
    // param) → honest Skip (non-derivable frame), never a guessed binding.
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

    // P_production — the REAL exec lowering. A construct the exec lowering does not
    // cover (a method call, a spec-only form) → honest Skip (out of the pure-exec
    // subset), NOT a faithfulness verdict.
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

    // The fn's source `req` is the BEST available overflow/index frame. It is
    // included ONLY when every var it references is env-declared (else its text
    // would reference an undeclared param and the obligation would not compile — a
    // FRAMING failure, not an infidelity). When included, its referenced vars JOIN
    // the obligation params so the `requires` typechecks. A `req` that cannot be
    // included is dropped (the expr is then checked with NO frame — adequate for a
    // total expr like a literal/comparison; an arithmetic expr without an adequate
    // bound discharges Unverifiable, honestly, NOT Faithful).
    let req_text = corpus_req(f);
    let req = match &req_text {
        Some(text) => {
            let req_vars: Vec<String> = collect_text_idents(text);
            if req_vars.iter().all(|v| env.declares(v)) {
                Some((text.clone(), req_vars))
            } else {
                None
            }
        }
        None => None,
    };

    // The obligation params: every var the EXPR references, plus every var the
    // (included) `req` references — declared from the env at its exec type.
    let mut needed: Vec<String> = referenced.clone();
    if let Some((_, req_vars)) = &req {
        for v in req_vars {
            if !needed.contains(v) {
                needed.push(v.clone());
            }
        }
    }
    let frame = ExecObligationFrame {
        spec_defs: Vec::new(),
        params: env
            .params
            .iter()
            .filter(|p| needed.iter().any(|r| r == &p.name))
            .cloned()
            .collect(),
        ret_type: ret_ty.to_string(),
        req: req.map(|(text, _)| text),
        slice_params: env
            .slice_params
            .iter()
            .filter(|n| needed.iter().any(|r| r == *n))
            .cloned()
            .collect(),
    };

    let program = match exec_equivalence_obligation(e, &p_production, &frame) {
        Ok(prog) => prog,
        Err(err) => {
            // The independent reference does not cover the expr → honest Skip (out of
            // the pure-exec subset; e.g. a method call / Vec-String accessor), NOT a
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

/// Extract the candidate IDENTIFIERS a `req` text references (a heuristic over the
/// verbatim source: alphanumeric/`_` runs starting with a letter/`_`, excluding the
/// dotted `.len()`-style method tail and numeric literals). A bare ident that is an
/// env param is a referenced var; anything else (a keyword, a method name, a
/// `u32::MAX`-style path segment) is harmlessly NOT an env param, so the all-declared
/// gate drops the `req` if it mentions any non-param ident. Only the LEADING segment
/// of a dotted access (`xs.len()` → `xs`) is a var; `.len`/`.MAX` tails are dropped.
fn collect_text_idents(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let bytes: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c.is_ascii_alphabetic() || c == '_' {
            // A leading-segment ident; consume the run.
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == '_') {
                i += 1;
            }
            let ident: String = bytes[start..i].iter().collect();
            // Skip a `.`-tail (a method / field / assoc access) — it is not a var.
            let after_dot = start > 0 && bytes[start - 1] == '.';
            // Skip a `::`-tail leading segment is kept; the tail after `::` is an
            // assoc item (`u32::MAX`'s `MAX`) — but it follows `:`, caught here.
            let after_colon = start > 0 && bytes[start - 1] == ':';
            if !after_dot && !after_colon && !out.contains(&ident) {
                out.push(ident);
            }
        } else {
            i += 1;
        }
    }
    out
}

/// The corpus fn's source `req` text as the obligation's enclosing `requires` (the
/// best available frame). `req true` → no requires (an empty frame). NB this is the
/// CONTRACT `req`, which may NOT adequately bound an exec arithmetic expr's overflow
/// (the source bound for `acc + xs[i]` lives in a loop `inv`, not the `req`); such an
/// expr then discharges Unverifiable (honest), never Faithful.
fn corpus_req(f: &FnItem) -> Option<String> {
    let text = f.contract.req.text.trim();
    if text.is_empty() || text == "true" {
        None
    } else {
        Some(text.to_string())
    }
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
        _ => {}
    }
}

/// The exec value-type spelling for a body var/return type, plus whether it is a
/// slice (`&[u32]` → indexed element-wise). `None` for a type outside the exec frame
/// sublanguage (Map/Option/struct/String/…) — an expr over such a var is Skipped
/// (non-derivable frame), honest.
fn exec_type_spelling(ty: &Type) -> Option<(String, bool)> {
    match ty {
        Type::Prim(PrimType::U32) => Some(("u32".to_string(), false)),
        Type::Prim(PrimType::U64) => Some(("u64".to_string(), false)),
        Type::Prim(PrimType::Usize) => Some(("usize".to_string(), false)),
        Type::Prim(PrimType::Bool) => Some(("bool".to_string(), false)),
        Type::Ref { inner, .. } => match inner.as_ref() {
            // `&[u32]` → the exec slice binding (indexed element-wise as `xs[i as
            // int]` in the reference, AC-5). Only a `u32` element slice is framed.
            Type::Slice(elem) if matches!(elem.as_ref(), Type::Prim(PrimType::U32)) => {
                Some(("&[u32]".to_string(), true))
            }
            // A `&u64`/`&usize` borrow frames as the inner scalar.
            other => exec_type_spelling(other),
        },
        Type::Slice(elem) if matches!(elem.as_ref(), Type::Prim(PrimType::U32)) => {
            Some(("&[u32]".to_string(), true))
        }
        _ => None,
    }
}

// ---- verus discharge (mirrors contract_tv::discharge) -----------------------

/// Discharge one exec obligation PROGRAM through `verus`, classifying the verdict
/// (REQ-5 — VERIFIED ⟺ Faithful; a GENUINE counterexample (errors, NO rlimit signal) /
/// compile-parse abort ⟺ Divergent; a Verus/Z3 rlimit timeout / verus-absent /
/// inadequate-frame non-discharge ⟺ Unverifiable). An `errors >= 1` run carrying a
/// rlimit signal degrades to Unverifiable AHEAD of the Divergent arm (the #189-class
/// gate via the shared `crate::tv_signal::is_rlimit_signal`; #192), so a solver-budget
/// timeout is never fabricated into an exec-lowering infidelity (R-HONEST-3 / R-CODE-4).
/// Runs in a per-run scratch dir removed wholesale on EVERY exit path (blocker #53,
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

    // The pinned `--rlimit` + `smt.random_seed` keep the discharge DETERMINISTIC
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
            // verus absent / spawn failure → Unverifiable (surfaced, never a silent
            // pass — R-CODE-4). NOT Divergent (that is reserved for a real infidelity
            // that DID reach verus).
            return ExecVerdict::Unverifiable {
                reason: "verus could not be spawned (absent on PATH or spawn failure)".to_string(),
            };
        }
    };
    let mut combined = String::from_utf8_lossy(&output.stdout).to_string();
    combined.push_str(&String::from_utf8_lossy(&output.stderr));

    // A Verus/Z3 RESOURCE-LIMIT (rlimit) exhaustion / timeout: verus prints an rlimit
    // diagnostic (`rlimit exceeded` / `Resource limit (rlimit) exceeded` / z3's own
    // `max. resource limit exceeded`) AND a results line counting the exhausted
    // obligation as an error. That is a DISCHARGE failure (the solver ran out of
    // budget), NOT a value mismatch — the #189-class hardening (the #192 root-cause fix:
    // the SHARED `crate::tv_signal::is_rlimit_signal` discriminator, mirroring body_tv /
    // contract_tv). An rlimit-hit error run is routed to Unverifiable, NEVER the
    // `errors >= 1` Divergent arm, so a genuine solver-budget timeout is never fabricated
    // into an exec-lowering infidelity (R-HONEST-3 / R-CODE-4 — a timeout degrades + is
    // reported, never a false finding).
    let rlimit_hit = crate::tv_signal::is_rlimit_signal(&combined);

    match parse_results(&combined) {
        // A clean verification (a results line, 0 errors, exit success) ⟺ Faithful.
        Some((verified, errors)) if errors == 0 && verified >= 1 && output.status.success() => {
            ExecVerdict::Faithful
        }
        // An error run that is REALLY an rlimit exhaustion → Unverifiable, never
        // Divergent (the #189-class mapping fix; this arm precedes the Divergent arm,
        // mirroring body_tv::run_obligation / contract_tv::discharge).
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
        // A results line with errors (NO rlimit signal) ⟺ a postcondition counterexample
        // — the production exec value differs from the bounded reference: a REAL infidelity.
        Some((_verified, errors)) if errors >= 1 => ExecVerdict::Divergent {
            detail: format!(
                "verus found {errors} error(s) on the exec equivalence obligation — \
                 the production exec lowering of `{label}` computes a value that differs \
                 from the independent bounded reference (a postcondition counterexample: \
                 the off-corpus exec-lowering infidelity finding)"
            ),
        },
        // NO parseable results line. If verus exited NON-SUCCESS, the production text
        // failed to COMPILE/PARSE (the #122 `E0308` / #146 `expected ','` catch shapes
        // abort before verification) → a REAL infidelity (Divergent). If verus exited
        // SUCCESS with no results line, the obligation did not discharge cleanly →
        // Unverifiable (honest, never a false Faithful).
        _ => {
            if !output.status.success() {
                ExecVerdict::Divergent {
                    detail: format!(
                        "verus ABORTED (compile/parse) on the exec obligation for `{label}` \
                         — the production exec text did not compile/parse (the #122 `E0308` / \
                         #146 cast-`<` mis-parse catch shapes): a real exec-lowering infidelity"
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
/// `contract_tv`'s parser / the teeth-test). `None` if no summary line is present.
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
/// output). One line per expr + the headline counts (the reported integers, the
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
/// regression-guard surface, reported so the guard is provably non-vacuous).
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

// ---- the forge-level Divergent teeth (REQ-5; blocker #157) -----------------
//
// The obligation-layer teeth (`thermite-tv/tests/exec_teeth.rs` E1-E4) prove a
// WRONG `P_production` -> a real verus error. They do NOT exercise the FORGE-level
// step that MAPS that verus error to `ExecVerdict::Divergent`: `discharge`'s
// four-way classification. Over the generated/corpus space the faithful lowerer
// never produces a Divergent, so the Divergent ARM had NO direct test coverage.
//
// This module is the end-to-end teeth for the FORGE classification: it builds a
// REAL exec obligation with a DELIBERATELY-WRONG production (the same E1/E3
// infidelity shapes the obligation layer pins), discharges it through the ACTUAL
// `discharge` fn, and asserts the verdict. It covers BOTH Divergent triggers
// (postcondition-counterexample AND non-compile) plus the positive control
// (faithful -> Faithful) and the degenerate boundary (no obligation ->
// Unverifiable, NOT Divergent -- the masking path the four-way classification must
// keep distinct).
//
// TEST-ONLY: no production-logic change. `discharge` is a private sibling fn,
// reachable here via `super::` (a child mod sees the parent's private items), so
// NO visibility tweak is needed either. The teeth are GENUINE: a real wrong
// production -> a real verus error -> the real `discharge` mapping, never a mocked
// verdict. Mirrors `thermite-tv/tests/exec_teeth.rs`'s skip-loudly verus gate --
// `discharge` spawns a bare `verus`, so the test gates on the same PATH-resolvable
// binary and SKIPS LOUDLY when it is genuinely absent.
#[cfg(test)]
mod divergent_teeth {
    use super::*;
    use thermite_syntax::ast::BinOp;

    /// `true` iff a bare `verus` is spawnable (the SAME resolution `discharge`
    /// uses -- `Command::new("verus")`, i.e. PATH). SKIP LOUDLY otherwise so the
    /// teeth never silently pass when the discharge cannot reach a solver.
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
    /// a VALUE difference). Reused for both the positive control + the
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

    /// POSITIVE CONTROL: a FAITHFUL production (`a + b`, the exact lowering of the
    /// source) -> the forge classification is `ExecVerdict::Faithful`. Without this,
    /// a `discharge` that returned Divergent unconditionally would pass the
    /// Divergent assertions vacuously.
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

    /// DIVERGENT trigger #1 (postcondition counterexample): a production that
    /// TYPECHECKS but computes the WRONG value (`a.wrapping_sub(b)` for source
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

    /// DIVERGENT trigger #2 (non-compile): the #122 paren-drop production
    /// `n - 1 as u8` (= `n - (1 as u8)`, a `u64 - u8` type mix) for source
    /// `(n - 1) as u8` -> verus ABORTS with `E0308`/`mismatched types` BEFORE
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
            ty: Type::Named("u8".to_string()),
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
        assert!(
            matches!(verdict, ExecVerdict::Divergent { .. }),
            "a NON-COMPILING production (the #122 paren-drop -> E0308) must classify \
             Divergent via the compile/parse-abort branch; got {verdict:?}"
        );
    }

    /// The DIVERGENT-vs-UNVERIFIABLE boundary (the critic's masking-path concern):
    /// a DEGENERATE program with ZERO exec obligations verifies as `0 verified,
    /// 0 errors` (verus succeeds, but no obligation reached a faithfulness verdict)
    /// -> `discharge` maps it to `ExecVerdict::Unverifiable`, NEVER `Divergent` and
    /// NEVER `Faithful`. This pins the `_ => if status.success()` arm so the
    /// non-infidelity no-discharge case stays DISTINCT from a real Divergent
    /// (errors >= 1) / a real Faithful (verified >= 1).
    #[test]
    fn degenerate_no_obligation_classifies_unverifiable() {
        if !verus_on_path() {
            eprintln!(
                "SKIP: verus not on PATH -- the forge-level Unverifiable boundary not \
                 discharged."
            );
            return;
        }
        // A well-formed verus program with NO proof/exec obligation: verus reports
        // `0 verified, 0 errors` and exits success -- neither verified >= 1
        // (Faithful) nor errors >= 1 (Divergent), so the four-way classification
        // must report Unverifiable (distinct, never masked as either).
        let degenerate = "use vstd::prelude::*;\nverus! {\n}\nfn main() {}\n";
        let verdict = discharge(degenerate, "teeth.degenerate", SEED, RLIMIT);
        assert!(
            matches!(verdict, ExecVerdict::Unverifiable { .. }),
            "a degenerate zero-obligation program must classify Unverifiable (the \
             Divergent-vs-Unverifiable boundary), never Divergent/Faithful; got {verdict:?}"
        );
    }

    /// THE #192/#189-class GATE (the missing-gate fix): `discharge` adds an
    /// `errors >= 1 && rlimit_hit -> Unverifiable` arm AHEAD of the Divergent arm, so
    /// a Verus/Z3 solver-budget timeout (an error run carrying a rlimit signal) is
    /// NEVER fabricated into an exec-lowering infidelity. exec_tv previously had NO
    /// rlimit gate at all (every `errors >= 1` run -> Divergent unconditionally — the
    /// same #189 class body_tv/contract_tv fixed); #192 centralizes the discriminator
    /// in `crate::tv_signal::is_rlimit_signal` and consumes it here.
    ///
    /// HAND-DERIVED (R-CHAR-3): a real Z3 rlimit exhaustion is not deterministically
    /// forcible, so this pins the discriminator that DRIVES the gate (the same seam
    /// body_tv / contract_tv pin). The full phrase set is detected (verus's two
    /// phrasings + z3's own `max. resource limit exceeded`); a genuine
    /// `postcondition not satisfied` counterexample is NOT (it stays in the Divergent
    /// class). The `discharge` source routes `errors >= 1 && rlimit_hit` to
    /// Unverifiable AHEAD of the `errors >= 1` Divergent arm, so the discriminator
    /// firing is exactly the gate firing.
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
        // The distributed z3 binary's OWN resourceout literal (#192 — the shared
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
