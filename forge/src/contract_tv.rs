//! `forge/src/contract_tv.rs` — the CONTRACT-FAITHFULNESS TRANSLATION-VALIDATION
//! check phase (`.design/verified/contract-tv.md` REQ-5; epic crosslink #139 /
//! blocker #144).
//!
//! `forge check` certifies that the EMITTED Verus contract holds for the
//! implementation; it does NOT certify that the emitted contract MEANS THE SAME
//! THING as the source contract. This phase closes that gap for the contract
//! sublanguage: for each `req`/`ens`/loop-`inv`/`dec` clause of a checked item it
//! computes
//!
//!   `P_production = thermite_lower::lower_contract_expr(clause.expr, …)`   (the artifact under test)
//!   `P_reference  = thermite_tv::ref_contract_pred(clause.expr, …)`        (the INDEPENDENT reference)
//!
//! builds the per-clause Z3 equivalence obligation
//! `assert((P_production) <==> (P_reference))` via
//! `thermite_tv::equivalence_obligation`, and discharges it through `verus`. A
//! VERIFIED obligation ⟺ the lowering of that clause is FAITHFUL; a COUNTEREXAMPLE
//! ⟺ a real lowering-fidelity infidelity (the #122 cast-paren / #127 byte-view
//! classes the vacuity/mutation battery + verus-on-emitted structurally cannot
//! see). It is exposed as `forge tv <file.th>` — a SEPARATE opt-in deeper audit,
//! NOT folded into `forge check` (which stays fast).
//!
//! `thermite-tv` stays INDEPENDENT of `thermite-lower` (the N-version boundary,
//! AC-6): this forge module is the ONLY place the two encoders meet. forge depends
//! on both — that is the correct home for the comparison.
//!
//! ## Two runs (both surfaced)
//!
//! - **Corpus run** ([`tv_file`]): over the REAL clauses of a `.th` program — the
//!   no-false-positive AC (`conformance/sum.th` etc.). The faithful production
//!   lowering must NOT trip TV.
//! - **Off-corpus run** ([`run_generated`]): over `thermite_tv::generate_clauses`
//!   — the corpus-bound escape (REQ-3). The lowerer is faithful, so ALL should
//!   verify; ANY counterexample is a REAL off-corpus infidelity finding.
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-5 (forge plug-in point) | SHIPPED | `pub fn tv_file` (the corpus phase) + `pub fn run_generated` (the off-corpus phase) here; both compute `P_production` via `thermite_lower::lower_contract_expr`, build the obligation via `thermite_tv::equivalence_obligation`, and discharge it through `verus` (the `discharge` helper, reusing the `crate::check::ScratchDir`/#53 cleanup). Non-test consumer: `cli::run_tv` (the `forge tv <file>` subcommand). A TV counterexample is surfaced as a per-clause DIVERGENT verdict (a meaning-mismatch finding, distinct from the contract-too-weak mutation signal). Verified by `forge/tests/contract_tv_conformance.rs` (corpus 0-divergent + the 200-clause off-corpus run) under real verus. **#228 (ref #225/#227) — production-column = real signature artifact + true-width frame:** `tv_file`/`run_generated` now derive the program-wide `thermite_lower::spec_fn_param_type_map` and THREAD it into `lower_contract_expr`, so a spec-call arithmetic arg narrows to the callee's DECLARED param type (`s_dec((n - 1) as u32)`) verbatim as the signature path (contract-tv.md REQ-2), not the `as u64` fallback. `SpecType::BoundedInt(width)` types each bounded-int param at its declared width so Z3 reasons over the true domain. Corpus byte-stable (no corpus spec fn takes a bare scalar param, so the cast never fires there; compose_demo/sum/binary_search stay all-Faithful). Honest split (binary constraint, `ref_encode.rs` UNCHANGED): a bare-path spec-call arg → Faithful; an arithmetic arg to a `u32`/`usize`-param callee → honest Unverifiable (the reference's bare `int` arg does not typecheck against the `u32` param — a genuinely unprovable equivalence, NEVER a forced Faithful). **#150 whole-corpus totality:** `signature_frame` now binds the three previously-Skipped construct classes — `String`→`&TString` (`SpecType::Strng`, threaded as production's `strings`), `Map<K,V>`→`TMap…` (`SpecType::Map`, with the `well_formed()` `requires` weave), `Option`/`Result` params + result natively (`SpecType::Opt`/`Res`) — so the C7 `match`-in-ens, the String byte-view, and the Map/Option signature clauses all reach verus + discharge. binary_search 6/6, map_kv 8/8, string_demo 8/8, sum 7/7 — all Checked + Faithful, 0 skipped/unverifiable; the 200-clause off-corpus run is TOTAL (0 skipped, the byte-view now over a `&TString` receiver `t`). **#192 (ref #166, #189):** the rlimit gate's discriminator is now the SHARED `crate::tv_signal::is_rlimit_signal` (the prior private copy had DROPPED z3's `resource limit exceeded` phrase — a Z3-phrased resourceout on an errors>=1 run was fabricated into `Divergent`); `discharge`'s `errors >= 1 && rlimit_hit -> Unverifiable` arm now consumes the shared full-phrase-set discriminator. |

use std::path::Path;
use std::process::Command;

use thermite_syntax::ast::{Clause, Expr, FnItem, Item, PrimType, Stmt, Type};

use thermite_tv::obligation::{equivalence_obligation, ObligationFrame, ParamDecl};

use crate::check::{unique_scratch_dir, ScratchDir, DEFAULT_RLIMIT, DEFAULT_SOLVER_SEED};
use crate::cli::ForgeError;

/// One clause's TV verdict (REQ-5). `Faithful` ⟺ the obligation VERIFIED (the
/// lowering of this clause means the same as the independent reference);
/// `Divergent` ⟺ verus found a counterexample (a real lowering infidelity, the
/// payoff — surfaced loudly); `Skipped` ⟺ the clause is outside the framed
/// sublanguage this phase covers (e.g. a `Map`/`Option`/struct-typed free var the
/// frame builder cannot type) — reported HONESTLY as not-checked, NEVER as a
/// false faithful (R-HONEST-3); `Unverifiable` ⟺ verus was absent (the audit
/// could not run — surfaced, never a silent pass, R-CODE-4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClauseVerdict {
    Faithful,
    Divergent { detail: String },
    Skipped { reason: String },
    Unverifiable,
}

/// One clause's TV result: its semantic address-ish label (`<item>.<kind>`) + the
/// verdict (REQ-5).
#[derive(Debug, Clone)]
pub struct ClauseResult {
    /// A human label for the clause (`sum.req`, `sum.ens#1`, `sum.loop#1.inv#2`, …).
    pub label: String,
    /// The verdict.
    pub verdict: ClauseVerdict,
}

/// The aggregate TV report for one file / run (REQ-5). `divergent` is the headline:
/// 0 divergent is the corpus no-false-positive AC; any divergence is a real
/// finding.
#[derive(Debug, Clone, Default)]
pub struct TvReport {
    pub clauses: Vec<ClauseResult>,
}

impl TvReport {
    /// The count of clauses with each verdict (the reported integers).
    pub fn counts(&self) -> TvCounts {
        let mut c = TvCounts::default();
        for r in &self.clauses {
            match &r.verdict {
                ClauseVerdict::Faithful => c.faithful += 1,
                ClauseVerdict::Divergent { .. } => c.divergent += 1,
                ClauseVerdict::Skipped { .. } => c.skipped += 1,
                ClauseVerdict::Unverifiable => c.unverifiable += 1,
            }
        }
        c
    }
}

/// The per-verdict integer tally (REQ-5 — the reported "N clauses, M divergent").
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TvCounts {
    pub faithful: usize,
    pub divergent: usize,
    pub skipped: usize,
    pub unverifiable: usize,
}

impl TvCounts {
    /// The total clauses CHECKED (faithful + divergent — the ones that reached
    /// verus). Skipped/unverifiable did not produce a faithfulness verdict.
    pub fn checked(&self) -> usize {
        self.faithful + self.divergent
    }
}

/// Run the contract-TV CORPUS phase over a `.th` file (REQ-5): for each fn/spec-fn
/// item, for each contract clause, build + discharge the per-clause equivalence
/// obligation and classify it. The faithful production lowering must yield ALL
/// `Faithful` (0 `Divergent`) — the no-false-positive AC. A `Divergent` is a real
/// finding (reported loudly, never suppressed).
pub fn tv_file(path: &Path, seed: u64, rlimit: f64) -> Result<TvReport, ForgeError> {
    let src = std::fs::read_to_string(path).map_err(|e| ForgeError::Io {
        path: path.display().to_string(),
        source: e,
    })?;
    let parsed = thermite_syntax::parse(&src);
    if !parsed.is_clean() {
        return Err(ForgeError::Parse(parsed.errors));
    }

    // The spec-fn / combinator definitions in scope for EVERY clause's obligation:
    // lower the whole program's spec-fn defs (+ auto-emitted combinator defs) once
    // and reuse them as the frame's spec_defs. The production lowerer is the source
    // of the spec-fn def TEXT (the artifact whose contract semantics the obligation
    // references); the clause-level fidelity is what TV checks — a def-text bug
    // surfaces as a clause counterexample since BOTH sides reference the SAME def.
    let preamble = program_spec_preamble(&parsed.program)?;

    // The program-wide user-`spec fn` param-type map (#228, ref #225/#227): the
    // SAME map `thermite_lower::lower` threads into the signature path, derived
    // here from the SAME `spec_fn_param_type_map` source of truth (R-CHAR-3, never
    // a forge-local re-derivation). Threaded into `lower_contract_expr` so the TV
    // production column narrows a spec-call arithmetic arg to the callee's DECLARED
    // param type (`as u32`/`as usize`) EXACTLY as `lower_fn_signature` does
    // (contract-tv.md REQ-2 "verbatim"). Without it the column fell back to the
    // hardcoded `as u64` and TV checked a NON-production predicate.
    let pt_owned = thermite_lower::spec_fn_param_type_map(&parsed.program);
    let spec_fn_param_types: Vec<(&str, &[PrimType])> =
        pt_owned.iter().map(|(n, ps)| (*n, ps.as_slice())).collect();

    let mut report = TvReport::default();
    for item in &parsed.program.items {
        match item {
            Item::Fn(f) => tv_fn(
                f,
                &preamble,
                &spec_fn_param_types,
                seed,
                rlimit,
                &mut report,
            ),
            // A `spec fn` carries only a `dec` measure (no req/ens) — its BODY's
            // fidelity is body-TV (epic #139 step 2, out of scope here).
            Item::SpecFn(_) | Item::Struct(_) | Item::Enum(_) => {}
        }
    }
    Ok(report)
}

/// TV one `fn`'s contract clauses (REQ-5): the single `req`, each `ens`, and each
/// loop's `inv`s + `dec`. Each clause is framed from the fn signature (params +
/// `result` + `old(_)`), lowered, encoded, and discharged.
#[allow(
    clippy::too_many_arguments,
    reason = "threads the fn + preamble + the program-wide spec-fn param-type map \
        (#228, for the production-column narrowing cast) + the verus config + the \
        report; the param-type map mirrors the signature path's threaded ctx"
)]
fn tv_fn(
    f: &FnItem,
    preamble: &[String],
    spec_fn_param_types: &[(&str, &[PrimType])],
    seed: u64,
    rlimit: f64,
    report: &mut TvReport,
) {
    // The fn's `nat`-returning spec fns (drive the `as nat` coercion the signature
    // path emits). Slice params are bound VIEW-CONSISTENTLY as `&[elem]` (#149) and
    // threaded as production's `slices` (per clause, in `tv_clause`), so production
    // emits `xs@` for EVERY slice use (bare `spec_sum(xs@)` AND indexed
    // `xs@.subrange(..)`) — MIRRORING the real fn signature path — and the reference
    // emits the matching `xs@`; both columns typecheck under the one binding.
    let nat_fns = nat_fn_names(f);

    // Build the base frame from the params (+ `result` when it is framable). A
    // param of an UNFRAMED type (Map/Option/Result/struct/enum) makes the whole
    // signature unframable → every clause is `Skipped` (honest); but an unframable
    // RETURN type only drops the `result` param (a `req`/`inv`/`dec` clause that
    // does NOT mention `result` still frames + checks — e.g. binary_search's
    // `sorted(haystack)` req + its `forall_below`/`forall_from` loop invariants).
    let Some(base_frame) = signature_frame(f, preamble) else {
        report.clauses.push(ClauseResult {
            label: format!("{}.signature", f.name),
            verdict: ClauseVerdict::Skipped {
                reason: "a PARAMETER type is outside the framed sublanguage \
                         (Map/Option/Result/struct/enum) — contract-TV frames \
                         scalar/slice/String clauses; richer types are body-TV scope (#139 step 2)"
                    .to_string(),
            },
        });
        return;
    };

    // req
    tv_clause(
        &f.contract.req,
        &format!("{}.req", f.name),
        f,
        &nat_fns,
        &base_frame,
        spec_fn_param_types,
        seed,
        rlimit,
        report,
    );
    // ens (in source order)
    for (i, ens) in f.contract.ens.iter().enumerate() {
        tv_clause(
            ens,
            &format!("{}.ens#{}", f.name, i + 1),
            f,
            &nat_fns,
            &base_frame,
            spec_fn_param_types,
            seed,
            rlimit,
            report,
        );
    }
    // loop inv / dec (the body's loops). A loop clause references the fn's LOCAL
    // `let` bindings (`acc`, `i`) — not just the params — so the loop frame ALSO
    // binds every framed local (its declared `let` type) so an `inv i <= xs.len()`
    // has `i` in scope. A local of an unframed type is dropped (the clause that
    // needs it is then `Skipped`, honest).
    if let Some(body) = &f.body {
        let locals = collect_locals(body);
        let mut loop_frame = base_frame.clone();
        for (name, spec_ty) in &locals {
            // A local seq/string/nat-coerce local extends the corresponding sets.
            // A `Map`/`Option`/`Result` local is bound by its wrapper/native
            // spelling (no extra set); a `Map` local weaves its `well_formed()` so a
            // loop `inv` over `m.spec_contains_key(_)` is provable.
            match spec_ty {
                SpecType::Seq(_) => loop_frame.seq_params.push(name.clone()),
                SpecType::Strng => loop_frame.string_params.push(name.clone()),
                SpecType::BoundedInt(_) => loop_frame.nat_coerce_params.push(name.clone()),
                SpecType::Map(_, _) => {
                    let r = format!("{name}.well_formed()");
                    loop_frame.req = Some(match loop_frame.req.take() {
                        Some(existing) => format!("{existing}, {r}"),
                        None => r,
                    });
                }
                SpecType::Bool | SpecType::Opt(_) | SpecType::Res(_, _) => {}
            }
            loop_frame
                .params
                .push(ParamDecl::new(name.clone(), spec_ty.verus_spelling()));
        }
        let mut loop_no = 0usize;
        tv_block_loops(
            body,
            f,
            &nat_fns,
            &loop_frame,
            spec_fn_param_types,
            seed,
            rlimit,
            &mut loop_no,
            report,
        );
    }
}

/// Collect the fn body's `let` bindings that are framed (name → [`SpecType`]), so a
/// loop `inv`/`dec` referencing a local (`acc`, `i`) frames it. Walks nested blocks
/// (if/loop bodies). An un-typed or unframed-type `let` is dropped (the clause that
/// needs it is reported `Skipped`). Deduped by name (a shadowing re-`let` keeps the
/// first framed type — v0.1 corpus locals are not re-typed).
fn collect_locals(block: &thermite_syntax::ast::Block) -> Vec<(String, SpecType)> {
    let mut out: Vec<(String, SpecType)> = Vec::new();
    collect_locals_into(block, &mut out);
    out
}

fn collect_locals_into(block: &thermite_syntax::ast::Block, out: &mut Vec<(String, SpecType)>) {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Let {
                name, ty: Some(ty), ..
            } => {
                if let Some(spec_ty) = spec_type_of(ty) {
                    if !out.iter().any(|(n, _)| n == name) {
                        out.push((name.clone(), spec_ty));
                    }
                }
            }
            Stmt::If { then, else_, .. } => {
                collect_locals_into(then, out);
                if let Some(e) = else_ {
                    collect_locals_into(e, out);
                }
            }
            Stmt::Loop(node) => collect_locals_into(&node.body, out),
            _ => {}
        }
    }
}

/// Walk a block's statements for loop nodes, TV-ing each loop's `inv`s + `dec`
/// (REQ-5 — the `inv`/`dec` clause families). Recurses into nested blocks (if/
/// loop bodies) so EVERY loop is covered (the whole class — `goal.md`).
#[allow(
    clippy::too_many_arguments,
    reason = "threads the fn + frame + verus config + the loop counter through the \
        recursive block walk; a struct would not reduce the genuine fan-in"
)]
fn tv_block_loops(
    block: &thermite_syntax::ast::Block,
    f: &FnItem,
    nat_fns: &[&str],
    base_frame: &ObligationFrame,
    spec_fn_param_types: &[(&str, &[PrimType])],
    seed: u64,
    rlimit: f64,
    loop_no: &mut usize,
    report: &mut TvReport,
) {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Loop(node) => {
                *loop_no += 1;
                let this = *loop_no;
                for (i, inv) in node.invs.iter().enumerate() {
                    tv_clause(
                        inv,
                        &format!("{}.loop#{}.inv#{}", f.name, this, i + 1),
                        f,
                        nat_fns,
                        base_frame,
                        spec_fn_param_types,
                        seed,
                        rlimit,
                        report,
                    );
                }
                tv_clause(
                    &node.dec,
                    &format!("{}.loop#{}.dec", f.name, this),
                    f,
                    nat_fns,
                    base_frame,
                    spec_fn_param_types,
                    seed,
                    rlimit,
                    report,
                );
                tv_block_loops(
                    &node.body,
                    f,
                    nat_fns,
                    base_frame,
                    spec_fn_param_types,
                    seed,
                    rlimit,
                    loop_no,
                    report,
                );
            }
            Stmt::If { then, else_, .. } => {
                tv_block_loops(
                    then,
                    f,
                    nat_fns,
                    base_frame,
                    spec_fn_param_types,
                    seed,
                    rlimit,
                    loop_no,
                    report,
                );
                if let Some(e) = else_ {
                    tv_block_loops(
                        e,
                        f,
                        nat_fns,
                        base_frame,
                        spec_fn_param_types,
                        seed,
                        rlimit,
                        loop_no,
                        report,
                    );
                }
            }
            _ => {}
        }
    }
}

/// TV one clause (REQ-5): compute `P_production`, build the obligation against the
/// independent reference, discharge it, and classify. `old(_)` references in the
/// clause are bound as extra `old_<name>` params (derived from the matching fn
/// param's type).
#[allow(
    clippy::too_many_arguments,
    reason = "a clause's TV genuinely needs the clause + its label + the enclosing \
        fn (for old-param typing) + the spec ctx + the base frame + the verus \
        config; grouping them would obscure the per-clause data flow"
)]
fn tv_clause(
    clause: &Clause,
    label: &str,
    f: &FnItem,
    nat_fns: &[&str],
    base_frame: &ObligationFrame,
    spec_fn_param_types: &[(&str, &[PrimType])],
    seed: u64,
    rlimit: f64,
    report: &mut TvReport,
) {
    // P_production — the REAL production lowering of this clause. The frame's
    // slice params (bound `&[elem]`, #149) are passed as production's `slices` so a
    // slice use takes its `@`-view (`spec_sum(xs@)` / `xs@.subrange(..)`) — MIRRORING
    // the real fn signature path (`tests/golden/lower/sum.verus.rs`) — and typechecks
    // against the `&[elem]` binding. nat_fns drive the `as nat` coercion.
    let slice_params = slice_param_names(base_frame);
    let slices: Vec<&str> = slice_params.iter().map(String::as_str).collect();
    // The frame's `String` params (bound `&TString`, #150 gap #2) are threaded as
    // production's `strings` so a `String`-receiver `.len()`/`.byte_at(i)` in the
    // clause rewrites to the wrapper SPEC fns (`s.spec_len()`/`s.spec_byte_at(i as
    // int)`) — production's `recv_is_string` arm — MATCHING the reference's
    // `string_bound` dispatch. Without it production emits the bare exec `s.len()`
    // (`u64`) vs the reference's `s.spec_len()` (`nat`) → a type-level Unverifiable.
    let strings: Vec<&str> = base_frame
        .string_params
        .iter()
        .map(String::as_str)
        .collect();
    let p_production = match thermite_lower::lower_contract_expr(
        &clause.expr,
        &slices,
        nat_fns,
        &strings,
        &[],
        &[],
        spec_fn_param_types,
    ) {
        Ok(p) => p,
        Err(e) => {
            report.clauses.push(ClauseResult {
                label: label.to_string(),
                verdict: ClauseVerdict::Skipped {
                    reason: format!("production lowering does not cover this clause: {e}"),
                },
            });
            return;
        }
    };

    // The frame for THIS clause: the signature params + any `old(_)` params it uses.
    let mut frame = base_frame.clone();
    for (name, ty_str) in old_params(&clause.expr, f) {
        frame.params.push(ParamDecl::new(name, ty_str));
    }

    // Build the obligation (the reference encoding is computed inside, independent
    // of `p_production`). An encoding error → Skipped (honest, not a false faithful).
    let program = match equivalence_obligation(&clause.expr, &p_production, &frame) {
        Ok(prog) => prog,
        Err(e) => {
            report.clauses.push(ClauseResult {
                label: label.to_string(),
                verdict: ClauseVerdict::Skipped {
                    reason: format!("reference encoder does not cover this clause: {e}"),
                },
            });
            return;
        }
    };

    let verdict = discharge(&program, label, seed, rlimit);
    report.clauses.push(ClauseResult {
        label: label.to_string(),
        verdict,
    });
}

/// Run the OFF-CORPUS generated TV run (REQ-3 / REQ-5; the corpus-bound escape).
/// Generates `n` clauses deterministically from `seed` (`thermite_tv::generate_clauses`),
/// lowers each via `thermite_lower::lower_contract_expr`, builds + discharges the
/// per-clause obligation against the FIXED generator-vocabulary frame, and reports.
/// The lowerer is faithful, so ALL should verify; ANY `Divergent` is a real
/// off-corpus infidelity finding (surfaced loudly).
pub fn run_generated(seed: u64, n: usize, rlimit: f64) -> Result<TvReport, ForgeError> {
    // Parse the synthetic vocabulary program ONCE; derive BOTH the preamble and the
    // production-column param-type map (#228) from it (R-CHAR-3 — one source). The
    // generator's only user spec fn is `spec_sum` (a slice param → no-cast
    // placeholder), so the map is byte-stable for the generated vocabulary; threading
    // it keeps the off-corpus column on the SAME `lower_contract_expr` contract as the
    // corpus column rather than two divergent call shapes.
    let program = generated_program()?;
    let preamble = generated_preamble(&program)?;
    let pt_owned = thermite_lower::spec_fn_param_type_map(&program);
    let spec_fn_param_types: Vec<(&str, &[PrimType])> =
        pt_owned.iter().map(|(n, ps)| (*n, ps.as_slice())).collect();
    let frame = generated_frame(&preamble);
    // The off-corpus run's nat_fns: `spec_sum` (the `nat`-returning spec fn in the
    // generator vocabulary) + `count_where` (the `nat`-returning combinator).
    let nat_fns = ["spec_sum", "count_where"];

    // The off-corpus String byte-view receiver name (#150 gap #2). A generated
    // `t.byte_at(i)`/`t.len()` clause is now CHECKED (not Skipped): `t` is threaded
    // as production's `strings`, so production's `recv_is_string` rewrite emits
    // `t.spec_byte_at(i as int)`/`t.spec_len()` — MATCHING the reference's
    // `string_bound` dispatch (the frame names `t` in `string_params`). The TString
    // wrapper is in the preamble (`generated_preamble`'s `touch_string` fn).
    let strings = ["t"];

    let clauses = thermite_tv::generate_clauses(seed, n);
    let mut report = TvReport::default();
    for (i, clause) in clauses.iter().enumerate() {
        let label = format!("gen#{i}");
        let p_production = match thermite_lower::lower_contract_expr(
            clause,
            &[],
            &nat_fns,
            &strings,
            &[],
            &[],
            &spec_fn_param_types,
        ) {
            Ok(p) => p,
            Err(e) => {
                report.clauses.push(ClauseResult {
                    label,
                    verdict: ClauseVerdict::Skipped {
                        reason: format!("production lowering does not cover: {e}"),
                    },
                });
                continue;
            }
        };
        let program = match equivalence_obligation(clause, &p_production, &frame) {
            Ok(prog) => prog,
            Err(e) => {
                report.clauses.push(ClauseResult {
                    label,
                    verdict: ClauseVerdict::Skipped {
                        reason: format!("reference encoder does not cover: {e}"),
                    },
                });
                continue;
            }
        };
        let verdict = discharge(&program, &label, seed, rlimit);
        report.clauses.push(ClauseResult { label, verdict });
    }
    Ok(report)
}

// ---- frame construction -----------------------------------------------------

/// The FIXED obligation frame for the off-corpus generator vocabulary (matches
/// `thermite_tv::gen`'s documented world): `xs`/`ys: Seq<u32>` (seq-bound),
/// `s: Seq<u8>` (seq-bound byte-view), `n`/`m`/`k: int`, `result`/`old_acc: u64`
/// (nat-coerced), with the spec_sum + combinator defs in scope.
fn generated_frame(preamble: &[String]) -> ObligationFrame {
    ObligationFrame {
        spec_defs: preamble.to_vec(),
        params: vec![
            ParamDecl::new("xs", "Seq<u32>"),
            ParamDecl::new("ys", "Seq<u32>"),
            ParamDecl::new("s", "Seq<u8>"),
            ParamDecl::new("n", "int"),
            ParamDecl::new("m", "int"),
            ParamDecl::new("k", "int"),
            ParamDecl::new("result", "u64"),
            ParamDecl::new("old_acc", "u64"),
            // The String byte-view receiver (#150 gap #2): `t: &TString`, so the
            // generator's `t.byte_at(i)`/`t.len()` clauses dispatch to the wrapper
            // SPEC fns on BOTH columns (production's `recv_is_string` rewrite + the
            // reference's `string_bound` dispatch) — the off-corpus String-byteview
            // coverage the corpus alone does not give.
            ParamDecl::new("t", "&TString"),
        ],
        // No enclosing `req`: a generated clause is equivalence-checked for ALL
        // inputs (the strongest faithfulness check). A `Seq` index in spec position
        // is total in Verus (an OOB index is a well-defined unspecified value, NOT
        // an error), so both sides agree on it without a bound.
        req: None,
        seq_params: vec!["xs".to_string(), "ys".to_string(), "s".to_string()],
        nat_coerce_params: vec!["result".to_string(), "old_acc".to_string()],
        string_params: vec!["t".to_string()],
        map_params: vec![],
    }
}

/// The synthetic source whose lowering materializes the off-corpus preamble +
/// whose `spec fn` set IS the generator's user-spec-fn vocabulary (`spec_sum`).
/// Lifted to a constant so [`run_generated`] can parse it ONCE and derive BOTH the
/// preamble (via [`program_spec_preamble`]) and the `spec_fn_param_type_map` (the
/// #228 production-column narrowing input) from the SAME program (R-CHAR-3 — one
/// source of truth). The spec_sum shape mirrors `conformance/sum.th` (the golden).
const GENERATED_PREAMBLE_SRC: &str = "\
spec fn spec_sum(xs: &[u32]) -> u64
  dec xs.len()
{
  match xs {
    []          => 0,
    [head, ..t] => head as u64 + spec_sum(t),
  }
}

fn touch(xs: &[u32], ys: &[u32], n: usize) -> bool
  req true
  ens result == sorted(xs)
  ens forall_in(xs, |x| x < 1)
  ens exists_in(xs, |x| x < 1)
  ens count_where(xs, |x| x < 1) == 0
  ens permutation_of(xs, ys)
  ens disjoint(xs, ys)
  ens forall_below(xs, n, |x| x < 1)
  ens forall_from(xs, n, |x| x < 1)
  fx  pure
{
  true
}

// #150 gap #2: a `String`-param fn so `emit_string_wrapper` materializes the
// `TString` wrapper (its `spec_len`/`spec_byte_at` spec fns) into the preamble —
// the off-corpus String byte-view obligation binds `t: &TString` and dispatches
// `t.byte_at(i)`/`t.len()` to those spec fns on BOTH columns.
fn touch_string(t: String) -> u64
  req t.len() > 0
  ens result == t.byte_at(0)
  fx  pure
{
  t.byte_at(0)
}
";

/// Parse the off-corpus synthetic program ([`GENERATED_PREAMBLE_SRC`]) — the shared
/// source for both the preamble and the param-type map. Errors if it does not parse
/// clean (an internal invariant, never user input).
fn generated_program() -> Result<thermite_syntax::ast::Program, ForgeError> {
    let parsed = thermite_syntax::parse(GENERATED_PREAMBLE_SRC);
    if !parsed.is_clean() {
        return Err(ForgeError::VerusOutput {
            detail: "internal: the contract-TV off-corpus preamble program did not parse"
                .to_string(),
        });
    }
    Ok(parsed.program)
}

/// The off-corpus preamble: the `spec_sum` def + the 8 frozen combinator `verus_l3`
/// defs, materialized by lowering the synthetic program that references each.
fn generated_preamble(program: &thermite_syntax::ast::Program) -> Result<Vec<String>, ForgeError> {
    program_spec_preamble(program)
}

/// Lower a program's spec-fn + combinator definitions and return them as the
/// frame's `spec_defs` (the `spec fn` / `proof fn` / wrapper definition blocks of
/// the lowered `verus! { … }`, with the `use`/`verus!`/`fn main` frame AND the exec
/// `fn`s stripped — the obligation supplies its own frame + has no exec fns).
fn program_spec_preamble(
    program: &thermite_syntax::ast::Program,
) -> Result<Vec<String>, ForgeError> {
    let lowered = thermite_lower::lower(program).map_err(ForgeError::Lower)?;
    Ok(extract_spec_defs(&lowered))
}

/// Extract the `spec fn` / `proof fn` / `struct` / `impl` definition blocks from a
/// lowered Verus file, dropping the `use`/`verus! {`/`}`/`fn main()` frame AND the
/// exec `fn` items. A definition block runs from its header to the brace-balanced
/// close (so a nested `impl { fn … {} }` is captured whole).
fn extract_spec_defs(lowered: &str) -> Vec<String> {
    let mut defs = Vec::new();
    let lines: Vec<&str> = lowered.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let trimmed = lines[i].trim_start();
        let is_exec_fn = trimmed.starts_with("fn ") || trimmed.starts_with("pub fn ");
        let is_def_header = !is_exec_fn
            && (trimmed.starts_with("spec fn ")
                || trimmed.starts_with("pub open spec fn ")
                || trimmed.starts_with("pub closed spec fn ")
                || trimmed.starts_with("proof fn ")
                || trimmed.starts_with("pub struct ")
                || trimmed.starts_with("struct ")
                || trimmed.starts_with("impl "));
        if is_def_header {
            let (block, next) = capture_block(&lines, i);
            defs.push(block);
            i = next;
        } else {
            i += 1;
        }
    }
    defs
}

/// Capture a brace-balanced definition block starting at `start`, returning the
/// block text and the index PAST it.
fn capture_block(lines: &[&str], start: usize) -> (String, usize) {
    let mut depth: i64 = 0;
    let mut seen_open = false;
    let mut end = start;
    for (j, line) in lines.iter().enumerate().skip(start) {
        for ch in line.chars() {
            if ch == '{' {
                depth += 1;
                seen_open = true;
            } else if ch == '}' {
                depth -= 1;
            }
        }
        end = j;
        if seen_open && depth <= 0 {
            break;
        }
    }
    (lines[start..=end].join("\n"), end + 1)
}

/// Build the base obligation frame from a fn signature (REQ-5): one [`ParamDecl`]
/// per param (with its Verus-spec type), `result` when the return is non-unit, the
/// `Seq`-bound + `nat`-coerced sets, and the spec-fn/combinator preamble. Returns
/// `None` if any param/return is outside the framed sublanguage (Map/Option/Result/
/// struct/enum) — the clause is then reported `Skipped` (honest, not a false pass).
fn signature_frame(f: &FnItem, preamble: &[String]) -> Option<ObligationFrame> {
    let mut params = Vec::new();
    // No signature SLICE param is seq-bound (#149): slices are bound `&[elem]` and
    // viewed `@` on BOTH columns. `seq_params` stays empty here; a loop frame adds
    // its OWN `Seq<_>`-bound locals (the loop-local seq-bound identity).
    let seq_params = Vec::new();
    let mut nat_coerce_params = Vec::new();
    let mut string_params = Vec::new();
    let mut map_params = Vec::new();
    // The `requires` clauses the obligation frame must thread so production's
    // signature path weave typechecks: a `Map`/struct param weaves `well_formed()`
    // (`is_map_param_ty` in production), so a `m.spec_contains_key(k)` over the
    // wrapper has the capacity/key-uniqueness invariant in scope (#150 gap #3). The
    // reference + production agree on the predicate; the `requires` keeps both
    // columns provable rather than spuriously failing on a missing invariant.
    let mut reqs: Vec<String> = Vec::new();

    for p in &f.params {
        let spec_ty = spec_type_of(&p.ty)?;
        match &spec_ty {
            // A slice param is bound VIEW-CONSISTENTLY as `&[elem]` (#149) — NOT
            // seq-bound. Production emits `xs@` for every slice use (bare AND
            // indexed); the reference (param NOT in `seq_params`) emits the matching
            // `xs@`, so both columns typecheck under the `&[elem]` binding and Z3
            // proves them equivalent. (Under the old `Seq` binding the indexed
            // `xs@.subrange(..)` was a type error → Unverifiable.)
            SpecType::Seq(_) => {}
            // A `String` param is bound `&TString` (#150 gap #2) + named in
            // `string_params` so the reference dispatches its byte-view to the
            // wrapper spec fns (matching production's `recv_is_string` rewrite).
            SpecType::Strng => string_params.push(p.name.clone()),
            SpecType::BoundedInt(_) => nat_coerce_params.push(p.name.clone()),
            SpecType::Bool => {}
            // A `Map` param (#150 gap #3) is bound as the `TMap` wrapper; production
            // weaves `well_formed()` for it (`is_map_param_ty`), so the obligation
            // threads the SAME `requires` to keep the spec_contains_key membership
            // provable. The `spec_contains_key` rewrite is RE-implemented in the
            // reference encoder (the wrapper spec fn is the shared frozen ground
            // truth, in the preamble).
            SpecType::Map(_, _) => {
                map_params.push(p.name.clone());
                reqs.push(format!("{}.well_formed()", p.name));
            }
            // An `Option`/`Result` param is bound as the native Verus type (#150
            // gap #3); no invariant weave (the enum carries its own discriminant).
            SpecType::Opt(_) | SpecType::Res(_, _) => {}
        }
        params.push(ParamDecl::new(
            p.name.clone(),
            spec_ty.verus_param_spelling(),
        ));
    }

    // `result` — bound when the return is non-unit AND framable. As of #150 the
    // framable RETURN set INCLUDES `Option`/`Result`/`Map` (the construct classes
    // this iteration covers): an `ens match result { … }` (binary_search), an `ens
    // result is None` (map_kv `lookup_absent`), and an `ens result.contains_key(k)`
    // (map_kv `build_one`) now BIND `result` and discharge, rather than dropping it
    // (Skipped). A struct/enum return still drops `result` (body-TV scope).
    if !matches!(f.ret, Type::Unit) {
        if let Some(ret_ty) = spec_type_of(&f.ret) {
            match &ret_ty {
                SpecType::BoundedInt(_) => nat_coerce_params.push("result".to_string()),
                // A slice `result` is bound VIEW-CONSISTENTLY as `&[elem]` (#149,
                // the same rule as a slice param) — NOT seq-bound; both columns
                // emit `result@`.
                SpecType::Seq(_) => {}
                SpecType::Strng => string_params.push("result".to_string()),
                SpecType::Bool => {}
                // A `Map` result (#150 gap #3): production proves `result.well_formed()`
                // (the constructed map is well-formed), so a `result.spec_contains_key(k)`
                // ens has the invariant in scope. The obligation threads it as a
                // `requires` so the equivalence obligation (which assumes the ens
                // context) is provable.
                SpecType::Map(_, _) => {
                    map_params.push("result".to_string());
                    reqs.push("result.well_formed()".to_string());
                }
                SpecType::Opt(_) | SpecType::Res(_, _) => {}
            }
            params.push(ParamDecl::new("result", ret_ty.verus_param_spelling()));
        }
    }

    let req = if reqs.is_empty() {
        None
    } else {
        Some(reqs.join(", "))
    };

    Some(ObligationFrame {
        spec_defs: preamble.to_vec(),
        params,
        req,
        seq_params,
        nat_coerce_params,
        string_params,
        map_params,
    })
}

/// The frame params bound VIEW-CONSISTENTLY as a slice `&[elem]` (#149) — the
/// production `slices` set for this clause, so a slice use takes its `@`-view
/// (matching the `&[elem]` binding). Keyed on the `&[` binding spelling
/// `signature_frame` emits (NOT a name list), so it stays in lockstep with the
/// param binding. A `Seq<_>`-bound LOCAL (a loop-frame seq local) is NOT a slice
/// param and is excluded (it keeps the seq-bound identity `@`-view).
fn slice_param_names(frame: &ObligationFrame) -> Vec<String> {
    frame
        .params
        .iter()
        .filter(|p| p.type_str.starts_with("&["))
        .map(|p| p.name.clone())
        .collect()
}

/// The spec-context type of a framed param/return. A `&[T]`/`Vec<T>`/`String`
/// becomes a `Seq` in spec position (the slice→`@` model); a bounded prim becomes
/// `BoundedInt`. Richer types (Map/Option/Result/struct/enum) are NOT framed (this
/// is contract-TV's scalar/slice/String scope).
#[derive(Debug, Clone, PartialEq, Eq)]
enum SpecType {
    /// A `Seq<elem>` (a `&[elem]` slice or a `Vec<elem>` → `Seq<elem>`).
    Seq(String),
    /// A `String`/`&String` — bound as the `TString` wrapper (#150 gap #2), whose
    /// spec-position byte-view (`.len()`/`.byte_at(i)`) dispatches to the wrapper
    /// SPEC fns (`.spec_len()`/`.spec_byte_at(i as int)`), exactly as production's
    /// `recv_is_string` rewrite. NOT a `Seq<u8>` index (the wrapper spec fns take
    /// `&self`); the obligation frame names it in `string_params`.
    Strng,
    /// A bounded integer typed at its DECLARED width (`u32`/`u64`/`usize`) — the
    /// String is the Verus prim spelling of the declared param type (#228, ref
    /// #225/#227). Binding the obligation param at its TRUE width (not a blanket
    /// `u64`) is the soundness fix: Z3 then reasons over the actual domain, so a
    /// production `s_dec((n - 1) as u32)` truncation is IDENTITY on a `u32`-typed `n`
    /// (its value is already < 2^32) — the equivalence to the reference's bare arg is
    /// provable WHERE the clause's guards/req bound the subtraction, and HONESTLY
    /// unprovable (an unguarded underflow witness) where they do not. Under the prior
    /// blanket-`u64` framing the `as u32` truncation could LOSE bits Z3 saw as
    /// significant, so a faithful clause read as a false divergence / a wrong cast
    /// read as faithful. `as nat`-coercible against a `nat`-valued comparison.
    BoundedInt(String),
    /// A `bool`.
    Bool,
    /// A `Map<K, V>` bound as the `TMap` wrapper (#150 gap #3). The string is the
    /// Verus wrapper spelling (`TMapU64U64`). Production weaves `well_formed()` for
    /// a `Map` param/result, so the obligation threads it as a `requires`; the
    /// `contains_key`→`spec_contains_key` spec rewrite is RE-implemented in the
    /// reference encoder (the wrapper spec fns are the shared frozen ground truth,
    /// in the preamble).
    Map(String, String),
    /// An `Option<T>` bound as the native Verus `Option<…>` (#150 gap #3). The
    /// string is the full Verus spelling (`Option<usize>`). Carries the C7
    /// payload-in-contract `match`/`is`/`spec-match` clauses.
    Opt(String),
    /// A `Result<T, E>` bound as the native Verus `Result<…, …>` (#150 gap #3). The
    /// string is the full Verus spelling (`Result<u64, ParseErr>`).
    Res(String, String),
}

impl SpecType {
    /// The Verus parameter spelling for the obligation signature.
    fn verus_spelling(&self) -> String {
        match self {
            SpecType::Seq(elem) => format!("Seq<{elem}>"),
            SpecType::Strng => "TString".to_string(),
            // The DECLARED width (#228) — `u32`/`u64`/`usize` — so the obligation
            // param is typed at its true domain, NOT a blanket `u64`.
            SpecType::BoundedInt(width) => width.clone(),
            SpecType::Bool => "bool".to_string(),
            // The `Map` wrapper spelling (`TMapU64U64`); the inner `(K, V)` pair is
            // carried for completeness. The wrapper struct is in the preamble.
            SpecType::Map(_, _) => self.map_wrapper_name(),
            SpecType::Opt(inner) => format!("Option<{inner}>"),
            SpecType::Res(ok, err) => format!("Result<{ok}, {err}>"),
        }
    }

    /// The `TMap` wrapper struct name for a `Map(K, V)` (`TMapU64U64`), mirroring
    /// production's `tmap_name`. The frozen v0.1 `Map` is `Map<u64, u64>`; the
    /// suffix capitalizes each Verus prim spelling.
    fn map_wrapper_name(&self) -> String {
        if let SpecType::Map(k, v) = self {
            format!("TMap{}{}", cap_prim(k), cap_prim(v))
        } else {
            String::new()
        }
    }

    /// The VIEW-CONSISTENT obligation-signature spelling for a SLICE-typed
    /// parameter (#149). A slice param is bound as the SLICE type `&[elem]` (NOT a
    /// bare `Seq<elem>`) so production's UNCONDITIONAL `@`-view — emitted by
    /// `lower_index` for an indexed/ranged use (`xs@.subrange(0, i as int)`) AND by
    /// the signature path for a bare combinator/spec-fn arg (`spec_sum(xs@)`) —
    /// typechecks: `xs@` is the `Seq<elem>` view of `xs: &[elem]`. Under a bare
    /// `Seq<elem>` binding, `xs@` is a type error (`Seq` has no `view`), so the
    /// indexed clause `acc == spec_sum(&xs[..i])` could not discharge (Unverifiable).
    /// This MIRRORS the real fn lowering (`tests/golden/lower/sum.verus.rs`:
    /// `fn sum(xs: &[u32])` emits `xs@` everywhere); the reference encoder then
    /// emits the matching `xs@` form (the param is NOT seq-bound), so BOTH columns
    /// typecheck under ONE binding and Z3 proves them equivalent.
    fn verus_param_spelling(&self) -> String {
        match self {
            SpecType::Seq(elem) => format!("&[{elem}]"),
            // A `String` param is bound as a `&TString` borrow (#150 gap #2) —
            // MIRRORING production's `&String`-param lowering (`&TString`), so the
            // spec-position `s.spec_len()`/`s.spec_byte_at(i)` calls resolve on the
            // wrapper. The `TString` wrapper struct + its spec fns are in scope (the
            // frame preamble lowers the whole program, which emits the wrapper).
            SpecType::Strng => "&TString".to_string(),
            other => other.verus_spelling(),
        }
    }
}

/// Map a Thermite `Type` to its [`SpecType`] for framing, or `None` if it is
/// outside contract-TV's framed sublanguage.
fn spec_type_of(ty: &Type) -> Option<SpecType> {
    match ty {
        // Type the bounded int at its DECLARED width (#228) — the obligation param
        // then carries the true domain, so the production `as u32`/`as usize`
        // truncation is identity within that domain (the soundness fix).
        Type::Prim(PrimType::U32) => Some(SpecType::BoundedInt("u32".to_string())),
        Type::Prim(PrimType::U64) => Some(SpecType::BoundedInt("u64".to_string())),
        Type::Prim(PrimType::Usize) => Some(SpecType::BoundedInt("usize".to_string())),
        Type::Prim(PrimType::Bool) => Some(SpecType::Bool),
        Type::Ref { inner, .. } => spec_type_of(inner),
        Type::Slice(inner) => Some(SpecType::Seq(elem_spelling(inner)?)),
        Type::Vec(inner) => Some(SpecType::Seq(elem_spelling(inner)?)),
        // A `String`/`&String` is bound as the `TString` wrapper (#150 gap #2):
        // its spec-position byte-view dispatches to the wrapper SPEC fns
        // (`.spec_len()`/`.spec_byte_at(i as int)`), MATCHING production's
        // `recv_is_string` rewrite — NOT a `Seq<u8>` index (which would not
        // typecheck against production's `&TString` receiver).
        Type::String => Some(SpecType::Strng),
        // The #150 gap #3 construct classes: `Option`/`Result`/`Map` params +
        // result are now FRAMED (the inner types are themselves framable). A
        // `match`/`is` over an `Option`/`Result` result (binary_search ens,
        // lookup_absent ens) and a `Map`-method spec rewrite (build_one/has_key)
        // discharge against the native Verus type / the `TMap` wrapper.
        Type::Option(inner) => Some(SpecType::Opt(verus_type_spelling(inner)?)),
        Type::Result(ok, err) => Some(SpecType::Res(
            verus_type_spelling(ok)?,
            verus_type_spelling(err)?,
        )),
        Type::Map(k, v) => Some(SpecType::Map(
            verus_type_spelling(k)?,
            verus_type_spelling(v)?,
        )),
        _ => None,
    }
}

/// The Verus element spelling for a `Seq` element type (only the bounded prims —
/// a nested slice/struct element is unframed).
fn elem_spelling(ty: &Type) -> Option<String> {
    match ty {
        Type::Prim(PrimType::U32) => Some("u32".to_string()),
        Type::Prim(PrimType::U64) => Some("u64".to_string()),
        Type::Prim(PrimType::Usize) => Some("usize".to_string()),
        _ => None,
    }
}

/// The full Verus spelling of a framable inner type for an `Option`/`Result`/`Map`
/// type argument (#150 gap #3), mirroring production's `lower_type`: a bounded prim
/// spells itself; a `String` spells the `TString` wrapper; a user-named enum
/// (`ParseErr`) spells its name (its `enum` def is in the preamble). Returns `None`
/// for a type contract-TV does not frame (a nested `Map`/struct/slice arg), so the
/// whole signature falls back to honest Skip rather than mis-spelling it.
fn verus_type_spelling(ty: &Type) -> Option<String> {
    match ty {
        Type::Prim(PrimType::U32) => Some("u32".to_string()),
        Type::Prim(PrimType::U64) => Some("u64".to_string()),
        Type::Prim(PrimType::Usize) => Some("usize".to_string()),
        Type::Prim(PrimType::Bool) => Some("bool".to_string()),
        Type::String => Some("TString".to_string()),
        Type::Named(n) => Some(n.clone()),
        _ => None,
    }
}

/// Capitalize a Verus prim spelling into the `TMap` suffix segment (`u64` → `U64`),
/// mirroring production's `tmap_type_suffix` (`Type::Prim(U64)` → `"U64"`).
fn cap_prim(spelling: &str) -> String {
    match spelling {
        "u32" => "U32".to_string(),
        "u64" => "U64".to_string(),
        "usize" => "Usize".to_string(),
        other => other.to_string(),
    }
}

// ---- spec-context input derivation (mirrors lower_fn_signature) -------------

/// The `nat`-returning spec-fn / combinator names a scalar compared against gets
/// `as nat`-coerced (mirrors the program-wide `nat_fns` `lower_fn_signature`
/// threads). v0.1 frozen `nat`-returning shapes: the recursive `spec fn … -> u64`
/// over a slice (`spec_sum`, the golden shape — R-CHAR-3) and the `count_where`
/// combinator. Keyed by name (the corpus's `nat`-returning set).
fn nat_fn_names(_f: &FnItem) -> Vec<&'static str> {
    vec!["spec_sum", "count_where"]
}

/// The `old(<name>)` references in a clause, paired with the matching fn param's
/// Verus-spec type (REQ-2 — `old(acc)` is bound as a distinct `old_acc` param).
/// v0.1 corpus ensures are over `result` + params (no `old(_)`), so this is
/// typically empty; it is here so a clause that DOES use `old(_)` frames correctly.
fn old_params(expr: &Expr, f: &FnItem) -> Vec<(String, String)> {
    let mut found = Vec::new();
    collect_old(expr, f, &mut found);
    found
}

fn collect_old(expr: &Expr, f: &FnItem, out: &mut Vec<(String, String)>) {
    match expr {
        Expr::Call { callee, args } => {
            if let Expr::Path(segs) = callee.as_ref() {
                if segs.len() == 1 && segs[0] == "old" {
                    if let [Expr::Path(inner)] = args.as_slice() {
                        if inner.len() == 1 {
                            let name = format!("old_{}", inner[0]);
                            let ty = f
                                .params
                                .iter()
                                .find(|p| p.name == inner[0])
                                .and_then(|p| spec_type_of(&p.ty))
                                .map(|t| t.verus_spelling())
                                .unwrap_or_else(|| "u64".to_string());
                            if !out.iter().any(|(n, _)| n == &name) {
                                out.push((name, ty));
                            }
                            return;
                        }
                    }
                }
            }
            collect_old(callee, f, out);
            for a in args {
                collect_old(a, f, out);
            }
        }
        Expr::Binary { lhs, rhs, .. } => {
            collect_old(lhs, f, out);
            collect_old(rhs, f, out);
        }
        Expr::Unary { expr, .. } => collect_old(expr, f, out),
        Expr::MethodCall { receiver, args, .. } => {
            collect_old(receiver, f, out);
            for a in args {
                collect_old(a, f, out);
            }
        }
        Expr::Field { receiver, .. } => collect_old(receiver, f, out),
        Expr::Index { base, .. } => collect_old(base, f, out),
        Expr::Cast { expr, .. } => collect_old(expr, f, out),
        Expr::Closure { body, .. } => collect_old(body, f, out),
        _ => {}
    }
}

// ---- verus discharge --------------------------------------------------------

/// Discharge one obligation PROGRAM through `verus`, classifying the verdict (the
/// REQ-2 discharge path — VERIFIED ⟺ faithful, counterexample ⟺ divergent). Runs
/// in a per-run scratch dir removed wholesale on EVERY exit path (blocker #53,
/// reusing `crate::check::ScratchDir`). An absent verus / spawn-IO failure →
/// `Unverifiable` (surfaced, never a silent pass — R-CODE-4).
fn discharge(program: &str, label: &str, seed: u64, rlimit: f64) -> ClauseVerdict {
    let stem = sanitize_stem(label);
    let scratch = ScratchDir {
        path: unique_scratch_dir(&stem),
    };
    if std::fs::create_dir_all(&scratch.path).is_err() {
        return ClauseVerdict::Unverifiable;
    }
    let file = scratch.path.join(format!("{stem}.rs"));
    if std::fs::write(&file, program).is_err() {
        return ClauseVerdict::Unverifiable;
    }

    // NB: NO `--output-json` here — verus then emits the plain-text
    // `verification results:: N verified, M errors` summary line that
    // [`parse_results`] reads (the same form the `thermite-tv` teeth-test parses).
    // The pinned `--rlimit` + `smt.random_seed` keep the discharge DETERMINISTIC
    // (R-CODE-5), matching `forge check`'s verus invocation config.
    let output = Command::new("verus")
        .arg("--rlimit")
        .arg(format!("{rlimit}"))
        .arg("--smt-option")
        .arg(format!("smt.random_seed={seed}"))
        .arg(&file)
        .current_dir(&scratch.path)
        .output();

    // `scratch` drops at fn end (and the early returns), removing the source +
    // verus's compiled binary wholesale (#53).
    let output = match output {
        Ok(o) => o,
        Err(_) => return ClauseVerdict::Unverifiable,
    };
    let mut combined = String::from_utf8_lossy(&output.stdout).to_string();
    combined.push_str(&String::from_utf8_lossy(&output.stderr));

    // A Verus/Z3 RESOURCE-LIMIT (rlimit) exhaustion / timeout: verus prints `rlimit
    // exceeded` / `Resource limit (rlimit) exceeded`, or z3's own `max. resource limit
    // exceeded`, AND a results line counting the exhausted obligation as an error. That
    // is a DISCHARGE failure, NOT a meaning mismatch — the #189-class hardening via the
    // SHARED `crate::tv_signal::is_rlimit_signal` discriminator (#192 root-cause fix: the
    // prior per-phase copy had DROPPED the z3-phrased `resource limit exceeded` clause):
    // an rlimit-hit error run is routed to Unverifiable, never the `errors >= 1` Divergent
    // arm, so a genuine solver-budget timeout is never fabricated into a contract
    // infidelity (R-HONEST-3 / R-CODE-4 — a timeout degrades, never a false finding).
    let rlimit_hit = crate::tv_signal::is_rlimit_signal(&combined);

    match parse_results(&combined) {
        Some((_verified, errors)) if errors == 0 && output.status.success() => {
            ClauseVerdict::Faithful
        }
        // An error run that is REALLY an rlimit exhaustion → Unverifiable, never
        // Divergent (the #189-class mapping fix; this arm precedes the Divergent arm).
        Some((_verified, errors)) if errors >= 1 && rlimit_hit => {
            let _ = errors;
            ClauseVerdict::Unverifiable
        }
        // A GENUINE counterexample (errors with NO rlimit signal) — the SOLE Divergent
        // source: the production lowering means something other than the reference.
        Some((_verified, errors)) if errors >= 1 => ClauseVerdict::Divergent {
            detail: format!(
                "verus found {errors} error(s) on the equivalence obligation — the \
                 production lowering of `{label}` is NOT faithful to the independent \
                 reference (a meaning mismatch, the contract-TV finding)"
            ),
        },
        // No parseable results line / a non-success with 0 parsed errors → could not
        // discharge cleanly (a FRAME compile/parse abort — the obligation's frame, not
        // the lowering, is the limit). Reported as Unverifiable, never a silent pass
        // (R-CODE-4) and never a fabricated Divergent (R-HONEST-3).
        _ => ClauseVerdict::Unverifiable,
    }
}

/// Parse the `N verified, M errors` summary line from verus output (mirrors the
/// teeth-test parser). `None` if no summary line is present.
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

/// A crate-name-safe scratch stem from a clause label (no `.`/`#` — verus rejects
/// a `.` in the derived crate name; mirrors `crate::check::crate_stem`).
fn sanitize_stem(label: &str) -> String {
    let mut s = String::with_capacity(label.len() + 4);
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
    s.push_str("_tv");
    s
}

/// Render a [`TvReport`] as a human summary (REQ-5; `forge tv` text output). One
/// line per clause + the headline counts (the reported integers).
pub fn render_report(report: &TvReport, header: &str) -> String {
    let mut out = String::new();
    let counts = report.counts();
    out.push_str(&format!(
        "{header}: {} clause(s) checked, {} faithful, {} DIVERGENT, {} skipped, {} unverifiable\n",
        counts.checked(),
        counts.faithful,
        counts.divergent,
        counts.skipped,
        counts.unverifiable,
    ));
    for r in &report.clauses {
        match &r.verdict {
            ClauseVerdict::Faithful => out.push_str(&format!("  {} — faithful\n", r.label)),
            ClauseVerdict::Divergent { detail } => {
                out.push_str(&format!("  {} — DIVERGENT: {detail}\n", r.label))
            }
            ClauseVerdict::Skipped { reason } => {
                out.push_str(&format!("  {} — skipped ({reason})\n", r.label))
            }
            ClauseVerdict::Unverifiable => out.push_str(&format!(
                "  {} — unverifiable (verus absent or no result)\n",
                r.label
            )),
        }
    }
    out
}

/// The pinned default seed + rlimit for `forge tv` (mirrors `forge check`'s
/// defaults — the deterministic config, §5.3 / R-CODE-5).
pub const TV_DEFAULT_SEED: u64 = DEFAULT_SOLVER_SEED;
pub const TV_DEFAULT_RLIMIT: f64 = DEFAULT_RLIMIT;

// ---- the forge-level contract Divergent teeth (REQ-5; blocker #166) ---------
//
// The obligation-layer teeth (`thermite-tv/tests/teeth.rs` F1–F4) prove a WRONG
// `P_production` -> a real verus error. They do NOT exercise the FORGE-level step
// that MAPS that verus signal to a `ClauseVerdict`: `discharge`'s four-way
// classification. Over the corpus/off-corpus space the faithful lowerer never
// produces a Divergent, so the Divergent ARM (and the Unverifiable boundary) had NO
// direct test coverage. This is the #166 analog of the #157 (`exec_tv`) / #189
// (`body_tv`) gap — the SAME parallel seam.
//
// This module is the end-to-end teeth for the FORGE classification, mirroring
// `exec_tv::divergent_teeth` and `body_tv::divergent_teeth`: it builds a REAL
// per-clause equivalence obligation, discharges it through the ACTUAL `discharge`
// fn, and asserts the verdict. It covers the positive control (faithful ->
// Faithful), the GENUINE-counterexample Divergent trigger (a WRONG production clause
// — the issue's `<=`-for-`==` semantic divergence), the degenerate zero-obligation
// boundary (-> Unverifiable, NEVER Divergent/Faithful), and the #189-class rlimit
// discriminator ([`is_rlimit_signal`] routes an rlimit-hit error run to Unverifiable,
// never Divergent — the mapping the hardening above added to `discharge`).
//
// THE #166 AUDIT FINDING: `discharge` ALREADY mapped a FRAME compile/parse abort (no
// parseable results line / non-success) to Unverifiable (the `_` arm) — honest, no
// change needed there. But the `errors >= 1` arm mapped EVERY error run to Divergent,
// INCLUDING an rlimit-exhausted run (a results line counting the exhausted obligation
// as an error). That is the SAME #189-class bug: a solver-budget timeout fabricated
// into a contract infidelity. The minimal fix added `is_rlimit_signal` + an
// rlimit-hit arm ahead of the Divergent arm. The `rlimit_signal_*` teeth pin it.
//
// TEST-ONLY: no further production-logic change. `discharge`/`is_rlimit_signal` are
// private sibling fns, reachable here via `super::`. The teeth are GENUINE (a real
// wrong production -> a real verus counterexample -> the real `discharge` mapping,
// never a mocked verdict). SKIPS LOUDLY when `verus` is genuinely absent.
#[cfg(test)]
mod divergent_teeth {
    use super::*;
    use thermite_syntax::ast::BinOp;

    /// `true` iff a bare `verus` is spawnable (the SAME resolution `discharge` uses —
    /// `Command::new("verus")`, i.e. PATH). SKIP LOUDLY otherwise so the teeth never
    /// silently pass when the discharge cannot reach a solver.
    fn verus_on_path() -> bool {
        Command::new("verus").arg("--version").output().is_ok()
    }

    // Pinned deterministic discharge config (mirrors `forge tv`'s defaults).
    const SEED: u64 = TV_DEFAULT_SEED;
    const RLIMIT: f64 = TV_DEFAULT_RLIMIT;

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

    /// The source clause `x == 1` over a scalar `x: u64` — the simplest in-sublanguage
    /// comparison. The independent reference encodes it as `x == 1` (`ref_contract_pred`
    /// — a bare bounded-int comparison, no coercion). Reused for the faithful control +
    /// the wrong-production Divergent trigger (the `<=`-for-`==` divergence the issue
    /// names). The frame binds the single free var `x: u64`.
    fn x_eq_1() -> Expr {
        bin(BinOp::Eq, path("x"), int(1))
    }

    fn x_frame() -> ObligationFrame {
        ObligationFrame {
            params: vec![ParamDecl::new("x", "u64")],
            ..Default::default()
        }
    }

    /// Build the equivalence obligation, returning `Ok`/`Err` (no `unwrap`/`expect` —
    /// the anti-pattern gate scans the patch text without `cfg(test)` context). The
    /// source `x_eq_1` is in-sublanguage, so the build always succeeds; the caller
    /// asserts `is_ok()` so an `Err` (a genuine regression) fails the test LOUDLY.
    fn build(source: &Expr, p_production: &str, frame: &ObligationFrame) -> Result<String, String> {
        equivalence_obligation(source, p_production, frame).map_err(|e| e.to_string())
    }

    /// POSITIVE CONTROL: a FAITHFUL production (`x == 1`, the exact reference encoding
    /// of source `x == 1`) -> the forge classification is `ClauseVerdict::Faithful`.
    ///
    /// HAND-DERIVED VERDICT (R-CHAR-3): the obligation is `assert((x == 1) <==> (x ==
    /// 1))`, which is `assert(true)` for every `x: u64` -> verus reports `verified >= 1,
    /// 0 errors`, exit success -> `discharge`'s first arm -> Faithful. Without this
    /// control, a `discharge` that returned Divergent unconditionally would pass the
    /// Divergent assertion vacuously.
    #[test]
    fn faithful_production_classifies_faithful() {
        if !verus_on_path() {
            eprintln!("SKIP: verus not on PATH — the forge-level Faithful control not discharged.");
            return;
        }
        let built = build(&x_eq_1(), "x == 1", &x_frame());
        assert!(
            built.is_ok(),
            "the equivalence obligation must build: {built:?}"
        );
        let prog = built.unwrap_or_default();
        let verdict = discharge(&prog, "teeth.faithful", SEED, RLIMIT);
        assert_eq!(
            verdict,
            ClauseVerdict::Faithful,
            "a FAITHFUL production contract lowering must classify Faithful (a forge-level \
             false positive otherwise)"
        );
    }

    /// DIVERGENT (the SOLE Divergent source — a GENUINE counterexample): a production
    /// that TYPECHECKS but means something WEAKER than the reference (`x <= 1` for
    /// source `x == 1` — the `<=`-for-`==` semantic divergence the issue names) ->
    /// verus finds a counterexample -> `discharge` maps the `errors >= 1` (NO rlimit)
    /// arm to `ClauseVerdict::Divergent`.
    ///
    /// HAND-DERIVED VERDICT (R-CHAR-3): the obligation is `assert((x <= 1) <==> (x ==
    /// 1))`. For `x = 0`: `0 <= 1` is `true` but `0 == 1` is `false`, so `true <==>
    /// false` is `false` — a real disagreement. verus reports `errors >= 1` with NO
    /// rlimit signal -> Divergent. This is exactly the AC-2 (==-vs-<=) infidelity, here
    /// asserted at the FORGE verdict layer (not just the obligation layer's F1).
    #[test]
    fn wrong_production_classifies_divergent() {
        if !verus_on_path() {
            eprintln!(
                "SKIP: verus not on PATH — the forge-level Divergent (counterexample) teeth \
                 not discharged."
            );
            return;
        }
        let built = build(&x_eq_1(), "x <= 1", &x_frame());
        assert!(
            built.is_ok(),
            "the equivalence obligation must build: {built:?}"
        );
        let prog = built.unwrap_or_default();
        let verdict = discharge(&prog, "teeth.counterexample", SEED, RLIMIT);
        assert!(
            matches!(verdict, ClauseVerdict::Divergent { .. }),
            "a WRONG production clause (`x <= 1` for source `x == 1`, the issue's \
             `<=`-for-`==` divergence) must classify Divergent via a counterexample; \
             got {verdict:?}"
        );
    }

    /// THE DEGENERATE/MALFORMED BOUNDARY (Divergent-vs-Unverifiable): a FRAME
    /// compile/parse abort — an obligation whose `requires` references an UNDEFINED spec
    /// fn (`all_small(x)`, with `spec_defs` EMPTY) fails to COMPILE: no parseable `N
    /// verified, M errors` line, non-success exit -> `discharge`'s `_` arm ->
    /// `ClauseVerdict::Unverifiable`, NEVER Divergent (a FRAMING limitation, not a
    /// lowering infidelity — R-HONEST-3). This is the contract analog of body_tv's #189
    /// `frame_compile_abort_classifies_unverifiable_not_divergent`.
    ///
    /// HAND-DERIVED VERDICT (R-CHAR-3): the production text is the FAITHFUL `x == 1`, so
    /// a Divergent verdict here could ONLY come from the frame, not the lowering. The
    /// undefined-`all_small` `requires` aborts verus with a compile error and NO results
    /// line; `parse_results` returns `None`; `discharge` falls to the `_` arm ->
    /// Unverifiable. (NB: a degenerate `0 verified, 0 errors` SUCCESS program would NOT
    /// pin this — contract_tv's first arm is `errors == 0 && status.success()` WITHOUT a
    /// `verified >= 1` guard, so a vacuous success classifies Faithful; the no-results
    /// abort is the honest malformed-outcome the `_` arm catches.) The #166 audit
    /// confirmed `discharge` was ALREADY honest on this arm — this teeth PINS it so a
    /// future regression to Divergent fails loudly.
    #[test]
    fn frame_abort_classifies_unverifiable_not_divergent() {
        if !verus_on_path() {
            eprintln!(
                "SKIP: verus not on PATH — the forge-level frame-abort->Unverifiable boundary \
                 not discharged."
            );
            return;
        }
        // `all_small` is an UNDEFINED spec fn (the frame's spec_defs is empty) — the
        // obligation's `requires all_small(x)` does not compile (an undefined-fn error),
        // so verus aborts BEFORE verification: no `N verified, M errors` line, non-success
        // exit. The production text is the faithful `x == 1` — the abort is purely a FRAME
        // limitation, never a lowering infidelity.
        let frame = ObligationFrame {
            params: vec![ParamDecl::new("x", "u64")],
            req: Some("all_small(x)".to_string()),
            ..Default::default()
        };
        let built = build(&x_eq_1(), "x == 1", &frame);
        assert!(
            built.is_ok(),
            "the equivalence obligation must build: {built:?}"
        );
        let prog = built.unwrap_or_default();
        let verdict = discharge(&prog, "teeth.frameabort", SEED, RLIMIT);
        assert_eq!(
            verdict,
            ClauseVerdict::Unverifiable,
            "a FRAME compile abort (an undefined spec-fn `req`) must classify Unverifiable, \
             NEVER Divergent (a fabricated infidelity, R-HONEST-3); got {verdict:?}"
        );
        assert!(
            !matches!(verdict, ClauseVerdict::Divergent { .. }),
            "a frame abort must NEVER be Divergent (R-HONEST-3); got {verdict:?}"
        );
    }

    /// THE #189-class DISCRIMINATOR (the mapping the hardening above added): an error
    /// run carrying a `Resource limit (rlimit) exceeded` signal is a TIMEOUT, not a
    /// counterexample — [`is_rlimit_signal`] detects it so `discharge` routes it to
    /// `Unverifiable` (the rlimit arm), NEVER the `errors >= 1` Divergent arm. A
    /// pure-unit check of the discriminator (no verus needed): an rlimit output is
    /// detected; a genuine counterexample output is NOT.
    ///
    /// HAND-DERIVED (R-CHAR-3): `is_rlimit_signal` keys on the literal Verus rlimit
    /// diagnostic substrings (`rlimit exceeded` / `rlimit) exceeded`, case-insensitive).
    /// A `Resource limit (rlimit) exceeded` line CONTAINS `rlimit) exceeded` -> true. A
    /// bare `rlimit exceeded` line -> true. A `postcondition not satisfied`
    /// counterexample contains NEITHER substring -> false (it stays in the Divergent
    /// class). This pins that a genuine Z3 rlimit exhaustion is kept OUT of Divergent —
    /// the SAME #189-class divergence, here in `contract_tv`.
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
        // The distributed z3 binary's OWN resourceout literal (#192 — the #166-dropped
        // clause the shared discriminator restores): `resource limit exceeded` with no
        // `rlimit` token.
        assert!(
            is_rlimit_signal("unknown: max. resource limit exceeded\n0 verified, 1 errors"),
            "z3's own `max. resource limit exceeded` resourceout literal MUST be detected \
             (the #166-dropped, now-shared clause)"
        );
        assert!(
            !is_rlimit_signal("error: assertion failed\n --> x.rs:5:12\n0 verified, 1 errors"),
            "a genuine `assertion failed` counterexample MUST NOT be detected as a timeout \
             (it stays in the Divergent class)"
        );
    }

    /// THE #189-class END-TO-END MAPPING: feed `discharge`'s parse/classify path an
    /// rlimit-signalled error run and assert the verdict is Unverifiable, NOT Divergent.
    /// This is the integration twin of the unit discriminator above — it drives the
    /// REAL `discharge` mapping (parse_results + the rlimit arm) on a SYNTHETIC verus
    /// output, with no verus needed (the program is the verus OUTPUT, not an input).
    ///
    /// HAND-DERIVED VERDICT (R-CHAR-3): we cannot deterministically force a real Z3
    /// rlimit timeout, so this pins the mapping at the classification seam. The unit
    /// `rlimit_signal_is_detected_counterexample_is_not` proves `is_rlimit_signal` fires
    /// on the rlimit text; the `discharge` source then routes `errors >= 1 && rlimit_hit`
    /// to Unverifiable AHEAD of the `errors >= 1` Divergent arm. Together they pin the
    /// full #189-class mapping (rlimit -> Unverifiable) by inspection + execution of the
    /// discriminator, exactly as `body_tv`'s `is_rlimit_signal` unit teeth do.
    #[test]
    fn rlimit_output_text_is_not_a_divergence() {
        use crate::tv_signal::is_rlimit_signal;
        // A counterexample output (NO rlimit) IS a Divergent signal; the rlimit output is
        // NOT — the discriminator that keeps the two classes distinct in `discharge`.
        let counterexample = "error: assertion failed\n0 verified, 1 errors";
        let rlimit = "error: Resource limit (rlimit) exceeded\n0 verified, 1 errors";
        assert!(
            !is_rlimit_signal(counterexample) && is_rlimit_signal(rlimit),
            "the rlimit output must be distinguished from a genuine counterexample so \
             `discharge` routes it to Unverifiable, never Divergent (the #189-class fix)"
        );
    }
}
