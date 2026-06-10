//! `forge/src/body_tv.rs` — the EXEC-BODY (statement / state-refinement)
//! TRANSLATION-VALIDATION check phase (`.design/verified/exec-stmt-tv.md` REQ-5 +
//! `.design/verified/loop-tv.md` REQ-5 / increment 2.2.2-iii; epic crosslink #169,
//! blocker #162). The state analogue of the sibling `forge/src/exec_tv.rs`
//! (exec-EXPRESSION TV): where `exec_tv` checks a single body-position VALUE,
//! `body_tv` checks the body's STATE TRANSFORMATION — the `let`/assignment/mutation/
//! `if`/sequencing thread (a DROPPED statement, a REORDERED mutation, a SWAPPED
//! `if`-branch all change the final state while every sub-expression stays
//! value-faithful — the class `exec_tv`'s per-expression check structurally cannot
//! see).
//!
//! For each checked fn item in a `.th` file this phase takes the fn's exec body and:
//!
//!   - **a STRAIGHT-LINE body** (the frozen 2.2.1 subset — `let`/mutable-`let`/
//!     assignment/`if`/sequencing/tail, NO loop as the last statement): lowers it via
//!     `thermite_lower::lower_exec_body` (`P_production`, the artifact under test) and
//!     discharges the body state-refinement obligation `fn tv_body_wrap(..) ensures
//!     result == <body_ref_state(body)> { <P_production> }`
//!     (`thermite_tv::body_equivalence_obligation`) through `verus`.
//!   - **a v1 frozen-subset `while` loop** as the body's last statement (`loop-tv.md`
//!     REQ-1: a single `while <cond>` with declared `inv`/`dec`, a straight-line
//!     scalar body): discharges the THREE per-run loop obligations (ENTRY /
//!     PRESERVATION / EXIT — `thermite_tv::{loop_entry_obligation,
//!     loop_preservation_obligation, loop_exit_obligation}`), reusing the SHIPPED
//!     `body_ref_state` single-iteration step.
//!
//! `thermite-tv` stays INDEPENDENT of `thermite-lower` (the N-version boundary,
//! `exec-stmt-tv.md` AC-6): this forge module is the ONLY place the two encoders meet.
//!
//! ## The four-way verdict (R-HONEST-3 — a skip NEVER masks an infidelity)
//!
//! Each item is reported in exactly one of four DISTINCT verdicts (distinct in both
//! the human/JSON output AND the exit code — see [`crate::cli`]'s `run_body_tv` exit
//! convention):
//!
//!   - **Faithful** — the obligation(s) VERIFIED (`verified >= 1, errors == 0`): the
//!     body's lowered state transformation MEANS the reference state-denotation for
//!     all inputs (Z3). For a loop, all THREE obligations VERIFIED.
//!   - **Divergent** — verus found a COUNTEREXAMPLE (`postcondition not satisfied` /
//!     an `assertion failed` exit characterization / a non-compiling production): the
//!     lowering and the reference DISAGREE. A hard finding, surfaced loudly, NEVER
//!     softened, and it drives a NON-ZERO exit code.
//!   - **Unverifiable** — the prover errored / timed out / could not be spawned (not
//!     a pass, not a divergence — honest, R-CODE-4). Reported DISTINCTLY; NEVER a
//!     Faithful.
//!   - **Skipped** — the body is OUTSIDE the frozen subset (an out-of-v1 loop, a
//!     non-scalar mutation, a mid-body `return`, a re-shadow, a non-derivable frame —
//!     the `Unsupported` class), with the REASON printed. A skip NEVER masks an
//!     infidelity (the honest 2.2.1-vs-2.2.2 boundary in the certificate).
//!
//! Exposed as `forge body-tv <file>` (the non-test consumer `cli::run_body_tv`), a
//! SEPARATE opt-in deeper audit (like `forge tv` / `forge exec-tv`, NOT folded into
//! `forge check`). It mirrors `exec_tv`'s conventions exactly (the verdict enum, the
//! per-run scratch dir reusing `crate::check::ScratchDir` / #53 cleanup, the output
//! format, the exit codes — nonzero on Divergent, zero on Faithful / Skipped /
//! Unverifiable).
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | exec-stmt-tv REQ-5 (forge `body_tv` plug-in point) | SHIPPED | `pub fn body_tv_file` walks each fn body; a STRAIGHT-LINE body lowers via `thermite_lower::lower_exec_body` + builds `thermite_tv::body_equivalence_obligation` + discharges through `verus` (the `discharge` helper, reusing `crate::check::ScratchDir` / #53). The four-way `BodyVerdict` (Faithful / Divergent / Unverifiable / Skipped) is REPORTED DISTINCTLY (R-HONEST-3). Non-test consumer: `cli::run_body_tv` (the `forge body-tv <file>` subcommand) — nonzero exit on Divergent, zero on Faithful/Skipped/Unverifiable. Verified by `forge/tests/body_tv.rs` (faithful straight-line → Faithful, mutated → Divergent, out-of-subset → Skipped) under real verus. This closes the `lower_exec_body` consumer loop (R-DEFER-1). |
//! | loop-tv REQ-5 (the forge `body_tv` loop wiring — increment 2.2.2-iii) | SHIPPED | `body_tv_file` recognizes a v1 frozen-subset `while` loop as the body's last statement and discharges the THREE per-run obligations via `thermite_tv::{loop_entry_obligation, loop_preservation_obligation, loop_exit_obligation}` (`loop_body_tv` / `discharge_loop`); all-three-VERIFY → Faithful, any counterexample → Divergent, an OUT-of-v1 loop (`loop`-kind / `break` / mid-body `return` / nested / non-scalar / weak `inv`) → an honest `Unsupported` → Skipped with reason (NEVER Faithful, R-HONEST-3). Verified by `forge/tests/body_tv.rs` (faithful `while` → Faithful all three; broken-invariant → Divergent; `binary_search.th`'s `loop`-kind body → Skipped-with-reason). **#192:** the rlimit discriminator `run_obligation` consumes is now the SHARED `crate::tv_signal::is_rlimit_signal` (the #189 phrase set, centralized as the SOLE copy across the three TV phases — body_tv was the authority the drifted contract_tv / missing exec_tv copies are now unified onto). |

use std::path::Path;
use std::process::Command;

use thermite_syntax::ast::{Block, FnItem, Item, LoopNode, PrimType, Stmt, Type};

use thermite_tv::obligation::{
    body_equivalence_obligation, loop_entry_obligation, loop_exit_obligation,
    loop_preservation_obligation, BodyObligationFrame, BodyParamDecl, LoopObligationFrame,
    LoopParamDecl,
};
use thermite_tv::{loop_ref_obligations, BodyRefCtx};

use crate::check::{unique_scratch_dir, ScratchDir, DEFAULT_RLIMIT, DEFAULT_SOLVER_SEED};
use crate::cli::ForgeError;

/// One body's TV verdict (REQ-5; the four-way classification, reported DISTINCTLY so
/// an Unverifiable / Skipped NEVER masks an infidelity — R-HONEST-3). `Faithful` ⟺
/// the obligation(s) VERIFIED (the body's state transformation MEANS the reference
/// state-denotation for all inputs); `Divergent` ⟺ verus found a counterexample (the
/// lowering and the reference DISAGREE — a dropped statement / reordered mutation /
/// swapped branch / broken loop invariant / wrong after-loop characterization — a
/// hard finding, surfaced loudly); `Unverifiable` ⟺ the prover errored / timed out /
/// could not be spawned (not a pass, not a divergence — honest); `Skipped` ⟺ the body
/// is OUTSIDE the frozen subset (an out-of-v1 loop, a non-scalar mutation, a mid-body
/// return, a re-shadow, a non-derivable frame — the `Unsupported` class), with the
/// reason — NEVER a false Faithful.
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
/// any divergent body is a real body-lowering state-transformation finding (the
/// payoff), which drives a non-zero exit (the meaning-mismatch verdict).
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
/// take its exec BODY and run the straight-line body state-refinement TV (or, when
/// the body's last statement is a v1 frozen-subset `while` loop, the three per-run
/// loop obligations). Each body is classified Faithful / Divergent / Unverifiable /
/// Skipped (a body OUTSIDE the frozen subset — an out-of-v1 loop, a non-scalar
/// mutation, a mid-body return, a non-derivable frame — is Skipped HONESTLY, never
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
            Item::Fn(f) => body_tv_fn(f, seed, rlimit, &mut report),
            // A `spec fn` body lowers in SPEC context (not exec); a struct/enum has no
            // exec body — out of scope for body-TV.
            Item::SpecFn(_) | Item::Struct(_) | Item::Enum(_) => {}
        }
    }
    Ok(report)
}

/// TV one fn body (REQ-5). A boundary fn (no in-language body) is silently skipped
/// (it has no exec body to validate). Otherwise: if the body's LAST statement is a
/// loop, route to the loop arm (the v1 `while` obligations, or an honest Skip);
/// else route to the straight-line body arm.
fn body_tv_fn(f: &FnItem, seed: u64, rlimit: f64, report: &mut BodyTvReport) {
    let Some(body) = &f.body else {
        return; // a boundary fn has no in-language body.
    };

    // #193/#195 OPEN-HOLE gate (`.design/forge/goal-repl.md` REQ-5; the four-way's
    // out-of-subset class): a fn carrying ANY open body hole (`?N`) is INCOMPLETE —
    // a hole is recorded on `FnItem.holes`, NOT in the `Stmt` stream, so lowering
    // `body` here would silently DROP the open goal and ship a hole-stripped body to
    // verus, fabricating `Faithful` for an unfinished body. An incomplete body is not
    // checkable, so it is Skipped HONESTLY with the OpenHole reason (NEVER Faithful —
    // R-HONEST-3) BEFORE the body lowers, mirroring `check`'s `OpenHole` reject
    // (the SHARED `goal_repl::open_hole_reason`, the #192 single-copy lesson).
    if let Some(reason) = crate::goal_repl::open_hole_reason(f) {
        report.results.push(BodyResult {
            label: f.name.clone(),
            verdict: BodyVerdict::Skipped { reason },
        });
        return;
    }

    if matches!(body.stmts.last(), Some(Stmt::Loop(_))) {
        loop_body_tv(f, body, seed, rlimit, report);
    } else {
        straight_line_body_tv(f, body, seed, rlimit, report);
    }
}

// ---- the straight-line body arm (exec-stmt-tv REQ-5) -----------------------

/// TV a straight-line fn body (REQ-5). Derives the obligation frame from the
/// signature (params at their exec types, the fn return type as the result type, the
/// source `req` as the well-formedness frame), lowers the body via
/// `thermite_lower::lower_exec_body` (`P_production`), builds the body
/// state-refinement obligation, and discharges it. A body the FRAME cannot be derived
/// for (a richer-typed param, a non-scalar return) or that the reference encoder /
/// lowerer does not cover (a non-scalar mutation, a re-shadow, a mid-body return) is
/// Skipped HONESTLY.
fn straight_line_body_tv(
    f: &FnItem,
    body: &Block,
    seed: u64,
    rlimit: f64,
    report: &mut BodyTvReport,
) {
    let label = f.name.clone();

    // The result type — the body's final-state projection type. A return type outside
    // the exec frame sublanguage (Option/Map/struct/…) is a non-derivable frame →
    // honest Skip (never a guessed projection).
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
    // non-derivable → honest Skip (never a guessed binding).
    let mut params: Vec<BodyParamDecl> = Vec::new();
    let mut slice_params: Vec<String> = Vec::new();
    for p in &f.params {
        match exec_type_spelling(&p.ty) {
            Some((ty_str, is_slice)) => {
                if is_slice {
                    slice_params.push(p.name.clone());
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

    // P_production — the REAL exec lowering of the straight-line body (the artifact
    // under test, the non-test consumer of `lower_exec_body`). A body the EXEC body
    // lowering does not cover (a `Stmt::Loop` it cannot lower standalone, a non-scalar
    // construct) → honest Skip (out of the frozen straight-line subset), NOT a verdict.
    let p_production = match thermite_lower::lower_exec_body(body) {
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

    // The req GATE (mirrors `exec_tv::check_corpus_expr`'s req gate): the source `req`
    // is threaded VERBATIM into the obligation frame, but the frame carries
    // `spec_defs: Vec::new()` — it declares ONLY the params (the names below). If the
    // `req` references an identifier the frame CANNOT declare (a `spec fn` helper — the
    // design's `req sorted(haystack)` idiom — or a local bound by an out-of-frame
    // construct), the obligation would NOT compile: a FRAMING limitation, NOT a
    // body-lowering infidelity. Skip HONESTLY (never a fabricated Divergent — R-HONEST-3,
    // exec-stmt-tv.md REQ-5). Unlike `exec_tv` (which DROPS the un-framed req and checks
    // with no frame), body-TV's `req` is the body's well-formedness / no-overflow frame —
    // dropping it could turn a faithful body into a false Divergent — so the honest class
    // here is Skipped, not a frame-less re-check.
    let declared: Vec<&str> = params.iter().map(|p| p.name.as_str()).collect();
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

    let frame = BodyObligationFrame {
        spec_defs: Vec::new(),
        params,
        ret_type: ret_ty,
        req: corpus_req(f),
        slice_params,
    };

    // Build the body state-refinement obligation. The reference state-denotation
    // (`body_ref_state`) HONESTLY rejects (an `Unsupported` Err) a body outside the
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

// ---- the loop arm (loop-tv REQ-5 / increment 2.2.2-iii) --------------------

/// TV a fn body whose LAST statement is a loop (REQ-5; `loop-tv.md` increment
/// 2.2.2-iii). A v1 frozen-subset `while` loop (a single `while <cond>` with declared
/// `inv`/`dec`, a straight-line scalar body) discharges the THREE per-run obligations
/// (ENTRY / PRESERVATION / EXIT); an OUT-of-v1 loop (`loop`-kind, `break`/`continue`,
/// a mid-body `return`, a nested loop, non-scalar state, a trivially-weak `inv`) is
/// Skipped HONESTLY (the `loop_ref_obligations` recognizer refuses to emit). NEVER a
/// false Faithful (R-HONEST-3). The `binary_search.th` corpus loop (a `loop`-kind with
/// mid-body `return`s) reaches here as Skipped-with-reason — the honest expected
/// result.
fn loop_body_tv(f: &FnItem, body: &Block, seed: u64, rlimit: f64, report: &mut BodyTvReport) {
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

    // The loop-obligation frame: the fn INPUTS (the slices / scalars the inv/cond
    // reference, at their exec types) + the mutated CELLS (the scalar cells the body
    // rebinds, in the SORTED order `loop_ref_obligations` reports them). A param /
    // cell of a non-exec-frame type makes the frame non-derivable → honest Skip. An
    // OUT-of-v1 loop surfaces its honest `Unsupported` here (the recognizer refuses).
    let frame = match build_loop_frame(f, body, loop_node) {
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
    // loop body the exec-body lowering does not cover → honest Skip.
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
/// `while` loop. The mutated CELLS are derived from `loop_ref_obligations` (the v1
/// recognizer — an OUT-of-v1 loop returns its honest `Unsupported`, surfaced as the
/// Skip reason here); the INPUTS are the fn params at their exec types (a cell is a
/// body-local `let mut`, not a signature param). A param of a non-exec-frame type is
/// a non-derivable frame.
fn build_loop_frame(
    f: &FnItem,
    body: &Block,
    _loop_node: &LoopNode,
) -> Result<LoopObligationFrame, String> {
    // The mutated cells (+ the v1-subset recognition) come from the SHIPPED
    // `loop_ref_obligations` — its `Unsupported` Err is the honest OUT-of-v1 reason.
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

    // The INPUTS — the fn params at their exec types (the slices / scalars the
    // inv/cond reference). A cell is body-local, never a signature param, so the
    // inputs are exactly the params (none of which is a cell).
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

    // The req GATE (mirrors the straight-line arm + `exec_tv::check_corpus_expr`): the
    // source `req` is threaded VERBATIM, but the loop frame declares ONLY the inputs +
    // cells (`spec_defs: Vec::new()`). A `req` referencing a `spec fn` helper (the
    // `req sorted(haystack)` idiom) makes EVERY obligation — including the ENTRY proof fn
    // (`loop-tv.md` REQ-2), which carries NO production text — fail to compile: a FRAMING
    // limitation, never "the production loop text did not compile". Skip HONESTLY
    // (R-HONEST-3 / loop-tv.md four-way; an undischarged frame is Skipped/Unverifiable,
    // never a fabricated Divergent).
    let mut declared: Vec<&str> = inputs.iter().map(|p| p.name.as_str()).collect();
    declared.extend(cells.iter().map(|c| c.name.as_str()));
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
        spec_defs: Vec::new(),
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
/// `Block`, so its statement-by-statement lowering is the SHIPPED
/// `thermite_lower::lower_exec_body` of the body PREFIX (the statements without a
/// tail); the stepped cells are then returned as the obligation's result tuple (a
/// single cell is the bare cell, multiple cells a `(c0, c1)` tuple). A loop body the
/// exec-body lowering does not cover is an honest Skip.
fn loop_step_production(
    loop_node: &LoopNode,
    frame: &LoopObligationFrame,
) -> Result<String, String> {
    // The loop body is straight-line (the v1 recognizer rejected the multi-exit forms),
    // and it carries no tail value (a loop body's statements mutate cells; the design's
    // v1 body is value-less). Lower the body's statements via the SHIPPED per-body exec
    // entry; the cells are mutated in place, then RETURNED as the result tuple.
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
/// empty frame). The `req` is emitted VERBATIM (the obligation's own precondition,
/// authored from the source, not lowered here — `exec-stmt-tv.md` REQ-3).
fn corpus_req(f: &FnItem) -> Option<String> {
    let text = f.contract.req.text.trim();
    if text.is_empty() || text == "true" {
        None
    } else {
        Some(text.to_string())
    }
}

/// The req GATE (mirrors `exec_tv::check_corpus_expr`'s req gate). Returns
/// `Some(<ident>)` for the FIRST identifier the source `req` references that the
/// obligation frame cannot declare (a `spec fn` helper — the `req sorted(haystack)`
/// design idiom — or a local bound by an out-of-frame construct), given the frame's
/// `declared` names (its params / inputs / cells). The obligation carries
/// `spec_defs: Vec::new()`, so a `req` mentioning an undeclarable ident would not
/// compile — a FRAMING limitation, not an infidelity. `None` when every referenced
/// ident is declared (or the `req` is empty / `true`), so a body whose `req` references
/// only its own params (`req x <= 1000`) is NOT over-skipped.
fn req_references_undeclarable(f: &FnItem, declared: &[&str]) -> Option<String> {
    let req = corpus_req(f)?;
    collect_text_idents(&req)
        .into_iter()
        .find(|ident| !declared.contains(&ident.as_str()))
}

/// Extract the candidate IDENTIFIERS a `req` text references (mirrors
/// `exec_tv::collect_text_idents`). A heuristic over the verbatim source: alphanumeric/
/// `_` runs starting with a letter/`_`, EXCLUDING the dotted `.len()`-style method tail
/// (only the leading segment of `xs.len()` → `xs` is a var) and the `::`-assoc tail
/// (`u32::MAX`'s `MAX`). A `spec fn` helper name (`all_small`, `small`, `sorted`) is a
/// leading-segment ident NOT among the frame's declared params, so the gate fires; an
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
/// Skipped (non-derivable frame), honest. Mirrors `exec_tv::exec_type_spelling`.
fn exec_type_spelling(ty: &Type) -> Option<(String, bool)> {
    match ty {
        Type::Prim(PrimType::U32) => Some(("u32".to_string(), false)),
        Type::Prim(PrimType::U64) => Some(("u64".to_string(), false)),
        Type::Prim(PrimType::Usize) => Some(("usize".to_string(), false)),
        Type::Prim(PrimType::Bool) => Some(("bool".to_string(), false)),
        Type::Ref { inner, .. } => match inner.as_ref() {
            // `&[u32]` → the exec slice binding (indexed element-wise as `xs[i as
            // int]` in the reference). Only a `u32` element slice is framed.
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

// ---- verus discharge (mirrors exec_tv::discharge) --------------------------

/// Discharge a STRAIGHT-LINE body obligation PROGRAM through `verus`, classifying the
/// verdict (REQ-5 — VERIFIED ⟺ Faithful; a GENUINE counterexample (errors, NO rlimit
/// signal) ⟺ Divergent; a Verus/Z3 rlimit timeout / a FRAME compile-parse abort /
/// verus-absent / inadequate-frame non-discharge ⟺ Unverifiable). `Divergent` is
/// reserved for a real disagreement that DID reach a verdict — NEVER a frame abort or a
/// timeout (exec-stmt-tv.md REQ-5 / R-HONEST-3). Runs in a per-run scratch dir removed
/// wholesale on EVERY exit path (blocker #53, reusing `crate::check::ScratchDir`).
fn discharge(program: &str, label: &str, seed: u64, rlimit: f64) -> BodyVerdict {
    match run_obligation(program, label, seed, rlimit) {
        // A clean verification (a results line, 0 errors, exit success) ⟺ Faithful.
        DischargeOutcome::Verified => BodyVerdict::Faithful,
        // A results line with errors ⟺ a postcondition counterexample — the production
        // body's final STATE differs from the reference state-denotation: a REAL
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
        // `loop-tv.md` four-way / R-CODE-4), NEVER Divergent (it is not a counterexample).
        DischargeOutcome::Timeout(reason) => BodyVerdict::Unverifiable { reason },
        // No results line + verus exited NON-SUCCESS ⟺ a FRAME compile/parse abort (the
        // obligation's `req`/wrapper did not compile). The req-gate catches the
        // spec-fn-helper-req case before this; a residual abort here is a FRAMING
        // limitation, NOT a body-lowering infidelity → Unverifiable (NEVER a fabricated
        // Divergent — exec-stmt-tv.md REQ-5 / R-HONEST-3). `Divergent` is reserved for a
        // GENUINE counterexample (the `Errors` arm).
        DischargeOutcome::CompileAbort(reason) => BodyVerdict::Unverifiable { reason },
        // verus absent / spawn failure / no results line on success ⟺ Unverifiable
        // (surfaced, NEVER a silent pass — R-CODE-4). NOT Divergent (reserved for a
        // real infidelity that DID reach verus and disagree).
        DischargeOutcome::Unverifiable(reason) => BodyVerdict::Unverifiable { reason },
    }
}

/// Discharge the THREE per-run LOOP obligations (`loop-tv.md` REQ-5; ENTRY /
/// PRESERVATION / EXIT) through `verus`, classifying the COMBINED verdict (REQ-5).
/// `Faithful` ⟺ ALL THREE verified; `Divergent` ⟺ ANY obligation found a GENUINE
/// counterexample (a broken-invariant preservation `postcondition not satisfied` / a
/// wrong-after-loop-state `assertion failed`, with NO rlimit signal); `Unverifiable` ⟺
/// any obligation could not discharge for a non-infidelity reason (a Verus/Z3 rlimit
/// timeout, a FRAME compile abort — the ENTRY obligation carries no production text so
/// its abort is NEVER an infidelity — verus absent / no results); a loop OUT of the v1
/// subset is already a Skip BEFORE this is reached. The after-loop
/// characterization the EXIT obligation pins is the reference's own `inv` over the
/// opaque cells (implied by, not stronger than, the assumed `inv ∧ ¬cond`), so a
/// faithful loop VERIFIES.
fn discharge_loop(
    block: &Block,
    p_production: &str,
    frame: &LoopObligationFrame,
    label: &str,
    seed: u64,
    rlimit: f64,
) -> BodyVerdict {
    // ENTRY — the invariant holds on the pre-loop entry state.
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
    // PRESERVATION — one straight-line iteration carries `inv ∧ cond` to `inv` (REUSES
    // the SHIPPED `body_ref_state` step); the production side is the loop-body lowering.
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
    // EXIT — the after-loop state is `inv ∧ ¬cond`-constrained. The pinned claim is the
    // reference's own after-loop characterization (`inv` over the opaque cells), which
    // genuinely follows from `inv ∧ ¬cond`, so a faithful loop VERIFIES.
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

    // Discharge all three; the COMBINED verdict. A Divergent on ANY is the headline
    // finding (surfaced loudly); an Unverifiable on ANY (with no Divergent) is honest.
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
            // A Verus/Z3 rlimit exhaustion / timeout on ANY loop obligation ⟺
            // Unverifiable (`loop-tv.md` four-way: "a Verus/Z3 timeout on an obligation");
            // NEVER Divergent (not a counterexample).
            DischargeOutcome::Timeout(reason) => {
                unverifiable.get_or_insert(format!("loop {sub} obligation: {reason}"));
            }
            // A FRAME compile/parse abort on a loop obligation (the ENTRY obligation
            // carries NO production text at all — `loop-tv.md` REQ-2 — so its abort can
            // NEVER be "the production loop text did not compile"). The req-gate catches
            // the spec-fn-helper-req case in `build_loop_frame`; a residual abort here is a
            // FRAMING limitation, NOT a loop-lowering infidelity → Unverifiable (NEVER a
            // fabricated Divergent — R-HONEST-3). `Divergent` is reserved for a GENUINE
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

/// The after-loop characterization claim the EXIT obligation pins: the reference's own
/// `inv` over the opaque-but-invariant-constrained after-loop cells (`loop-tv.md`
/// REQ-2.3 — after-loop = `inv ∧ ¬cond`; the obligation already assumes `inv ∧ ¬cond`
/// as its `requires`, so asserting `inv` is the faithful, non-vacuous after-loop claim
/// the continuation reads — it is implied by, not stronger than, `inv ∧ ¬cond`). An
/// OUT-of-v1 loop surfaces its honest `Unsupported`.
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
    /// A results line with `errors >= 1` AND NO rlimit/timeout signal (a GENUINE
    /// counterexample — `postcondition not satisfied` / `assertion failed`). This is
    /// the SOLE source of a `Divergent` verdict (the lowering and the reference
    /// DISAGREE).
    Errors(u32),
    /// A Verus/Z3 RESOURCE-LIMIT (rlimit) exhaustion / timeout (an error run whose
    /// output carries the `rlimit`/`resource limit exceeded` signal — `loop-tv.md`
    /// four-way: "a Verus/Z3 timeout on an obligation"). NOT a counterexample, NOT an
    /// infidelity — `Unverifiable`, NEVER `Divergent` (R-CODE-4 — report, never a
    /// silent pass; the `forge check` `classify_verus_outcome` `Timeout` precedent).
    Timeout(String),
    /// No results line + a non-success exit (a FRAME compile/parse abort — the
    /// obligation's `req`/wrapper did not compile). NOT a body-lowering infidelity:
    /// `Unverifiable` (the gate catches the spec-fn-helper-req case before this; this
    /// is the residual frame-abort safety net — NEVER `Divergent`, R-HONEST-3).
    CompileAbort(String),
    /// verus absent / spawn failure / a no-results-on-success non-discharge (honest,
    /// never a silent pass).
    Unverifiable(String),
}

/// Run one obligation PROGRAM through `verus` in a per-run scratch dir (blocker #53,
/// reusing `crate::check::ScratchDir`), returning the [`DischargeOutcome`]. The pinned
/// `--rlimit` + `smt.random_seed` keep the discharge DETERMINISTIC (R-CODE-5),
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
            // pass — R-CODE-4). NOT Divergent (reserved for a real infidelity that DID
            // reach verus).
            return DischargeOutcome::Unverifiable(
                "verus could not be spawned (absent on PATH or spawn failure)".to_string(),
            );
        }
    };
    let mut combined = String::from_utf8_lossy(&output.stdout).to_string();
    combined.push_str(&String::from_utf8_lossy(&output.stderr));

    // A Verus/Z3 RESOURCE-LIMIT (rlimit) exhaustion signal. verus prints `rlimit
    // exceeded` / `Resource limit (rlimit) exceeded` (and z3 its own `max. resource
    // limit exceeded`) WITH a results line counting it as an error (probed live, issue
    // #189); the `forge check` `classify_verus_outcome` separates this `Timeout` from a
    // counterexample. A timeout is `Unverifiable`, NEVER `Divergent` (`loop-tv.md`
    // four-way; R-CODE-4). The discriminator is the SHARED `crate::tv_signal::
    // is_rlimit_signal` (#192 — the SOLE copy across all three TV phases).
    let rlimit_hit = crate::tv_signal::is_rlimit_signal(&combined);

    match parse_results(&combined) {
        Some((verified, errors)) if errors == 0 && verified >= 1 && output.status.success() => {
            DischargeOutcome::Verified
        }
        // An error run that is REALLY an rlimit exhaustion → Timeout (Unverifiable), never
        // a fabricated counterexample/Divergent.
        Some((_verified, errors)) if errors >= 1 && rlimit_hit => {
            DischargeOutcome::Timeout(format!(
                "verus exhausted its SMT resource budget (rlimit) on `{label}` before \
                     proving the obligation — a Verus/Z3 timeout (loop-tv.md four-way), not a \
                     counterexample"
            ))
        }
        // A GENUINE counterexample (errors with NO rlimit signal) → the SOLE Divergent
        // source.
        Some((_verified, errors)) if errors >= 1 => DischargeOutcome::Errors(errors),
        _ => {
            if rlimit_hit {
                DischargeOutcome::Timeout(format!(
                    "verus exhausted its SMT resource budget (rlimit) on `{label}` before \
                     producing a results line — a Verus/Z3 timeout, not an infidelity"
                ))
            } else if !output.status.success() {
                DischargeOutcome::CompileAbort(format!(
                    "verus ABORTED (compile/parse) on the obligation for `{label}` with no \
                     parseable results line — a FRAME compile abort (the obligation's \
                     `req`/wrapper did not compile, e.g. a spec-fn-helper `req` the frame \
                     does not carry), not a body-lowering infidelity"
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
/// `exec_tv`'s parser / the teeth-test). `None` if no summary line is present.
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
/// classification surfaced DISTINCTLY). Mirrors `exec_tv::render_report`.
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

// ---- the forge-level Divergent teeth (REQ-5; blocker #189) -----------------
//
// The obligation-layer teeth (`thermite-tv/tests/body_teeth.rs` / `loop_teeth.rs`)
// prove a WRONG `P_production` -> a real verus error. They do NOT exercise the
// FORGE-level step that MAPS that verus signal to a `BodyVerdict`: `discharge`'s
// four-way classification. Over the corpus the faithful lowerer never produces a
// Divergent, and the req-gate now keeps a spec-fn-helper-`req` FRAME abort OUT of
// `discharge` entirely — so the mapping itself (`CompileAbort`/`Timeout` ->
// Unverifiable, a GENUINE counterexample -> Divergent) had NO direct test coverage.
// This is exactly the divergence #189 pinned: a frame abort fabricated a Divergent.
//
// This module is the end-to-end teeth for the FORGE classification, mirroring
// `exec_tv::divergent_teeth`: it builds a REAL body obligation, discharges it through
// the ACTUAL `discharge` fn, and asserts the verdict. It covers the positive control
// (faithful -> Faithful), the GENUINE-counterexample Divergent trigger (a wrong-value
// production -> a postcondition counterexample), AND the masking-path boundary the
// fix turns on: a FRAME compile abort (an undefined spec-fn `req`) and a degenerate
// zero-obligation program each classify Unverifiable, NEVER Divergent.
//
// TEST-ONLY: no production-logic change. `discharge` is a private sibling fn,
// reachable here via `super::`. The teeth are GENUINE (a real wrong production / a
// real frame abort -> a real verus signal -> the real `discharge` mapping, never a
// mocked verdict). SKIPS LOUDLY when `verus` is genuinely absent.
#[cfg(test)]
mod divergent_teeth {
    use super::*;
    use thermite_syntax::ast::{BinOp, Expr};

    /// `true` iff a bare `verus` is spawnable (the SAME resolution `discharge` uses).
    /// SKIP LOUDLY otherwise so the teeth never silently pass.
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
    /// `is_ok()` so an `Err` (a genuine regression) fails the test LOUDLY.
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

    /// POSITIVE CONTROL: a FAITHFUL production (`let a = x + 1; let b = a * 2; b`) ->
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

    /// DIVERGENT (the ONLY Divergent source): a production that TYPECHECKS but computes
    /// the WRONG final state (the B2 REORDERED mutation shape) -> verus finds a
    /// `postcondition not satisfied` counterexample (errors >= 1, NO rlimit signal) ->
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
        // REORDERED production `s = s * 2; s = s + 1` has final state `(x*2)+1 != (x+1)*2`.
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

    /// THE FIX (divergence #189): a FRAME compile abort — a `req` referencing an
    /// UNDEFINED spec fn (`all_small(x)`) with `spec_defs` EMPTY — makes the obligation
    /// fail to COMPILE (no parseable results line, non-success exit). `discharge` MUST
    /// map this `CompileAbort` to `BodyVerdict::Unverifiable`, NEVER `Divergent`: a frame
    /// abort is a FRAMING limitation, not a body-lowering infidelity (exec-stmt-tv.md
    /// REQ-5 / R-HONEST-3). This is the very mapping the pinned divergence got wrong.
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
            // `all_small` is an UNDEFINED spec fn (spec_defs is empty) — the obligation's
            // `requires all_small(x)` does not compile (an undefined-fn error). The exact
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

    /// THE BOUNDARY: a degenerate ZERO-obligation program verifies as `0 verified,
    /// 0 errors` (verus succeeds, no obligation reached a verdict) -> `discharge` maps it
    /// to `BodyVerdict::Unverifiable`, NEVER `Divergent`/`Faithful`. Pins the
    /// `_ => if status.success()` arm distinct from a real Divergent / Faithful.
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

    /// The rlimit/timeout DISCRIMINATOR (the riding fix): an error run carrying a
    /// `Resource limit (rlimit) exceeded` signal is a TIMEOUT, not a counterexample —
    /// [`is_rlimit_signal`] detects it so `run_obligation` routes it to `Timeout`
    /// (Unverifiable), NEVER the `Errors` (Divergent) arm. A pure-unit check of the
    /// discriminator (no verus needed): a counterexample output has NO rlimit signal; an
    /// rlimit output does. This keeps a genuine Z3 rlimit exhaustion out of Divergent
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
        // The distributed z3 binary's OWN resourceout literal (#192 — now the shared
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
