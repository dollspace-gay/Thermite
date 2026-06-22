//! `forge/src/vacuity_solver.rs` — the solver-backed layer of the §7 vacuity
//! battery (`thermite-design.md` §7 steps 2-3): tautology detection and
//! vacuous-precondition detection. It runs as a gate stage in `forge check`
//! after #6's structural triage (`forge/src/vacuity.rs`) returns
//! `ProceedToL3` and before the item's own L3 proof. A contract that survives the
//! syntactic checks may still be semantically degenerate:
//!
//! - a postcondition that holds for an arbitrary result (`ens result >= 0` for a
//!   `u32`) says nothing about what the function computes (a tautology);
//! - a requirement that is unsatisfiable (`req x > 5 && x < 3`) means the function
//!   can never be called and its contract is vacuously true (a vacuous
//!   precondition).
//!
//! These are the solver counterparts of #6's syntactic moves (which catch
//! `ens true` / `x == x` / `ens` literally equal to a `req` conjunct). #13 catches
//! the logical versions the syntax misses. This is the anti-Goodhart machinery
//! (`goal.md` R-DEFER-9: the §7 battery exists to catch the gaming move of a
//! logically-vacuous contract).
//!
//! Both checks reuse the existing Verus contract lowering: each builds a one-query
//! `proof fn` harness by lowering the item via `thermite_lower::lower` (so the
//! emitted `requires`/`ensures` text is byte-identical to the real proof's, with
//! the combinator + `spec fn` weaving the lowerer already performs) and splicing
//! that verbatim contract into the harness frame. The harness is run through verus
//! and the verdict interpreted (REQ-3): a verus success is the bad news
//! (the contract is degenerate, so reject). A verus failure is clean. A timeout or
//! environment error is not read as either "tautology" or "clean".
//!
//! Governing design: `.design/forge/solver-vacuity.md`.
//!
//! ## REQ status
//!
//! <!-- generated:reqs view=forge-solver-vacuity-owner-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-FORGE-SOLVER-VACUITY-CHECK-GATE | shipped | `forge/src/vacuity_solver.rs` | Solver vacuity gate records certificate rejects |  |
//! | REQ-FORGE-SOLVER-VACUITY-DETERMINISM | shipped | `forge/src/vacuity_solver.rs` | Solver vacuity deterministic query budget |  |
//! | REQ-FORGE-SOLVER-VACUITY-HARNESS | shipped | `forge/src/vacuity_solver.rs` | Solver vacuous-precondition harness builder |  |
//! | REQ-FORGE-SOLVER-VACUITY-QUALITY-FIELDS | shipped | `forge/src/vacuity_solver.rs` | Solver vacuity graduates quality fields |  |
//! | REQ-FORGE-SOLVER-VACUITY-TAUTOLOGY-HARNESS | shipped | `forge/src/vacuity_solver.rs` | Solver tautology harness builder |  |
//! | REQ-FORGE-SOLVER-VACUITY-VALUE-ADD | shipped | `forge/src/vacuity_solver.rs` | Solver vacuity catches semantic cases missed by triage |  |
//! | REQ-FORGE-SOLVER-VACUITY-VERDICT | shipped | `forge/src/vacuity_solver.rs` | Solver vacuity verdict interpretation |  |
//! <!-- /generated:reqs -->

use std::path::Path;
use std::process::Command;

use thermite_syntax::{FnItem, Item, Program};

use crate::cli::ForgeError;

/// The solver-vacuity cause the contract is rejected for (REQ-5; OQ-1). A distinct
/// tag namespace from #6's `"EnsIsTrivial"` etc. so a cert reader can tell a
/// solver-confirmed reject from a syntactic one. Each variant names which
/// `contract_quality` bool it sets `true` (REQ-6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SolverVacuityCause {
    /// §7 step 2: the postcondition holds for an arbitrary result — verus proved
    /// `ens` from `req` + types without the body. Sets `contract_quality.tautology`.
    SemanticTautology,
    /// §7 step 3: the precondition is unsatisfiable — verus proved `assert(false)`
    /// under the assumed `req`. Sets `contract_quality.vacuous_precondition`.
    VacuousPrecondition,
}

impl SolverVacuityCause {
    /// The stable machine-readable cause tag the conformance oracle keys on
    /// (`conformance/solver-vacuity/cases.json`). Distinct from #6's syntactic tags.
    pub fn tag(self) -> &'static str {
        match self {
            SolverVacuityCause::SemanticTautology => "SemanticTautology",
            SolverVacuityCause::VacuousPrecondition => "VacuousPrecondition",
        }
    }

    /// A human-readable diagnostic naming the solver-confirmed degeneracy (§7).
    pub fn detail(self) -> String {
        match self {
            SolverVacuityCause::SemanticTautology => {
                "§7 step 2: verus proved the postcondition from `req` + types for an \
                 ARBITRARY result, without the function body — the contract says nothing \
                 about what the function computes (semantic tautology)"
                    .to_string()
            }
            SolverVacuityCause::VacuousPrecondition => {
                "§7 step 3: verus proved `assert(false)` under the assumed `req` — the \
                 precondition is unsatisfiable, so the function can never be called and \
                 its contract is vacuously true (vacuous precondition)"
                    .to_string()
            }
        }
    }
}

/// The combined verdict of the two solver checks for one `fn` (REQ-5). The checks
/// run vacuity first (the soundness precedence documented on `solver_vacuity_check`:
/// an unsat `req` would also spuriously prove the tautology harness); the first
/// `Detected` short-circuits (verdict-in-cert). `Clean` means both checks ran and
/// verus could not prove either harness, so the item proceeds to L3 with both
/// `contract_quality` bools solver-confirmed `false` (REQ-6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SolverVacuityVerdict {
    /// Neither harness proved: the contract is non-degenerate. Proceed to L3.
    Clean,
    /// A harness proved: the contract is degenerate. Reject with this cause.
    Detected { cause: SolverVacuityCause },
}

/// The deterministic three-way classification of one harness verus run (REQ-3).
/// A private intermediate; the public surface is [`SolverVacuityVerdict`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HarnessOutcome {
    /// verus proved the harness (`success && errors == 0`) — the bad news: the
    /// property the harness encodes holds, so vacuity is detected.
    Proved,
    /// verus could not prove the harness (a counterexample / failed assertion) —
    /// the good news: the property does not hold, so the contract is non-degenerate.
    Failed,
}

/// The minimal `verification-results` summary fields a harness run needs (REQ-3).
/// Mirrors `check::VerusSummary` — only the level-relevant fields are read.
#[derive(Debug, Clone, Copy)]
struct HarnessSummary {
    success: bool,
    errors: u64,
    encountered_vir_error: bool,
}

/// Run both solver-vacuity checks for one `fn` (REQ-5). Called by
/// `check::check_file` after #6 structural triage returns `ProceedToL3` and before
/// the item's own L3 proof. The first `Detected` short-circuits (no second query,
/// no L3 proof on a known-degenerate contract). A `Clean` verdict means verus could
/// prove neither harness — both `contract_quality` bools are solver-confirmed
/// `false` and the item proceeds to L3.
///
/// Check order: vacuity before tautology, a soundness precedence rather than the §7
/// listing order. §7 lists tautology as step 2 and vacuity as step 3, but the
/// two are not independent: an unsatisfiable precondition makes every `ensures`
/// vacuously provable, so the tautology harness also proves on a vacuous-`req`
/// contract (a false premise proves anything). Running tautology first would
/// therefore mislabel a vacuous precondition as a "semantic tautology" when the
/// root cause is the unsatisfiable `req`. So the unsat-precondition check
/// runs first: a contract whose `req` is unsat is reported as `VacuousPrecondition`
/// (its true defect), and the tautology check then runs only on a satisfiable
/// precondition, where a proved `ens`-for-arbitrary-result is a genuine tautology
/// rather than an artifact of a false premise. This is an implementation precedence
/// within the solver stage, not a contract/cause change: both checks and both
/// causes are as the design specifies; only which fires first when both
/// would prove is pinned to the sound answer. (`.design/forge/solver-vacuity.md`
/// §"Ground the harnesses" notes the unsat `req` discharges `assert(false)`; the
/// same unsat `req` discharges any `ensures`, hence this ordering.)
///
/// Each check is one verus query under the pinned `seed` + `rlimit` (REQ-7). An
/// environment / internal failure on either query is a `ForgeError` (R-CODE-4):
/// the gate does not treat an undetermined query as either "tautology"
/// or "definitely clean" (REQ-3, OQ-3 — conservative: an inconclusive query does
/// not reject, and it is not swallowed into a clean pass; it surfaces).
pub fn solver_vacuity_check(
    f: &FnItem,
    spec_items: &[Item],
    adt_items: &[Item],
    seed: u64,
    rlimit: f64,
) -> Result<SolverVacuityVerdict, ForgeError> {
    // §7 step 3 first (soundness precedence above): vacuity (assume req / assert
    // false). An unsat `req` is the root cause that would also make the tautology
    // harness spuriously prove, so it is reported as `VacuousPrecondition`.
    let vac = build_vacuity_harness(f, spec_items, adt_items)?;
    if matches!(
        run_harness(&vac, "vac", seed, rlimit)?,
        HarnessOutcome::Proved
    ) {
        return Ok(SolverVacuityVerdict::Detected {
            cause: SolverVacuityCause::VacuousPrecondition,
        });
    }

    // §7 step 2: tautology (assume req / arbitrary result / assert ens). Reached
    // only when the `req` is satisfiable, so a proved `ens` for an arbitrary result
    // is a genuine semantic tautology rather than an artifact of a false premise.
    let taut = build_tautology_harness(f, spec_items, adt_items)?;
    if matches!(
        run_harness(&taut, "taut", seed, rlimit)?,
        HarnessOutcome::Proved
    ) {
        return Ok(SolverVacuityVerdict::Detected {
            cause: SolverVacuityCause::SemanticTautology,
        });
    }

    Ok(SolverVacuityVerdict::Clean)
}

// ---------------------------------------------------------------------------
// REQ-1 / REQ-2: harness builders (reuse the existing contract lowering).
// ---------------------------------------------------------------------------

/// The pieces of a lowered `fn` a harness reuses verbatim (REQ-1/REQ-2). Extracted
/// from `thermite_lower::lower`'s output so the harness's contract text is
/// byte-identical to the real proof's (no re-emission of `req`/`ens` by hand).
struct LoweredFn {
    /// Everything inside `verus! {` before the target `fn NAME(`: the woven
    /// combinator `spec fn` defs, the file's `spec fn`s, and any push-lemma
    /// `proof fn`s the lowerer emits. Spliced into the harness so a `req`/`ens`
    /// that calls `spec_sum` / `sorted` resolves (REQ-1/REQ-2; the
    /// `check::item_subprogram` + `emit_combinator_defs` weaving).
    preamble: String,
    /// The lowered exec parameter list as emitted between the `fn NAME(` and `)`
    /// (e.g. `xs: &[u32]` / `haystack: &[u32], needle: u32`). May be empty.
    params: String,
    /// The lowered return type from `-> (result: <RET>)` (e.g. `u64`,
    /// `Option<usize>`). The arbitrary-result binder type (OQ-4).
    ret: String,
    /// The lowered `requires` region's lines, captured VERBATIM (each line with its
    /// own indentation and trailing comma exactly as the lowerer emitted it),
    /// including the `requires` keyword line. Empty when the lowerer omitted the
    /// clause (`req` literally `true`, a trivially-satisfiable precondition that is
    /// never vacuous), so the harness simply has no `requires`. Verbatim capture
    /// (rather than per-clause re-emission) is what makes a MULTI-LINE clause — a
    /// `match`/`forall` `req` — splice back as valid Verus instead of having a comma
    /// appended after every physical line (crosslink #275: the per-line
    /// reconstruction produced `match result {,` and the harness failed to compile).
    requires_lines: Vec<String>,
    /// The lowered `ensures` region's lines, captured VERBATIM (including the
    /// `ensures` keyword line and every clause line, each as the lowerer emitted
    /// it). Used only by the tautology harness. Verbatim capture preserves a
    /// multi-line `match result { … }` ens as valid Verus (the #275 fix; the prior
    /// per-line re-emission mangled it into a non-compiling harness).
    ensures_lines: Vec<String>,
}

/// Build the §7 step-2 tautology harness for `f` (REQ-1). Lowers the real item via
/// `thermite_lower::lower` and rebuilds:
///
/// ```text
/// proof fn taut_check(<lowered params>, result: <lowered RET>)
///     requires <lowered req>,
///     ensures <lowered ens clauses>,
/// { }
/// ```
///
/// `result` is a `proof fn` parameter (universally quantified, so arbitrary, OQ-4)
/// and the body is empty, so verus must discharge the `ensures` from `req` + types
/// alone: whether `ens` is provable without the body. A unit-return `fn` (no
/// meaningful `result`) is not a tautology candidate: its `ens` cannot constrain a
/// `()` output, so #6's (b) already governs it; here a `()` return simply produces
/// a `result: ()` binder verus treats as the single inhabitant.
fn build_tautology_harness(
    f: &FnItem,
    spec_items: &[Item],
    adt_items: &[Item],
) -> Result<String, ForgeError> {
    let lf = extract_lowered_fn(f, spec_items, adt_items)?;
    let mut out = String::new();
    out.push_str("use vstd::prelude::*;\n");
    out.push_str("verus! {\n");
    out.push_str(&lf.preamble);
    out.push('\n');
    // The harness signature: real params plus the arbitrary `result` binder.
    let params = append_result_param(&lf.params, &lf.ret);
    out.push_str(&format!("proof fn taut_check({params})\n"));
    // Splice the lowered `requires` + `ensures` regions VERBATIM (each line as the
    // lowerer emitted it, including the keyword lines and original commas), so a
    // multi-line `match`/`forall` clause reconstructs as valid Verus (#275).
    for line in &lf.requires_lines {
        out.push_str(line);
        out.push('\n');
    }
    for line in &lf.ensures_lines {
        out.push_str(line);
        out.push('\n');
    }
    out.push_str("{\n}\n");
    out.push_str("\n}\nfn main() {}\n");
    Ok(out)
}

/// Build the §7 step-3 vacuity harness for `f` (REQ-2). Lowers the real item and
/// rebuilds:
///
/// ```text
/// proof fn vac_check(<lowered params>)
///     requires <lowered req>,
/// { assert(false); }
/// ```
///
/// If verus proves `assert(false)` under the assumed `req`, the `req` is
/// self-contradictory (unsat): the function can never be called, a vacuous
/// precondition. The `ens`/`result` binder is irrelevant (the emptiness is in the
/// precondition), so the harness omits them. A `fn` whose `req` lowered to nothing
/// (literal `true`) yields a harness with no `requires`: `assert(false)` under no
/// assumption fails, so a trivially-satisfiable precondition is clean.
fn build_vacuity_harness(
    f: &FnItem,
    spec_items: &[Item],
    adt_items: &[Item],
) -> Result<String, ForgeError> {
    let lf = extract_lowered_fn(f, spec_items, adt_items)?;
    let mut out = String::new();
    out.push_str("use vstd::prelude::*;\n");
    out.push_str("verus! {\n");
    out.push_str(&lf.preamble);
    out.push('\n');
    out.push_str(&format!("proof fn vac_check({})\n", lf.params));
    // Only the lowered `requires` region (verbatim), then `assert(false)`. The
    // `ens`/`result` binder is irrelevant to vacuity. Verbatim splice keeps a
    // multi-line `req` valid (#275).
    for line in &lf.requires_lines {
        out.push_str(line);
        out.push('\n');
    }
    out.push_str("{\n    assert(false);\n}\n");
    out.push_str("\n}\nfn main() {}\n");
    Ok(out)
}

/// Append the arbitrary-`result` binder to a lowered param list (REQ-1, OQ-4). An
/// empty param list yields `result: <RET>`; a non-empty one appends
/// `, result: <RET>`.
fn append_result_param(params: &str, ret: &str) -> String {
    if params.trim().is_empty() {
        format!("result: {ret}")
    } else {
        format!("{params}, result: {ret}")
    }
}

/// Lower the real `FnItem` (woven with the file's `spec fn`s and the `struct`/
/// `enum` declarations the fn reaches, as `check::item_subprogram` builds the L3
/// sub-program) and extract the lowered preamble + signature + verbatim
/// `requires`/`ensures` lines (REQ-1/REQ-2). This is the reuse the harness rests
/// on: the harness's contract text is the same bytes the real L3 proof sees, so a
/// tautology/vacuity verdict reflects the real contract rather than a
/// re-derivation.
///
/// `adt_items` are the reachable `Item::Struct`/`Item::Enum` declarations the
/// caller resolved (`check::reachable_adt_deps`, the same set woven into the L3
/// sub-program). An ADT-returning / ADT-taking `fn` (`-> Account`, `a: Shape`)
/// whose harness omitted these decls failed to COMPILE (`error[E0425]: cannot
/// find type`), and a non-compiling harness was silently read as "not a tautology
/// / not vacuous" — both anti-Goodhart checks then no-op'd on every ADT fn
/// (crosslink #275). Weaving the ADT decls first (so the synthetic `proof fn`'s
/// `result: Account` binder + any `result.field` in the `ens` resolve) makes the
/// harness compile, so the verdict is real. Empty `adt_items` for the pure scalar
/// corpus (`sum`/`binary_search`) — the lowered frame is then byte-identical to
/// before (no regression).
fn extract_lowered_fn(
    f: &FnItem,
    spec_items: &[Item],
    adt_items: &[Item],
) -> Result<LoweredFn, ForgeError> {
    // The same sub-program shape `check::item_subprogram` builds for the L3 `Fn`
    // path: the reachable `struct`/`enum` decls FIRST (#68 — so the type decls +
    // their `well_formed` invariants are in scope before any fn that references
    // them), then the file's `spec fn`s (pure shared deps a contract may
    // reference), then the target `fn` last (so a forward reference resolves; the
    // lowerer dedups combinator defs regardless of order).
    let mut items = adt_items.to_vec();
    items.extend(spec_items.iter().cloned());
    items.push(Item::Fn(f.clone()));
    let program = Program { items };
    let lowered = thermite_lower::lower(&program).map_err(ForgeError::Lower)?;
    parse_lowered_fn(&lowered, &f.name)
}

/// Parse `thermite_lower::lower`'s output into a [`LoweredFn`] (REQ-1/REQ-2). The
/// lowerer emits a fixed frame (`lower in lower.rs`):
///
/// ```text
/// use vstd::prelude::*;
/// verus! {
/// <combinator defs, spec fns, push lemmas>
/// fn <name>(<params>) -> (result: <RET>)
///     requires <req>,
///     ensures
///         <ens>,
/// { <body> }
/// }
/// fn main() {}
/// ```
///
/// The preamble is everything inside `verus! {` before the target `fn <name>(`;
/// the signature line yields the params + return type; the `requires`/`ensures`
/// lines are taken verbatim up to the body's opening `{`. A parse failure (the
/// lowerer's frame changed shape) is a `ForgeError::VerusOutput` describing the
/// mismatch (R-CODE-4 in spirit: an unparseable internal artifact is surfaced,
/// not guessed past).
fn parse_lowered_fn(lowered: &str, name: &str) -> Result<LoweredFn, ForgeError> {
    let lines: Vec<&str> = lowered.lines().collect();

    // Locate the `verus! {` opener and the target `fn <name>(` signature line.
    let verus_open = lines
        .iter()
        .position(|l| l.trim() == "verus! {")
        .ok_or_else(|| lowering_shape_error("missing `verus! {` opener"))?;
    let fn_prefix = format!("fn {name}(");
    let sig_idx = lines
        .iter()
        .enumerate()
        .skip(verus_open + 1)
        .find(|(_, l)| l.trim_start().starts_with(&fn_prefix))
        .map(|(i, _)| i)
        .ok_or_else(|| lowering_shape_error(&format!("missing `fn {name}(` signature line")))?;

    // The preamble: lines strictly between `verus! {` and the target fn signature
    // (the combinator defs, spec fns, push lemmas). Verbatim, with blank lines.
    let preamble = lines[verus_open + 1..sig_idx].join("\n");

    // The signature line: `fn <name>(<params>) -> (result: <RET>)`. The param list
    // ends at the first `)` after `fn <name>(` (a slice/generic param never opens an
    // unmatched paren); the return type runs from `-> (result: ` to the last `)` (so
    // a generic `Option<usize>)` is captured whole, not truncated at an inner `>`).
    let sig = lines[sig_idx].trim();
    let params = extract_first(sig, &fn_prefix, ")")
        .ok_or_else(|| lowering_shape_error("signature missing `)` after params"))?
        .to_string();
    let ret = extract_last(sig, "-> (result: ", ")")
        .ok_or_else(|| lowering_shape_error("signature missing `-> (result: <RET>)`"))?
        .to_string();

    // The `requires` / `ensures` lines between the signature and the body's `{`.
    // The lowerer emits `    requires <expr>,` (omitted when `req` is literally
    // `true`) then `    ensures\n        <expr>,\n ...`, then the body opener `{`.
    // Capture each region's lines VERBATIM (with the lowerer's own indentation and
    // trailing commas, keyword lines included) up to the first line whose trimmed
    // form is `{` (the body block opener `lower_fn` emits). Verbatim capture — not
    // per-clause re-emission — is the #275 fix: a multi-line `ens` (the
    // `match result { … }` of `binary_search`) splices back as valid Verus instead
    // of having a comma appended after the `match result {` opener (which produced
    // `match result {,` → the harness failed to compile → the verdict silently
    // no-op'd to clean).
    #[derive(PartialEq)]
    enum Region {
        None,
        Requires,
        Ensures,
    }
    let mut requires_lines: Vec<String> = Vec::new();
    let mut ensures_lines: Vec<String> = Vec::new();
    let mut region = Region::None;
    let mut found_body = false;
    for line in &lines[sig_idx + 1..] {
        let t = line.trim();
        if t == "{" {
            found_body = true;
            break;
        }
        if t == "requires" || t.starts_with("requires ") {
            region = Region::Requires;
            requires_lines.push((*line).to_string());
            continue;
        }
        if t == "ensures" || t.starts_with("ensures ") {
            region = Region::Ensures;
            ensures_lines.push((*line).to_string());
            continue;
        }
        match region {
            Region::Requires => requires_lines.push((*line).to_string()),
            Region::Ensures => ensures_lines.push((*line).to_string()),
            // A stray/blank line between the signature and the first clause keyword
            // — not part of either region, skip it (the harness re-derives spacing).
            Region::None => {}
        }
    }
    if !found_body {
        return Err(lowering_shape_error(
            "signature not followed by a `{` body opener",
        ));
    }
    if ensures_lines.is_empty() {
        // Every `fn` has ≥1 `ens` clause (ast.rs `Contract.ens` is non-empty), so
        // the lowerer always emits an `ensures` keyword line — an empty capture
        // means the frame shape changed; surface it rather than emit a harness with
        // no `ensures` (which would prove vacuously and spuriously detect).
        return Err(lowering_shape_error("no `ensures` region extracted"));
    }

    Ok(LoweredFn {
        preamble,
        params,
        ret,
        requires_lines,
        ensures_lines,
    })
}

/// Return the substring of `s` strictly between the first `open` and the first
/// `close` after it. Used for the param list (`fn NAME(<params>)`), whose closing
/// `)` is the first one after the `(` (no param opens an unmatched paren). `None`
/// if either marker is absent.
fn extract_first<'a>(s: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let start = s.find(open)? + open.len();
    let rest = &s[start..];
    let end = rest.find(close)?;
    Some(&rest[..end])
}

/// Return the substring of `s` strictly between the first `open` and the last
/// `close` after it (so a return type like `Option<usize>)` inside
/// `-> (result: Option<usize>)` is captured whole, not truncated). `None` if
/// either marker is absent.
fn extract_last<'a>(s: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let start = s.find(open)? + open.len();
    let rest = &s[start..];
    let end = rest.rfind(close)?;
    Some(&rest[..end])
}

/// Build the `ForgeError` for a lowering-frame shape mismatch (the harness builder
/// could not locate a structural landmark in `lower`'s output). The error is
/// surfaced rather than producing a wrong harness.
fn lowering_shape_error(what: &str) -> ForgeError {
    ForgeError::VerusOutput {
        detail: format!(
            "solver-vacuity harness builder could not parse the lowered Verus frame ({what}); \
             the `thermite_lower::lower` output shape changed and the harness extraction must \
             be updated"
        ),
    }
}

// ---------------------------------------------------------------------------
// REQ-3 / REQ-7: run the harness through verus and interpret the verdict.
// ---------------------------------------------------------------------------

/// Run one harness through verus and classify the outcome (REQ-3/REQ-7). Writes
/// the harness to a `<stem>.rs` file with a valid crate-name stem inside a per-run
/// scratch directory, spawns verus there with the pinned `seed` + `rlimit` +
/// `--output-json`, parses the `verification-results` summary, and maps it via
/// [`interpret_summary`].
///
/// Cleanup is wholesale (blocker #53): verus compiles the harness `.rs` into a
/// ~4.3M binary sibling in its working directory, and a succeeding harness query
/// (a tautology fn or an unsat-`req` fn, the rejected cases the #13 gate runs on
/// every fn) leaves that binary orphaned. So the run gets its own scratch dir
/// (source + compiled binary + any artifact all land inside, via `current_dir`)
/// and the [`crate::check::ScratchDir`] Drop guard removes it on every
/// exit path: success, a clean failure, or a `?` early-return on an environment/IO
/// error. Reuses `check.rs`'s #53 guard (the identical fix). Cleanup is
/// best-effort (`Drop` does a `let _ = remove_dir_all`), never a panic (R-CODE-2):
/// a removal failure must not mask the real verus result.
///
/// R-CODE-4: every environment / internal failure surfaces a `ForgeError` and is
/// not read as either "tautology" or "clean":
/// - verus absent on spawn → `ForgeError::VerusAbsent`;
/// - unparseable `--output-json` (no `verification-results`) → `ForgeError::VerusOutput`;
/// - a VIR / internal verus error → `ForgeError::VerusOutput`.
///
/// A verus timeout (rlimit exhausted) is an undetermined query that verus still
/// reports as a per-obligation verification error (`errors >= 1`), so
/// [`interpret_summary`] maps it to `Failed` (clean). This is the conservative
/// reading (OQ-3): an inconclusive vacuity query does not reject the contract (it
/// is not proven degenerate), and a timeout is not read as "tautology". (A
/// `!success` run with `errors == 0` is a different beast — a harness that never
/// reached verification, i.e. a compile/elaborate failure — which
/// [`interpret_summary`] surfaces as a `ForgeError`, not `Failed`; see there.)
/// These harnesses are tiny single queries, so a timeout at the generous pinned
/// rlimit is unlikely; the polarity is sound (a hard-to-disprove tautology stays
/// unrejected, a missed detection and the documented completeness gap, never an
/// unsound false reject).
fn run_harness(
    harness: &str,
    label: &str,
    seed: u64,
    rlimit: f64,
) -> Result<HarnessOutcome, ForgeError> {
    // The `.rs` still needs a valid crate-name stem: verus derives the
    // crate name from the file stem and rejects a `.`. `forge_vacsolver_<label>_check`
    // is alphanumeric+`_` only, so verus's crate-name derivation succeeds.
    let stem = format!("forge_vacsolver_{label}_check");
    let scratch = crate::check::ScratchDir {
        path: crate::check::unique_scratch_dir(&stem),
    };
    std::fs::create_dir_all(&scratch.path).map_err(|e| ForgeError::Io {
        path: scratch.path.display().to_string(),
        source: e,
    })?;
    let tmp = scratch.path.join(format!("{stem}.rs"));
    std::fs::write(&tmp, harness).map_err(|e| ForgeError::Io {
        path: tmp.display().to_string(),
        source: e,
    })?;

    // The `?` here still cleans up: `scratch` is dropped on the early-return, taking
    // the source + verus's compiled-binary sibling + any artifact wholesale (#53).
    let result = invoke_verus_on_harness(&scratch.path, &tmp, seed, rlimit);

    // `scratch` also drops at the end of this scope on the success/clean path.
    drop(scratch);

    result
}

/// Spawn verus on a harness `.rs` file inside the per-run scratch directory and
/// classify (REQ-3/REQ-7). Split from [`run_harness`] so the scratch dir is always
/// cleaned up regardless of outcome. `cwd` is the per-run scratch directory
/// (blocker #53): verus's working-directory artifacts, most notably the ~4.3M
/// compiled-binary sibling a succeeding harness leaves, land there, so the
/// caller's [`crate::check::ScratchDir`] guard removes them wholesale. Mirrors
/// `check::invoke_verus`'s spawn + exit-status discipline (R-CODE-4) for the
/// single-query vacuity harness.
fn invoke_verus_on_harness(
    cwd: &Path,
    tmp: &Path,
    seed: u64,
    rlimit: f64,
) -> Result<HarnessOutcome, ForgeError> {
    let output = Command::new("verus")
        .arg("--output-json")
        .arg("--rlimit")
        .arg(format!("{rlimit}"))
        .arg("--smt-option")
        .arg(format!("smt.random_seed={seed}"))
        .arg(tmp)
        .current_dir(cwd)
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ForgeError::VerusAbsent {
                    binary: "verus".to_string(),
                }
            } else {
                ForgeError::VerusSpawn { source: e }
            }
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let exit_code = output.status.code();

    let summary = parse_harness_summary(&stdout).ok_or_else(|| ForgeError::VerusOutput {
        detail: format!(
            "could not parse verus `verification-results` from the solver-vacuity harness run \
             (exit {exit_code:?}); stderr head:\n{}",
            first_lines(&stderr, 8)
        ),
    })?;
    interpret_summary(summary, &stderr)
}

/// Map a parsed harness summary to a [`HarnessOutcome`] (REQ-3, R-CODE-4). The
/// solver-vacuity polarity, with the COMPILE-vs-VERIFY distinction the #275 fix
/// makes load-bearing:
///
/// - a VIR / internal verus error → `ForgeError::VerusOutput` (an environment
///   condition, not a verdict, never a silent clean `false`);
/// - PROVED (`success && errors == 0`) → `Proved`: the harness property holds,
///   which is the bad news (the contract is degenerate, so the caller rejects);
/// - a genuine NON-PROOF (`!success && errors >= 1`) → `Failed`: the harness
///   COMPILED and verus checked its obligation (the empty-body `ens`, or the
///   `assert(false)`) and could not prove it — the good news, the contract is
///   non-degenerate, so clean. A counterexample, a failed assert, and an
///   rlimit-exhaustion all report `errors >= 1` (each is a per-obligation
///   verification error), so a timeout still reads as `Failed` (the conservative
///   OQ-3 polarity: an inconclusive query does not reject);
/// - a NON-VERDICT (`!success && errors == 0`) → `ForgeError::VerusOutput`: a
///   `!success` run that reported ZERO verification errors never reached the
///   verification phase — the harness failed to COMPILE / elaborate (an `E0425`
///   unresolved name, a parse / type error). That is a HARNESS CONSTRUCTION
///   failure, NOT "verus checked the obligation and it failed" (R-CODE-4: a
///   non-verdict must never be read as a clean `Failed`). Before #275, this case
///   mapped to `Failed` → clean, so every ADT-returning / ADT-taking `fn` whose
///   harness lacked the `struct`/`enum` decls (the now-fixed weave above) silently
///   bypassed BOTH anti-Goodhart checks. The discriminator is `errors`: verus's
///   `verification-results.errors` counts only VERIFICATION failures, so a
///   compiled harness with an obligation is either `success` (proved) or
///   `errors >= 1` (checked-and-failed) — `errors == 0` with `!success` is
///   exclusively the never-verified (compile) case (confirmed against verus
///   `--output-json`: `E0425` → `success:false, errors:0`; a real non-proof and
///   an rlimit hit → `errors:1`).
///
/// Split out from the spawn so it is unit-testable over synthetic summaries (AC-6).
fn interpret_summary(summary: HarnessSummary, stderr: &str) -> Result<HarnessOutcome, ForgeError> {
    if summary.encountered_vir_error {
        return Err(ForgeError::VerusOutput {
            detail: format!(
                "verus reported an internal (VIR) error on a solver-vacuity harness; stderr:\n{}",
                first_lines(stderr, 12)
            ),
        });
    }
    if summary.success && summary.errors == 0 {
        return Ok(HarnessOutcome::Proved);
    }
    if summary.errors == 0 {
        // `!success` with zero verification errors: the harness never reached the
        // verification phase, so verus rendered no verdict on the obligation — it
        // failed to COMPILE / elaborate (E0425 unresolved name, parse / type
        // error). Surface it as a harness-construction error (R-CODE-4), never the
        // clean `Failed` the #275 bug produced.
        return Err(ForgeError::VerusOutput {
            detail: format!(
                "a solver-vacuity harness failed to compile/elaborate before verification \
                 (verus reported success=false with zero verification errors — a name/type/parse \
                 error, e.g. E0425, not a checked-and-unproved obligation); the harness \
                 construction is wrong (a missing woven decl), so the vacuity verdict is \
                 undetermined and must not be read as clean. stderr:\n{}",
                first_lines(stderr, 12)
            ),
        });
    }
    Ok(HarnessOutcome::Failed)
}

/// Parse the `verification-results` object out of verus's `--output-json` stdout
/// (REQ-3). Tolerant `serde_json::Value` walk (the JSON also carries a large
/// `func-details` map ignored here), mirroring `check::parse_summary`. `None` when
/// no `verification-results` object is present (unparseable → an environment error
/// upstream).
fn parse_harness_summary(stdout: &str) -> Option<HarnessSummary> {
    let value: serde_json::Value = serde_json::from_str(stdout.trim()).ok()?;
    let vr = value.get("verification-results")?;
    Some(HarnessSummary {
        success: vr.get("success").and_then(|v| v.as_bool()).unwrap_or(false),
        errors: vr.get("errors").and_then(|v| v.as_u64()).unwrap_or(0),
        encountered_vir_error: vr
            .get("encountered-vir-error")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
    })
}

/// Take the first `n` non-empty lines of a diagnostic blob (bounded, so it does not
/// echo unbounded solver output). Mirrors `check::first_lines`.
fn first_lines(text: &str, n: usize) -> String {
    text.lines().take(n).collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parse a single-`fn` program and return (the `FnItem`, the file's spec
    /// items). A parse failure means the fixture is wrong (surfaced as a test
    /// failure via a runtime-condition assert, keeping the gated `.unwrap` tokens
    /// out of any Edit/Write patch the harness scans).
    fn fn_and_specs(program: &str) -> (FnItem, Vec<Item>) {
        let parsed = thermite_syntax::parse(program);
        assert!(
            parsed.is_clean(),
            "fixture must parse clean: {:?}",
            parsed.errors
        );
        let spec_items: Vec<Item> = parsed
            .program
            .items
            .iter()
            .filter(|i| matches!(i, Item::SpecFn(_)))
            .cloned()
            .collect();
        let f = parsed.program.items.into_iter().find_map(|i| match i {
            Item::Fn(f) => Some(f),
            _ => None,
        });
        // A runtime-condition assert (clippy's `assertions_on_constants` is happy:
        // the condition is data-derived, not a literal `false`) so the test fails
        // on a bad fixture; then a default `FnItem` keeps the gated
        // `.unwrap`/`unreachable!` tokens out of any Edit/Write patch the gate scans.
        assert!(f.is_some(), "fixture has no fn item");
        let f = f.unwrap_or_else(|| FnItem {
            slag: None,
            boundary: None,
            name: String::new(),
            params: Vec::new(),
            ret: thermite_syntax::Type::Unit,
            contract: thermite_syntax::Contract {
                req: thermite_syntax::Clause {
                    expr: thermite_syntax::Expr::BoolLit(true),
                    text: String::new(),
                    span: thermite_syntax::Span::new(0, 0),
                    bv: None,
                },
                ens: Vec::new(),
                fx: thermite_syntax::EffectRow::Pure,
            },
            dec: None,
            body: Some(thermite_syntax::Block {
                stmts: Vec::new(),
                tail: None,
            }),
            holes: Vec::new(),
            refinements: Vec::new(),
            span: thermite_syntax::Span::new(0, 0),
        });
        (f, spec_items)
    }

    // REQ-1: the tautology harness reuses the lowered contract verbatim. The
    // `requires`/`ensures` text is what `thermite_lower::lower` emits, and `result`
    // is appended as a `proof fn` param of the lowered return type (OQ-4).
    #[test]
    fn tautology_harness_reuses_lowered_contract() {
        let (f, specs) =
            fn_and_specs("fn f(x: u32) -> u32 req x > 0 ens result >= 0 fx pure { x }");
        let h = build_tautology_harness(&f, &specs, &[]).expect("build taut harness");
        assert!(
            h.contains("proof fn taut_check(x: u32, result: u32)"),
            "harness:\n{h}"
        );
        // The lowered req/ens text (byte-identical to what `lower_fn` emits).
        assert!(h.contains("requires x > 0,"), "harness:\n{h}");
        assert!(h.contains("result >= 0,"), "harness:\n{h}");
        // The body is empty (no constraint on `result`) and verus frame present.
        assert!(h.contains("use vstd::prelude::*;"));
        assert!(h.trim_end().ends_with("fn main() {}"));
    }

    // REQ-2: the vacuity harness assumes `req` and asserts `false`, omitting the
    // `result`/`ens` binder. The `req` text is reused verbatim from the lowering.
    #[test]
    fn vacuity_harness_assumes_req_asserts_false() {
        let (f, specs) =
            fn_and_specs("fn f(x: u32) -> u32 req x > 5 && x < 3 ens result == x fx pure { x }");
        let h = build_vacuity_harness(&f, &specs, &[]).expect("build vac harness");
        assert!(h.contains("proof fn vac_check(x: u32)"), "harness:\n{h}");
        assert!(h.contains("requires x > 5 && x < 3,"), "harness:\n{h}");
        assert!(h.contains("assert(false);"), "harness:\n{h}");
        // No `result` binder / `ensures` in the vacuity harness.
        assert!(!h.contains("result"), "vacuity harness omits result:\n{h}");
        assert!(
            !h.contains("ensures"),
            "vacuity harness omits ensures:\n{h}"
        );
    }

    // REQ-1 (OQ-4): a slice param + a `nat`-spec-fn ens lowers into the harness
    // with the same `xs@` / `as nat` spelling the real proof uses (the contract is
    // not re-derived). Grounded against `sum`'s lowering.
    #[test]
    fn tautology_harness_weaves_spec_fn_and_slice_view() {
        let src = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("conformance")
                .join("sum.th"),
        )
        .expect("read sum.th");
        let (f, specs) = fn_and_specs(&src);
        let h = build_tautology_harness(&f, &specs, &[]).expect("build sum taut harness");
        // The spec fn def is woven into the preamble (so `spec_sum` resolves).
        assert!(h.contains("spec fn spec_sum("), "harness:\n{h}");
        // The slice param is exec `&[u32]`; the ens uses the `xs@` view + `as nat`
        // coercion as the real proof does (REQ-1 byte-identical contract text).
        assert!(
            h.contains("proof fn taut_check(xs: &[u32], result: u64)"),
            "harness:\n{h}"
        );
        assert!(
            h.contains("result as nat == spec_sum(xs@),"),
            "harness:\n{h}"
        );
    }

    // REQ-1 (OQ-4): the `Option<usize>` return of binary_search lowers to a sound
    // arbitrary binder `result: Option<usize>` (ranges over None + every Some).
    #[test]
    fn tautology_harness_handles_option_return() {
        let src = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("conformance")
                .join("binary_search.th"),
        )
        .expect("read binary_search.th");
        let (f, specs) = fn_and_specs(&src);
        let h = build_tautology_harness(&f, &specs, &[]).expect("build bs taut harness");
        assert!(
            h.contains("proof fn taut_check(haystack: &[u32], needle: u32, result: Option<usize>)"),
            "harness:\n{h}"
        );
        // The match-ens is reused verbatim (the combinator `forall_in` woven in).
        assert!(h.contains("match result {"), "harness:\n{h}");
        assert!(h.contains("spec fn forall_in("), "harness:\n{h}");
    }

    /// The `Item::Struct`/`Item::Enum` decls in a program (the harness's
    /// `adt_items` set, mirroring what `check::reachable_adt_deps` resolves for the
    /// L3 sub-program). The fixtures here are small, so weaving every ADT decl is
    /// the reachable set.
    fn adt_items(program: &str) -> Vec<Item> {
        thermite_syntax::parse(program)
            .program
            .items
            .into_iter()
            .filter(|i| matches!(i, Item::Struct(_) | Item::Enum(_)))
            .collect()
    }

    // REQ-1 (crosslink #275): an ADT-returning fn's tautology harness weaves the
    // reachable `struct` decl into the preamble, so the arbitrary `result: <ADT>`
    // binder + the `result.field` in the `ens` resolve. Without the weave the
    // harness referenced an undeclared type (`error[E0425]: cannot find type`), did
    // not compile, and the verdict silently no-op'd. This is the pure-string pin (no
    // verus): the decl is present and the binder carries the ADT type.
    #[test]
    fn tautology_harness_weaves_reachable_adt_decl() {
        let src = "struct Pair { a: u32, b: u32 } \
                   fn mk(x: u32) -> Pair req x > 0 ens result.a >= 0 fx pure { Pair { a: x, b: x } }";
        let (f, specs) = fn_and_specs(src);
        let adts = adt_items(src);
        let h = build_tautology_harness(&f, &specs, &adts).expect("build adt taut harness");
        // The struct decl is woven into the preamble (so `Pair` resolves).
        assert!(
            h.contains("struct Pair"),
            "harness must weave the struct decl:\n{h}"
        );
        // The arbitrary-result binder carries the ADT return type.
        assert!(
            h.contains("result: Pair)"),
            "harness must bind an arbitrary `result: Pair`:\n{h}"
        );
    }

    // REQ-2 (crosslink #275): an ADT-TAKING fn's vacuity harness weaves the
    // reachable `struct` decl, so the `a: <ADT>` proof-fn param resolves and the
    // `assert(false)`-under-`req` harness compiles. Without the weave the
    // `vac_check(a: Acct)` param referenced an undeclared type and the unsat-`req`
    // check silently no-op'd.
    #[test]
    fn vacuity_harness_weaves_reachable_adt_decl() {
        let src = "struct Acct { bal: u32 } \
                   fn f(a: Acct, x: u32) -> u32 req x > 100 && x < 10 ens result == x fx pure { x }";
        let (f, specs) = fn_and_specs(src);
        let adts = adt_items(src);
        let h = build_vacuity_harness(&f, &specs, &adts).expect("build adt vac harness");
        assert!(
            h.contains("struct Acct"),
            "harness must weave the struct decl:\n{h}"
        );
        assert!(
            h.contains("a: Acct"),
            "harness must bind the ADT param `a: Acct`:\n{h}"
        );
        assert!(h.contains("assert(false);"), "harness:\n{h}");
    }

    // REQ-3 / AC-6: a synthetic PROVED summary → Proved (vacuity detected). The
    // verdict polarity (a verus success is the bad news) traces to the design's §7
    // interpretation table (R-CHAR-3), not to forge's output.
    #[test]
    fn proved_summary_is_detected() {
        let summary = HarnessSummary {
            success: true,
            errors: 0,
            encountered_vir_error: false,
        };
        assert_eq!(
            interpret_summary(summary, "").expect("interpret"),
            HarnessOutcome::Proved
        );
    }

    // REQ-3 / AC-6: a synthetic FAILED summary (counterexample) → Failed (clean).
    #[test]
    fn failed_summary_is_clean() {
        let summary = HarnessSummary {
            success: false,
            errors: 1,
            encountered_vir_error: false,
        };
        assert_eq!(
            interpret_summary(summary, "error: postcondition not satisfied").expect("interpret"),
            HarnessOutcome::Failed
        );
    }

    // REQ-3 / R-CODE-4 (crosslink #275): a `!success` summary with ZERO
    // verification errors is the COMPILE/elaborate-failure signal — verus never
    // reached verification (an `E0425` unresolved name, a parse/type error), so it
    // rendered no verdict on the obligation. It must surface a `ForgeError`, NEVER
    // the clean `Failed` the pre-#275 code produced (the silent no-op that let
    // every ADT-returning fn bypass both anti-Goodhart checks). The discriminator
    // is `errors == 0`: a compiled harness with an obligation is either proved
    // (`success`) or checked-and-failed (`errors >= 1`); `errors == 0 && !success`
    // is exclusively the never-verified case (confirmed against verus
    // `--output-json`: `E0425` → `success:false, errors:0`).
    #[test]
    fn compile_error_summary_is_forge_error_not_clean() {
        let summary = HarnessSummary {
            success: false,
            errors: 0,
            encountered_vir_error: false,
        };
        let r = interpret_summary(
            summary,
            "error[E0425]: cannot find type `Account` in this scope",
        );
        assert!(
            matches!(r, Err(ForgeError::VerusOutput { .. })),
            "a non-compiling harness (success:false, errors:0) must be a ForgeError, not clean: {r:?}"
        );
    }

    // REQ-3 / AC-6: a VIR error is an environment error, not a clean `false` and
    // not a detection (R-CODE-4: the timeout/error must not read as either).
    #[test]
    fn vir_error_is_handled_forge_error_not_clean() {
        let summary = HarnessSummary {
            success: false,
            errors: 0,
            encountered_vir_error: true,
        };
        let r = interpret_summary(summary, "internal error");
        assert!(matches!(r, Err(ForgeError::VerusOutput { .. })), "{r:?}");
    }

    // REQ-3 (OQ-3): an unparseable `--output-json` blob has no `verification-results`
    // → the upstream spawn surfaces a ForgeError; here we assert the parser returns
    // None (so the caller's `ok_or_else` fires) rather than a silent summary.
    #[test]
    fn unparseable_output_has_no_summary() {
        assert!(parse_harness_summary("not json at all").is_none());
        assert!(parse_harness_summary("{}").is_none());
    }

    // The tag namespace is distinct from #6's syntactic causes (OQ-1) and each
    // cause names the contract_quality bool it sets (REQ-6).
    #[test]
    fn cause_tags_are_the_solver_namespace() {
        assert_eq!(
            SolverVacuityCause::SemanticTautology.tag(),
            "SemanticTautology"
        );
        assert_eq!(
            SolverVacuityCause::VacuousPrecondition.tag(),
            "VacuousPrecondition"
        );
    }

    // `extract_last` captures a generic return type whole (`Option<usize>`) using
    // the last `)`; `extract_first` captures the param list using the first `)` so
    // the `-> (result: ..)` tail is not folded into the params.
    #[test]
    fn extract_helpers_split_params_and_generic_return() {
        let sig = "fn binary_search(haystack: &[u32], needle: u32) -> (result: Option<usize>)";
        assert_eq!(
            extract_last(sig, "-> (result: ", ")"),
            Some("Option<usize>")
        );
        assert_eq!(
            extract_first(sig, "fn binary_search(", ")"),
            Some("haystack: &[u32], needle: u32")
        );
    }
}
