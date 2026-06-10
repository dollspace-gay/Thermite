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
//! | loop-tv REQ-5 (the forge `body_tv` loop wiring — increment 2.2.2-iii) | SHIPPED | `body_tv_file` recognizes a v1 frozen-subset `while` loop as the body's last statement and discharges the THREE per-run obligations via `thermite_tv::{loop_entry_obligation, loop_preservation_obligation, loop_exit_obligation}` (`loop_body_tv` / `discharge_loop`); all-three-VERIFY → Faithful, any counterexample → Divergent, an OUT-of-v1 loop (`loop`-kind / `break` / mid-body `return` / nested / non-scalar / weak `inv`) → an honest `Unsupported` → Skipped with reason (NEVER Faithful, R-HONEST-3). Verified by `forge/tests/body_tv.rs` (faithful `while` → Faithful all three; broken-invariant → Divergent; `binary_search.th`'s `loop`-kind body → Skipped-with-reason). |

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
/// verdict (REQ-5 — VERIFIED ⟺ Faithful; a counterexample / compile-parse abort ⟺
/// Divergent; verus-absent / inadequate-frame non-discharge ⟺ Unverifiable). Runs in
/// a per-run scratch dir removed wholesale on EVERY exit path (blocker #53, reusing
/// `crate::check::ScratchDir`). Mirrors `exec_tv::discharge` exactly.
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
        // No results line + verus exited NON-SUCCESS ⟺ the production text failed to
        // COMPILE/PARSE (the #122/#146 abort shapes) → a REAL infidelity (Divergent).
        DischargeOutcome::CompileAbort => BodyVerdict::Divergent {
            detail: format!(
                "verus ABORTED (compile/parse) on the body obligation for `{label}` — the \
                 production body text did not compile/parse (a type / parse infidelity): a \
                 real body-lowering infidelity"
            ),
        },
        // verus absent / spawn failure / no results line on success ⟺ Unverifiable
        // (surfaced, NEVER a silent pass — R-CODE-4). NOT Divergent (reserved for a
        // real infidelity that DID reach verus and disagree).
        DischargeOutcome::Unverifiable(reason) => BodyVerdict::Unverifiable { reason },
    }
}

/// Discharge the THREE per-run LOOP obligations (`loop-tv.md` REQ-5; ENTRY /
/// PRESERVATION / EXIT) through `verus`, classifying the COMBINED verdict (REQ-5).
/// `Faithful` ⟺ ALL THREE verified; `Divergent` ⟺ ANY obligation found a
/// counterexample (a broken-invariant preservation `postcondition not satisfied` / a
/// wrong-after-loop-state `assertion failed`); `Unverifiable` ⟺ any obligation could
/// not discharge for a non-infidelity reason (verus absent / no results); a loop OUT
/// of the v1 subset is already a Skip BEFORE this is reached. The after-loop
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
            DischargeOutcome::CompileAbort => {
                return BodyVerdict::Divergent {
                    detail: format!(
                        "verus ABORTED (compile/parse) on the loop {sub} obligation for \
                         `{label}` — the production loop text did not compile/parse: a real \
                         loop-lowering infidelity"
                    ),
                };
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
    /// A results line with `errors >= 1` (a postcondition counterexample / assertion
    /// failure).
    Errors(u32),
    /// No results line + a non-success exit (the production text failed to
    /// compile/parse).
    CompileAbort,
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

    match parse_results(&combined) {
        Some((verified, errors)) if errors == 0 && verified >= 1 && output.status.success() => {
            DischargeOutcome::Verified
        }
        Some((_verified, errors)) if errors >= 1 => DischargeOutcome::Errors(errors),
        _ => {
            if !output.status.success() {
                DischargeOutcome::CompileAbort
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
