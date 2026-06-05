//! `forge/src/repair.rs` — the background L1/L2 → L3 proof-repair loop (issue #18,
//! `.design/forge/proof-repair.md`; `thermite-design.md` §6, Appendix B). Once a
//! `forge check` run has left an item BELOW L3 because verus could not PROVE it
//! within its resource budget (a TIMEOUT — INCONCLUSIVENESS, the #10 ladder /
//! #11 classification), repair tries to drive that item back UP to L3 by the one
//! mechanical, checkable move §6 sanctions: BUDGET ESCALATION. It re-verifies the
//! item along a FIXED, BOUNDED geometric `--rlimit` ladder (reusing the SAME
//! `check.rs` verus driver); the first budget at which the item PROVES is a real
//! upgrade to L3 (the winning budget is recorded). An item that never proves even
//! at the cap stays sub-L3 and repair surfaces the #11 repair PROMPT (the
//! `SolverProfile` + `suggested_move` + the failing obligation) for the agent's
//! own move (a custom proof hint).
//!
//! **THE ANTI-CHEAT INVARIANT (R-DEFER-9, §12 — the load-bearing property).**
//! Repair escalates a TIMEOUT and NOTHING ELSE. A COUNTEREXAMPLE (verus DISPROVED
//! the contract — `postcondition not satisfied`, NO profile report), a vacuity
//! reject (`rejected_vacuity`), a weak-contract reject (`rejected_weak_contract`),
//! and any non-timeout sub-L3 status are HARD FAILS: repair REPORTS them and NEVER
//! retries them at a higher budget, NEVER upgrades them to L3. More budget never
//! makes a false thing true. The gate is [`classify_sub_l3`] returning
//! [`SubL3Status::Timeout`] (the SOLE trigger for [`escalate`]); every other
//! verdict short-circuits to [`SubL3Status::NotRepairable`] and the escalation
//! closure is NEVER invoked. This is the exact dual of the #10 down-ladder's
//! anti-cheat (`degrade.rs`): inconclusiveness moves, falsity does not.
//!
//! This module COMPOSES the shipped pieces; it owns NO new prover-invocation
//! logic — it re-drives `check::check_file_with_rlimit` (the `--rlimit` seam, #11)
//! at each escalated rung, maps the resulting `Certificate` into a
//! [`RepairVerdict`], and reads the #11 `SolverProfile`/`suggested_move` off a
//! still-failing cert for the prompt.
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (`forge repair [item]` upgrade loop) | SHIPPED | `pub fn repair_file` runs `check::check_file_with_rlimit(path, DEFAULT_RLIMIT)`, classifies each cert via `classify_sub_l3`, and for a `SubL3Status::Timeout` drives `escalate` along [`REPAIR_LADDER`], upgrading to L3 on the first rung that PROVES (recording the budget) else surfacing the prompt. Consumer: `cli::run_repair` (`cli.rs`). |
//! | REQ-2 (anti-cheat: only a TIMEOUT is retried) | SHIPPED | `classify_sub_l3` returns `Timeout` ONLY for a `VerusTimeout` reject / a `lowered_assurance` (`VerusTimeout` degrade) cert; a counterexample / vacuity / weak-contract reject / un-discharged `L0` returns `NotRepairable`. `escalate` is called ONLY on `Timeout` (gated in `repair_item`), so the escalation closure NEVER runs for a non-timeout. Tests `counterexample_is_never_retried` + `rejects_are_never_retried`. |
//! | REQ-3 (bounded escalation ladder) | SHIPPED | [`REPAIR_LADDER`] is a frozen `[f64; 4]` of `--rlimit` multipliers (`2,4,8,16`); `escalate` iterates it in order and STOPS at the cap, so it ALWAYS terminates per item (a never-proving timeout makes exactly `REPAIR_LADDER.len()` attempts). Test `escalation_is_bounded_and_terminates`. |
//! | REQ-4 (cache-backed re-verify) | SHIPPED | each rung calls `check::check_file_with_rlimit` at a NON-default (escalated) budget, which `check.rs` documents bypasses the cache (a budget-dependent verdict is never cached as the canonical proof); per OQ-2 reading (a), escalated verifies are uncached in the v0.5 kernel. |
//! | REQ-5 (determinism of the achieved level) | SHIPPED | [`REPAIR_LADDER`] is a const + the per-rung verdict is the deterministic `check::classify_verus_outcome` under the pinned seed, so the achieved level + the recorded winning budget are a deterministic function of the item + the ladder (R-CODE-5). Test `escalation_is_deterministic`. |
//! | REQ-6 (the repair report) | SHIPPED | `pub struct RepairReport { items: Vec<RepairItem> }` with `enum RepairOutcome { UpgradedToL3 { budget }, StillSubL3 { level, prompt }, NotRepairable { reason } }`; an already-L3 item is a NO-OP (not in `items`). Consumer: `cli::run_repair` + `cli::render_repair`. |
//! | REQ-7 (subprocess failure is an error, never an upgrade) | SHIPPED | `escalate`'s verifier closure returns `Result<RepairVerdict, ForgeError>`; an environment failure (`VerusAbsent`/`VerusOutput`) propagates via the `?` out of `repair_file`, NEVER a still-sub-L3 verdict and NEVER a silent upgrade (R-CODE-4). Test `environment_error_propagates`. |

use crate::check::{self, DEFAULT_RLIMIT};
use crate::cli::ForgeError;
use crate::manifest::{Certificate, Level, SuggestedMove};
use crate::profile::SolverProfile;

/// The FIXED, BOUNDED geometric escalation ladder (REQ-3;
/// `.design/forge/proof-repair.md` REQ-3). Each entry is a MULTIPLIER of the
/// canonical [`DEFAULT_RLIMIT`] verus SMT budget, tried in ascending order: a
/// TIMEOUT item is re-verified at `DEFAULT_RLIMIT * 2`, then `* 4`, `* 8`, `* 16`
/// (the cap). FROZEN const (no wall-clock, no adaptive budget): the achieved
/// level after repair is a deterministic function of the item + this ladder
/// (REQ-5, R-CODE-5). The ladder is finite, so [`escalate`] ALWAYS terminates per
/// item — an item that never proves even at the `* 16` cap is reported still-
/// sub-L3, never retried forever (the §11 "never weaken the gate", bounded).
pub const REPAIR_LADDER: [f64; 4] = [2.0, 4.0, 8.0, 16.0];

/// The repair-relevant THREE-WAY verdict of one re-verify at an escalated budget
/// (REQ-1/REQ-2). The production path maps a re-verified [`Certificate`] into this
/// via [`verdict_from_cert`]; the hermetic tests synthesize it directly. Mirrors
/// the #11 `check::VerusOutcome` discriminants the loop cares about, but is a
/// public repair-local type so the bounded loop ([`escalate`]) is unit-testable on
/// SYNTHESIZED per-rung verdicts without provoking a fragile live resourceout
/// (OQ-1, the #10/#11 precedent).
#[derive(Debug, Clone)]
pub enum RepairVerdict {
    /// verus PROVED the item at this budget → upgrade to L3 (terminal).
    Proved,
    /// verus TIMED OUT at this budget (still inconclusive) → try the next rung.
    /// Carries the #11 repair prompt material (the `SolverProfile` + the headline
    /// `suggested_move`) for the still-sub-L3 report when the ladder is exhausted.
    Timeout {
        /// The Z3 instantiation profile from this (still-failing) rung, surfaced as
        /// the repair prompt if the cap is reached without a proof (REQ-6).
        profile: Option<SolverProfile>,
        /// The profile-derived headline proof-repair hint (REQ-6).
        suggested_move: Option<SuggestedMove>,
        /// The failing-obligation detail ("here's where I got lost").
        detail: String,
    },
    /// verus DISPROVED the item (a COUNTEREXAMPLE) at this budget → HARD FAIL. The
    /// anti-cheat (REQ-2) gates [`escalate`] so it is NEVER reached for an item
    /// that started as a counterexample; this variant exists for completeness (a
    /// timeout item that DISPROVES at a higher budget — sound: report, never
    /// upgrade) and is treated as the terminal not-repairable outcome.
    Counterexample {
        /// The counterexample diagnostic.
        detail: String,
    },
}

/// The classification of a SUB-L3 item's certificate for repair (REQ-2 — the
/// anti-cheat gate). A cert that already certifies (`Level::L3`, or any certified
/// rung with no reject) is NOT a sub-L3 item and yields `None` (a NO-OP, REQ-6 —
/// not in the repair set).
#[derive(Debug, Clone)]
pub enum SubL3Status {
    /// A genuine INCONCLUSIVENESS — a verus TIMEOUT (`VerusTimeout` reject, or a
    /// `lowered_assurance` cert carrying a `VerusTimeout` degrade). This is the
    /// SOLE status [`escalate`] retries (REQ-2). Carries the #11 prompt material
    /// off the timed-out cert for the still-sub-L3 report.
    Timeout {
        /// The level the #10 ladder left the item at (`L0` for a raw timeout cert,
        /// `L1`/`L2` for a degraded one) — the level the report shows if the
        /// escalation does not recover L3.
        level: Level,
        /// The Z3 profile off the timed-out cert (the repair prompt material).
        profile: Option<SolverProfile>,
        /// The profile-derived headline hint.
        suggested_move: Option<SuggestedMove>,
        /// The timeout detail.
        detail: String,
    },
    /// A HARD FAIL that is NEVER retried (REQ-2): a COUNTEREXAMPLE (verus disproved
    /// the contract), a vacuity reject, a weak-contract reject, or any other
    /// non-timeout non-certifying verdict. Repair REPORTS it and stops.
    NotRepairable {
        /// The level (typically `L0`).
        level: Level,
        /// The machine-readable reason tag (the reject `cause`, or
        /// `"Counterexample"` for an un-discharged `L0` with no reject).
        cause: String,
        /// The human-readable detail (the reject detail / the first failed
        /// obligation's diagnostic).
        detail: String,
    },
}

/// Classify a per-item certificate (from a DEFAULT-budget `forge check`) for
/// repair (REQ-2 — the anti-cheat gate). Returns:
///
/// - `None` — the item ALREADY CERTIFIES (a NO-OP; not a repair-set member,
///   REQ-6 / AC-1): `Level::L3` (proved), or a certified `L1`/`L2` with no reject
///   (a `#[slag]`/boundary/explicit/degraded rung — repair drives the L3 verus
///   budget, and a non-degraded certified lower rung was a deliberate choice).
/// - `Some(Timeout)` — a genuine INCONCLUSIVENESS the ladder retries: a
///   `VerusTimeout` reject, OR a `lowered_assurance` cert whose `degrade_reason`
///   is a `VerusTimeout` (the #10 down-ladder's record of the very timeout repair
///   re-attempts). This is the ONLY status [`escalate`] runs for.
/// - `Some(NotRepairable)` — a HARD FAIL repair REPORTS but NEVER retries: a
///   counterexample (`L0`, no reject or a non-timeout reject), a vacuity reject,
///   a weak-contract reject.
///
/// The ANTI-CHEAT (R-DEFER-9): the ONLY path to `Timeout` is the `VerusTimeout`
/// tag — a profile-PRESENT verdict. A counterexample (profile-ABSENT, the
/// `postcondition not satisfied` witness path) lands in `NotRepairable`, so more
/// budget can never be thrown at it.
pub fn classify_sub_l3(cert: &Certificate) -> Option<SubL3Status> {
    // A `lowered_assurance` cert (the #10 down-ladder degraded a timeout to L1/L2):
    // its UNDERLYING obstruction was a verus TIMEOUT, so repair may re-attempt the
    // L3 proof at a higher budget (it carries `degrade_reason` = VerusTimeout). The
    // degrade reason is the gate (not merely the `lowered_assurance` flag) so a
    // future non-timeout degrade is not silently retried.
    if cert.lowered_assurance {
        if let Some(reason) = &cert.degrade_reason {
            if reason.cause == "VerusTimeout" {
                return Some(SubL3Status::Timeout {
                    level: cert.level,
                    profile: cert.solver_profile.clone(),
                    suggested_move: cert.suggested_move.clone(),
                    detail: reason.detail.clone(),
                });
            }
        }
        // A degraded cert whose degrade was NOT a verus timeout is not repairable
        // by budget escalation (defensive — v0.1 only degrades on timeout).
        return Some(SubL3Status::NotRepairable {
            level: cert.level,
            cause: cert
                .degrade_reason
                .as_ref()
                .map(|r| r.cause.clone())
                .unwrap_or_else(|| "LoweredAssurance".to_string()),
            detail: cert
                .degrade_reason
                .as_ref()
                .map(|r| r.detail.clone())
                .unwrap_or_default(),
        });
    }

    // A reject cert: a `VerusTimeout` reject is a TIMEOUT (retry); EVERYTHING ELSE
    // (a vacuity / weak-contract / triage / slag reject) is a HARD FAIL the
    // anti-cheat forbids retrying (REQ-2).
    if let Some(reject) = &cert.reject {
        if reject.cause == "VerusTimeout" {
            return Some(SubL3Status::Timeout {
                level: cert.level,
                profile: cert.solver_profile.clone(),
                suggested_move: cert.suggested_move.clone(),
                detail: reject.detail.clone(),
            });
        }
        return Some(SubL3Status::NotRepairable {
            level: cert.level,
            cause: reject.cause.clone(),
            detail: reject.detail.clone(),
        });
    }

    // No reject + a CERTIFIED rung (L1/L2/L3) → already certifies → NO-OP. Repair
    // drives the L3 budget; a non-degraded certified lower rung (slag/boundary/an
    // explicit `--level l2`) is not a timeout to escalate.
    if matches!(cert.level, Level::L1 | Level::L2 | Level::L3) {
        return None;
    }

    // No reject + `Level::L0` (an un-discharged proof with no structured reject —
    // the bare counterexample path). NEVER retried (REQ-2): more budget does not
    // discharge a disproved obligation.
    let detail = cert
        .obligations
        .iter()
        .find(|o| matches!(o.status, crate::manifest::ObligationStatus::Failed))
        .and_then(|o| o.diagnostic.clone())
        .unwrap_or_else(|| {
            "verus did not discharge the obligation (no profile report)".to_string()
        });
    Some(SubL3Status::NotRepairable {
        level: cert.level,
        cause: "Counterexample".to_string(),
        detail,
    })
}

/// The per-item repair outcome (REQ-6). One of: upgraded to L3 (with the winning
/// budget), still sub-L3 (with the #11 prompt), or not-repairable (a hard fail
/// reported, never retried).
#[derive(Debug, Clone)]
pub enum RepairOutcome {
    /// The item PROVED at an escalated budget → upgraded to L3. `budget` is the
    /// absolute `--rlimit` (a [`REPAIR_LADDER`] rung × [`DEFAULT_RLIMIT`]) at which
    /// it first proved (REQ-1).
    UpgradedToL3 {
        /// The absolute winning `--rlimit` budget.
        budget: f64,
    },
    /// The item TIMED OUT at every rung up to the cap → still sub-L3 (REQ-3/AC-4).
    /// Carries the #11 repair PROMPT (the residual profile + the headline hint +
    /// the obligation detail) for the agent's own custom-proof move (REQ-6).
    StillSubL3 {
        /// The level the item remains at.
        level: Level,
        /// The residual Z3 instantiation profile from the highest-budget attempt
        /// (the #11 repair prompt material), if verus emitted one (REQ-6).
        profile: Option<SolverProfile>,
        /// The headline repair hint (the #11 `suggested_move`), if any.
        suggested_move: Option<SuggestedMove>,
        /// The failing-obligation detail.
        detail: String,
    },
    /// A HARD FAIL: a counterexample / vacuity / weak-contract reject. REPORTED,
    /// NEVER retried, NEVER upgraded (REQ-2 — the anti-cheat). No escalation rung
    /// was attempted.
    NotRepairable {
        /// The level (typically `L0`).
        level: Level,
        /// The machine-readable reason tag.
        cause: String,
        /// The human-readable detail (the counterexample witness / reject reason).
        detail: String,
    },
}

/// One item's repair record (REQ-6): its name + its [`RepairOutcome`].
#[derive(Debug, Clone)]
pub struct RepairItem {
    /// The item name.
    pub item: String,
    /// The outcome of repairing it.
    pub outcome: RepairOutcome,
}

/// The repair report over a file (REQ-6). Holds one [`RepairItem`] per SUB-L3 item
/// (an already-L3 item is a NO-OP and is NOT included — the corpus produces an
/// EMPTY `items`, AC-1). `total_checked` is the number of items the underlying
/// `forge check` produced (so a caller can report "N items, 0 to repair").
#[derive(Debug, Clone)]
pub struct RepairReport {
    /// The total number of items the underlying check produced.
    pub total_checked: usize,
    /// One record per SUB-L3 item (empty when every item already certifies).
    pub items: Vec<RepairItem>,
}

impl RepairReport {
    /// `true` iff repair found NO sub-L3 item to act on (the corpus no-op, AC-1).
    pub fn is_noop(&self) -> bool {
        self.items.is_empty()
    }

    /// `true` iff EVERY repaired item is now upgraded to L3 (no still-sub-L3 and no
    /// not-repairable item remains). A no-op report is vacuously fully-repaired.
    pub fn all_upgraded(&self) -> bool {
        self.items
            .iter()
            .all(|i| matches!(i.outcome, RepairOutcome::UpgradedToL3 { .. }))
    }
}

/// Drive the BOUNDED escalation ladder for ONE timeout item (REQ-1/REQ-3 — the
/// repair loop core). `verify` is the per-rung re-verify: given an ABSOLUTE
/// `--rlimit` budget it returns the [`RepairVerdict`] at that budget (the
/// production path re-drives `check::check_file_with_rlimit`; the tests synthesize
/// it). The `timeout` argument is the ORIGINAL default-budget timeout status (its
/// level + the #11 prompt material), used to build the still-sub-L3 report when
/// the ladder is exhausted.
///
/// The loop walks [`REPAIR_LADDER`] in ascending order, re-verifying at each
/// `multiplier * DEFAULT_RLIMIT`:
/// - a [`RepairVerdict::Proved`] → STOP, [`RepairOutcome::UpgradedToL3`] with that
///   budget (REQ-1 — the first budget that proves wins);
/// - a [`RepairVerdict::Counterexample`] at a rung → STOP,
///   [`RepairOutcome::NotRepairable`] (sound: a budget that DISPROVES the item is
///   reported, never upgraded — R-DEFER-9);
/// - a [`RepairVerdict::Timeout`] → try the NEXT rung (or, if this was the last
///   rung, fall through to the still-sub-L3 report).
///
/// BOUNDED (REQ-3): the loop makes AT MOST `REPAIR_LADDER.len()` re-verify calls
/// and always terminates. An ENVIRONMENT failure from `verify` propagates as the
/// `Err` (REQ-7), NEVER a still-sub-L3 verdict and NEVER an upgrade.
///
/// This function is ONLY called on a [`SubL3Status::Timeout`] (gated in
/// [`repair_item`]) — the anti-cheat (REQ-2): a counterexample / reject never
/// reaches here, so `verify` is never invoked for a non-timeout item.
pub fn escalate<V>(timeout: &SubL3Status, mut verify: V) -> Result<RepairOutcome, ForgeError>
where
    V: FnMut(f64) -> Result<RepairVerdict, ForgeError>,
{
    // Extract the original-timeout report material (the level + the #11 prompt the
    // still-sub-L3 outcome surfaces). A non-`Timeout` status is a programming error
    // at the call site (gated), but handle it without panicking (R-CODE-2): treat
    // it as not-repairable rather than escalating.
    let (orig_level, mut prompt_profile, mut prompt_move, mut prompt_detail) = match timeout {
        SubL3Status::Timeout {
            level,
            profile,
            suggested_move,
            detail,
        } => (
            *level,
            profile.clone(),
            suggested_move.clone(),
            detail.clone(),
        ),
        SubL3Status::NotRepairable {
            level,
            cause,
            detail,
        } => {
            return Ok(RepairOutcome::NotRepairable {
                level: *level,
                cause: cause.clone(),
                detail: detail.clone(),
            });
        }
    };

    for &multiplier in REPAIR_LADDER.iter() {
        let budget = DEFAULT_RLIMIT * multiplier;
        match verify(budget)? {
            // The first budget that PROVES is the upgrade (REQ-1).
            RepairVerdict::Proved => return Ok(RepairOutcome::UpgradedToL3 { budget }),
            // A budget that DISPROVES the item → report, never upgrade (sound,
            // R-DEFER-9). Should not happen for a true-but-slow obligation, but a
            // disproof at a higher budget is a hard fail, not a still-sub-L3.
            RepairVerdict::Counterexample { detail } => {
                return Ok(RepairOutcome::NotRepairable {
                    level: Level::L0,
                    cause: "Counterexample".to_string(),
                    detail,
                });
            }
            // Still a timeout at this rung → refresh the prompt material from the
            // most-recent (highest-budget) attempt and try the next rung.
            RepairVerdict::Timeout {
                profile,
                suggested_move,
                detail,
            } => {
                if profile.is_some() {
                    prompt_profile = profile;
                }
                if suggested_move.is_some() {
                    prompt_move = suggested_move;
                }
                if !detail.is_empty() {
                    prompt_detail = detail;
                }
            }
        }
    }

    // The ladder is exhausted (every rung up to the cap timed out) → STILL sub-L3
    // + the #11 prompt (REQ-3/AC-4 — bounded, the loop stops). The level is the
    // original timeout level (repair did not lower it; it could not raise it).
    Ok(RepairOutcome::StillSubL3 {
        level: orig_level,
        profile: prompt_profile,
        suggested_move: prompt_move,
        detail: prompt_detail,
    })
}

/// Repair ONE item given its default-budget classification (REQ-1/REQ-2). This is
/// the anti-cheat GATE (REQ-2): [`escalate`] (and thus the verifier closure) runs
/// ONLY for a [`SubL3Status::Timeout`]; a [`SubL3Status::NotRepairable`]
/// short-circuits to [`RepairOutcome::NotRepairable`] WITHOUT a single re-verify
/// — a counterexample / vacuity / weak-contract reject is reported, never retried,
/// never upgraded.
pub fn repair_item<V>(status: &SubL3Status, verify: V) -> Result<RepairOutcome, ForgeError>
where
    V: FnMut(f64) -> Result<RepairVerdict, ForgeError>,
{
    match status {
        // THE ANTI-CHEAT (REQ-2): a hard fail is REPORTED, never escalated. The
        // `verify` closure is dropped UNUSED — not one re-verify is attempted.
        SubL3Status::NotRepairable {
            level,
            cause,
            detail,
        } => Ok(RepairOutcome::NotRepairable {
            level: *level,
            cause: cause.clone(),
            detail: detail.clone(),
        }),
        // A genuine TIMEOUT (inconclusiveness) → the ONLY status that escalates.
        SubL3Status::Timeout { .. } => escalate(status, verify),
    }
}

/// Map a re-verified [`Certificate`] (from `check::check_file_with_rlimit` at an
/// escalated budget) into a [`RepairVerdict`] (REQ-1). A `Level::L3` cert with no
/// reject is `Proved`; a `VerusTimeout` reject (or a `lowered_assurance` timeout
/// degrade) is `Timeout` (carry the prompt material); anything else is a
/// `Counterexample` (a hard fail at this budget — sound to report, never upgrade).
fn verdict_from_cert(cert: &Certificate) -> RepairVerdict {
    if cert.reject.is_none() && cert.level == Level::L3 {
        return RepairVerdict::Proved;
    }
    match classify_sub_l3(cert) {
        Some(SubL3Status::Timeout {
            profile,
            suggested_move,
            detail,
            ..
        }) => RepairVerdict::Timeout {
            profile,
            suggested_move,
            detail,
        },
        Some(SubL3Status::NotRepairable { detail, .. }) => RepairVerdict::Counterexample { detail },
        // A certified lower rung at an escalated budget is not "proved at L3"; treat
        // it as still-inconclusive for the L3 goal (no profile to attach).
        None => RepairVerdict::Timeout {
            profile: None,
            suggested_move: None,
            detail: "re-verify did not reach L3 at this budget".to_string(),
        },
    }
}

/// Run the proof-repair loop over `path`, optionally restricted to a single
/// `item` (REQ-1 — `forge repair [item]`). The one-shot, deterministic,
/// re-runnable CLI pass (OQ-4 reading (a) — no daemon; the agent's outer loop
/// schedules it).
///
/// 1. Re-derive the per-item certs at the DEFAULT budget
///    (`check::check_file_with_rlimit(path, DEFAULT_RLIMIT)` — the same certs
///    `forge check` produced).
/// 2. For each cert (optionally filtered to `item`), `classify_sub_l3`:
///    - `None` → NO-OP (already certifies; not in the report set, AC-1);
///    - `Some(Timeout)` → escalate the bounded ladder, re-driving
///      `check::check_file_with_rlimit` at each rung (REQ-1/REQ-3);
///    - `Some(NotRepairable)` → report-only, NEVER retried (REQ-2 anti-cheat).
///
/// An ENVIRONMENT failure (verus absent / unparseable) at any stage propagates as
/// the `Err` (REQ-7), never a silent upgrade or a still-sub-L3 verdict.
pub fn repair_file(path: &std::path::Path, item: Option<&str>) -> Result<RepairReport, ForgeError> {
    let certs = check::check_file_with_rlimit(path, DEFAULT_RLIMIT)?;
    let total_checked = certs.len();

    let mut items = Vec::new();
    for cert in &certs {
        if let Some(target) = item {
            if cert.item != target {
                continue;
            }
        }
        let Some(status) = classify_sub_l3(cert) else {
            // Already certifies → NO-OP (not in the repair set, REQ-6 / AC-1).
            continue;
        };
        // The per-rung re-verify closure: re-drive the SAME verus path at the
        // escalated budget over the WHOLE file, then map THIS item's re-verified
        // cert into a verdict (per-item isolation is preserved by `check.rs`'s
        // sub-program split). The closure is invoked ONLY for a `Timeout` status
        // (the anti-cheat gate in `repair_item`).
        let item_name = cert.item.clone();
        let verify = |budget: f64| -> Result<RepairVerdict, ForgeError> {
            let reverified = check::check_file_with_rlimit(path, budget)?;
            let this = reverified
                .iter()
                .find(|c| c.item == item_name)
                .ok_or_else(|| ForgeError::VerusOutput {
                    detail: format!(
                        "re-verify at rlimit {budget} produced no certificate for item \
                         `{item_name}` (the item vanished between budgets)"
                    ),
                })?;
            Ok(verdict_from_cert(this))
        };
        let outcome = repair_item(&status, verify)?;
        items.push(RepairItem {
            item: cert.item.clone(),
            outcome,
        });
    }

    Ok(RepairReport {
        total_checked,
        items,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{ObligationResult, RejectReason};
    use std::cell::Cell;

    /// A synthesized DEFAULT-budget TIMEOUT status (the hermetic loop driver — no
    /// live verus). Mirrors what `classify_sub_l3` returns for a `VerusTimeout`
    /// cert.
    fn timeout_status() -> SubL3Status {
        SubL3Status::Timeout {
            level: Level::L0,
            profile: None,
            suggested_move: Some(SuggestedMove {
                kind: "trigger-hint".to_string(),
                detail: "narrow the trigger".to_string(),
            }),
            detail: "rlimit exhausted".to_string(),
        }
    }

    /// A synthesized timeout verdict (still inconclusive at a rung).
    fn timeout_verdict() -> RepairVerdict {
        RepairVerdict::Timeout {
            profile: None,
            suggested_move: Some(SuggestedMove {
                kind: "trigger-hint".to_string(),
                detail: "narrow the trigger".to_string(),
            }),
            detail: "rlimit exhausted".to_string(),
        }
    }

    // REQ-1 / AC-2 (HERMETIC upgrade): a verify closure that TIMES OUT at low
    // budgets and PROVES at budget >= K (here the `* 8` rung = DEFAULT_RLIMIT*8) →
    // the loop escalates the ladder and UPGRADES to L3, recording the WINNING
    // budget (the first rung that proves). Pins the upgrade path deterministically
    // without a fragile live timeout (OQ-1).
    #[test]
    fn escalation_upgrades_at_the_proving_budget() {
        let k = DEFAULT_RLIMIT * 8.0;
        let calls = Cell::new(0usize);
        let status = timeout_status();
        let outcome = escalate(&status, |budget| {
            calls.set(calls.get() + 1);
            if budget >= k {
                Ok(RepairVerdict::Proved)
            } else {
                Ok(timeout_verdict())
            }
        })
        .expect("no environment error");
        match outcome {
            RepairOutcome::UpgradedToL3 { budget } => {
                assert_eq!(
                    budget, k,
                    "the recorded budget is the first rung that proves"
                );
            }
            other => panic!("expected UpgradedToL3, got {other:?}"),
        }
        // It escalated `*2`, `*4` (timeout) then `*8` (proved) = 3 attempts, and
        // STOPPED (did not try `*16`).
        assert_eq!(calls.get(), 3, "stops at the first proving rung");
    }

    // REQ-3 / AC-4 (HERMETIC bounded termination): a verify closure that TIMES OUT
    // at EVERY rung → still-sub-L3 + the prompt, having made EXACTLY the ladder's
    // rung-count of attempts then STOPPED (never retried forever).
    #[test]
    fn escalation_is_bounded_and_terminates() {
        let calls = Cell::new(0usize);
        let status = timeout_status();
        let outcome = escalate(&status, |_budget| {
            calls.set(calls.get() + 1);
            Ok(timeout_verdict())
        })
        .expect("no environment error");
        match outcome {
            RepairOutcome::StillSubL3 {
                level,
                suggested_move,
                ..
            } => {
                assert_eq!(level, Level::L0, "stays at the original sub-L3 level");
                assert!(
                    suggested_move.is_some(),
                    "surfaces the #11 repair prompt (suggested_move)"
                );
            }
            other => panic!("expected StillSubL3, got {other:?}"),
        }
        assert_eq!(
            calls.get(),
            REPAIR_LADDER.len(),
            "exactly the ladder's rung-count of attempts, then stops (bounded)"
        );
    }

    // REQ-2 (THE ANTI-CHEAT, hermetic): a COUNTEREXAMPLE status is NEVER retried —
    // `repair_item` short-circuits to NotRepairable and the verify closure is NEVER
    // invoked (assert the call count stays 0). The load-bearing invariant.
    #[test]
    fn counterexample_is_never_retried() {
        let calls = Cell::new(0usize);
        let status = SubL3Status::NotRepairable {
            level: Level::L0,
            cause: "Counterexample".to_string(),
            detail: "postcondition not satisfied".to_string(),
        };
        let outcome = repair_item(&status, |_budget| {
            calls.set(calls.get() + 1);
            Ok(RepairVerdict::Proved) // would falsely upgrade IF ever called
        })
        .expect("no environment error");
        assert!(
            matches!(outcome, RepairOutcome::NotRepairable { .. }),
            "a counterexample is reported, never upgraded: {outcome:?}"
        );
        assert_eq!(
            calls.get(),
            0,
            "THE ANTI-CHEAT: the escalation closure is NEVER invoked for a counterexample"
        );
    }

    // REQ-2 (anti-cheat, the gate): only a `Timeout` status reaches `escalate`. A
    // vacuity / weak-contract reject is also NotRepairable and never retried.
    #[test]
    fn rejects_are_never_retried() {
        for cause in ["SemanticTautology", "VacuousPrecondition", "WeakContract"] {
            let calls = Cell::new(0usize);
            let status = SubL3Status::NotRepairable {
                level: Level::L0,
                cause: cause.to_string(),
                detail: "degenerate contract".to_string(),
            };
            let outcome = repair_item(&status, |_b| {
                calls.set(calls.get() + 1);
                Ok(RepairVerdict::Proved)
            })
            .expect("no env error");
            assert!(matches!(outcome, RepairOutcome::NotRepairable { .. }));
            assert_eq!(calls.get(), 0, "{cause} reject is never retried");
        }
    }

    // REQ-2 (classification gate): `classify_sub_l3` routes a `VerusTimeout` cert to
    // `Timeout` (retry) and a counterexample / vacuity / weak-contract cert to
    // `NotRepairable` (never retry). The profile-PRESENT vs ABSENT distinction the
    // anti-cheat rests on.
    #[test]
    fn classify_routes_timeout_vs_falsity() {
        // A VerusTimeout reject → Timeout (retry).
        let timeout = Certificate::timeout(
            "slow",
            vec!["pure".to_string()],
            0,
            SolverProfile {
                total_instantiations: 5,
                quantifiers: vec![],
            },
            Some(SuggestedMove {
                kind: "trigger-hint".to_string(),
                detail: "x".to_string(),
            }),
            "rlimit exhausted".to_string(),
        );
        assert!(matches!(
            classify_sub_l3(&timeout),
            Some(SubL3Status::Timeout { .. })
        ));

        // A bare L0 counterexample (no reject) → NotRepairable.
        let cx = Certificate::new(
            "wrong",
            Level::L0,
            vec!["pure".to_string()],
            0,
            vec![ObligationResult::failed(
                "postcondition not satisfied",
                Some("x.rs:5:13".to_string()),
                Some("error: postcondition not satisfied".to_string()),
            )],
        );
        assert!(matches!(
            classify_sub_l3(&cx),
            Some(SubL3Status::NotRepairable { .. })
        ));

        // A vacuity reject → NotRepairable.
        let vac = Certificate::rejected_vacuity(
            "vacuous",
            vec!["pure".to_string()],
            RejectReason {
                cause: "VacuousPrecondition".to_string(),
                detail: "req is unsatisfiable".to_string(),
            },
            false,
            true,
        );
        assert!(matches!(
            classify_sub_l3(&vac),
            Some(SubL3Status::NotRepairable { cause, .. }) if cause == "VacuousPrecondition"
        ));

        // An L3 proved cert → NO-OP (None).
        let proved = Certificate::new("ok", Level::L3, vec!["pure".to_string()], 0, vec![]);
        assert!(classify_sub_l3(&proved).is_none());
    }

    // REQ-2: a `lowered_assurance` cert whose degrade was a verus timeout (the #10
    // down-ladder degraded it) IS a `Timeout` (repair re-attempts the L3 budget);
    // a `lowered_assurance` cert with a non-timeout degrade is NotRepairable.
    #[test]
    fn classify_routes_lowered_assurance_timeout() {
        let degraded = Certificate::new("deg", Level::L2, vec!["pure".to_string()], 0, vec![])
            .into_degraded(RejectReason {
                cause: "VerusTimeout".to_string(),
                detail: "rlimit exhausted at L3".to_string(),
            });
        assert!(matches!(
            classify_sub_l3(&degraded),
            Some(SubL3Status::Timeout {
                level: Level::L2,
                ..
            })
        ));
    }

    // REQ-5 (determinism): re-running the SAME escalation (same ladder, same
    // synthesized per-rung verdicts) yields the SAME achieved outcome + budget.
    #[test]
    fn escalation_is_deterministic() {
        let k = DEFAULT_RLIMIT * 4.0;
        let run = || {
            escalate(&timeout_status(), |budget| {
                if budget >= k {
                    Ok(RepairVerdict::Proved)
                } else {
                    Ok(timeout_verdict())
                }
            })
            .expect("no env error")
        };
        let a = run();
        let b = run();
        match (a, b) {
            (
                RepairOutcome::UpgradedToL3 { budget: ba },
                RepairOutcome::UpgradedToL3 { budget: bb },
            ) => {
                assert_eq!(ba, bb, "deterministic winning budget");
                assert_eq!(ba, k);
            }
            other => panic!("expected two equal UpgradedToL3, got {other:?}"),
        }
    }

    // REQ-7 / AC-6: an ENVIRONMENT failure during an escalated re-verify propagates
    // as the Err — NEVER a still-sub-L3 verdict and NEVER a silent upgrade.
    #[test]
    fn environment_error_propagates() {
        let status = timeout_status();
        let result = escalate(&status, |_budget| {
            Err(ForgeError::VerusAbsent {
                binary: "verus".to_string(),
            })
        });
        assert!(
            matches!(result, Err(ForgeError::VerusAbsent { .. })),
            "an environment failure is an Err, never an upgrade or a still-sub-L3 verdict"
        );
    }

    // REQ-1: a timeout that DISPROVES at a higher budget is reported NotRepairable
    // (sound: a budget that disproves the item is never an upgrade — R-DEFER-9).
    #[test]
    fn disproof_at_higher_budget_is_not_an_upgrade() {
        let status = timeout_status();
        let outcome = escalate(&status, |_budget| {
            Ok(RepairVerdict::Counterexample {
                detail: "postcondition not satisfied".to_string(),
            })
        })
        .expect("no env error");
        assert!(
            matches!(outcome, RepairOutcome::NotRepairable { .. }),
            "a disproof at a higher budget is a hard fail, not an upgrade: {outcome:?}"
        );
    }
}
