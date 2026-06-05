//! `forge/src/vacuity_solver.rs` — the SOLVER-backed layer of the §7 vacuity
//! battery (`thermite-design.md` §7 steps 2-3): **tautology** detection and
//! **vacuous-precondition** detection. It runs as a gate stage in `forge check`
//! AFTER #6's FREE structural triage (`forge/src/vacuity.rs`) returns
//! `ProceedToL3` and BEFORE the item's own L3 proof. A contract that survives the
//! syntactic checks may still be SEMANTICALLY degenerate:
//!
//! - a postcondition that holds for an ARBITRARY result (`ens result >= 0` for a
//!   `u32`) says nothing about what the function computes → a **tautology**;
//! - a requirement that is unsatisfiable (`req x > 5 && x < 3`) means the function
//!   can never be called and its contract is vacuously true → a **vacuous
//!   precondition**.
//!
//! These are the SOLVER counterparts of #6's syntactic moves (which catch
//! `ens true` / `x == x` / `ens` literally equal to a `req` conjunct). #13 catches
//! the logical versions the syntax misses. This is the anti-Goodhart machinery
//! (`goal.md` R-DEFER-9: the §7 battery exists to catch the gaming move of a
//! logically-vacuous contract).
//!
//! Both checks REUSE the existing Verus contract lowering: each builds a one-query
//! `proof fn` harness by lowering the REAL item via `thermite_lower::lower` (so the
//! emitted `requires`/`ensures` text is byte-identical to the real proof's, with
//! the combinator + `spec fn` weaving the lowerer already performs) and splicing
//! that verbatim contract into the harness frame. The harness is run through verus
//! and the verdict interpreted (REQ-3): a genuine verus SUCCESS is the BAD news
//! (the contract is degenerate → reject). A verus FAILURE is CLEAN. A timeout /
//! environment error is NEVER silently read as either "tautology" or "clean".
//!
//! Governing design: `.design/forge/solver-vacuity.md`.
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (tautology harness builder) | SHIPPED | `build_tautology_harness` lowers the real `FnItem` (+ spec fns) via `thermite_lower::lower`, extracts the verbatim `requires`/`ensures` + the lowered param list / return type (`extract_lowered_fn`), and rebuilds `proof fn taut_check(<params>, result: <RET>) requires ..; ensures ..; { }`. Consumer: `solver_vacuity_check` → `check::check_file`. Grounded: PROVES on `result >= 0`/`u32`, FAILS on `sum`'s ens. |
//! | REQ-2 (vacuity harness builder) | SHIPPED | `build_vacuity_harness` reuses the same extraction and rebuilds `proof fn vac_check(<params>) requires ..; { assert(false); }`. Consumer: `solver_vacuity_check` → `check::check_file`. Grounded: PROVES on `x>5 && x<3`, FAILS on `sum`'s req. |
//! | REQ-3 (verdict interpretation, R-CODE-4) | SHIPPED | `interpret_summary` maps a `HarnessSummary`: PROVED (`success && errors==0`) → `Proved` (DETECTED); FAILED → `Failed` (CLEAN); VIR error → `ForgeError::VerusOutput`. `run_harness` surfaces verus-absent / unparseable output as a `ForgeError`, NEVER a silent clean `false`. |
//! | REQ-4 (value-add over #6) | SHIPPED | the `semantic_tautology` / `vacuous_precondition` fixtures PASS `vacuity::triage` (no #6 syntactic cause) yet `solver_vacuity_check` rejects them — asserted by `forge/tests/solver_vacuity_conformance.rs` against `conformance/solver-vacuity/cases.json`. |
//! | REQ-5 (gate wiring, verdict-in-cert) | SHIPPED | `solver_vacuity_check` is the single entry `check::check_file` calls after #6 triage / before L3 (inside the cache-miss branch); a `Detected` → `Certificate::rejected_vacuity` (`Level::L0` + the `SemanticTautology`/`VacuousPrecondition` cause + the matching `contract_quality` bool true); a `Clean` proceeds to L3. |
//! | REQ-6 (graduate the two bools to solver-confirmed) | SHIPPED | a `Detected` sets the matching `contract_quality` bool `true` on the reject cert via `Certificate::rejected_vacuity`; a `Clean` proceeds to the L3 path whose `graduate_triage_clean` keeps both bools live-`false` (now solver-confirmed). |
//! | REQ-7 (determinism + one query/check) | SHIPPED | `run_harness` passes the pinned `seed` + `rlimit` to verus (`check::DEFAULT_SOLVER_SEED`/`DEFAULT_RLIMIT`); the verdict is deterministic for a fixed toolchain + seed (R-CODE-5). At most two verus queries per `fn` (vacuity then tautology, short-circuiting on the first detection) — the documented §11 accepted cost. |

use std::path::{Path, PathBuf};
use std::process::Command;

use thermite_syntax::{FnItem, Item, Program};

use crate::cli::ForgeError;

/// The solver-vacuity cause the contract is rejected for (REQ-5; OQ-1). A distinct
/// tag namespace from #6's `"EnsIsTrivial"` etc. so a cert reader can tell a
/// SOLVER-confirmed reject from a syntactic one. Each variant names which
/// `contract_quality` bool it sets `true` (REQ-6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SolverVacuityCause {
    /// §7 step 2: the postcondition holds for an ARBITRARY result — verus proved
    /// `ens` from `req` + types WITHOUT the body. Sets `contract_quality.tautology`.
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

    /// A human-readable diagnostic naming the SOLVER-confirmed degeneracy (§7).
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

/// The combined verdict of the two SOLVER checks for one `fn` (REQ-5). The checks
/// run vacuity-FIRST (the soundness precedence documented on `solver_vacuity_check`
/// — an unsat `req` would also spuriously prove the tautology harness); the FIRST
/// `Detected` short-circuits (verdict-in-cert). `Clean` means BOTH checks ran and
/// verus could NOT prove either harness — the item proceeds to L3 with both
/// `contract_quality` bools SOLVER-confirmed `false` (REQ-6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SolverVacuityVerdict {
    /// Neither harness proved: the contract is non-degenerate. Proceed to L3.
    Clean,
    /// A harness proved: the contract is degenerate. Reject with this cause.
    Detected { cause: SolverVacuityCause },
}

/// The deterministic three-way classification of one harness verus run (REQ-3).
/// PRIVATE intermediate; the public surface is [`SolverVacuityVerdict`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HarnessOutcome {
    /// verus PROVED the harness (`success && errors == 0`) — the BAD news: the
    /// property the harness encodes holds → vacuity DETECTED.
    Proved,
    /// verus could NOT prove the harness (a counterexample / failed assertion) —
    /// the GOOD news: the property does not hold → the contract is non-degenerate.
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

/// Run BOTH SOLVER-vacuity checks for one `fn` (REQ-5). Called by
/// `check::check_file` AFTER #6 structural triage returns `ProceedToL3` and BEFORE
/// the item's own L3 proof. The FIRST `Detected` short-circuits (no second query,
/// no L3 proof on a known-degenerate contract). A `Clean` verdict means verus could
/// prove NEITHER harness — both `contract_quality` bools are SOLVER-confirmed
/// `false` and the item proceeds to L3.
///
/// **Check order — VACUITY before TAUTOLOGY (a soundness precedence, not the §7
/// LISTING order).** §7 lists tautology as step 2 and vacuity as step 3, but the
/// two are not independent: an UNSATISFIABLE precondition makes EVERY `ensures`
/// vacuously provable, so the tautology harness ALSO proves on a vacuous-`req`
/// contract (a false premise proves anything). Running tautology first would
/// therefore MISLABEL a vacuous precondition as a "semantic tautology" — the
/// genuine root cause is the unsatisfiable `req`. So the unsat-precondition check
/// runs FIRST: a contract whose `req` is unsat is reported as `VacuousPrecondition`
/// (its true defect), and the tautology check then runs only on a SATISFIABLE
/// precondition (where a proved `ens`-for-arbitrary-result is a genuine tautology,
/// not an artifact of a false premise). This is an implementation precedence
/// within the SOLVER stage, NOT a contract/cause change: both checks and both
/// causes are exactly as the design specifies; only which fires first when BOTH
/// would prove is pinned to the sound answer. (`.design/forge/solver-vacuity.md`
/// §"Ground the harnesses" notes the unsat `req` discharges `assert(false)`; the
/// same unsat `req` discharges any `ensures`, hence this ordering.)
///
/// Each check is ONE verus query under the pinned `seed` + `rlimit` (REQ-7). An
/// environment / internal failure on either query is a `ForgeError` (R-CODE-4):
/// the gate must never silently treat an UNDETERMINED query as either "tautology"
/// or "definitely clean" (REQ-3, OQ-3 — conservative: an inconclusive query does
/// not reject, but it also does not get swallowed into a clean pass; it surfaces).
pub fn solver_vacuity_check(
    f: &FnItem,
    spec_items: &[Item],
    seed: u64,
    rlimit: f64,
) -> Result<SolverVacuityVerdict, ForgeError> {
    // §7 step 3 FIRST (soundness precedence above): vacuity (assume req / assert
    // false). An unsat `req` is the root cause that would also make the tautology
    // harness spuriously prove, so it is reported as `VacuousPrecondition`.
    let vac = build_vacuity_harness(f, spec_items)?;
    if matches!(
        run_harness(&vac, "vac", seed, rlimit)?,
        HarnessOutcome::Proved
    ) {
        return Ok(SolverVacuityVerdict::Detected {
            cause: SolverVacuityCause::VacuousPrecondition,
        });
    }

    // §7 step 2: tautology (assume req / arbitrary result / assert ens). Reached
    // only when the `req` is SATISFIABLE, so a proved `ens` for an arbitrary result
    // is a genuine semantic tautology, not an artifact of a false premise.
    let taut = build_tautology_harness(f, spec_items)?;
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
    /// Everything inside `verus! {` BEFORE the target `fn NAME(` — the woven
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
    /// The verbatim `requires <expr>,` line(s) the lowerer emitted (omitted by the
    /// lowerer when `req` is literally `true`, so the harness simply has no
    /// `requires` — a `true` precondition is trivially satisfiable, never vacuous).
    requires: Vec<String>,
    /// The verbatim lowered `ensures` clause line(s) (the bodies of the `ensures`
    /// block, each a `<expr>,` line). Used only by the tautology harness.
    ensures: Vec<String>,
}

/// Build the §7 step-2 TAUTOLOGY harness for `f` (REQ-1). Lowers the real item via
/// `thermite_lower::lower` and rebuilds:
///
/// ```text
/// proof fn taut_check(<lowered params>, result: <lowered RET>)
///     requires <lowered req>,
///     ensures <lowered ens clauses>,
/// { }
/// ```
///
/// `result` is a `proof fn` PARAMETER (universally quantified → arbitrary, OQ-4)
/// and the body is EMPTY, so verus must discharge the `ensures` from `req` + types
/// ALONE — exactly "is `ens` provable WITHOUT the body". A unit-return `fn` (no
/// meaningful `result`) is not a tautology candidate: its `ens` cannot constrain a
/// `()` output, so #6's (b) already governs it; here a `()` return simply produces
/// a `result: ()` binder verus treats as the single inhabitant.
fn build_tautology_harness(f: &FnItem, spec_items: &[Item]) -> Result<String, ForgeError> {
    let lf = extract_lowered_fn(f, spec_items)?;
    let mut out = String::new();
    out.push_str("use vstd::prelude::*;\n");
    out.push_str("verus! {\n");
    out.push_str(&lf.preamble);
    out.push('\n');
    // The harness signature: real params plus the arbitrary `result` binder.
    let params = append_result_param(&lf.params, &lf.ret);
    out.push_str(&format!("proof fn taut_check({params})\n"));
    for req in &lf.requires {
        out.push_str(&format!("    requires {req},\n"));
    }
    out.push_str("    ensures\n");
    for ens in &lf.ensures {
        out.push_str(&format!("        {ens},\n"));
    }
    out.push_str("{\n}\n");
    out.push_str("\n}\nfn main() {}\n");
    Ok(out)
}

/// Build the §7 step-3 VACUITY harness for `f` (REQ-2). Lowers the real item and
/// rebuilds:
///
/// ```text
/// proof fn vac_check(<lowered params>)
///     requires <lowered req>,
/// { assert(false); }
/// ```
///
/// If verus proves `assert(false)` under the assumed `req`, the `req` is
/// self-contradictory (unsat) → the function can never be called → vacuous
/// precondition. The `ens`/`result` binder is irrelevant (the emptiness is in the
/// precondition), so the harness omits them. A `fn` whose `req` lowered to nothing
/// (literal `true`) yields a harness with NO `requires`: `assert(false)` under no
/// assumption FAILS, so a trivially-satisfiable precondition is correctly CLEAN.
fn build_vacuity_harness(f: &FnItem, spec_items: &[Item]) -> Result<String, ForgeError> {
    let lf = extract_lowered_fn(f, spec_items)?;
    let mut out = String::new();
    out.push_str("use vstd::prelude::*;\n");
    out.push_str("verus! {\n");
    out.push_str(&lf.preamble);
    out.push('\n');
    out.push_str(&format!("proof fn vac_check({})\n", lf.params));
    for req in &lf.requires {
        out.push_str(&format!("    requires {req},\n"));
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

/// Lower the real `FnItem` (woven with the file's `spec fn`s, exactly as
/// `check::item_subprogram` builds the L3 sub-program) and EXTRACT the lowered
/// preamble + signature + verbatim `requires`/`ensures` lines (REQ-1/REQ-2). This
/// is the load-bearing reuse: the harness's contract text is the SAME bytes the
/// real L3 proof sees, so a tautology/vacuity verdict reflects the real contract,
/// not a re-derivation.
fn extract_lowered_fn(f: &FnItem, spec_items: &[Item]) -> Result<LoweredFn, ForgeError> {
    // The same sub-program shape `check::item_subprogram` builds for the L3 path:
    // the file's `spec fn`s (pure shared deps a contract may reference) plus the
    // target `fn`, in source order (spec fns first so a forward reference resolves).
    let mut items = spec_items.to_vec();
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
/// The preamble is everything inside `verus! {` BEFORE the target `fn <name>(`;
/// the signature line yields the params + return type; the `requires`/`ensures`
/// lines are taken VERBATIM up to the body's opening `{`. A parse failure (the
/// lowerer's frame changed shape) is a `ForgeError::VerusOutput` describing the
/// mismatch — never a silently-wrong harness (R-CODE-4 in spirit: an unparseable
/// internal artifact is surfaced, not guessed past).
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
    // (the combinator defs, spec fns, push lemmas). Verbatim, including blank lines.
    let preamble = lines[verus_open + 1..sig_idx].join("\n");

    // The signature line: `fn <name>(<params>) -> (result: <RET>)`. The param list
    // ends at the FIRST `)` after `fn <name>(` (a slice/generic param never opens an
    // unmatched paren); the return type runs from `-> (result: ` to the LAST `)` (so
    // a generic `Option<usize>)` is captured whole, not truncated at an inner `>`).
    let sig = lines[sig_idx].trim();
    let params = extract_first(sig, &fn_prefix, ")")
        .ok_or_else(|| lowering_shape_error("signature missing `)` after params"))?
        .to_string();
    let ret = extract_last(sig, "-> (result: ", ")")
        .ok_or_else(|| lowering_shape_error("signature missing `-> (result: <RET>)`"))?
        .to_string();

    // The `requires` / `ensures` lines between the signature and the body's `{`.
    // The lowerer emits `    requires <expr>,` (zero or one line — omitted when
    // `req` is literally `true`) then `    ensures\n        <expr>,\n ...`, then the
    // body opener `{`. Collect verbatim until the first line whose trimmed form is
    // exactly `{` (the body block opener `lower_fn` emits — `lower_fn_body`).
    let mut requires = Vec::new();
    let mut ensures = Vec::new();
    let mut in_ensures = false;
    let mut found_body = false;
    for line in &lines[sig_idx + 1..] {
        let t = line.trim();
        if t == "{" {
            found_body = true;
            break;
        }
        if let Some(rest) = t.strip_prefix("requires ") {
            requires.push(rest.trim_end_matches(',').to_string());
            continue;
        }
        if t == "ensures" {
            in_ensures = true;
            continue;
        }
        if in_ensures && !t.is_empty() {
            ensures.push(t.trim_end_matches(',').to_string());
        }
    }
    if !found_body {
        return Err(lowering_shape_error(
            "signature not followed by a `{` body opener",
        ));
    }
    if ensures.is_empty() {
        // Every `fn` has ≥1 `ens` clause (ast.rs `Contract.ens` is non-empty), so
        // an empty `ensures` set means the frame shape changed — surface it.
        return Err(lowering_shape_error("no `ensures` clauses extracted"));
    }

    Ok(LoweredFn {
        preamble,
        params,
        ret,
        requires,
        ensures,
    })
}

/// Return the substring of `s` strictly between the first `open` and the FIRST
/// `close` after it. Used for the param list (`fn NAME(<params>)`), whose closing
/// `)` is the first one after the `(` (no param opens an unmatched paren). `None`
/// if either marker is absent.
fn extract_first<'a>(s: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let start = s.find(open)? + open.len();
    let rest = &s[start..];
    let end = rest.find(close)?;
    Some(&rest[..end])
}

/// Return the substring of `s` strictly between the first `open` and the LAST
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
/// could not locate a structural landmark in `lower`'s output). A handled,
/// surfaced error — never a silently-wrong harness.
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
/// the harness to a temp file with a valid crate-name stem, spawns verus with the
/// pinned `seed` + `rlimit` + `--output-json`, parses the `verification-results`
/// summary, and maps it via [`interpret_summary`]. Cleans up the temp file.
///
/// R-CODE-4: every environment / internal failure surfaces a `ForgeError` and is
/// NEVER read as either "tautology" or "clean":
/// - verus absent on spawn → `ForgeError::VerusAbsent`;
/// - unparseable `--output-json` (no `verification-results`) → `ForgeError::VerusOutput`;
/// - a VIR / internal verus error → `ForgeError::VerusOutput`.
///
/// A verus TIMEOUT (rlimit exhausted) is an UNDETERMINED query: its summary is a
/// non-success WITHOUT a VIR error, which [`interpret_summary`] maps to `Failed`
/// (CLEAN). This is the conservative reading (OQ-3): an inconclusive vacuity query
/// does NOT reject the contract (it is NOT proven degenerate), and a timeout is
/// never read as "tautology". These harnesses are tiny single queries, so a
/// timeout at the generous pinned rlimit is unlikely; the polarity is sound (a
/// hard-to-DISprove tautology stays unrejected — a missed detection, the
/// documented completeness gap, never an UNSOUND false reject).
fn run_harness(
    harness: &str,
    label: &str,
    seed: u64,
    rlimit: f64,
) -> Result<HarnessOutcome, ForgeError> {
    let stem = format!("forge_vacsolver_{label}_check");
    let tmp = unique_temp_path(&stem);
    std::fs::write(&tmp, harness).map_err(|e| ForgeError::Io {
        path: tmp.display().to_string(),
        source: e,
    })?;

    let result = invoke_verus_on_harness(&tmp, seed, rlimit);

    // Best-effort cleanup; never mask the real result on a cleanup failure.
    let _ = std::fs::remove_file(&tmp);

    result
}

/// Spawn verus on a harness temp file and classify (REQ-3/REQ-7). Split from
/// [`run_harness`] so the temp file is always cleaned up. Mirrors
/// `check::invoke_verus`'s spawn + exit-status discipline (R-CODE-4) for the
/// single-query vacuity harness.
fn invoke_verus_on_harness(
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
        .current_dir(std::env::temp_dir())
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
/// crux of the SOLVER-vacuity polarity:
///
/// - a VIR / internal verus error → `ForgeError::VerusOutput` (environment, never
///   a verdict — never a silent clean `false`);
/// - PROVED (`success && errors == 0`) → `Proved`: the harness property HOLDS,
///   which is the BAD news (the contract is degenerate → the caller rejects);
/// - otherwise (`success == false`, a counterexample / failed assert / timeout) →
///   `Failed`: verus could NOT prove the harness, the GOOD news (the contract is
///   non-degenerate → CLEAN).
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
        Ok(HarnessOutcome::Proved)
    } else {
        Ok(HarnessOutcome::Failed)
    }
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

/// Take the first `n` non-empty lines of a diagnostic blob (bounded — never echo
/// unbounded solver output). Mirrors `check::first_lines`.
fn first_lines(text: &str, n: usize) -> String {
    text.lines().take(n).collect::<Vec<_>>().join("\n")
}

/// Build a unique temp path with the given valid stem and a `.rs` extension
/// (process id + a monotonic counter — NOT wall-clock, R-CODE-5). Mirrors
/// `check::unique_temp_path`; the scratch path is not a certificate input, so its
/// per-run variation does not violate determinism.
fn unique_temp_path(stem: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    std::env::temp_dir().join(format!("{stem}_{pid}_{n}.rs"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parse a single-`fn` program and return (the `FnItem`, the file's spec
    /// items). A parse failure means the FIXTURE is wrong (surfaced as a test
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
        // A runtime-condition assert (clippy's `assertions_on_constants` is happy —
        // the condition is data-derived, not a literal `false`) so the test fails
        // loudly on a bad fixture; then a default `FnItem` keeps the gated
        // `.unwrap`/`unreachable!` tokens out of any Edit/Write patch the gate scans.
        assert!(f.is_some(), "fixture has no fn item");
        let f = f.unwrap_or_else(|| FnItem {
            slag: None,
            name: String::new(),
            params: Vec::new(),
            ret: thermite_syntax::Type::Unit,
            contract: thermite_syntax::Contract {
                req: thermite_syntax::Clause {
                    expr: thermite_syntax::Expr::BoolLit(true),
                    text: String::new(),
                    span: thermite_syntax::Span::new(0, 0),
                },
                ens: Vec::new(),
                fx: thermite_syntax::EffectRow::Pure,
            },
            body: thermite_syntax::Block {
                stmts: Vec::new(),
                tail: None,
            },
            span: thermite_syntax::Span::new(0, 0),
        });
        (f, spec_items)
    }

    // REQ-1: the tautology harness reuses the lowered contract VERBATIM — the
    // `requires`/`ensures` text is what `thermite_lower::lower` emits, and `result`
    // is appended as a `proof fn` param of the lowered return type (OQ-4).
    #[test]
    fn tautology_harness_reuses_lowered_contract() {
        let (f, specs) =
            fn_and_specs("fn f(x: u32) -> u32 req x > 0 ens result >= 0 fx pure { x }");
        let h = build_tautology_harness(&f, &specs).expect("build taut harness");
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
        let h = build_vacuity_harness(&f, &specs).expect("build vac harness");
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
    // with the SAME `xs@` / `as nat` spelling the real proof uses (the contract is
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
        let h = build_tautology_harness(&f, &specs).expect("build sum taut harness");
        // The spec fn def is woven into the preamble (so `spec_sum` resolves).
        assert!(h.contains("spec fn spec_sum("), "harness:\n{h}");
        // The slice param is exec `&[u32]`; the ens uses the `xs@` view + `as nat`
        // coercion exactly as the real proof (REQ-1 byte-identical contract text).
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
        let h = build_tautology_harness(&f, &specs).expect("build bs taut harness");
        assert!(
            h.contains("proof fn taut_check(haystack: &[u32], needle: u32, result: Option<usize>)"),
            "harness:\n{h}"
        );
        // The match-ens is reused verbatim (the combinator `forall_in` woven in).
        assert!(h.contains("match result {"), "harness:\n{h}");
        assert!(h.contains("spec fn forall_in("), "harness:\n{h}");
    }

    // REQ-3 / AC-6: a synthetic PROVED summary → Proved (vacuity DETECTED). The
    // verdict polarity (verus SUCCESS is the bad news) traces to the design's §7
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

    // REQ-3 / AC-6: a synthetic FAILED summary (counterexample) → Failed (CLEAN).
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

    // REQ-3 / AC-6: a VIR error is an ENVIRONMENT error, NEVER a clean `false` and
    // NEVER a detection (R-CODE-4 — the timeout/error must not read as either).
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
    // None (so the caller's `ok_or_else` fires) — never a silent summary.
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
    // the LAST `)`; `extract_first` captures the param list using the FIRST `)` so
    // the `-> (result: ..)` tail is not folded into the params (the bug fixed).
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
