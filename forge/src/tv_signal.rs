//! `forge/src/tv_signal.rs` — the shared Verus/Z3 resource-limit (rlimit) / timeout
//! signal discriminator for all three translation-validation check phases
//! (`forge/src/{contract_tv,body_tv,exec_tv}.rs`). Governed by
//! `.design/verified/exec-stmt-tv.md` REQ-5 (the body-TV four-way Faithful/Divergent/
//! Unverifiable/Skipped contract; R-HONEST-3 / R-CODE-4 — a Verus/Z3 timeout degrades
//! and is reported, not fabricated into a counterexample/Divergent). Tracking:
//! crosslink #192 (ref #166, #189).
//!
//! ## Why one helper (the #192 root cause)
//!
//! Before #192, each of the three TV phases carried its own private `is_rlimit_signal`
//! copy. The copies drifted: `body_tv` (#189, the authority) matched three phrases;
//! `contract_tv` (#166) dropped `"resource limit exceeded"`; `exec_tv` had no rlimit
//! gate at all (every `errors >= 1` run was mapped unconditionally to Divergent — the
//! #189-class bug). A drifted or missing discriminator fabricates a solver-budget
//! exhaustion into a contract/body/exec infidelity, the false finding the #189
//! fix closes. The fix: one shared discriminator, consumed by
//! all three phases, so the phrase set does not drift again.
//!
//! ## The full phrase set (case-insensitive)
//!
//! - `"rlimit exceeded"` — verus's bare rlimit phrasing.
//! - `"rlimit) exceeded"` — verus's `Resource limit (rlimit) exceeded` (the `air`
//!   literal — the parenthesised tail `rlimit) exceeded`).
//! - `"resource limit exceeded"` — which also catches the distributed z3 binary's own
//!   resourceout diagnostic `max. resource limit exceeded` (its `:reason-unknown`
//!   text on an rcounts exhaustion — present as a `strings` literal in the bundled
//!   `verus-x86-linux/z3`, independent of any Thermite source). This is the
//!   phrase #166's `contract_tv` copy had dropped and #189's `exec_tv`
//!   never had.
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | exec-stmt-tv REQ-5 (the rlimit discriminator — a Verus/Z3 timeout is Unverifiable, NEVER Divergent; R-HONEST-3 / R-CODE-4) | SHIPPED | `pub(crate) fn is_rlimit_signal` here is the SOLE discriminator. Non-test consumers: `contract_tv::discharge`, `body_tv::run_obligation`, `exec_tv::discharge` (all three TV-phase verus-discharge paths route an `errors >= 1` rlimit-hit run to their Unverifiable-equivalent verdict AHEAD of the Divergent arm). Verified by the `discriminator` unit tests here (each phrase detected; `postcondition not satisfied`/`assertion failed` NOT detected) + the per-phase `divergent_teeth` (rlimit → not Divergent) + `forge/tests/divergence_rlimit_phrase_drift.rs` (phrase parity + the z3 `max. resource limit exceeded` literal caught). |

/// `true` iff the combined verus output carries a Verus/Z3 resource-limit (rlimit)
/// exhaustion / timeout signal (case-insensitive). Such an error run is a discharge
/// failure (the solver ran out of budget), not a meaning mismatch or value
/// counterexample, so the TV phases route it to their Unverifiable-equivalent
/// verdict rather than Divergent (exec-stmt-tv.md REQ-5 four-way; R-HONEST-3 / R-CODE-4 —
/// a timeout degrades and is reported, not treated as a counterexample).
///
/// The full phrase set (covering verus's two phrasings + the distributed z3 binary's
/// own resourceout literal):
/// - `rlimit exceeded` — verus's bare phrasing;
/// - `rlimit) exceeded` — verus's `Resource limit (rlimit) exceeded`;
/// - `resource limit exceeded` — which also catches z3's `max. resource limit
///   exceeded` (the bundled binary's `:reason-unknown` text — the #166-dropped,
///   #189-missing phrase).
pub(crate) fn is_rlimit_signal(output: &str) -> bool {
    let lower = output.to_ascii_lowercase();
    lower.contains("rlimit exceeded")
        || lower.contains("rlimit) exceeded")
        || lower.contains("resource limit exceeded")
}

/// `true` iff the combined Lean (`lake env lean`) output carries a kernel/elaboration
/// BUDGET exhaustion signal (case-insensitive) — distinct from the solver `rlimit`
/// signal above (`.design/stage1-forge-tier.md` REQ-1b / AC-2, Q-KBSIGNAL). This is the
/// second discriminator in the same one-shared-helper pattern: a budget-exhausted Lean
/// discharge is the cert verdict `KernelBudget` (the forge tier's Q4 30s/clause
/// elaboration budget), NOT a solver `Timeout` and NOT a meaning mismatch / `Stuck`.
///
/// ## Q-KBSIGNAL probe result (recorded in the increment commit)
///
/// The probe asked whether Lean emits a textually-distinct budget signal vs the Z3
/// rlimit text. It DOES — `lake env lean` was driven to each budget edge and emits:
/// - `(deterministic) timeout at <op>, maximum number of heartbeats (N) has been reached`
///   — the elaboration HEARTBEAT budget (`set_option maxHeartbeats`);
/// - `maximum recursion depth has been reached` — the elaboration/kernel RECURSION
///   budget (`set_option maxRecDepth`).
///
/// So the probe selected the DISCRIMINATOR path (AC-2's "a distinct signal exists"
/// branch), NOT the wall-clock-wrapper fallback. The phrase set is mutually exclusive
/// with [`is_rlimit_signal`]'s (neither carries `rlimit`/`resource limit`; the rlimit
/// phrases carry no `timeout`/`heartbeats`/`recursion depth`), proven by the
/// `kernel_budget_and_rlimit_cannot_be_confused` negative test.
pub(crate) fn is_kernel_budget_signal(output: &str) -> bool {
    let lower = output.to_ascii_lowercase();
    lower.contains("(deterministic) timeout")
        || lower.contains("maximum number of heartbeats")
        || lower.contains("maximum recursion depth")
}

// ---- the discriminator teeth (the shared-helper unit coverage) --------------
//
// Both directions, hand-derived (R-CHAR-3): each rlimit phrase is detected; a
// counterexample diagnostic (`postcondition not satisfied` / `assertion
// failed`) is not. The expected values come from the verus/air rlimit literals + the
// distributed z3 binary's `max. resource limit exceeded` literal, not from the
// helper's own output.
#[cfg(test)]
mod discriminator {
    use super::is_rlimit_signal;

    /// Each rlimit phrase is detected (case-insensitively), including the z3-phrased
    /// resourceout `max. resource limit exceeded` (the #166-dropped, #189-missing
    /// phrase the #192 root-cause fix restores for all three phases).
    #[test]
    fn each_rlimit_phrase_is_detected() {
        // verus's bare phrasing.
        assert!(
            is_rlimit_signal("error: rlimit exceeded; consider raising the budget"),
            "the bare `rlimit exceeded` phrasing must be detected"
        );
        // verus's `Resource limit (rlimit) exceeded` (the `air` literal — its tail
        // `rlimit) exceeded`), with mixed case to pin case-insensitivity.
        assert!(
            is_rlimit_signal("error: Resource limit (rlimit) exceeded\n0 verified, 1 errors"),
            "verus's `Resource limit (rlimit) exceeded` must be detected"
        );
        // The distributed z3 binary's own resourceout diagnostic — contains
        // `resource limit exceeded` but neither `rlimit exceeded` nor `rlimit) exceeded`
        // (no `rlimit` token), so only the third phrase catches it (the #166-dropped,
        // #189-missing clause).
        assert!(
            is_rlimit_signal("unknown: max. resource limit exceeded\n0 verified, 1 errors"),
            "z3's own `max. resource limit exceeded` resourceout literal must be detected \
             (the third phrase is load-bearing: it carries `resource limit exceeded` but no \
             `rlimit` token)"
        );
    }

    use super::is_kernel_budget_signal;

    /// Each Lean kernel/elaboration-budget phrase is detected (case-insensitively):
    /// the `maxHeartbeats` deterministic-timeout edge and the `maxRecDepth` recursion
    /// edge, the two signals the Q-KBSIGNAL probe observed `lake env lean` emit.
    #[test]
    fn each_kernel_budget_phrase_is_detected() {
        assert!(
            is_kernel_budget_signal(
                "error: (deterministic) timeout at `isDefEq`, maximum number of heartbeats \
                 (200000) has been reached"
            ),
            "the maxHeartbeats `(deterministic) timeout … heartbeats` phrasing must be detected"
        );
        assert!(
            is_kernel_budget_signal("error: maximum recursion depth has been reached"),
            "the maxRecDepth `maximum recursion depth` phrasing must be detected"
        );
    }

    /// The negative test (REQ-1b / AC-2): the kernel-budget and solver-rlimit
    /// discriminators CANNOT be confused. The canonical rlimit signals are NOT detected
    /// as a kernel budget, and the canonical kernel-budget signals are NOT detected as an
    /// rlimit — the two phrase sets are mutually exclusive, so a solver timeout is never
    /// mis-classed `KernelBudget` and a Lean budget exhaustion is never mis-classed
    /// `Timeout`.
    #[test]
    fn kernel_budget_and_rlimit_cannot_be_confused() {
        // The rlimit signals are NOT kernel-budget.
        for rlimit in [
            "error: rlimit exceeded; consider raising the budget",
            "error: Resource limit (rlimit) exceeded\n0 verified, 1 errors",
            "unknown: max. resource limit exceeded\n0 verified, 1 errors",
        ] {
            assert!(
                is_rlimit_signal(rlimit),
                "precondition: a real rlimit signal"
            );
            assert!(
                !is_kernel_budget_signal(rlimit),
                "an rlimit signal must NOT be detected as a kernel/elaboration budget: {rlimit:?}"
            );
        }
        // The kernel-budget signals are NOT rlimit.
        for budget in [
            "error: (deterministic) timeout at `whnf`, maximum number of heartbeats (1) has been \
             reached",
            "error: maximum recursion depth has been reached",
        ] {
            assert!(
                is_kernel_budget_signal(budget),
                "precondition: a real kernel-budget signal"
            );
            assert!(
                !is_rlimit_signal(budget),
                "a kernel/elaboration budget signal must NOT be detected as an rlimit: {budget:?}"
            );
        }
    }

    /// A counterexample diagnostic is not detected — it stays in the
    /// Divergent class (the discriminator's negative direction). Neither the rlimit nor
    /// the kernel-budget discriminator fires on a genuine counterexample.
    #[test]
    fn genuine_counterexample_is_not_detected() {
        assert!(
            !is_rlimit_signal(
                "error: postcondition not satisfied\n --> x.rs:5:12\n0 verified, 1 errors"
            ),
            "a `postcondition not satisfied` counterexample must NOT be detected as a timeout \
             (it stays in the Divergent class)"
        );
        assert!(
            !is_rlimit_signal("error: assertion failed\n --> x.rs:5:12\n0 verified, 1 errors"),
            "an `assertion failed` counterexample must NOT be detected as a timeout (it stays \
             in the Divergent class)"
        );
    }
}
