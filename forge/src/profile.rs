//! `forge/src/profile.rs` — solver profiles as proof-repair prompts (issue #11,
//! `.design/forge/solver-profiles.md`). When verus cannot PROVE an item within
//! its resource budget — a TIMEOUT / rlimit exhaustion, NOT a real
//! COUNTEREXAMPLE — `forge check` surfaces WHY, not an opaque "timeout". This
//! module parses verus's `--profile` / `--profile-all` Z3
//! quantifier-instantiation report (landing on STDERR) into a structured
//! [`SolverProfile`] and renders it as actionable proof-repair prompts (the
//! top-instantiated quantifier, its selected trigger, its share of the budget,
//! and a heuristic hint).
//!
//! This component PRODUCES the structured prompts. It does NOT retry (the
//! proof-repair loop is #18) and does NOT auto-degrade L3→L2→L1 (#10).
//!
//! The profile is DIAGNOSTIC and NON-deterministic (§5.3) — like
//! `solver_time_ms`, it is oracle-EXCLUDED. The RENDERING ([`render_prompts`] /
//! [`suggested_move`]) is deterministic given a `SolverProfile`; the input
//! profile (the Z3 instantiation counts) is not.
//!
//! ## Grounded profiler format (real verus 0.2026.05.24, Z3 4.12.5)
//!
//! Captured by running `~/.local/bin/verus --profile-all --verify-root` on a
//! transitivity / connectivity quantifier set (the checked-in fixture
//! `tests/golden/profile/connectivity.profile.txt`). [`parse_profile`] anchors
//! to this shape (R-CHAR-3 — verus's report, never forge's own output):
//!
//! ```text
//! note: Observed 14 total instantiations of user-level quantifiers
//! note: Cost * Instantiations: 150 (Instantiated 10 times - 71% of the total, cost 15) top 1 of 2 user-level quantifiers.
//!
//!  --> /tmp/pa_check.rs:13:51
//!    |
//! 13 |         forall|x: int, y: int, z: int| #[trigger] e(x, y) && #[trigger] e(y, z) ==> e(x, z),
//!    |         ------------------------------------------^^^^^^^---------------^^^^^^^------------ Triggers selected for this quantifier
//! ```
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (`SolverProfile` schema) | SHIPPED | `pub struct SolverProfile { total_instantiations, quantifiers: Vec<QuantifierProfile> }` + `pub struct QuantifierProfile { trigger, instantiations, pct_of_total, cost, cost_x_instantiations, span }`. Consumer: `check::classify_verus_outcome` attaches it on a timeout. Oracle-EXCLUDED (`manifest::Certificate::oracle_subset` omits `solver_profile`). |
//! | REQ-2 (profile capture on rlimit-hit) | SHIPPED | `check::invoke_verus` always passes `--profile` + the pinned `--rlimit`; the profiler report lands on STDERR and is parsed by `parse_profile`. The capture is the verus stderr blob (no separate spawn — the single `--profile` run carries it on an rlimit-hit, the cheapest correct path, OQ-3). |
//! | REQ-3 (parse the Z3 report) | SHIPPED | `pub fn parse_profile(stderr) -> Option<SolverProfile>` parses the `Observed N total instantiations` line + each `Cost * Instantiations:` block + its `--> file:line:col` span + the selected-trigger source line; tolerant (returns `None` when no report present). Consumer: `check::classify_verus_outcome`. |
//! | REQ-4 (render proof-repair prompts) | SHIPPED | `pub fn render_prompts(&SolverProfile) -> Vec<SuggestedMove>` + `pub fn suggested_move(&SolverProfile) -> Option<SuggestedMove>` name the top quantifier's trigger + share with a trigger-loop hint when one quantifier dominates. Deterministic. Consumer: `check::classify_verus_outcome` populates `Certificate.suggested_move`. |
//! | REQ-5 (three-way classification) | SHIPPED (in `check.rs`) | `check::classify_verus_outcome` is the timeout-vs-counterexample-vs-success split; this module supplies the `SolverProfile` it attaches on the timeout branch. |
//! | REQ-6 (additive cert slot, oracle-excluded) | SHIPPED (in `manifest.rs`) | `Certificate.solver_profile: Option<SolverProfile>` (additive, skip-if-none) + the reserved `suggested_move`; both excluded from `oracle_subset`. This module defines `SolverProfile`. |
//! | REQ-7 (timeout cert level, distinct) | SHIPPED (in `check.rs`) | a timeout cert is `Certificate::timeout` (`Level::L0` + `RejectReason { cause: "VerusTimeout" }` + the profile), distinct from a counterexample-L0 (no profile, a `postcondition not satisfied` reason). |

use serde::{Deserialize, Serialize};

use crate::manifest::SuggestedMove;

/// The structured Z3 quantifier-instantiation report verus emits under
/// `--profile` / `--profile-all` on an rlimit-hit (`.design/forge/solver-profiles.md`
/// REQ-1). DIAGNOSTIC and NON-deterministic (§5.3) — oracle-EXCLUDED, like
/// `solver_time_ms`. Additive on the certificate (`Certificate.solver_profile`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SolverProfile {
    /// The total user-level quantifier instantiation count (the
    /// `note: Observed N total instantiations of user-level quantifiers` line).
    pub total_instantiations: u64,
    /// The ranked quantifier entries (verus ranks by `cost * instantiations`,
    /// descending — `top 1 of k`, `top 2 of k`, …), in report order.
    pub quantifiers: Vec<QuantifierProfile>,
}

/// One ranked quantifier in the Z3 instantiation report (REQ-1). Each
/// `Cost * Instantiations:` block plus its `-->` span and selected-trigger
/// source annotation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuantifierProfile {
    /// The selected trigger text reconstructed from the source annotation (the
    /// `^^^^` carets under the `forall|…|` line mark the chosen trigger terms),
    /// e.g. `e(x, y) && e(y, z)`. Best-effort: falls back to the whole `forall`
    /// body when the caret reconstruction is ambiguous.
    pub trigger: String,
    /// How many times this quantifier was instantiated
    /// (`Instantiated N times`).
    pub instantiations: u64,
    /// This quantifier's share of `total_instantiations`, as a whole-number
    /// percent (`N% of the total`).
    pub pct_of_total: u64,
    /// The per-instantiation cost verus reports (`cost C`).
    pub cost: u64,
    /// The `cost * instantiations` product verus ranks by (the leading
    /// `Cost * Instantiations: P`).
    pub cost_x_instantiations: u64,
    /// The `file:line:col` span of the quantifier (the `--> file:line:col`
    /// line), basename only so the cert does not leak the temp path.
    pub span: String,
}

/// A quantifier whose share of the instantiation budget exceeds this percent is
/// flagged as the dominant bottleneck / a likely trigger loop in the rendered
/// prompt (REQ-4 heuristic). A deterministic threshold (R-CODE-5).
const DOMINANCE_PCT: u64 = 50;

/// Parse verus's `--profile` / `--profile-all` Z3 instantiation report from a
/// stderr blob into a [`SolverProfile`] (REQ-3). Returns `None` when no profiler
/// report is present (the `Observed N total instantiations` line is absent) —
/// the signal that this run did NOT emit a profile (so it is NOT a timeout;
/// `check::classify_verus_outcome` uses presence as the timeout discriminator).
///
/// Tolerant / best-effort: a `Cost * Instantiations:` block whose fields cannot
/// all be parsed is skipped rather than failing the whole parse (do not over-fit
/// to one Z3 version's exact wording).
pub fn parse_profile(stderr: &str) -> Option<SolverProfile> {
    let total = parse_total_instantiations(stderr)?;
    let quantifiers = parse_quantifier_blocks(stderr);
    Some(SolverProfile {
        total_instantiations: total,
        quantifiers,
    })
}

/// Parse the `note: Observed N total instantiations of user-level quantifiers`
/// line. The presence of this line is the profiler-report signal (REQ-3).
fn parse_total_instantiations(stderr: &str) -> Option<u64> {
    for line in stderr.lines() {
        let trimmed = line.trim_start();
        // Tolerant of the `note: ` prefix being present or absent.
        let rest = trimmed.strip_prefix("note: ").unwrap_or(trimmed);
        if let Some(after) = rest.strip_prefix("Observed ") {
            // `Observed N total instantiations of user-level quantifiers`.
            let n: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(value) = n.parse::<u64>() {
                return Some(value);
            }
        }
    }
    None
}

/// Parse every `Cost * Instantiations:` block (REQ-3). Each block is the
/// `note: Cost * Instantiations: P (Instantiated N times - X% of the total,
/// cost C) top i of k …` line; its `--> file:line:col` span and selected-trigger
/// source line follow within the next few lines.
fn parse_quantifier_blocks(stderr: &str) -> Vec<QuantifierProfile> {
    let lines: Vec<&str> = stderr.lines().collect();
    let mut out = Vec::new();
    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        let rest = trimmed.strip_prefix("note: ").unwrap_or(trimmed);
        let Some(after) = rest.strip_prefix("Cost * Instantiations: ") else {
            continue;
        };
        let Some((cost_x_instantiations, instantiations, pct_of_total, cost)) =
            parse_cost_line(after)
        else {
            // Tolerant: a block whose header does not parse is skipped, not fatal.
            continue;
        };
        // The `--> file:line:col` span and the `forall … Triggers selected …`
        // source annotation land on the following lines (a blank line, then the
        // span, then `|`, then the source, then the caret line). Scan a bounded
        // window forward.
        let window = lines.iter().skip(idx + 1).take(8).copied();
        let (span, trigger) = parse_span_and_trigger(window);
        out.push(QuantifierProfile {
            trigger,
            instantiations,
            pct_of_total,
            cost,
            cost_x_instantiations,
            span,
        });
    }
    out
}

/// Parse the body of a `Cost * Instantiations:` line into
/// `(cost_x_instantiations, instantiations, pct_of_total, cost)`. The grounded
/// shape is `P (Instantiated N times - X% of the total, cost C) top i of k …`.
fn parse_cost_line(after: &str) -> Option<(u64, u64, u64, u64)> {
    // Leading `P`.
    let cost_x: u64 = take_leading_u64(after)?;
    // `Instantiated N times`.
    let inst = number_after(after, "Instantiated ")?;
    // `N% of the total` — the percent immediately precedes a `%`.
    let pct = percent_before_sign(after)?;
    // `cost C`.
    let cost = number_after(after, "cost ")?;
    Some((cost_x, inst, pct, cost))
}

/// Parse the leading run of ASCII digits at the start of `s` into a `u64`.
fn take_leading_u64(s: &str) -> Option<u64> {
    let digits: String = s
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

/// Parse the run of ASCII digits immediately following the first occurrence of
/// `marker` in `s` into a `u64`.
fn number_after(s: &str, marker: &str) -> Option<u64> {
    let pos = s.find(marker)? + marker.len();
    let digits: String = s[pos..]
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

/// Parse the percent value: the run of digits immediately preceding the first
/// `%` in `s` (the `X% of the total` field).
fn percent_before_sign(s: &str) -> Option<u64> {
    let pct_pos = s.find('%')?;
    let prefix = &s[..pct_pos];
    let digits: String = prefix
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    digits.parse().ok()
}

/// From the lines following a cost block, extract the `(span, trigger)`: the
/// `--> file:line:col` span (basename only) and the selected-trigger text
/// reconstructed from the source `forall|…|` line + its caret annotation. A
/// missing span yields the empty string; a missing trigger falls back to the
/// `forall` body.
fn parse_span_and_trigger<'a>(window: impl Iterator<Item = &'a str>) -> (String, String) {
    let mut span = String::new();
    let mut source_line: Option<String> = None;
    let mut caret_line: Option<String> = None;
    for raw in window {
        let line = raw.trim_start();
        if span.is_empty() {
            if let Some(parsed) = parse_span(line) {
                span = parsed;
                continue;
            }
        }
        // The source line carries the `forall|` keyword (after the `N | ` gutter).
        if source_line.is_none() && line.contains("forall|") {
            source_line = Some(strip_gutter(line));
            continue;
        }
        // The caret line carries the `Triggers selected for this quantifier`
        // annotation under the source.
        if source_line.is_some()
            && caret_line.is_none()
            && line.contains("Triggers selected for this quantifier")
        {
            caret_line = Some(strip_gutter(line));
            break;
        }
    }
    let trigger = reconstruct_trigger(source_line.as_deref(), caret_line.as_deref());
    (span, trigger)
}

/// Strip a verus source-gutter prefix (`<digits> | ` or `| `) from a diagnostic
/// line, returning the content after it.
fn strip_gutter(line: &str) -> String {
    if let Some((before, after)) = line.split_once('|') {
        // Only strip when the part before `|` is a gutter (blank or a line
        // number), so a body that itself contains `|` is not mangled.
        if before.trim().chars().all(|c| c.is_ascii_digit()) {
            return after.trim_start().to_string();
        }
    }
    line.to_string()
}

/// Reconstruct the selected trigger from the source `forall` line and the caret
/// annotation underneath it. The carets (`^`) mark the trigger TERMS; the
/// dashes (`-`) mark non-trigger spans. We extract each maximal caret run's
/// underlying source substring and join them with ` && ` (the conventional
/// multi-trigger conjunction rendering, REQ-4). Best-effort: when there is no
/// usable caret line, fall back to the `forall|…|` body.
fn reconstruct_trigger(source: Option<&str>, caret: Option<&str>) -> String {
    let Some(source) = source else {
        return String::new();
    };
    if let Some(caret) = caret {
        let src: Vec<char> = source.chars().collect();
        let mut terms: Vec<String> = Vec::new();
        let mut current = String::new();
        for (i, c) in caret.chars().enumerate() {
            if c == '^' {
                if let Some(&sc) = src.get(i) {
                    current.push(sc);
                }
            } else if !current.is_empty() {
                terms.push(current.trim().to_string());
                current.clear();
            }
        }
        if !current.trim().is_empty() {
            terms.push(current.trim().to_string());
        }
        let terms: Vec<String> = terms.into_iter().filter(|t| !t.is_empty()).collect();
        if !terms.is_empty() {
            return terms.join(" && ");
        }
    }
    // Fallback: the `forall|…|` body (after the binder list), trimmed of a
    // trailing comma.
    fallback_forall_body(source)
}

/// Fallback trigger text: the body of a `forall|binders| BODY` source line
/// (everything after the closing `|` of the binder list), trimmed of trailing
/// punctuation. Used when the caret annotation is unavailable.
fn fallback_forall_body(source: &str) -> String {
    let Some(after_kw) = source.find("forall|") else {
        return source.trim().trim_end_matches(',').to_string();
    };
    let rest = &source[after_kw + "forall|".len()..];
    // Skip past the binder list's closing `|`.
    if let Some(bar) = rest.find('|') {
        rest[bar + 1..].trim().trim_end_matches(',').to_string()
    } else {
        rest.trim().trim_end_matches(',').to_string()
    }
}

/// Parse a `--> <file>:<line>:<col>` span line into `file:line:col` (basename
/// only, so the cert does not leak the temp path). Returns `None` if the line is
/// not a span. Mirrors `check::parse_span` (kept local so `profile.rs` has no
/// cross-module private dependency).
fn parse_span(line: &str) -> Option<String> {
    let rest = line.strip_prefix("--> ")?;
    let (path_part, loc) = rest.rsplit_once(':').and_then(|(head, col)| {
        head.rsplit_once(':')
            .map(|(p, line)| (p, format!("{line}:{col}")))
    })?;
    let base = std::path::Path::new(path_part)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(path_part);
    Some(format!("{base}:{loc}"))
}

/// Render a [`SolverProfile`] into proof-repair prompts — one [`SuggestedMove`]
/// per ranked quantifier (REQ-4). Each names the quantifier's trigger, its
/// instantiation count and share of the budget, and a heuristic hint
/// (trigger-loop suspicion when one quantifier dominates, [`DOMINANCE_PCT`]).
/// DETERMINISTIC given the profile (R-CODE-5 / AC-6).
pub fn render_prompts(profile: &SolverProfile) -> Vec<SuggestedMove> {
    profile
        .quantifiers
        .iter()
        .enumerate()
        .map(|(rank, q)| {
            let dominant = q.pct_of_total >= DOMINANCE_PCT;
            let hint = if dominant {
                "likely a trigger loop / the bottleneck — narrow its range, add a tighter \
                 trigger, or introduce a lemma"
            } else {
                "consider a tighter trigger or a supporting lemma if the proof times out"
            };
            let detail = format!(
                "quantifier `{trigger}` (at {span}) instantiated {inst} times ({pct}% of the \
                 {total} total, cost {cost}) — {hint}",
                trigger = q.trigger,
                span = q.span,
                inst = q.instantiations,
                pct = q.pct_of_total,
                total = profile.total_instantiations,
                cost = q.cost,
            );
            let kind = if rank == 0 && dominant {
                "trigger-loop"
            } else {
                "trigger-hint"
            };
            SuggestedMove {
                kind: kind.to_string(),
                detail,
            }
        })
        .collect()
}

/// The single human-facing proof-repair hint for the certificate's reserved
/// `suggested_move` slot (REQ-4, §5.1): the TOP-instantiated quantifier's prompt
/// (the dominant bottleneck), or `None` when the profile ranks no quantifiers.
/// DETERMINISTIC given the profile.
pub fn suggested_move(profile: &SolverProfile) -> Option<SuggestedMove> {
    render_prompts(profile).into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The captured real-verus profiler blob (the checked-in fixture), spliced
    /// inline so this unit test is hermetic. Hand-derived expected values are
    /// asserted in `parse_profile_*` (R-CHAR-3 — verus's report, not forge's).
    const BLOB: &str = "\
note: verifying root module

note: Analyzing prover log for root module ...

Z3 4.12.5
note: Log analysis complete for root module

note: Profile statistics for root module

note: Observed 14 total instantiations of user-level quantifiers

note: Cost * Instantiations: 150 (Instantiated 10 times - 71% of the total, cost 15) top 1 of 2 user-level quantifiers.

  --> /tmp/pa_check.rs:13:51
   |
13 |         forall|x: int, y: int, z: int| #[trigger] e(x, y) && #[trigger] e(y, z) ==> e(x, z),
   |         ------------------------------------------^^^^^^^---------------^^^^^^^------------ Triggers selected for this quantifier

note: Cost * Instantiations: 44 (Instantiated 4 times - 28% of the total, cost 11) top 2 of 2 user-level quantifiers.

  --> /tmp/pa_check.rs:12:43
   |
12 |         forall|x: int, y: int| #[trigger] e(x, y) ==> e(y, x),
   |         ----------------------------------^^^^^^^------------ Triggers selected for this quantifier
";

    // REQ-3 / AC-1: parse the captured blob; assert HAND-DERIVED top fields read
    // off the real report (14 total; top quantifier 10 inst / 71% / cost 15 /
    // cost*inst 150; second 4 / 28% / cost 11 / 44). R-CHAR-3 — these come from
    // the fixture's text, not from re-running the parser.
    #[test]
    fn parse_profile_hand_derived_fields() {
        let profile = parse_profile(BLOB).expect("blob carries a profiler report");
        assert_eq!(profile.total_instantiations, 14);
        assert_eq!(profile.quantifiers.len(), 2);

        let top = &profile.quantifiers[0];
        assert_eq!(top.instantiations, 10);
        assert_eq!(top.pct_of_total, 71);
        assert_eq!(top.cost, 15);
        assert_eq!(top.cost_x_instantiations, 150);
        assert_eq!(top.span, "pa_check.rs:13:51");

        let second = &profile.quantifiers[1];
        assert_eq!(second.instantiations, 4);
        assert_eq!(second.pct_of_total, 28);
        assert_eq!(second.cost, 11);
        assert_eq!(second.cost_x_instantiations, 44);
        assert_eq!(second.span, "pa_check.rs:12:43");
    }

    // REQ-3 / AC-1: the reconstructed trigger of the transitivity quantifier is
    // the two caret-marked terms joined — `e(x, y) && e(y, z)` (hand-read off the
    // caret line in the blob, R-CHAR-3).
    #[test]
    fn parse_profile_reconstructs_trigger_from_carets() {
        let profile = parse_profile(BLOB).expect("report present");
        assert_eq!(profile.quantifiers[0].trigger, "e(x, y) && e(y, z)");
        // The symmetry quantifier has a single caret term `e(x, y)`.
        assert_eq!(profile.quantifiers[1].trigger, "e(x, y)");
    }

    // REQ-3: a stderr WITHOUT a profiler report (the fast-unknown / counterexample
    // path) yields `None` — the discriminator `check.rs` relies on (no `Observed`
    // line → not a timeout).
    #[test]
    fn parse_profile_none_without_report() {
        let no_report = "error: postcondition not satisfied\n --> /tmp/x.rs:5:13\n  |\nerror: aborting due to 1 previous error\n";
        assert!(parse_profile(no_report).is_none());
        assert!(parse_profile("").is_none());
    }

    // REQ-3 tolerance: a malformed cost block is skipped, but the total + the
    // well-formed block still parse (best-effort, do not over-fit).
    #[test]
    fn parse_profile_tolerant_of_malformed_block() {
        let blob = "\
note: Observed 7 total instantiations of user-level quantifiers
note: Cost * Instantiations: garbage line that does not parse
note: Cost * Instantiations: 30 (Instantiated 3 times - 42% of the total, cost 10) top 1 of 1 user-level quantifiers.

 --> /tmp/x.rs:9:5
  |
9 |         forall|x: int| #[trigger] q(x) ==> q(x + 1),
  |                        ----------^^^^----------- Triggers selected for this quantifier
";
        let profile = parse_profile(blob).expect("total present");
        assert_eq!(profile.total_instantiations, 7);
        assert_eq!(
            profile.quantifiers.len(),
            1,
            "the unparseable block is skipped, the good one kept"
        );
        assert_eq!(profile.quantifiers[0].instantiations, 3);
    }

    // REQ-4 / AC-2: the rendered top prompt names the trigger, the instantiation
    // count, and (since 71% >= DOMINANCE_PCT) the trigger-loop hint.
    #[test]
    fn render_prompts_names_bottleneck() {
        let profile = parse_profile(BLOB).expect("report present");
        let prompts = render_prompts(&profile);
        assert_eq!(prompts.len(), 2);
        let top = &prompts[0];
        assert_eq!(
            top.kind, "trigger-loop",
            "the dominant quantifier is flagged"
        );
        assert!(
            top.detail.contains("e(x, y) && e(y, z)"),
            "names the trigger: {}",
            top.detail
        );
        assert!(
            top.detail.contains("10 times"),
            "names the instantiation count: {}",
            top.detail
        );
        assert!(
            top.detail.contains("71%"),
            "names the share: {}",
            top.detail
        );
        assert!(
            top.detail.contains("trigger loop"),
            "carries the trigger-loop hint: {}",
            top.detail
        );
        // The non-dominant second quantifier is a plain trigger-hint.
        assert_eq!(prompts[1].kind, "trigger-hint");
    }

    // REQ-4: `suggested_move` is the TOP prompt; `None` for an empty profile.
    #[test]
    fn suggested_move_is_top_prompt() {
        let profile = parse_profile(BLOB).expect("report present");
        let mv = suggested_move(&profile).expect("a ranked quantifier");
        assert_eq!(mv.kind, "trigger-loop");
        assert!(mv.detail.contains("e(x, y) && e(y, z)"));

        let empty = SolverProfile {
            total_instantiations: 0,
            quantifiers: vec![],
        };
        assert!(suggested_move(&empty).is_none());
    }

    // AC-6: rendering the SAME profile twice is byte-identical (R-CODE-5). Does
    // NOT assert the profile is reproducible across verus runs (it is not).
    #[test]
    fn render_is_deterministic() {
        let profile = parse_profile(BLOB).expect("report present");
        let a = render_prompts(&profile);
        let b = render_prompts(&profile);
        assert_eq!(a, b);
    }
}
