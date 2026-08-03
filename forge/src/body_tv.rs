//! `forge/src/body_tv.rs` — the exec-body (statement / state-refinement)
//! translation-validation check phase (`.design/verified/exec-stmt-tv.md` REQ-5 +
//! `.design/verified/loop-tv.md` REQ-5 / increment 2.2.2-iii; epic crosslink #169,
//! blocker #162). The state analogue of the sibling `forge/src/exec_tv.rs`
//! (exec-expression TV): where `exec_tv` checks a single body-position value,
//! `body_tv` checks the body's state transformation — the `let`/assignment/mutation/
//! `if`/sequencing thread (a dropped statement, a reordered mutation, a swapped
//! `if`-branch all change the final state while every sub-expression stays
//! value-faithful, the class `exec_tv`'s per-expression check structurally cannot
//! see).
//!
//! For each checked fn item in a `.th` file this phase takes the fn's exec body and:
//!
//!   - **a straight-line body** (the frozen 2.2.1 subset — `let`/mutable-`let`/
//!     assignment/`if`/sequencing/tail, no loop as the last statement): lowers it via
//!     `thermite_lower::lower_exec_body` (`P_production`, the artifact under test) and
//!     discharges the body state-refinement obligation `fn tv_body_wrap(..) ensures
//!     result == <body_ref_state(body)> { <P_production> }`
//!     (`thermite_tv::body_equivalence_obligation`) through `verus`.
//!   - **a v1 frozen-subset `while` loop** as the body's last statement (`loop-tv.md`
//!     REQ-1: a single `while <cond>` with declared `inv`/`dec`, a straight-line
//!     scalar body): discharges the three per-run loop obligations (entry /
//!     preservation / exit — `thermite_tv::{loop_entry_obligation,
//!     loop_preservation_obligation, loop_exit_obligation}`), reusing the shipped
//!     `body_ref_state` single-iteration step.
//!
//! `thermite-tv` stays independent of `thermite-lower` (the N-version boundary,
//! `exec-stmt-tv.md` AC-6): this forge module is the only place the two encoders meet.
//!
//! ## The four-way verdict (R-HONEST-3 — a skip does not mask an infidelity)
//!
//! Each item is reported in one of four distinct verdicts (distinct in both
//! the human/JSON output and the exit code — see [`crate::cli`]'s `run_body_tv` exit
//! convention):
//!
//!   - **Faithful** — the obligation(s) verified (`verified >= 1, errors == 0`): the
//!     body's lowered state transformation means the reference state-denotation for
//!     all inputs (Z3). For a loop, all three obligations verified.
//!   - **Divergent** — verus found a counterexample (`postcondition not satisfied` /
//!     an `assertion failed` exit characterization / a non-compiling production): the
//!     lowering and the reference disagree. A finding that drives a non-zero
//!     exit code.
//!   - **Unverifiable** — the prover errored / timed out / could not be spawned (not
//!     a pass, not a divergence, R-CODE-4). Reported distinctly, never a
//!     Faithful.
//!   - **Skipped** — the body is outside the frozen subset (an out-of-v1 loop, a
//!     non-scalar mutation, a mid-body `return`, a re-shadow, a non-derivable frame —
//!     the `Unsupported` class), with the reason printed. A skip does not mask an
//!     infidelity (the 2.2.1-vs-2.2.2 boundary in the certificate).
//!
//! Exposed as `forge body-tv <file>` (the non-test consumer `cli::run_body_tv`), a
//! separate opt-in deeper audit (like `forge tv` / `forge exec-tv`, not folded into
//! `forge check`). It mirrors `exec_tv`'s conventions (the verdict enum, the
//! per-run scratch dir reusing `crate::check::ScratchDir` / #53 cleanup, the output
//! format, the exit codes — nonzero on Divergent, zero on Faithful / Skipped /
//! Unverifiable).
//!
//! ## REQ status
//!
//! <!-- generated:reqs view=forge-body-tv-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-FORGE-BODY-TV-LOOP | shipped | `forge/src/body_tv.rs` | Forge body-TV loop obligation wiring |  |
//! | REQ-FORGE-BODY-TV-PLUGIN | shipped | `forge/src/body_tv.rs` | Forge body-TV straight-line plugin point |  |
//! <!-- /generated:reqs -->

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

use thermite_syntax::ast::{Block, FnItem, Item, LoopNode, PrimType, Stmt, Type};

use thermite_tv::obligation::{
    body_equivalence_obligation, loop_entry_obligation, loop_exit_obligation,
    loop_preservation_obligation, BodyObligationFrame, BodyParamDecl, LoopObligationFrame,
    LoopParamDecl,
};
use thermite_tv::{loop_ref_obligations, BodyRefCtx};
use thermite_tv::{ExecCallDecl, ExecStructFieldDecl};

use crate::check::{unique_scratch_dir, ScratchDir, DEFAULT_RLIMIT, DEFAULT_SOLVER_SEED};
use crate::cli::ForgeError;

/// One body's TV verdict (REQ-5; the four-way classification, reported distinctly so
/// an Unverifiable / Skipped does not mask an infidelity — R-HONEST-3). `Faithful` ⟺
/// the obligation(s) verified (the body's state transformation means the reference
/// state-denotation for all inputs); `Divergent` ⟺ verus found a counterexample (the
/// lowering and the reference disagree — a dropped statement / reordered mutation /
/// swapped branch / broken loop invariant / wrong after-loop characterization — a
/// hard finding); `Unverifiable` ⟺ the prover errored / timed out /
/// could not be spawned (not a pass, not a divergence); `Skipped` ⟺ the body
/// is outside the frozen subset (an out-of-v1 loop, a non-scalar mutation, a mid-body
/// return, a re-shadow, a non-derivable frame — the `Unsupported` class), with the
/// reason, never a false Faithful.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BodyVerdict {
    Faithful,
    Divergent { detail: String },
    Unverifiable { reason: String },
    Skipped { reason: String },
}

/// One body's TV result: a human label (the fn name, with a `.loop` suffix for the
/// loop arm) + the verdict (REQ-5).
#[derive(Debug, Clone)]
pub struct BodyResult {
    /// A human label (`sum`, `binary_search.loop`, …).
    pub label: String,
    /// The verdict.
    pub verdict: BodyVerdict,
}

/// The aggregate body-TV report for one file (REQ-5). `divergent` is the headline:
/// any divergent body is a body-lowering state-transformation finding, which
/// drives a non-zero exit (the meaning-mismatch verdict).
#[derive(Debug, Clone, Default)]
pub struct BodyTvReport {
    pub results: Vec<BodyResult>,
}

impl BodyTvReport {
    /// The per-verdict integer tally (the reported counts).
    pub fn counts(&self) -> BodyCounts {
        let mut c = BodyCounts::default();
        for r in &self.results {
            match &r.verdict {
                BodyVerdict::Faithful => c.faithful += 1,
                BodyVerdict::Divergent { .. } => c.divergent += 1,
                BodyVerdict::Unverifiable { .. } => c.unverifiable += 1,
                BodyVerdict::Skipped { .. } => c.skipped += 1,
            }
        }
        c
    }
}

/// The per-verdict integer tally (REQ-5 — the reported "N checked, M divergent").
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BodyCounts {
    pub faithful: usize,
    pub divergent: usize,
    pub unverifiable: usize,
    pub skipped: usize,
}

impl BodyCounts {
    /// The bodies that reached verus and produced a faithfulness verdict (faithful +
    /// divergent). Unverifiable / Skipped did not.
    pub fn checked(&self) -> usize {
        self.faithful + self.divergent
    }
}

// ---- the corpus body-TV file walk ------------------------------------------

/// Run the body-state TV over a `.th` file (REQ-5). For each in-language `fn` item
/// take its exec body and run the straight-line body state-refinement TV (or, when
/// the body's last statement is a v1 frozen-subset `while` loop, the three per-run
/// loop obligations). Each body is classified Faithful / Divergent / Unverifiable /
/// Skipped (a body outside the frozen subset — an out-of-v1 loop, a non-scalar
/// mutation, a mid-body return, a non-derivable frame — is Skipped rather than
/// masking an infidelity).
pub fn body_tv_file(path: &Path, seed: u64, rlimit: f64) -> Result<BodyTvReport, ForgeError> {
    let src = std::fs::read_to_string(path).map_err(|e| ForgeError::Io {
        path: path.display().to_string(),
        source: e,
    })?;
    let parsed = thermite_syntax::parse(&src);
    if !parsed.is_clean() {
        return Err(ForgeError::Parse(parsed.errors));
    }

    let mut report = BodyTvReport::default();
    for item in &parsed.program.items {
        match item {
            Item::Fn(f) => body_tv_fn(&parsed.program, f, seed, rlimit, &mut report),
            // A `spec fn` body lowers in spec context (not exec); a struct/enum has no
            // exec body — out of scope for body-TV.
            Item::SpecFn(_) | Item::Struct(_) | Item::Enum(_) => {}
            // Forge-tier item (stage1-forge-tier.md REQ-3): no v1 body-TV consumer
            // yet (increments 2b-3); inert here, mirroring the spec/ADT no-op arm.
            Item::Forge(_) => {}
        }
    }
    Ok(report)
}

/// TV one fn body (REQ-5). A boundary fn (no in-language body) is silently skipped
/// (it has no exec body to validate). Otherwise: if the body's last statement is a
/// loop, route to the loop arm (the v1 `while` obligations, or a Skip); else route
/// to the straight-line body arm.
fn body_tv_fn(
    program: &thermite_syntax::Program,
    f: &FnItem,
    seed: u64,
    rlimit: f64,
    report: &mut BodyTvReport,
) {
    let Some(body) = &f.body else {
        return; // a boundary fn has no in-language body.
    };
    let is_loop = matches!(body.stmts.last(), Some(Stmt::Loop(_)));
    let structural_label = if is_loop {
        format!("{}.loop", f.name)
    } else {
        f.name.clone()
    };

    // #193/#195 open-hole gate (`.design/forge/goal-repl.md` REQ-5; the four-way's
    // out-of-subset class): a fn carrying any open body hole (`?N`) is incomplete.
    // A hole is recorded on `FnItem.holes`, not in the `Stmt` stream, so lowering
    // `body` here would drop the open goal and ship a hole-stripped body to
    // verus, fabricating `Faithful` for an unfinished body. An incomplete body is not
    // checkable, so it is Skipped with the OpenHole reason (never Faithful —
    // R-HONEST-3) before the body lowers, mirroring `check`'s `OpenHole` reject
    // (the shared `goal_repl::open_hole_reason`, the #192 single-copy lesson).
    if let Some(reason) = crate::goal_repl::open_hole_reason(f) {
        report.results.push(BodyResult {
            label: structural_label.clone(),
            verdict: BodyVerdict::Skipped { reason },
        });
        return;
    }

    let (support_defs, support_names) = match body_tv_support(program, f) {
        Ok(support) => support,
        Err(reason) => {
            report.results.push(BodyResult {
                label: structural_label,
                verdict: BodyVerdict::Skipped { reason },
            });
            return;
        }
    };

    if is_loop {
        loop_body_tv(f, body, &support_defs, &support_names, seed, rlimit, report);
    } else {
        straight_line_body_tv(
            program,
            f,
            body,
            &support_defs,
            &support_names,
            seed,
            rlimit,
            report,
        );
    }
}

/// Lower the exact in-file executable/spec dependency closure needed by one
/// body-TV obligation. The function under test is deliberately omitted: its
/// production body remains the text inside `tv_body_wrap`, while callees are
/// ordinary verified definitions available to both production and reference
/// expressions. This closes the call-frame hole without replacing the
/// independent state denotation.
pub(crate) fn body_tv_support(
    program: &thermite_syntax::Program,
    f: &FnItem,
) -> Result<(Vec<String>, BTreeSet<String>), String> {
    let closure = match crate::closure::verified_closure(program, std::slice::from_ref(&f.name)) {
        Ok(closure) => closure,
        // The verified-build closure gate owns fail-closed unresolved-call
        // diagnostics. Preserve the standalone body-TV four-way behavior here;
        // the resulting obligation will classify the missing frame honestly.
        Err(_) => return Ok((Vec::new(), BTreeSet::new())),
    };
    let support_names: BTreeSet<String> = closure
        .functions
        .iter()
        .filter(|name| *name != &f.name)
        .chain(closure.spec_functions.iter())
        .cloned()
        .collect();
    let referrers: Vec<&Item> = program
        .items
        .iter()
        .filter(|item| match item {
            Item::Fn(dep) => dep.name == f.name || support_names.contains(&dep.name),
            Item::SpecFn(dep) => support_names.contains(&dep.name),
            Item::Struct(_) | Item::Enum(_) | Item::Forge(_) => false,
        })
        .collect();
    let adt_names: BTreeSet<String> = crate::check::reachable_adt_deps(program, &referrers)
        .into_iter()
        .map(|item| item.name().to_string())
        .collect();
    if support_names.is_empty() && adt_names.is_empty() {
        return Ok((Vec::new(), support_names));
    }
    let support = thermite_syntax::Program {
        items: program
            .items
            .iter()
            .filter(|item| match item {
                Item::Fn(dep) => support_names.contains(&dep.name),
                Item::SpecFn(dep) => support_names.contains(&dep.name),
                Item::Struct(dep) => adt_names.contains(&dep.name),
                Item::Enum(dep) => adt_names.contains(&dep.name),
                Item::Forge(_) => false,
            })
            .cloned()
            .collect(),
    };
    let lowered = thermite_lower::lower(&support)
        .map_err(|error| format!("could not lower body-TV dependency frame: {error}"))?;
    let open = "verus! {\n";
    let close = "\n}\nfn main() {}\n";
    let start = lowered
        .find(open)
        .map(|offset| offset + open.len())
        .ok_or_else(|| "body-TV dependency frame had no Verus opening".to_string())?;
    let end = lowered
        .rfind(close)
        .filter(|end| *end >= start)
        .ok_or_else(|| "body-TV dependency frame had no canonical closing".to_string())?;
    let mut inner = lowered[start..end].to_string();
    let mut reference_defs = String::new();
    let struct_fields = exec_struct_field_decls(&support);
    let call_decls = exec_call_decls(&support);
    for item in &support.items {
        let Item::Fn(dep) = item else {
            continue;
        };
        let Some(body) = &dep.body else {
            return Err(format!(
                "body-TV dependency `{}` has no in-language body",
                dep.name
            ));
        };
        let mut params = Vec::new();
        let mut slices = Vec::new();
        let mut named = Vec::new();
        for param in &dep.params {
            let Some((ty, is_slice)) = exec_type_spelling(&param.ty) else {
                return Err(format!(
                    "body-TV dependency `{}` has an unframeable parameter `{}`",
                    dep.name, param.name
                ));
            };
            if is_slice {
                slices.push(param.name.clone());
            }
            if let Type::Named(name) = &param.ty {
                named.push((param.name.clone(), name.clone()));
            }
            params.push(format!("{}: {ty}", param.name));
        }
        let Some((ret, _)) = exec_type_spelling(&dep.ret) else {
            return Err(format!(
                "body-TV dependency `{}` has an unframeable return type",
                dep.name
            ));
        };
        let reference_ctx = BodyRefCtx::with_bounds(slices, std::iter::empty::<String>(), named)
            .with_struct_fields(struct_fields.iter().cloned())
            .with_calls(call_decls.iter().cloned())
            .with_result_struct(ret.clone());
        let reference = thermite_tv::body_ref_state(body, &reference_ctx).map_err(|error| {
            format!(
                "body-TV dependency `{}` is outside the independent body reference: {error}",
                dep.name
            )
        })?;
        let spec_name = format!("thermite_tv_ref_{}", dep.name);
        reference_defs.push_str(&format!(
            "\nspec fn {spec_name}({}) -> {ret} {{ {reference} }}\n",
            params.join(", ")
        ));
        let needle = format!("\nfn {}(", dep.name);
        let replacement = format!(
            "\n#[verifier::when_used_as_spec({spec_name})]\nfn {}(",
            dep.name
        );
        if !inner.contains(&needle) {
            return Err(format!(
                "body-TV dependency frame could not locate lowered function `{}`",
                dep.name
            ));
        }
        inner = inner.replacen(&needle, &replacement, 1);
    }
    reference_defs.push_str(&inner);
    Ok((vec![reference_defs], support_names))
}

// ---- the straight-line body arm (exec-stmt-tv REQ-5) -----------------------

/// TV a straight-line fn body (REQ-5). Derives the obligation frame from the
/// signature (params at their exec types, the fn return type as the result type, the
/// source `req` as the well-formedness frame), lowers the body via
/// `thermite_lower::lower_exec_body` (`P_production`), builds the body
/// state-refinement obligation, and discharges it. A body the frame cannot be derived
/// for (a richer-typed param, a non-scalar return) or that the reference encoder /
/// lowerer does not cover (a non-scalar mutation, a re-shadow, a mid-body return) is
/// Skipped.
fn straight_line_body_tv(
    program: &thermite_syntax::Program,
    f: &FnItem,
    body: &Block,
    support_defs: &[String],
    support_names: &BTreeSet<String>,
    seed: u64,
    rlimit: f64,
    report: &mut BodyTvReport,
) {
    let label = f.name.clone();

    // The result type — the body's final-state projection type. A return type outside
    // the exec frame sublanguage (Option/Map/struct/…) is a non-derivable frame →
    // Skip (never a guessed projection).
    let Some((ret_ty, _)) = exec_type_spelling(&f.ret) else {
        report.results.push(BodyResult {
            label,
            verdict: BodyVerdict::Skipped {
                reason: format!(
                    "the fn return type is outside the exec frame sublanguage (not a \
                     bounded u8/u16/u32/u64/usize/bool) — non-derivable result-state \
                     projection type: {:?}",
                    f.ret
                ),
            },
        });
        return;
    };

    // The signature param frame: each param at its exec value type. A param of a type
    // the exec frame cannot spell (Map/Option/struct/String/…) makes the frame
    // non-derivable → Skip (never a guessed binding).
    let mut params: Vec<BodyParamDecl> = Vec::new();
    let mut slice_params: Vec<String> = Vec::new();
    let mut mutable_slice_params: Vec<String> = Vec::new();
    for p in &f.params {
        match exec_type_spelling(&p.ty) {
            Some((ty_str, is_slice)) => {
                if is_slice {
                    slice_params.push(p.name.clone());
                    if is_mutable_slice(&p.ty) {
                        mutable_slice_params.push(p.name.clone());
                    }
                }
                params.push(BodyParamDecl::new(p.name.clone(), ty_str));
            }
            None => {
                report.results.push(BodyResult {
                    label,
                    verdict: BodyVerdict::Skipped {
                        reason: format!(
                            "the param `{}` has a type outside the exec frame sublanguage \
                             (Map/Option/struct/String/…) — non-derivable body frame: {:?}",
                            p.name, p.ty
                        ),
                    },
                });
                return;
            }
        }
    }

    // P_production — the exec lowering of the straight-line body (the artifact
    // under test, the non-test consumer of `lower_exec_body`). A body the exec body
    // lowering does not cover (a `Stmt::Loop` it cannot lower standalone, a non-scalar
    // construct) → Skip (out of the frozen straight-line subset), not a verdict.
    let p_production = match thermite_lower::lower_exec_body_in_program(program, body) {
        Ok(p) => p,
        Err(e) => {
            report.results.push(BodyResult {
                label,
                verdict: BodyVerdict::Skipped {
                    reason: format!(
                        "production exec-body lowering does not cover this body (out of \
                         the frozen straight-line subset — a loop / non-scalar / \
                         out-of-subset construct): {e}"
                    ),
                },
            });
            return;
        }
    };

    // The req gate (mirrors `exec_tv::check_corpus_expr`'s req gate): the source `req`
    // is threaded verbatim into the obligation frame, but the frame carries
    // `spec_defs: Vec::new()` — it declares only the params (the names below). If the
    // `req` references an identifier the frame cannot declare (a `spec fn` helper — the
    // design's `req sorted(haystack)` idiom — or a local bound by an out-of-frame
    // construct), the obligation would not compile: a framing limitation, not a
    // body-lowering infidelity. Skip (never a fabricated Divergent — R-HONEST-3,
    // exec-stmt-tv.md REQ-5). Unlike `exec_tv` (which drops the un-framed req and checks
    // with no frame), body-TV's `req` is the body's well-formedness / no-overflow frame;
    // dropping it could turn a faithful body into a false Divergent, so the class
    // here is Skipped, not a frameless re-check.
    let declared: Vec<&str> = params
        .iter()
        .map(|p| p.name.as_str())
        .chain(support_names.iter().map(String::as_str))
        .collect();
    if let Some(undeclared) = req_references_undeclarable(f, &declared) {
        report.results.push(BodyResult {
            label,
            verdict: BodyVerdict::Skipped {
                reason: format!(
                    "the `req` references `{undeclared}` — a spec-fn helper (the \
                     `req sorted(haystack)` design idiom) the v1 body-TV frame does not \
                     carry (`spec_defs: Vec::new()`); the obligation would not compile — a \
                     FRAMING limitation, not a body-lowering infidelity (exec-stmt-tv.md \
                     REQ-5; the exec_tv req-gate)"
                ),
            },
        });
        return;
    }

    let req = match lowered_body_req(f) {
        Ok(req) => req,
        Err(reason) => {
            report.results.push(BodyResult {
                label,
                verdict: BodyVerdict::Skipped { reason },
            });
            return;
        }
    };

    let frame = BodyObligationFrame {
        spec_defs: support_defs.to_vec(),
        params,
        ret_type: ret_ty,
        req,
        slice_params,
        mutable_slice_params,
        named_params: f
            .params
            .iter()
            .filter_map(|param| match &param.ty {
                Type::Named(name) => Some((param.name.clone(), name.clone())),
                _ => None,
            })
            .collect(),
        struct_fields: exec_struct_field_decls(program),
        call_decls: exec_call_decls(program),
    };

    // Build the body state-refinement obligation. The reference state-denotation
    // (`body_ref_state`) rejects (an `Unsupported` Err) a body outside the
    // frozen subset (a re-shadow, a mid-body return, a non-scalar mutation) → Skipped,
    // never a false faithful.
    let program = match body_equivalence_obligation(body, &p_production, &frame) {
        Ok(prog) => prog,
        Err(e) => {
            report.results.push(BodyResult {
                label,
                verdict: BodyVerdict::Skipped {
                    reason: format!(
                        "body reference state-denotation does not cover this body (outside \
                         the frozen straight-line subset — a re-shadow / mid-body return / \
                         non-scalar mutation / no-tail body): {e}"
                    ),
                },
            });
            return;
        }
    };

    let verdict = discharge(&program, &label, seed, rlimit);
    report.results.push(BodyResult { label, verdict });
}

/// Lower the source precondition into the same Verus spec position occupied by a
/// body obligation's `requires`. Contract-TV independently proves this lowering;
/// reusing that certified predicate here is the composition bridge. In particular,
/// frozen `FixedArray8.get(i)` becomes its callable spec accessor rather than an
/// illegal exec-method call in a specification.
pub(crate) fn lowered_body_req(f: &FnItem) -> Result<Option<String>, String> {
    if f.contract.req.text.trim().is_empty() || f.contract.req.text.trim() == "true" {
        return Ok(None);
    }
    let slice_names: Vec<&str> = f
        .params
        .iter()
        .filter(|param| matches!(param.ty, Type::Ref { ref inner, .. } if matches!(inner.as_ref(), Type::Slice(_))))
        .map(|param| param.name.as_str())
        .collect();
    let string_names: Vec<&str> = f
        .params
        .iter()
        .filter(|param| matches!(param.ty, Type::String))
        .map(|param| param.name.as_str())
        .collect();
    thermite_lower::lower_contract_expr(
        &f.contract.req.expr,
        &slice_names,
        &[],
        &string_names,
        &[],
        &[],
        &[],
    )
    .map(Some)
    .map_err(|error| {
        format!("the source `req` cannot be lowered into the body-TV Verus frame: {error}")
    })
}

// ---- the loop arm (loop-tv REQ-5 / increment 2.2.2-iii) --------------------

/// TV a fn body whose last statement is a loop (REQ-5; `loop-tv.md` increment
/// 2.2.2-iii). A v1 frozen-subset `while` loop (a single `while <cond>` with declared
/// `inv`/`dec`, a straight-line scalar body) discharges the three per-run obligations
/// (entry / preservation / exit); an out-of-v1 loop (`loop`-kind, `break`/`continue`,
/// a mid-body `return`, a nested loop, non-scalar state, a trivially-weak `inv`) is
/// Skipped (the `loop_ref_obligations` recognizer refuses to emit), never a
/// false Faithful (R-HONEST-3). The `binary_search.th` corpus loop (a `loop`-kind with
/// mid-body `return`s) reaches here as Skipped-with-reason, the expected
/// result.
fn loop_body_tv(
    f: &FnItem,
    body: &Block,
    support_defs: &[String],
    support_names: &BTreeSet<String>,
    seed: u64,
    rlimit: f64,
    report: &mut BodyTvReport,
) {
    let label = format!("{}.loop", f.name);

    // The loop node is the body's last statement (matched by the caller).
    let Some(Stmt::Loop(loop_node)) = body.stmts.last() else {
        // Unreachable (the caller matched a trailing `Stmt::Loop`); kept total.
        report.results.push(BodyResult {
            label,
            verdict: BodyVerdict::Skipped {
                reason: "no trailing loop statement (internal: the loop arm expects the \
                         body's last statement to be a loop)"
                    .to_string(),
            },
        });
        return;
    };

    // The loop-obligation frame: the fn inputs (the slices / scalars the inv/cond
    // reference, at their exec types) + the mutated cells (the scalar cells the body
    // rebinds, in the sorted order `loop_ref_obligations` reports them). A param /
    // cell of a non-exec-frame type makes the frame non-derivable → Skip. An
    // out-of-v1 loop surfaces its `Unsupported` here (the recognizer refuses).
    let frame = match build_loop_frame(f, body, loop_node, support_defs, support_names) {
        Ok(frame) => frame,
        Err(reason) => {
            report.results.push(BodyResult {
                label,
                verdict: BodyVerdict::Skipped { reason },
            });
            return;
        }
    };

    // The single-iteration production loop-body lowering, shaped to the preservation
    // obligation's `(cell0', cell1', …)`-returning step (the artifact under test). A
    // loop body the exec-body lowering does not cover → Skip.
    let p_production = match loop_step_production(loop_node, &frame) {
        Ok(p) => p,
        Err(reason) => {
            report.results.push(BodyResult {
                label,
                verdict: BodyVerdict::Skipped { reason },
            });
            return;
        }
    };

    let verdict = discharge_loop(body, &p_production, &frame, &label, seed, rlimit);
    report.results.push(BodyResult { label, verdict });
}

/// Build the [`LoopObligationFrame`] for a fn whose body's last statement is a v1
/// `while` loop. The mutated cells are derived from `loop_ref_obligations` (the v1
/// recognizer — an out-of-v1 loop returns its `Unsupported`, surfaced as the
/// Skip reason here); the inputs are the fn params at their exec types (a cell is a
/// body-local `let mut`, not a signature param). A param of a non-exec-frame type is
/// a non-derivable frame.
fn build_loop_frame(
    f: &FnItem,
    body: &Block,
    _loop_node: &LoopNode,
    support_defs: &[String],
    support_names: &BTreeSet<String>,
) -> Result<LoopObligationFrame, String> {
    // The mutated cells (+ the v1-subset recognition) come from the shipped
    // `loop_ref_obligations` — its `Unsupported` Err is the out-of-v1 reason.
    let ctx = loop_body_ref_ctx(f);
    let obs = loop_ref_obligations(body, &ctx).map_err(|e| {
        format!(
            "the loop is OUTSIDE the v1 frozen subset (a `loop`-kind / `break` / \
             mid-body `return` / nested loop / non-scalar state / trivially-weak \
             `inv`) — Skipped honestly: {e}"
        )
    })?;

    // The cells (the body-rebound scalar cells) at their exec types. A cell's exec
    // type is the type of the `let mut <cell>: T = ..` that introduced it in the body
    // prefix (the entry state). A cell with no derivable scalar type is non-derivable.
    let mut cells: Vec<LoopParamDecl> = Vec::with_capacity(obs.cells.len());
    for cell in &obs.cells {
        let ty = cell_decl_type(body, cell).ok_or_else(|| {
            format!(
                "the loop cell `{cell}` has no `let mut <cell>: T = ..` typed \
                 introducer in the body prefix (the cell's exec type is not derivable \
                 — a non-derivable loop frame)"
            )
        })?;
        cells.push(LoopParamDecl::new(cell.clone(), ty));
    }

    // The inputs — the fn params at their exec types (the slices / scalars the
    // inv/cond reference). A cell is body-local, never a signature param, so the
    // inputs are the params (none of which is a cell).
    let mut inputs: Vec<LoopParamDecl> = Vec::new();
    let mut slice_params: Vec<String> = Vec::new();
    for p in &f.params {
        let (ty_str, is_slice) = exec_type_spelling(&p.ty).ok_or_else(|| {
            format!(
                "the param `{}` has a type outside the exec frame sublanguage \
                 (Map/Option/struct/String/…) — non-derivable loop frame: {:?}",
                p.name, p.ty
            )
        })?;
        if is_slice {
            slice_params.push(p.name.clone());
        }
        inputs.push(LoopParamDecl::new(p.name.clone(), ty_str));
    }

    // The req gate (mirrors the straight-line arm + `exec_tv::check_corpus_expr`): the
    // source `req` is threaded verbatim, but the loop frame declares only the inputs +
    // cells (`spec_defs: Vec::new()`). A `req` referencing a `spec fn` helper (the
    // `req sorted(haystack)` idiom) makes every obligation — including the entry proof fn
    // (`loop-tv.md` REQ-2), which carries no production text — fail to compile: a framing
    // limitation, not the production loop text failing to compile. Skip
    // (R-HONEST-3 / loop-tv.md four-way; an undischarged frame is Skipped/Unverifiable,
    // not a fabricated Divergent).
    let mut declared: Vec<&str> = inputs.iter().map(|p| p.name.as_str()).collect();
    declared.extend(cells.iter().map(|c| c.name.as_str()));
    declared.extend(support_names.iter().map(String::as_str));
    if let Some(undeclared) = req_references_undeclarable(f, &declared) {
        return Err(format!(
            "the `req` references `{undeclared}` — a spec-fn helper (the \
             `req sorted(haystack)` design idiom) the v1 body-TV loop frame does not carry \
             (`spec_defs: Vec::new()`); every loop obligation (the ENTRY proof fn carries no \
             production text) would not compile — a FRAMING limitation, not a loop-lowering \
             infidelity (loop-tv.md four-way; the exec_tv req-gate)"
        ));
    }

    Ok(LoopObligationFrame {
        spec_defs: support_defs.to_vec(),
        inputs,
        cells,
        req: corpus_req(f),
        slice_params,
    })
}

/// The [`BodyRefCtx`] for the loop reference encoder of a fn: the slice-bound param
/// names (so an index in the inv / cond / cell encodes to the spec-view element
/// value). Derived from the fn signature.
fn loop_body_ref_ctx(f: &FnItem) -> BodyRefCtx {
    let slice_params: Vec<String> = f
        .params
        .iter()
        .filter_map(|p| match exec_type_spelling(&p.ty) {
            Some((_, true)) => Some(p.name.clone()),
            _ => None,
        })
        .collect();
    BodyRefCtx::with_slice_bound(slice_params)
}

/// Shape the production single-iteration loop-body lowering to the preservation
/// obligation's `(cell0', cell1', …)`-returning step. The loop body is a straight-line
/// `Block`, so its statement-by-statement lowering is the shipped
/// `thermite_lower::lower_exec_body` of the body prefix (the statements without a
/// tail); the stepped cells are then returned as the obligation's result tuple (a
/// single cell is the bare cell, multiple cells a `(c0, c1)` tuple). A loop body the
/// exec-body lowering does not cover is a Skip.
fn loop_step_production(
    loop_node: &LoopNode,
    frame: &LoopObligationFrame,
) -> Result<String, String> {
    // The loop body is straight-line (the v1 recognizer rejected the multi-exit forms),
    // and it carries no tail value (a loop body's statements mutate cells; the design's
    // v1 body is value-less). Lower the body's statements via the shipped per-body exec
    // entry; the cells are mutated in place, then returned as the result tuple.
    let body_block = Block {
        stmts: loop_node.body.stmts.clone(),
        tail: None,
    };
    let lowered = thermite_lower::lower_exec_body(&body_block).map_err(|e| {
        format!(
            "production exec-body lowering does not cover this loop body (out of the \
             straight-line scalar subset): {e}"
        )
    })?;

    // The returned step: the mutated cells as the obligation's `(c0', c1', …)` tuple
    // (a single cell is the bare cell). The cells are the loop-step's `let mut`
    // shadows (the obligation binds them as params), mutated by the lowered body, then
    // returned.
    let cell_names: Vec<String> = frame.cells.iter().map(|c| c.name.clone()).collect();
    let returned = if cell_names.len() == 1 {
        cell_names[0].clone()
    } else {
        format!("({})", cell_names.join(", "))
    };
    // Re-bind each cell as a `let mut` shadow so the lowered body mutates a local (the
    // obligation params are by-value), then return the stepped cells.
    let mut shadows = String::new();
    for name in &cell_names {
        shadows.push_str(&format!("    let mut {name} = {name};\n"));
    }
    Ok(format!("{shadows}{lowered}    {returned}\n"))
}

/// The exec value-type spelling of the `let mut <cell>: T = ..` that introduces a
/// loop cell in the body prefix (the cell's exec type for its obligation param). A
/// cell with no typed `let mut` introducer (an untyped `let mut`, or a cell mutated
/// without a prior `let mut`) yields `None` (a non-derivable frame).
fn cell_decl_type(body: &Block, cell: &str) -> Option<String> {
    for stmt in &body.stmts {
        if let Stmt::Let {
            name, ty: Some(ty), ..
        } = stmt
        {
            if name == cell {
                return exec_type_spelling(ty).map(|(s, _)| s);
            }
        }
    }
    None
}

// ---- the source `req` frame + exec type spelling (mirrors exec_tv) ---------

/// The corpus fn's source `req` text as the obligation's enclosing `requires` (the
/// best available well-formedness / no-overflow frame). `req true` → no requires (an
/// empty frame). The `req` is emitted verbatim (the obligation's own precondition,
/// authored from the source, not lowered here — `exec-stmt-tv.md` REQ-3).
fn corpus_req(f: &FnItem) -> Option<String> {
    let text = f.contract.req.text.trim();
    if text.is_empty() || text == "true" {
        None
    } else {
        Some(text.to_string())
    }
}

/// The req gate (mirrors `exec_tv::check_corpus_expr`'s req gate). Returns
/// `Some(<ident>)` for the first identifier the source `req` references that the
/// obligation frame cannot declare (a `spec fn` helper — the `req sorted(haystack)`
/// design idiom — or a local bound by an out-of-frame construct), given the frame's
/// `declared` names (its params / inputs / cells). The obligation carries
/// `spec_defs: Vec::new()`, so a `req` mentioning an undeclarable ident would not
/// compile — a framing limitation, not an infidelity. `None` when every referenced
/// ident is declared (or the `req` is empty / `true`), so a body whose `req` references
/// only its own params (`req x <= 1000`) is not over-skipped.
fn req_references_undeclarable(f: &FnItem, declared: &[&str]) -> Option<String> {
    let req = corpus_req(f)?;
    collect_text_idents(&req)
        .into_iter()
        .find(|ident| !declared.contains(&ident.as_str()))
}

/// Extract the candidate identifiers a `req` text references (mirrors
/// `exec_tv::collect_text_idents`). A heuristic over the verbatim source: alphanumeric/
/// `_` runs starting with a letter/`_`, excluding the dotted `.len()`-style method tail
/// (only the leading segment of `xs.len()` → `xs` is a var) and the `::`-assoc tail
/// (`u32::MAX`'s `MAX`). A `spec fn` helper name (`all_small`, `small`, `sorted`) is a
/// leading-segment ident not among the frame's declared params, so the gate fires; an
/// operator / comparison / numeric literal contributes no ident.
fn collect_text_idents(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let ident: String = chars[start..i].iter().collect();
            // Skip a `.`-tail (a method / field access) and a `:`-tail (an assoc item) —
            // neither is a frame var.
            let after_dot = start > 0 && chars[start - 1] == '.';
            let after_colon = start > 0 && chars[start - 1] == ':';
            if !after_dot && !after_colon && !out.contains(&ident) {
                out.push(ident);
            }
        } else {
            i += 1;
        }
    }
    out
}

/// The exec value-type spelling for a param / return / cell type, plus whether it is
/// a slice (`&[u32]` → indexed element-wise). `None` for a type outside the exec
/// frame sublanguage (Map/Option/struct/String/…) — a body over such a type is
/// Skipped (non-derivable frame). Mirrors `exec_tv::exec_type_spelling`.
fn exec_type_spelling(ty: &Type) -> Option<(String, bool)> {
    match ty {
        Type::Prim(PrimType::U8) => Some(("u8".to_string(), false)),
        Type::Prim(PrimType::U16) => Some(("u16".to_string(), false)),
        Type::Prim(PrimType::U32) => Some(("u32".to_string(), false)),
        Type::Prim(PrimType::U64) => Some(("u64".to_string(), false)),
        Type::Prim(PrimType::Usize) => Some(("usize".to_string(), false)),
        Type::Prim(PrimType::Bool) => Some(("bool".to_string(), false)),
        Type::Named(name) => Some((name.clone(), false)),
        Type::Generic { name, arg } if name == "FixedArray8" => {
            let suffix = match arg.as_ref() {
                Type::Prim(PrimType::U8) => "U8",
                Type::Prim(PrimType::U16) => "U16",
                Type::Prim(PrimType::U32) => "U32",
                Type::Prim(PrimType::U64) => "U64",
                Type::Prim(PrimType::Usize) => "Usize",
                Type::Prim(PrimType::Bool) => "Bool",
                Type::Named(name) => name,
                _ => return None,
            };
            Some((format!("TFixedArray8{suffix}"), false))
        }
        Type::Ref { mutable, inner } => match inner.as_ref() {
            // `&[u32]` → the exec slice binding (indexed element-wise as `xs[i as
            // int]` in the reference). Only a `u32` element slice is framed.
            Type::Slice(elem) => exec_type_spelling(elem).map(|(spelling, _)| {
                let borrowed = if *mutable {
                    format!("&mut [{spelling}]")
                } else {
                    format!("&[{spelling}]")
                };
                (borrowed, true)
            }),
            // A `&u64`/`&usize` borrow frames as the inner scalar.
            other => exec_type_spelling(other),
        },
        Type::Slice(elem) => {
            exec_type_spelling(elem).map(|(spelling, _)| (format!("&[{spelling}]"), true))
        }
        _ => None,
    }
}

/// Exact named-struct field declarations available to exec/body-TV. A field whose
/// type cannot be represented in the frozen exec frame is omitted; construction of
/// that struct then fails closed in the independent reference rather than guessing.
pub(crate) fn exec_struct_field_decls(
    program: &thermite_syntax::Program,
) -> Vec<ExecStructFieldDecl> {
    let mut fields: Vec<ExecStructFieldDecl> = Vec::new();
    for item in &program.items {
        let Item::Struct(item) = item else {
            continue;
        };
        for field in &item.fields {
            if let Some((type_str, _)) = exec_type_spelling(&field.ty) {
                if type_str.starts_with("TFixedArray8") {
                    let Type::Generic { arg, .. } = &field.ty else {
                        continue;
                    };
                    let Some((element, _)) = exec_type_spelling(arg) else {
                        continue;
                    };
                    for index in 0..8 {
                        let slot = format!("slot{index}");
                        if !fields
                            .iter()
                            .any(|decl| decl.struct_path == type_str && decl.field == slot)
                        {
                            fields.push(ExecStructFieldDecl::new(
                                type_str.clone(),
                                slot,
                                element.clone(),
                            ));
                        }
                    }
                }
                fields.push(ExecStructFieldDecl::new(
                    item.name.clone(),
                    field.name.clone(),
                    type_str,
                ));
            }
        }
    }
    fields
}

/// Exact executable signatures for every framable Thermite function in a program.
/// An unframable signature is omitted so its call remains a loud type/proof failure
/// rather than receiving a guessed cast.
pub(crate) fn exec_call_decls(program: &thermite_syntax::Program) -> Vec<ExecCallDecl> {
    program
        .items
        .iter()
        .filter_map(|item| {
            let Item::Fn(function) = item else {
                return None;
            };
            let params = function
                .params
                .iter()
                .map(|param| exec_type_spelling(&param.ty).map(|(ty, _)| ty))
                .collect::<Option<Vec<_>>>()?;
            let (ret, _) = exec_type_spelling(&function.ret)?;
            Some(ExecCallDecl::new(function.name.clone(), params, ret))
        })
        .collect()
}

fn is_mutable_slice(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Ref {
            mutable: true,
            inner,
        } if matches!(inner.as_ref(), Type::Slice(_))
    )
}

// ---- verus discharge (mirrors exec_tv::discharge) --------------------------

/// Discharge a straight-line body obligation program through `verus`, classifying the
/// verdict (REQ-5 — verified ⟺ Faithful; a counterexample (errors, no rlimit
/// signal) ⟺ Divergent; a Verus/Z3 rlimit timeout / a frame compile-parse abort /
/// verus-absent / inadequate-frame non-discharge ⟺ Unverifiable). `Divergent` is
/// reserved for a disagreement that reached a verdict, not a frame abort or a
/// timeout (exec-stmt-tv.md REQ-5 / R-HONEST-3). Runs in a per-run scratch dir removed
/// wholesale on every exit path (blocker #53, reusing `crate::check::ScratchDir`).
fn discharge(program: &str, label: &str, seed: u64, rlimit: f64) -> BodyVerdict {
    match run_obligation(program, label, seed, rlimit) {
        // A clean verification (a results line, 0 errors, exit success) ⟺ Faithful.
        DischargeOutcome::Verified => BodyVerdict::Faithful,
        // A results line with errors ⟺ a postcondition counterexample — the production
        // body's final state differs from the reference state-denotation: a
        // state-transformation infidelity (a dropped statement / reordered mutation /
        // swapped branch).
        DischargeOutcome::Errors(errors) => BodyVerdict::Divergent {
            detail: format!(
                "verus found {errors} error(s) on the body state-refinement obligation — \
                 the production body lowering of `{label}` produces a FINAL STATE that \
                 differs from the independent reference state-denotation (a postcondition \
                 counterexample: a dropped statement / reordered mutation / swapped \
                 `if`-branch state-transformation infidelity)"
            ),
        },
        // A Verus/Z3 rlimit exhaustion / timeout ⟺ Unverifiable (the ladder degrades —
        // `loop-tv.md` four-way / R-CODE-4), not Divergent (it is not a counterexample).
        DischargeOutcome::Timeout(reason) => BodyVerdict::Unverifiable { reason },
        // No results line + verus exited non-success ⟺ a frame compile/parse abort (the
        // obligation's `req`/wrapper did not compile). The req-gate catches the
        // spec-fn-helper-req case before this; a residual abort here is a framing
        // limitation, not a body-lowering infidelity → Unverifiable, not a fabricated
        // Divergent (exec-stmt-tv.md REQ-5 / R-HONEST-3). `Divergent` is reserved for a
        // counterexample (the `Errors` arm).
        DischargeOutcome::CompileAbort(reason) => BodyVerdict::Unverifiable { reason },
        // verus absent / spawn failure / no results line on success ⟺ Unverifiable
        // (surfaced, never a silent pass — R-CODE-4). Not Divergent, which is reserved
        // for an infidelity that reached verus and disagreed.
        DischargeOutcome::Unverifiable(reason) => BodyVerdict::Unverifiable { reason },
    }
}

/// Discharge the three per-run loop obligations (`loop-tv.md` REQ-5; entry /
/// preservation / exit) through `verus`, classifying the combined verdict (REQ-5).
/// `Faithful` ⟺ all three verified; `Divergent` ⟺ any obligation found a
/// counterexample (a broken-invariant preservation `postcondition not satisfied` / a
/// wrong-after-loop-state `assertion failed`, with no rlimit signal); `Unverifiable` ⟺
/// any obligation could not discharge for a non-infidelity reason (a Verus/Z3 rlimit
/// timeout, a frame compile abort — the entry obligation carries no production text so
/// its abort is never an infidelity — verus absent / no results); a loop out of the v1
/// subset is already a Skip before this is reached. The after-loop
/// characterization the exit obligation pins is the reference's own `inv` over the
/// opaque cells (implied by, not stronger than, the assumed `inv ∧ ¬cond`), so a
/// faithful loop verifies.
fn discharge_loop(
    block: &Block,
    p_production: &str,
    frame: &LoopObligationFrame,
    label: &str,
    seed: u64,
    rlimit: f64,
) -> BodyVerdict {
    // Entry — the invariant holds on the pre-loop entry state.
    let entry = match loop_entry_obligation(block, frame) {
        Ok(prog) => prog,
        Err(e) => {
            return BodyVerdict::Skipped {
                reason: format!(
                    "the loop is OUTSIDE the v1 frozen subset (entry obligation refused): {e}"
                ),
            }
        }
    };
    // Preservation — one straight-line iteration carries `inv ∧ cond` to `inv` (reuses
    // the shipped `body_ref_state` step); the production side is the loop-body lowering.
    let preservation = match loop_preservation_obligation(block, p_production, frame) {
        Ok(prog) => prog,
        Err(e) => {
            return BodyVerdict::Skipped {
                reason: format!(
                    "the loop is OUTSIDE the v1 frozen subset (preservation obligation \
                     refused): {e}"
                ),
            }
        }
    };
    // Exit — the after-loop state is `inv ∧ ¬cond`-constrained. The pinned claim is the
    // reference's own after-loop characterization (`inv` over the opaque cells), which
    // follows from `inv ∧ ¬cond`, so a faithful loop verifies.
    let after_loop = match loop_after_loop_claim(block, frame) {
        Ok(claim) => claim,
        Err(reason) => return BodyVerdict::Skipped { reason },
    };
    let exit = match loop_exit_obligation(block, &after_loop, frame) {
        Ok(prog) => prog,
        Err(e) => {
            return BodyVerdict::Skipped {
                reason: format!(
                    "the loop is OUTSIDE the v1 frozen subset (exit obligation refused): {e}"
                ),
            }
        }
    };

    // Discharge all three; the combined verdict. A Divergent on any is the headline
    // finding; an Unverifiable on any (with no Divergent) is reported as such.
    let mut unverifiable: Option<String> = None;
    for (sub, prog) in [
        ("entry", &entry),
        ("preservation", &preservation),
        ("exit", &exit),
    ] {
        let sub_label = format!("{label}.{sub}");
        match run_obligation(prog, &sub_label, seed, rlimit) {
            DischargeOutcome::Verified => {}
            DischargeOutcome::Errors(errors) => {
                return BodyVerdict::Divergent {
                    detail: format!(
                        "verus found {errors} error(s) on the loop {sub} obligation for \
                         `{label}` — the production loop lowering DISAGREES with the \
                         independent reference (a per-iteration state-lowering / \
                         broken-invariant / wrong-after-loop-state infidelity)"
                    ),
                };
            }
            // A Verus/Z3 rlimit exhaustion / timeout on any loop obligation ⟺
            // Unverifiable (`loop-tv.md` four-way: "a Verus/Z3 timeout on an obligation");
            // not Divergent (not a counterexample).
            DischargeOutcome::Timeout(reason) => {
                unverifiable.get_or_insert(format!("loop {sub} obligation: {reason}"));
            }
            // A frame compile/parse abort on a loop obligation (the entry obligation
            // carries no production text at all — `loop-tv.md` REQ-2 — so its abort can
            // never be the production loop text failing to compile). The req-gate catches
            // the spec-fn-helper-req case in `build_loop_frame`; a residual abort here is a
            // framing limitation, not a loop-lowering infidelity → Unverifiable, not a
            // fabricated Divergent (R-HONEST-3). `Divergent` is reserved for a
            // counterexample (the `Errors` arm above).
            DischargeOutcome::CompileAbort(reason) => {
                unverifiable.get_or_insert(format!("loop {sub} obligation: {reason}"));
            }
            DischargeOutcome::Unverifiable(reason) => {
                unverifiable.get_or_insert(format!("loop {sub} obligation: {reason}"));
            }
        }
    }

    match unverifiable {
        Some(reason) => BodyVerdict::Unverifiable { reason },
        None => BodyVerdict::Faithful,
    }
}

/// The after-loop characterization claim the exit obligation pins: the reference's own
/// `inv` over the opaque-but-invariant-constrained after-loop cells (`loop-tv.md`
/// REQ-2.3 — after-loop = `inv ∧ ¬cond`; the obligation already assumes `inv ∧ ¬cond`
/// as its `requires`, so asserting `inv` is the faithful, non-vacuous after-loop claim
/// the continuation reads — it is implied by, not stronger than, `inv ∧ ¬cond`). An
/// out-of-v1 loop surfaces its `Unsupported`.
fn loop_after_loop_claim(block: &Block, frame: &LoopObligationFrame) -> Result<String, String> {
    let ctx = BodyRefCtx::with_slice_bound(frame.slice_params.iter().cloned());
    let obs = loop_ref_obligations(block, &ctx).map_err(|e| {
        format!("the loop is OUTSIDE the v1 frozen subset (after-loop claim refused): {e}")
    })?;
    Ok(obs.inv)
}

/// The discharge outcome of one obligation program (the four verus signals the
/// four-way classification maps from). Kept distinct so a Skipped is never an Errors
/// and an Unverifiable is never a Verified.
enum DischargeOutcome {
    /// A clean verification (`verified >= 1, errors == 0`, exit success).
    Verified,
    /// A results line with `errors >= 1` and no rlimit/timeout signal (a
    /// counterexample — `postcondition not satisfied` / `assertion failed`). This is
    /// the sole source of a `Divergent` verdict (the lowering and the reference
    /// disagree).
    Errors(u32),
    /// A Verus/Z3 resource-limit (rlimit) exhaustion / timeout (an error run whose
    /// output carries the `rlimit`/`resource limit exceeded` signal — `loop-tv.md`
    /// four-way: "a Verus/Z3 timeout on an obligation"). Not a counterexample, not an
    /// infidelity — `Unverifiable`, not `Divergent` (R-CODE-4 — reported, not a
    /// silent pass; the `forge check` `classify_verus_outcome` `Timeout` precedent).
    Timeout(String),
    /// No results line + a non-success exit (a frame compile/parse abort — the
    /// obligation's `req`/wrapper did not compile). Not a body-lowering infidelity:
    /// `Unverifiable` (the gate catches the spec-fn-helper-req case before this; this
    /// is the residual frame-abort safety net, not `Divergent`, R-HONEST-3).
    CompileAbort(String),
    /// verus absent / spawn failure / a no-results-on-success non-discharge (reported,
    /// never a silent pass).
    Unverifiable(String),
}

/// Run one obligation program through `verus` in a per-run scratch dir (blocker #53,
/// reusing `crate::check::ScratchDir`), returning the [`DischargeOutcome`]. The pinned
/// `--rlimit` + `smt.random_seed` keep the discharge deterministic (R-CODE-5),
/// matching `forge check` / `forge exec-tv`'s verus config.
fn run_obligation(program: &str, label: &str, seed: u64, rlimit: f64) -> DischargeOutcome {
    let stem = sanitize_stem(label);
    let scratch = ScratchDir {
        path: unique_scratch_dir(&stem),
    };
    if std::fs::create_dir_all(&scratch.path).is_err() {
        return DischargeOutcome::Unverifiable(
            "could not create the scratch dir for the verus discharge".to_string(),
        );
    }
    let file = scratch.path.join(format!("{stem}.rs"));
    if std::fs::write(&file, program).is_err() {
        return DischargeOutcome::Unverifiable(
            "could not write the obligation program to the scratch dir".to_string(),
        );
    }

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
            // pass — R-CODE-4). Not Divergent (reserved for an infidelity that did
            // reach verus).
            return DischargeOutcome::Unverifiable(
                "verus could not be spawned (absent on PATH or spawn failure)".to_string(),
            );
        }
    };
    let mut combined = String::from_utf8_lossy(&output.stdout).to_string();
    combined.push_str(&String::from_utf8_lossy(&output.stderr));

    // A Verus/Z3 resource-limit (rlimit) exhaustion signal. verus prints `rlimit
    // exceeded` / `Resource limit (rlimit) exceeded` (and z3 its own `max. resource
    // limit exceeded`) with a results line counting it as an error (probed live, issue
    // #189); the `forge check` `classify_verus_outcome` separates this `Timeout` from a
    // counterexample. A timeout is `Unverifiable`, not `Divergent` (`loop-tv.md`
    // four-way; R-CODE-4). The discriminator is the shared `crate::tv_signal::
    // is_rlimit_signal` (#192 — the sole copy across all three TV phases).
    let rlimit_hit = crate::tv_signal::is_rlimit_signal(&combined);

    match parse_results(&combined) {
        Some((verified, errors)) if errors == 0 && verified >= 1 && output.status.success() => {
            DischargeOutcome::Verified
        }
        // An error run that is in fact an rlimit exhaustion → Timeout (Unverifiable), not
        // a fabricated counterexample/Divergent.
        Some((_verified, errors)) if errors >= 1 && rlimit_hit => {
            DischargeOutcome::Timeout(format!(
                "verus exhausted its SMT resource budget (rlimit) on `{label}` before \
                     proving the obligation — a Verus/Z3 timeout (loop-tv.md four-way), not a \
                     counterexample"
            ))
        }
        // A counterexample (errors with no rlimit signal) → the sole Divergent
        // source.
        Some((_verified, errors)) if errors >= 1 => DischargeOutcome::Errors(errors),
        _ => {
            if rlimit_hit {
                DischargeOutcome::Timeout(format!(
                    "verus exhausted its SMT resource budget (rlimit) on `{label}` before \
                     producing a results line — a Verus/Z3 timeout, not an infidelity"
                ))
            } else if !output.status.success() {
                let diagnostic = combined.lines().take(12).collect::<Vec<_>>().join(" | ");
                DischargeOutcome::CompileAbort(format!(
                    "verus ABORTED (compile/parse) on the obligation for `{label}` with no \
                     parseable results line — a FRAME compile abort (the obligation's \
                     `req`/wrapper did not compile, e.g. a spec-fn-helper `req` the frame \
                     does not carry), not a body-lowering infidelity; tool diagnostic: \
                     {diagnostic}"
                ))
            } else {
                DischargeOutcome::Unverifiable(format!(
                    "verus produced no parseable results line for `{label}` (the obligation \
                     did not discharge — likely an INADEQUATE frame; reported distinctly, \
                     never as Faithful)"
                ))
            }
        }
    }
}

/// Parse the `N verified, M errors` summary line from verus output (mirrors
/// `exec_tv`'s parser and the negative test). `None` if no summary line is present.
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

/// A crate-name-safe scratch stem from a body label (no `.`/`#` — verus rejects a
/// `.` in the derived crate name; mirrors `exec_tv::sanitize_stem`).
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
    s.push_str("_bodytv");
    s
}

/// Render a [`BodyTvReport`] as a human summary (REQ-5; `forge body-tv` text output).
/// One line per body + the headline counts (the reported integers, the four-way
/// classification surfaced distinctly). Mirrors `exec_tv::render_report`.
pub fn render_report(report: &BodyTvReport, header: &str) -> String {
    let mut out = String::new();
    let counts = report.counts();
    out.push_str(&format!(
        "{header}: {} body/bodies checked, {} faithful, {} DIVERGENT, {} unverifiable, \
         {} skipped\n",
        counts.checked(),
        counts.faithful,
        counts.divergent,
        counts.unverifiable,
        counts.skipped,
    ));
    for r in &report.results {
        match &r.verdict {
            BodyVerdict::Faithful => out.push_str(&format!("  {} — faithful\n", r.label)),
            BodyVerdict::Divergent { detail } => {
                out.push_str(&format!("  {} — DIVERGENT: {detail}\n", r.label))
            }
            BodyVerdict::Unverifiable { reason } => {
                out.push_str(&format!("  {} — unverifiable ({reason})\n", r.label))
            }
            BodyVerdict::Skipped { reason } => {
                out.push_str(&format!("  {} — skipped ({reason})\n", r.label))
            }
        }
    }
    out
}

/// The pinned default seed + rlimit for `forge body-tv` (mirrors `forge check` /
/// `forge exec-tv` — the deterministic config, §5.3 / R-CODE-5).
pub const BODY_TV_DEFAULT_SEED: u64 = DEFAULT_SOLVER_SEED;
pub const BODY_TV_DEFAULT_RLIMIT: f64 = DEFAULT_RLIMIT;

#[cfg(test)]
mod structural_label_tests {
    use super::*;

    #[test]
    fn loop_keeps_loop_label_when_its_rich_dependency_frame_is_unavailable() {
        let source = format!(
            "{}\n{}\n{}\n",
            include_str!("../../conformance/kernel/atomic.th"),
            include_str!("../../conformance/kernel/memory.th"),
            include_str!("../../conformance/kernel/allocator.th"),
        );
        let parsed = thermite_syntax::parse(&source);
        assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
        let allocator = parsed
            .program
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fn(function) if function.name == "allocator_claim_first" => Some(function),
                _ => None,
            })
            .expect("allocator claim function");
        let mut report = BodyTvReport::default();

        body_tv_fn(
            &parsed.program,
            allocator,
            BODY_TV_DEFAULT_SEED,
            BODY_TV_DEFAULT_RLIMIT,
            &mut report,
        );

        assert_eq!(report.results.len(), 1, "{:#?}", report.results);
        assert_eq!(report.results[0].label, "allocator_claim_first.loop");
        assert!(
            matches!(report.results[0].verdict, BodyVerdict::Skipped { .. }),
            "the rich Atomic frame is expected to be completed only by exact composition"
        );
    }
}

// ---- forge-level Divergent regression tests (REQ-5; blocker #189) ----------
//
// The obligation-layer tests (`thermite-tv/tests/body_teeth.rs` / `loop_teeth.rs`)
// prove a wrong `P_production` -> a verus error. They do not exercise the
// forge-level step that maps that verus signal to a `BodyVerdict`: `discharge`'s
// four-way classification. Over the corpus the faithful lowerer never produces a
// Divergent, and the req-gate now keeps a spec-fn-helper-`req` frame abort out of
// `discharge` entirely, so the mapping itself (`CompileAbort`/`Timeout` ->
// Unverifiable, a counterexample -> Divergent) had no direct test coverage.
// This is the divergence #189 pinned: a frame abort fabricated a Divergent.
//
// This module tests the forge classification end to end, mirroring
// `exec_tv::divergent_teeth`: it builds a body obligation, discharges it through
// the `discharge` fn, and asserts the verdict. It covers the positive control
// (faithful -> Faithful), the counterexample Divergent trigger (a wrong-value
// production -> a postcondition counterexample), and the masking-path boundary the
// fix turns on: a frame compile abort (an undefined spec-fn `req`) and a degenerate
// zero-obligation program each classify Unverifiable, not Divergent.
//
// Test-only: no production-logic change. `discharge` is a private sibling fn,
// reachable here via `super::`. The tests drive a wrong production or a
// frame abort -> a verus signal -> the `discharge` mapping, not a
// mocked verdict. Skips with a printed reason when `verus` is absent.
#[cfg(test)]
mod divergent_teeth {
    use super::*;
    use thermite_syntax::ast::{BinOp, Expr, IndexArg};

    /// `true` iff a bare `verus` is spawnable (the same resolution `discharge` uses).
    /// Skips with a printed reason when the solver cannot be reached.
    fn verus_on_path() -> bool {
        Command::new("verus").arg("--version").output().is_ok()
    }

    const SEED: u64 = BODY_TV_DEFAULT_SEED;
    const RLIMIT: f64 = BODY_TV_DEFAULT_RLIMIT;

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

    /// Build a body obligation, returning `Ok`/`Err` (no `unwrap`/`expect` — the
    /// anti-pattern gate scans the patch text without `cfg(test)` context). The source
    /// bodies below are in-subset, so the build always succeeds; the caller asserts
    /// `is_ok()` so an `Err` (a regression) fails the test.
    fn build(
        body: &Block,
        production: &str,
        frame: &BodyObligationFrame,
    ) -> Result<String, String> {
        body_equivalence_obligation(body, production, frame).map_err(|e| e.to_string())
    }

    /// The source body `{ let a = x + 1; let b = a * 2; b }` (reference
    /// `((x as nat) + 1) * 2`), reused for the positive control + the frame-abort arm.
    fn sl_body() -> Block {
        Block {
            stmts: vec![
                Stmt::Let {
                    mutable: false,
                    name: "a".to_string(),
                    ty: None,
                    init: bin(BinOp::Add, path("x"), int(1)),
                },
                Stmt::Let {
                    mutable: false,
                    name: "b".to_string(),
                    ty: None,
                    init: bin(BinOp::Mul, path("a"), int(2)),
                },
            ],
            tail: Some(Box::new(path("b"))),
        }
    }

    fn sl_frame() -> BodyObligationFrame {
        BodyObligationFrame {
            params: vec![BodyParamDecl::new("x", "u64")],
            ret_type: "u64".to_string(),
            req: Some("x <= 1000".to_string()),
            ..Default::default()
        }
    }

    fn slice_write_body(index: Expr, value: Expr) -> Block {
        Block {
            stmts: vec![Stmt::Assign {
                target: Expr::Index {
                    base: Box::new(path("data")),
                    index: IndexArg::Single(Box::new(index)),
                },
                value,
            }],
            tail: Some(Box::new(path("value"))),
        }
    }

    fn slice_write_frame() -> BodyObligationFrame {
        BodyObligationFrame {
            params: vec![
                BodyParamDecl::new("data", "&mut [u8]"),
                BodyParamDecl::new("at", "usize"),
                BodyParamDecl::new("value", "u8"),
            ],
            ret_type: "u8".to_string(),
            req: Some("at < data.len()".to_string()),
            slice_params: vec!["data".to_string()],
            mutable_slice_params: vec!["data".to_string()],
            ..Default::default()
        }
    }

    fn pair_body() -> Block {
        Block {
            stmts: vec![],
            tail: Some(Box::new(Expr::StructLit {
                path: vec!["Pair".to_string()],
                fields: vec![
                    ("first".to_string(), path("a")),
                    ("second".to_string(), path("b")),
                ],
            })),
        }
    }

    fn pair_frame() -> BodyObligationFrame {
        BodyObligationFrame {
            spec_defs: vec!["struct Pair { first: u64, second: u64 }".to_string()],
            params: vec![
                BodyParamDecl::new("a", "u64"),
                BodyParamDecl::new("b", "u64"),
            ],
            ret_type: "Pair".to_string(),
            struct_fields: vec![
                ExecStructFieldDecl::new("Pair", "first", "u64"),
                ExecStructFieldDecl::new("Pair", "second", "u64"),
            ],
            ..Default::default()
        }
    }

    fn bounded_call_body() -> Block {
        Block {
            stmts: vec![],
            tail: Some(Box::new(Expr::Call {
                callee: Box::new(path("dep")),
                args: vec![bin(BinOp::Sub, path("a"), int(1))],
            })),
        }
    }

    fn bounded_call_frame() -> BodyObligationFrame {
        BodyObligationFrame {
            spec_defs: vec!["spec fn dep_spec(x: u64) -> u64 { x }\n\
                 #[verifier::when_used_as_spec(dep_spec)]\n\
                 fn dep(x: u64) -> (result: u64)\n\
                     ensures result == x,\n\
                 { x }"
                .to_string()],
            params: vec![BodyParamDecl::new("a", "u64")],
            ret_type: "u64".to_string(),
            req: Some("a > 0 && a < 100".to_string()),
            call_decls: vec![ExecCallDecl::new("dep", vec!["u64".to_string()], "u64")],
            ..Default::default()
        }
    }

    fn aggregate_call_body() -> Block {
        Block {
            stmts: vec![],
            tail: Some(Box::new(Expr::Call {
                callee: Box::new(path("pair_seed")),
                args: vec![path("a")],
            })),
        }
    }

    fn aggregate_call_frame(complete_contract: bool) -> BodyObligationFrame {
        let second_ensure = if complete_contract {
            "        result.second == 0,\n"
        } else {
            ""
        };
        BodyObligationFrame {
            spec_defs: vec![format!(
                "struct Pair {{ first: u64, second: u64 }}\n\
                 spec fn pair_seed_spec(a: u64) -> Pair {{ Pair {{ first: a, second: 0 }} }}\n\
                 #[verifier::when_used_as_spec(pair_seed_spec)]\n\
                 fn pair_seed(a: u64) -> (result: Pair)\n\
                     ensures\n\
                         result.first == a,\n\
                 {second_ensure}\
                 {{ Pair {{ first: a, second: 0 }} }}"
            )],
            params: vec![BodyParamDecl::new("a", "u64")],
            ret_type: "Pair".to_string(),
            struct_fields: vec![
                ExecStructFieldDecl::new("Pair", "first", "u64"),
                ExecStructFieldDecl::new("Pair", "second", "u64"),
            ],
            call_decls: vec![ExecCallDecl::new(
                "pair_seed",
                vec!["u64".to_string()],
                "Pair",
            )],
            ..Default::default()
        }
    }

    #[test]
    fn faithful_bounded_call_argument_classifies_faithful() {
        if !verus_on_path() {
            eprintln!("SKIP: verus not on PATH -- bounded-call control not discharged.");
            return;
        }
        let built = build(
            &bounded_call_body(),
            "    dep(a - 1)\n",
            &bounded_call_frame(),
        )
        .expect("the faithful bounded-call obligation builds");
        assert_eq!(
            discharge(&built, "teeth.call.faithful", SEED, RLIMIT),
            BodyVerdict::Faithful
        );
    }

    #[test]
    fn wrong_bounded_call_argument_classifies_divergent() {
        if !verus_on_path() {
            eprintln!("SKIP: verus not on PATH -- bounded-call wrong-arg tooth not discharged.");
            return;
        }
        let built = build(
            &bounded_call_body(),
            "    dep(a + 1)\n",
            &bounded_call_frame(),
        )
        .expect("the wrong bounded-call obligation builds");
        let verdict = discharge(&built, "teeth.call.wrong-arg", SEED, RLIMIT);
        assert!(
            matches!(verdict, BodyVerdict::Divergent { .. }),
            "a changed bounded call argument must be Divergent; got {verdict:?}"
        );
    }

    #[test]
    fn complete_aggregate_callee_contract_classifies_faithful() {
        if !verus_on_path() {
            eprintln!("SKIP: verus not on PATH -- aggregate-call control not discharged.");
            return;
        }
        let built = build(
            &aggregate_call_body(),
            "    pair_seed(a)\n",
            &aggregate_call_frame(true),
        )
        .expect("the complete aggregate-call obligation builds");
        assert_eq!(
            discharge(&built, "teeth.call.aggregate-complete", SEED, RLIMIT),
            BodyVerdict::Faithful
        );
    }

    #[test]
    fn omitted_aggregate_callee_field_classifies_divergent() {
        if !verus_on_path() {
            eprintln!(
                "SKIP: verus not on PATH -- aggregate-call omitted-field tooth not discharged."
            );
            return;
        }
        let built = build(
            &aggregate_call_body(),
            "    pair_seed(a)\n",
            &aggregate_call_frame(false),
        )
        .expect("the incomplete aggregate-call obligation builds");
        let verdict = discharge(&built, "teeth.call.aggregate-omitted-field", SEED, RLIMIT);
        assert!(
            matches!(verdict, BodyVerdict::Divergent { .. }),
            "an aggregate callee contract that omits a referenced field must be Divergent; got {verdict:?}"
        );
    }

    #[test]
    fn faithful_struct_construction_classifies_faithful() {
        if !verus_on_path() {
            eprintln!("SKIP: verus not on PATH -- struct construction control not discharged.");
            return;
        }
        let built = build(
            &pair_body(),
            "    Pair { first: a, second: b }\n",
            &pair_frame(),
        )
        .expect("the faithful struct obligation builds");
        assert_eq!(
            discharge(&built, "teeth.struct.faithful", SEED, RLIMIT),
            BodyVerdict::Faithful
        );
    }

    #[test]
    fn swapped_struct_fields_classify_divergent() {
        if !verus_on_path() {
            eprintln!("SKIP: verus not on PATH -- struct wrong-field tooth not discharged.");
            return;
        }
        let built = build(
            &pair_body(),
            "    Pair { first: b, second: a }\n",
            &pair_frame(),
        )
        .expect("the wrong-field struct obligation builds");
        let verdict = discharge(&built, "teeth.struct.swapped", SEED, RLIMIT);
        assert!(
            matches!(verdict, BodyVerdict::Divergent { .. }),
            "swapped aggregate fields must be Divergent; got {verdict:?}"
        );
    }

    /// The complete mutable-slice post-state is part of the body refinement, not
    /// merely the scalar return value. This positive control proves the production
    /// write at the source index with the source value.
    #[test]
    fn faithful_mutable_slice_write_classifies_faithful() {
        if !verus_on_path() {
            eprintln!(
                "SKIP: verus not on PATH — the mutable-slice Faithful control not discharged."
            );
            return;
        }
        let built = build(
            &slice_write_body(path("at"), path("value")),
            "    data[at] = value;\n    value\n",
            &slice_write_frame(),
        );
        assert!(
            built.is_ok(),
            "the slice body obligation must build: {built:?}"
        );
        let verdict = discharge(
            &built.unwrap_or_default(),
            "teeth.slice.faithful",
            SEED,
            RLIMIT,
        );
        assert_eq!(verdict, BodyVerdict::Faithful);
    }

    /// A production write to `0` still returns the expected scalar, but disagrees
    /// with the complete source sequence effect whenever `at != 0`.
    #[test]
    fn wrong_mutable_slice_index_classifies_divergent() {
        if !verus_on_path() {
            eprintln!(
                "SKIP: verus not on PATH — the mutable-slice wrong-index tooth not discharged."
            );
            return;
        }
        let built = build(
            &slice_write_body(path("at"), path("value")),
            "    data[0] = value;\n    value\n",
            &slice_write_frame(),
        );
        assert!(
            built.is_ok(),
            "the slice body obligation must build: {built:?}"
        );
        let verdict = discharge(
            &built.unwrap_or_default(),
            "teeth.slice.wrong-index",
            SEED,
            RLIMIT,
        );
        assert!(
            matches!(verdict, BodyVerdict::Divergent { .. }),
            "a wrong slice index must be Divergent; got {verdict:?}"
        );
    }

    /// A production write of zero still returns the expected scalar, but disagrees
    /// with the source sequence effect whenever `value != 0`.
    #[test]
    fn wrong_mutable_slice_value_classifies_divergent() {
        if !verus_on_path() {
            eprintln!(
                "SKIP: verus not on PATH — the mutable-slice wrong-value tooth not discharged."
            );
            return;
        }
        let built = build(
            &slice_write_body(path("at"), path("value")),
            "    data[at] = 0;\n    value\n",
            &slice_write_frame(),
        );
        assert!(
            built.is_ok(),
            "the slice body obligation must build: {built:?}"
        );
        let verdict = discharge(
            &built.unwrap_or_default(),
            "teeth.slice.wrong-value",
            SEED,
            RLIMIT,
        );
        assert!(
            matches!(verdict, BodyVerdict::Divergent { .. }),
            "a wrong slice value must be Divergent; got {verdict:?}"
        );
    }

    /// Positive control: a faithful production (`let a = x + 1; let b = a * 2; b`) ->
    /// `BodyVerdict::Faithful`. Without it, a `discharge` returning Faithful
    /// unconditionally would pass the other arms vacuously.
    #[test]
    fn faithful_production_classifies_faithful() {
        if !verus_on_path() {
            eprintln!("SKIP: verus not on PATH — the forge-level Faithful control not discharged.");
            return;
        }
        let built = build(
            &sl_body(),
            "    let a: u64 = x + 1;\n    let b: u64 = a * 2;\n    b\n",
            &sl_frame(),
        );
        assert!(
            built.is_ok(),
            "the body obligation TEXT must build: {built:?}"
        );
        let prog = built.unwrap_or_default();
        let verdict = discharge(&prog, "teeth.faithful", SEED, RLIMIT);
        assert_eq!(
            verdict,
            BodyVerdict::Faithful,
            "a FAITHFUL production body lowering must classify Faithful"
        );
    }

    /// Divergent (the only Divergent source): a production that typechecks but computes
    /// the wrong final state (the B2 reordered mutation shape) -> verus finds a
    /// `postcondition not satisfied` counterexample (errors >= 1, no rlimit signal) ->
    /// `discharge` maps the `Errors` arm to `BodyVerdict::Divergent`.
    #[test]
    fn wrong_state_production_classifies_divergent() {
        if !verus_on_path() {
            eprintln!(
                "SKIP: verus not on PATH — the forge-level Divergent (counterexample) teeth \
                 not discharged."
            );
            return;
        }
        // Source `{ let mut s = x; s = s + 1; s = s * 2; s }` (reference `(x+1)*2`); the
        // reordered production `s = s * 2; s = s + 1` has final state `(x*2)+1 != (x+1)*2`.
        let body = Block {
            stmts: vec![
                Stmt::Let {
                    mutable: true,
                    name: "s".to_string(),
                    ty: None,
                    init: path("x"),
                },
                Stmt::Assign {
                    target: path("s"),
                    value: bin(BinOp::Add, path("s"), int(1)),
                },
                Stmt::Assign {
                    target: path("s"),
                    value: bin(BinOp::Mul, path("s"), int(2)),
                },
            ],
            tail: Some(Box::new(path("s"))),
        };
        let built = build(
            &body,
            "    let mut s = x;\n    s = s * 2;\n    s = s + 1;\n    s\n",
            &sl_frame(),
        );
        assert!(
            built.is_ok(),
            "the body obligation TEXT must build: {built:?}"
        );
        let prog = built.unwrap_or_default();
        let verdict = discharge(&prog, "teeth.counterexample", SEED, RLIMIT);
        assert!(
            matches!(verdict, BodyVerdict::Divergent { .. }),
            "a WRONG-STATE production must classify Divergent via a postcondition \
             counterexample; got {verdict:?}"
        );
    }

    /// The fix (divergence #189): a frame compile abort — a `req` referencing an
    /// undefined spec fn (`all_small(x)`) with `spec_defs` empty — makes the obligation
    /// fail to compile (no parseable results line, non-success exit). `discharge` maps
    /// this `CompileAbort` to `BodyVerdict::Unverifiable`, not `Divergent`: a frame
    /// abort is a framing limitation, not a body-lowering infidelity (exec-stmt-tv.md
    /// REQ-5 / R-HONEST-3). This is the mapping the pinned divergence got wrong.
    #[test]
    fn frame_compile_abort_classifies_unverifiable_not_divergent() {
        if !verus_on_path() {
            eprintln!(
                "SKIP: verus not on PATH — the forge-level CompileAbort->Unverifiable \
                 mapping not discharged."
            );
            return;
        }
        let frame = BodyObligationFrame {
            params: vec![BodyParamDecl::new("x", "u64")],
            ret_type: "u64".to_string(),
            // `all_small` is an undefined spec fn (spec_defs is empty) — the obligation's
            // `requires all_small(x)` does not compile (an undefined-fn error). The
            // shape the pinned divergence fabricated a Divergent from.
            req: Some("all_small(x)".to_string()),
            ..Default::default()
        };
        let built = build(
            &sl_body(),
            "    let a: u64 = x + 1;\n    let b: u64 = a * 2;\n    b\n",
            &frame,
        );
        assert!(
            built.is_ok(),
            "the body obligation TEXT must build: {built:?}"
        );
        let prog = built.unwrap_or_default();
        let verdict = discharge(&prog, "teeth.frameabort", SEED, RLIMIT);
        assert!(
            matches!(verdict, BodyVerdict::Unverifiable { .. }),
            "a FRAME compile abort (an undefined spec-fn `req`) must classify Unverifiable, \
             NEVER Divergent (the pinned divergence #189 — a fabricated infidelity); got \
             {verdict:?}"
        );
        assert!(
            !matches!(verdict, BodyVerdict::Divergent { .. }),
            "a frame abort must NEVER be Divergent (R-HONEST-3); got {verdict:?}"
        );
    }

    /// The boundary: a degenerate zero-obligation program verifies as `0 verified,
    /// 0 errors` (verus succeeds, no obligation reached a verdict) -> `discharge` maps it
    /// to `BodyVerdict::Unverifiable`, not `Divergent`/`Faithful`. Pins the
    /// `_ => if status.success()` arm distinct from a Divergent / Faithful.
    #[test]
    fn degenerate_no_obligation_classifies_unverifiable() {
        if !verus_on_path() {
            eprintln!(
                "SKIP: verus not on PATH — the forge-level Unverifiable boundary not discharged."
            );
            return;
        }
        let degenerate = "use vstd::prelude::*;\nverus! {\n}\nfn main() {}\n";
        let verdict = discharge(degenerate, "teeth.degenerate", SEED, RLIMIT);
        assert!(
            matches!(verdict, BodyVerdict::Unverifiable { .. }),
            "a degenerate zero-obligation program must classify Unverifiable (the \
             Divergent-vs-Unverifiable boundary), never Divergent/Faithful; got {verdict:?}"
        );
    }

    /// The rlimit/timeout discriminator (the riding fix): an error run carrying a
    /// `Resource limit (rlimit) exceeded` signal is a timeout, not a counterexample —
    /// [`is_rlimit_signal`] detects it so `run_obligation` routes it to `Timeout`
    /// (Unverifiable), not the `Errors` (Divergent) arm. A pure-unit check of the
    /// discriminator (no verus needed): a counterexample output has no rlimit signal; an
    /// rlimit output does. This keeps a Z3 rlimit exhaustion out of Divergent
    /// (loop-tv.md four-way — "a Verus/Z3 timeout").
    #[test]
    fn rlimit_signal_is_detected_counterexample_is_not() {
        use crate::tv_signal::is_rlimit_signal;
        assert!(
            is_rlimit_signal("error: Resource limit (rlimit) exceeded\n0 verified, 1 errors"),
            "a `Resource limit (rlimit) exceeded` output MUST be detected as a timeout \
             signal (routed to Unverifiable, never Divergent)"
        );
        assert!(
            is_rlimit_signal("error: rlimit exceeded; consider raising the budget"),
            "a bare `rlimit exceeded` output MUST be detected as a timeout signal"
        );
        // The distributed z3 binary's own resourceout literal (#192 — now the shared
        // discriminator): `resource limit exceeded` with no `rlimit` token.
        assert!(
            is_rlimit_signal("unknown: max. resource limit exceeded\n0 verified, 1 errors"),
            "z3's own `max. resource limit exceeded` resourceout literal MUST be detected"
        );
        assert!(
            !is_rlimit_signal(
                "error: postcondition not satisfied\n --> x.rs:5:13\n0 verified, 1 errors"
            ),
            "a genuine `postcondition not satisfied` counterexample MUST NOT be detected as \
             a timeout (it stays in the Divergent class)"
        );
    }
}
