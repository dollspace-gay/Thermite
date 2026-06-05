//! `forge/src/degrade.rs` — the automatic L3→L2→L1 degrade ladder + the
//! project-level assurance manifest (issue #10, `.design/forge/degrade-ladder.md`;
//! `thermite-design.md` §5.2 "the gate degrades, it never blocks", §6, §12).
//!
//! On the DEFAULT `forge check` path (no `--level` flag) an item is first
//! attempted at L3 (the verus SMT proof). When verus cannot PROVE it within its
//! resource budget — a **TIMEOUT** (inconclusive) — the ladder automatically
//! attempts **L2** (the kani bounded model check, #9). If L2 also cannot
//! bound-verify (an UNDER-BOUND, the L2 analog of a timeout) the obligation drops
//! to **L1** (the SpecTherm contract compiled to runtime checks always exists,
//! §4.2). Each rung below L3 carries a `lowered-assurance` flag + the degrade
//! reason (the #11 `VerusTimeout` reason).
//!
//! **THE CRITICAL ANTI-CHEAT INVARIANT (§5.2, R-DEFER-9, R-CODE-4).** The ladder
//! degrades **INCONCLUSIVENESS, never FALSITY**. A degrade edge is taken ONLY on a
//! classified **Timeout** (L3) / **UnderBound** (L2). A **Counterexample** (verus
//! OR kani DISPROVED the contract — a real bug) is a HARD FAILURE: the ladder
//! short-circuits to the existing non-certifying counterexample cert and runs NO
//! L2 and NO L1 rung. Degrading a disproved contract to L1 would hide a real bug
//! behind a `lowered-assurance` stamp — the worst possible outcome (§12, §7).
//!
//! This module COMPOSES the shipped pieces; it owns NO prover-invocation logic:
//! - the L3 outcome is #11's classification (`check::classify_verus_outcome` →
//!   `VerusOutcome`), mapped into [`L3Verdict`] by the caller;
//! - the L2 rung is #9's `kani::run_kani` + `kani::classify_l2_outcome`
//!   (`L2Verdict`, the OQ-2 split);
//! - the L1 rung is OQ-3 reading (b): the ladder RECORDS `Level::L1` +
//!   lowered-assurance; the runtime-check EMISSION stays `thermite_lower::lower_l1`'s
//!   build-time job (the `Certificate::slag_l1` precedent records L1 without
//!   running `lower_l1`).
//!
//! The assurance manifest (`manifest::AssuranceManifest::aggregate`) is the
//! project-level aggregate over the per-fn certs; its headline is the
//! min-over-functions (REQ-6), rendered by `cli::run_check`.
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (degrade state machine L3→L2→L1) | SHIPPED | `pub fn run_ladder` runs the rungs in order: `L3Verdict::Proved` → certify L3; `Timeout` → `attempt_l2` closure → `L2Verdict::Verified` certify L2 (+ lowered-assurance + bound) / `UnderBound` → `attempt_l1` closure → certify L1 (+ lowered-assurance). Consumer: `check::check_file_with_options`'s default per-item path (`degrade::ladder_l3_timeout`). |
//! | REQ-2 (anti-cheat: a counterexample NEVER degrades) | SHIPPED | `run_ladder` matches `L3Verdict::Counterexample` → returns the supplied hard-fail cert with NO `attempt_l2`/`attempt_l1` call (the closures are not invoked); an `L2Verdict::Counterexample` from `attempt_l2` → returns the L2 counterexample cert with NO `attempt_l1` call. Hermetic test `counterexample_never_degrades` + `l2_counterexample_never_drops_to_l1`. |
//! | REQ-3 (L1 fallback rung) | SHIPPED | `run_ladder` on `L2Verdict::UnderBound` calls `attempt_l1`, which in the live path (`check::degrade_l1_cert`) records `Level::L1` (`lower_l1` exists; emission is build-time, OQ-3 (b)). Test `l2_under_bound_drops_to_l1`. |
//! | REQ-4 (lowered-assurance flag + degrade reason) | SHIPPED | the L2 and L1 certs `run_ladder` returns are `Certificate::into_degraded(reason)` — `lowered_assurance = true` + the `VerusTimeout` degrade reason (`manifest.rs`). Test `degraded_l2_carries_flag_and_reason`. |
//! | REQ-5 (assurance manifest — per-fn aggregate) | SHIPPED | `manifest::AssuranceManifest::aggregate(&[Certificate])` (per-fn `FunctionAssurance` rows + the project headline); consumer `cli::run_check`. |
//! | REQ-6 (min-over-functions project assurance) | SHIPPED | `AssuranceManifest::aggregate` headline = `ProjectAssurance::Certified(min over levels)` via `Level`'s `Ord` (`L0<L1<L2<L3`), or `Failed` if any fn does not certify. Test `aggregate_is_min_over_functions` + `hard_fail_caps_project_at_failure`. |
//! | REQ-7 (determinism) | SHIPPED | `run_ladder` is a pure function of its verdict + closures; `aggregate` is a pure function of the cert collection; no wall-clock / unseeded input enters the verdict (R-CODE-5). Test `ladder_is_deterministic`. |
//! | REQ-8 (subprocess failures never silently degrade) | SHIPPED | `attempt_l2` / `attempt_l1` return `Result<_, ForgeError>`; an environment failure propagates as the `Err` (the `?` in `run_ladder`), NEVER a degrade — only a classified `Timeout`/`UnderBound` verdict takes a degrade edge. Test `l2_environment_error_is_not_a_degrade`. |
//!
//! ## #17 extension (the §9 project assurance-scope claim, this iteration)
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | e2e-vs-boundary REQ-4 (project END-TO-END iff every fn is) | SHIPPED | `manifest::AssuranceManifest::aggregate` now also computes `manifest::ProjectScope` (via `project_scope`): `EndToEnd` iff every cert's `assurance_scope` is end-to-end (a `None` reads end-to-end — the golden default), else `ToBoundary { crossings }` listing the reached `#[boundary]`/`#[slag]` fns (sorted + deduplicated, deterministic). ORTHOGONAL to the min-over-functions `project` level headline. Consumer: `cli::run_check` (the manifest it aggregates + renders). Tests `aggregate_project_scope_*` below. |

use crate::cli::ForgeError;
use crate::kani::L2Verdict;
use crate::manifest::{Certificate, RejectReason};

/// The DEGRADE-relevant view of an L3 (verus) run the ladder consumes (REQ-1).
/// The caller (`check.rs`) maps #11's `VerusOutcome` into this: a `Proved` →
/// [`L3Verdict::Proved`] with the assembled L3 cert; a `Timeout` →
/// [`L3Verdict::Timeout`] with the degrade reason (the `VerusTimeout` reason, the
/// "here's where I got lost"); a `Counterexample` → [`L3Verdict::Counterexample`]
/// with the existing non-certifying counterexample cert.
///
/// The ladder treats `Proved` / `Counterexample` as TERMINAL (no degrade) and
/// `Timeout` as the SOLE degrade trigger (REQ-2 anti-cheat).
pub enum L3Verdict {
    /// verus PROVED the item → certify L3 (the carried cert, terminal).
    Proved(Certificate),
    /// verus TIMED OUT (inconclusive) → the SOLE degrade trigger. The ladder
    /// attempts L2 and, on an under-bound, L1; the achieved lower-rung cert is
    /// stamped with this degrade `reason` (REQ-4). The L3 timeout cert itself is
    /// fully superseded: on this edge a lower rung ALWAYS produces the cert (L2
    /// verified / L2 counterexample hard-fail / L1 recorded), or a subprocess
    /// failure propagates as an `Err` (REQ-8) — the L0 timeout cert is never the
    /// final word on the DEFAULT path (that was the v0.1 STOP #10 removes).
    Timeout {
        /// The structured degrade reason carried onto the L2/L1 cert (REQ-4) — the
        /// #11 `VerusTimeout` reason, the "here's where I got lost".
        reason: RejectReason,
    },
    /// verus DISPROVED the item (a real bug) → HARD FAIL, NEVER a degrade (REQ-2
    /// anti-cheat). Carries the existing non-certifying counterexample cert.
    Counterexample(Certificate),
}

/// The result of one L2 (kani) rung attempt the ladder reads (REQ-1). The caller
/// produces this from `kani::run_kani` + `kani::classify_l2_outcome`: the
/// `L2Verdict` (the OQ-2 split) plus the assembled L2 cert (its `Level::L2`
/// discharged cert on `Verified`, or the `Level::L0` counterexample cert on a real
/// failure). On `UnderBound` the cert is unused (the ladder drops to L1).
pub struct L2Attempt {
    /// The OQ-2 classification of the kani run (`Verified` / `UnderBound` /
    /// `Counterexample`).
    pub verdict: L2Verdict,
    /// The assembled L2 certificate (`kani::assemble_l2_certificate`): the
    /// `Level::L2` cert on `Verified`, the `Level::L0` counterexample cert on a
    /// real failure. Unused on `UnderBound`.
    pub cert: Certificate,
}

/// Run the per-item L3→L2→L1 degrade ladder (REQ-1, the DEFAULT `forge check`
/// path) and return the achieved-level certificate. The state machine:
///
/// ```text
/// L3  ─[Proved]────────────▶ certify L3
///     ─[Counterexample]────▶ HARD FAIL (no L2, no L1)            (REQ-2)
///     ─[Timeout]───────────▶ attempt_l2:
///            L2 ─[Verified]──────▶ certify L2 + lowered-assurance + reason
///               ─[Counterexample]▶ HARD FAIL (no L1)             (REQ-2)
///               ─[UnderBound]────▶ attempt_l1 → certify L1 + lowered-assurance
/// ```
///
/// `attempt_l2` / `attempt_l1` are LAZY (run only on the timeout / under-bound
/// edge) and FALLIBLE (`Result<_, ForgeError>`): an ENVIRONMENT failure (kani
/// absent, unparseable output) propagates as the `Err` (REQ-8), NEVER a degrade —
/// only a classified `Timeout`/`UnderBound` VERDICT takes a degrade edge. The L2
/// `attempt_l2` returns an [`L2Attempt`]; `attempt_l1` returns the assembled
/// `Level::L1` cert (OQ-3 (b): the ladder RECORDS L1; runtime-check emission stays
/// `l1.rs`'s build-time job).
///
/// The DEGRADE fields (`lowered_assurance` + the reason) are stamped here via
/// `Certificate::into_degraded` (REQ-4); a `Counterexample` cert is returned
/// UNCHANGED (never stamped — it did not degrade, it FAILED, REQ-2).
pub fn run_ladder<L2, L1>(
    l3: L3Verdict,
    attempt_l2: L2,
    attempt_l1: L1,
) -> Result<Certificate, ForgeError>
where
    L2: FnOnce() -> Result<L2Attempt, ForgeError>,
    L1: FnOnce() -> Result<Certificate, ForgeError>,
{
    match l3 {
        // PROVED → certify L3, terminal. No degrade.
        L3Verdict::Proved(cert) => Ok(cert),
        // COUNTEREXAMPLE → HARD FAIL. The closures are NOT invoked: no L2, no L1,
        // no lowered-assurance stamp (REQ-2 anti-cheat — falsity never degrades).
        L3Verdict::Counterexample(cert) => Ok(cert),
        // TIMEOUT → the SOLE degrade trigger. Attempt L2.
        L3Verdict::Timeout { reason } => {
            let l2 = attempt_l2()?;
            match l2.verdict {
                // L2 VERIFIED → certify L2, stamped lowered-assurance + reason.
                L2Verdict::Verified => Ok(l2.cert.into_degraded(reason)),
                // L2 COUNTEREXAMPLE → HARD FAIL, NOT a drop to L1 (REQ-2, 2nd rung).
                // Return the L2 counterexample cert UNCHANGED (never stamped).
                L2Verdict::Counterexample => Ok(l2.cert),
                // L2 UNDER-BOUND (inconclusive, the L2 analog of a timeout) → drop
                // to L1. Record Level::L1 + lowered-assurance + reason (REQ-3).
                L2Verdict::UnderBound => {
                    let l1 = attempt_l1()?;
                    Ok(l1.into_degraded(reason))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{AssuranceManifest, Level, ObligationResult, ProjectAssurance};

    /// A synthesized L3 PROVED cert (the hermetic ladder driver — no live verus).
    fn proved_cert(item: &str) -> Certificate {
        Certificate::new(
            item,
            Level::L3,
            vec!["pure".to_string()],
            0,
            vec![ObligationResult::discharged("1 obligations discharged")],
        )
    }

    /// A synthesized L3 counterexample cert (Level::L0, a failed obligation, NO
    /// profile — the shape `check::assemble_certificate` produces on
    /// `VerusOutcome::Counterexample`).
    fn counterexample_cert(item: &str) -> Certificate {
        Certificate::new(
            item,
            Level::L0,
            vec!["pure".to_string()],
            0,
            vec![ObligationResult::failed(
                "postcondition not satisfied",
                Some("x.rs:5:13".to_string()),
                Some("error: postcondition not satisfied".to_string()),
            )],
        )
    }

    /// The synthesized `VerusTimeout` degrade reason (the #11 reason material).
    fn timeout_reason() -> RejectReason {
        RejectReason {
            cause: "VerusTimeout".to_string(),
            detail: "verus exhausted its SMT resource budget before proving this item".to_string(),
        }
    }

    /// A synthesized L2 attempt at a given verdict, with a matching cert.
    fn l2_attempt(verdict: L2Verdict) -> L2Attempt {
        let (level, obs) = match verdict {
            L2Verdict::Verified => (
                Level::L2,
                vec![ObligationResult::discharged(
                    "bounded model check passed (slice <= 4, unwind 5)",
                )],
            ),
            L2Verdict::Counterexample => (
                Level::L0,
                vec![ObligationResult::failed(
                    "assertion failed: result == spec_sum(xs)",
                    None,
                    None,
                )],
            ),
            L2Verdict::UnderBound => (
                Level::L0,
                vec![ObligationResult::failed(
                    "unwinding assertion loop 0",
                    None,
                    None,
                )],
            ),
        };
        L2Attempt {
            verdict,
            cert: Certificate::new("f", level, vec!["pure".to_string()], 0, obs),
        }
    }

    /// A synthesized L1 fallback cert (the achieved-level RECORD, OQ-3 (b)).
    fn l1_cert(item: &str) -> Certificate {
        Certificate::new(
            item,
            Level::L1,
            vec!["pure".to_string()],
            0,
            vec![ObligationResult::discharged(
                "contract recorded at L1 (runtime checks emitted at build by lower_l1)",
            )],
        )
    }

    // REQ-1: a PROVED L3 verdict certifies L3 and runs NO lower rung (the closures
    // panic if called — they must NOT be).
    #[test]
    fn proved_certifies_l3_no_lower_rung() {
        let cert = run_ladder(
            L3Verdict::Proved(proved_cert("f")),
            || panic!("attempt_l2 must NOT run on a PROVED verdict"),
            || panic!("attempt_l1 must NOT run on a PROVED verdict"),
        )
        .expect("ladder");
        assert_eq!(cert.level, Level::L3);
        assert!(!cert.lowered_assurance, "an L3 proof is not a degrade");
        assert!(cert.degrade_reason.is_none());
    }

    // REQ-1 / AC-2: a TIMEOUT whose L2 VERIFIES certifies L2 with the
    // lowered-assurance flag + the degrade reason; no L1 rung runs.
    #[test]
    fn timeout_then_l2_verified_certifies_l2_degraded() {
        let cert = run_ladder(
            L3Verdict::Timeout {
                reason: timeout_reason(),
            },
            || Ok(l2_attempt(L2Verdict::Verified)),
            || panic!("attempt_l1 must NOT run when L2 verifies"),
        )
        .expect("ladder");
        assert_eq!(cert.level, Level::L2, "the degrade target is L2");
        assert!(
            cert.lowered_assurance,
            "a degraded cert is lowered-assurance"
        );
    }

    // REQ-4: the degraded L2 cert carries the flag AND the degrade reason.
    #[test]
    fn degraded_l2_carries_flag_and_reason() {
        let cert = run_ladder(
            L3Verdict::Timeout {
                reason: timeout_reason(),
            },
            || Ok(l2_attempt(L2Verdict::Verified)),
            || panic!("no L1"),
        )
        .expect("ladder");
        assert!(cert.lowered_assurance);
        assert_eq!(
            cert.degrade_reason.as_ref().map(|r| r.cause.as_str()),
            Some("VerusTimeout"),
            "the degrade reason is the VerusTimeout reason"
        );
    }

    // REQ-3 / AC-3: a TIMEOUT whose L2 is UNDER-BOUND drops to L1 with the
    // lowered-assurance flag + the degrade reason.
    #[test]
    fn l2_under_bound_drops_to_l1() {
        let cert = run_ladder(
            L3Verdict::Timeout {
                reason: timeout_reason(),
            },
            || Ok(l2_attempt(L2Verdict::UnderBound)),
            || Ok(l1_cert("f")),
        )
        .expect("ladder");
        assert_eq!(cert.level, Level::L1, "L2 under-bound degrades to L1");
        assert!(cert.lowered_assurance);
        assert_eq!(
            cert.degrade_reason.as_ref().map(|r| r.cause.as_str()),
            Some("VerusTimeout")
        );
    }

    // REQ-2 — THE KEY ANTI-CHEAT TEST: an L3 COUNTEREXAMPLE is a HARD FAIL, NEVER a
    // degrade. The L2/L1 closures panic if invoked (they must NOT be); the returned
    // cert is the non-certifying counterexample cert (Level::L0), NOT lowered-
    // assurance, NOT L1/L2. A regression here hides a real bug behind a lowered
    // stamp — the worst possible failure (§12, R-DEFER-9).
    #[test]
    fn counterexample_never_degrades() {
        let cert = run_ladder(
            L3Verdict::Counterexample(counterexample_cert("f")),
            || panic!("attempt_l2 must NEVER run on a COUNTEREXAMPLE (anti-cheat REQ-2)"),
            || panic!("attempt_l1 must NEVER run on a COUNTEREXAMPLE (anti-cheat REQ-2)"),
        )
        .expect("ladder");
        assert_eq!(
            cert.level,
            Level::L0,
            "a counterexample is non-certifying L0"
        );
        assert!(
            !cert.lowered_assurance,
            "a counterexample is NOT a lowered-assurance degrade — it is a FAILURE"
        );
        assert!(
            cert.degrade_reason.is_none(),
            "a hard fail carries no degrade reason"
        );
        assert_ne!(cert.level, Level::L1, "NEVER certified L1");
        assert_ne!(cert.level, Level::L2, "NEVER certified L2");
    }

    // REQ-2 (2nd rung) — an L2 COUNTEREXAMPLE (kani DISPROVED the contract) is a
    // HARD FAIL, NEVER a drop to L1. The L1 closure panics if invoked.
    #[test]
    fn l2_counterexample_never_drops_to_l1() {
        let cert = run_ladder(
            L3Verdict::Timeout {
                reason: timeout_reason(),
            },
            || Ok(l2_attempt(L2Verdict::Counterexample)),
            || panic!("attempt_l1 must NEVER run on an L2 COUNTEREXAMPLE (anti-cheat REQ-2)"),
        )
        .expect("ladder");
        assert_eq!(
            cert.level,
            Level::L0,
            "an L2 counterexample is non-certifying L0"
        );
        assert!(
            !cert.lowered_assurance,
            "an L2 counterexample is NOT a degrade — it is a FAILURE"
        );
        assert_ne!(cert.level, Level::L1);
    }

    // REQ-8: an ENVIRONMENT failure on the L2 rung (kani absent) propagates as the
    // Err, NEVER a degrade — only a classified UnderBound verdict drops to L1.
    #[test]
    fn l2_environment_error_is_not_a_degrade() {
        let r = run_ladder(
            L3Verdict::Timeout {
                reason: timeout_reason(),
            },
            || {
                Err(ForgeError::KaniAbsent {
                    binary: "cargo-kani".to_string(),
                })
            },
            || panic!("attempt_l1 must NOT run when L2 errored (the error propagates)"),
        );
        assert!(
            matches!(r, Err(ForgeError::KaniAbsent { .. })),
            "an environment failure is an Err, never a silent degrade (REQ-8): {r:?}"
        );
    }

    // REQ-7: the ladder is deterministic — the same verdict + closures yield the
    // same achieved level twice.
    #[test]
    fn ladder_is_deterministic() {
        let run = || {
            run_ladder(
                L3Verdict::Timeout {
                    reason: timeout_reason(),
                },
                || Ok(l2_attempt(L2Verdict::Verified)),
                || Ok(l1_cert("f")),
            )
            .expect("ladder")
        };
        assert_eq!(run().level, run().level);
        assert_eq!(run().lowered_assurance, run().lowered_assurance);
    }

    // REQ-5 / REQ-6 / AC-5: the assurance manifest aggregate is the MIN over
    // functions. A {L3, L2, L1} set → project Certified(L1). Expected: Level's Ord
    // L0<L1<L2<L3 (`manifest.rs` REQ-6), not forge's output (R-CHAR-3).
    #[test]
    fn aggregate_is_min_over_functions() {
        let certs = vec![
            proved_cert("f"), // L3
            Certificate::new("g", Level::L2, vec!["pure".to_string()], 0, vec![])
                .into_degraded(timeout_reason()),
            Certificate::new("h", Level::L1, vec!["pure".to_string()], 0, vec![])
                .into_degraded(timeout_reason()),
        ];
        let manifest = AssuranceManifest::aggregate(&certs);
        assert_eq!(
            manifest.project,
            ProjectAssurance::Certified(Level::L1),
            "the project headline is the min over functions"
        );
        assert_eq!(manifest.functions.len(), 3);
        // The L2 and L1 fns are flagged lowered-assurance; the L3 fn is not.
        assert!(!manifest.functions[0].lowered_assurance);
        assert!(manifest.functions[1].lowered_assurance);
        assert!(manifest.functions[2].lowered_assurance);
    }

    // REQ-6 / REQ-2 / AC-5: a single hard-failed (counterexample) fn makes the
    // WHOLE project a FAILURE — NOT a lowered level. Falsity is not a rung.
    #[test]
    fn hard_fail_caps_project_at_failure() {
        let certs = vec![proved_cert("f"), counterexample_cert("g")];
        let manifest = AssuranceManifest::aggregate(&certs);
        assert_eq!(
            manifest.project,
            ProjectAssurance::Failed,
            "a non-certifying fn is a project FAILURE, never a lowered rung (REQ-2)"
        );
    }

    // REQ-6: the no-degrade corpus shape — all-L3 certs → project Certified(L3), no
    // lowered-assurance flags (AC-1).
    #[test]
    fn all_l3_is_project_l3_no_lowering() {
        let certs = vec![proved_cert("spec_sum"), proved_cert("sum")];
        let manifest = AssuranceManifest::aggregate(&certs);
        assert_eq!(manifest.project, ProjectAssurance::Certified(Level::L3));
        assert!(manifest.functions.iter().all(|f| !f.lowered_assurance));
        assert!(manifest.functions.iter().all(|f| f.certified));
    }

    // #17 e2e-vs-boundary REQ-4 / AC-5: a project of only end-to-end fns claims
    // ProjectScope::EndToEnd. A `None` scope (the golden default) reads end-to-end.
    // Expected from the design REQ-4 (R-CHAR-3), not forge output.
    #[test]
    fn aggregate_project_scope_all_end_to_end() {
        use crate::manifest::{AssuranceScope, ProjectScope};
        let certs = vec![
            proved_cert("spec_sum").with_assurance_scope(AssuranceScope::EndToEnd),
            proved_cert("sum").with_assurance_scope(AssuranceScope::EndToEnd),
        ];
        let manifest = AssuranceManifest::aggregate(&certs);
        assert_eq!(manifest.scope, ProjectScope::EndToEnd);
        // ORTHOGONAL: the level headline is still the min-over-functions (L3).
        assert_eq!(manifest.project, ProjectAssurance::Certified(Level::L3));
    }

    // #17 e2e-vs-boundary REQ-4 / AC-5: a project with ANY to-boundary fn claims
    // ProjectScope::ToBoundary listing the (sorted, deduplicated) crossings. The
    // scope is ORTHOGONAL to the level — every fn can be Certified(L3) AND the
    // project be to-the-boundary. Expected from REQ-4 (R-CHAR-3).
    #[test]
    fn aggregate_project_scope_any_to_boundary_lists_crossings() {
        use crate::manifest::{AssuranceScope, ProjectScope};
        let certs = vec![
            proved_cert("h").with_assurance_scope(AssuranceScope::ToBoundary {
                via: "ext_id".to_string(),
            }),
            proved_cert("g").with_assurance_scope(AssuranceScope::ToBoundary {
                via: "ext_id".to_string(),
            }),
            proved_cert("pure").with_assurance_scope(AssuranceScope::EndToEnd),
        ];
        let manifest = AssuranceManifest::aggregate(&certs);
        assert_eq!(
            manifest.scope,
            ProjectScope::ToBoundary {
                crossings: vec!["ext_id".to_string()]
            },
            "the crossing is listed once (deduplicated) even when two fns reach it"
        );
        // The level headline is independent — all proved L3 → Certified(L3).
        assert_eq!(manifest.project, ProjectAssurance::Certified(Level::L3));
    }

    // #17 e2e-vs-boundary REQ-4: an empty cert collection is vacuously END-TO-END
    // (nothing crosses a boundary). Expected from REQ-4 (R-CHAR-3).
    #[test]
    fn aggregate_project_scope_empty_is_end_to_end() {
        use crate::manifest::ProjectScope;
        let manifest = AssuranceManifest::aggregate(&[]);
        assert_eq!(manifest.scope, ProjectScope::EndToEnd);
    }
}
