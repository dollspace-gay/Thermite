//! `forge/src/tv_signal.rs` — the SHARED Verus/Z3 RESOURCE-LIMIT (rlimit) / timeout
//! signal discriminator for ALL THREE translation-validation check phases
//! (`forge/src/{contract_tv,body_tv,exec_tv}.rs`). Governed by
//! `.design/verified/exec-stmt-tv.md` REQ-5 (the body-TV four-way Faithful/Divergent/
//! Unverifiable/Skipped contract; R-HONEST-3 / R-CODE-4 — a Verus/Z3 timeout DEGRADES
//! and is reported, NEVER fabricated into a counterexample/Divergent). Tracking:
//! crosslink #192 (ref #166, #189).
//!
//! ## Why ONE helper (the #192 root cause)
//!
//! Before #192, EACH of the three TV phases carried its OWN private `is_rlimit_signal`
//! copy. The copies DRIFTED: `body_tv` (#189, the authority) matched THREE phrases;
//! `contract_tv` (#166) dropped `"resource limit exceeded"`; `exec_tv` had NO rlimit
//! gate at all (every `errors >= 1` run was mapped UNCONDITIONALLY to Divergent — the
//! #189-class bug). A drifted/missing discriminator fabricates a solver-budget
//! exhaustion into a contract/body/exec INFIDELITY — the exact false finding the #189
//! fix claims to close. The architectural fix: ONE shared discriminator, consumed by
//! all three phases, so the phrase set can NEVER drift again.
//!
//! ## The full phrase set (case-insensitive)
//!
//! - `"rlimit exceeded"` — verus's bare rlimit phrasing.
//! - `"rlimit) exceeded"` — verus's `Resource limit (rlimit) exceeded` (the `air`
//!   literal — the parenthesised tail `rlimit) exceeded`).
//! - `"resource limit exceeded"` — which ALSO catches the distributed z3 binary's OWN
//!   resourceout diagnostic `max. resource limit exceeded` (its `:reason-unknown`
//!   text on an rcounts exhaustion — present as a `strings` literal in the bundled
//!   `verus-x86-linux/z3`, independent of any Thermite source). This is the
//!   load-bearing phrase #166's `contract_tv` copy had dropped and #189's `exec_tv`
//!   never had.
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | exec-stmt-tv REQ-5 (the rlimit discriminator — a Verus/Z3 timeout is Unverifiable, NEVER Divergent; R-HONEST-3 / R-CODE-4) | SHIPPED | `pub(crate) fn is_rlimit_signal` here is the SOLE discriminator. Non-test consumers: `contract_tv::discharge`, `body_tv::run_obligation`, `exec_tv::discharge` (all three TV-phase verus-discharge paths route an `errors >= 1` rlimit-hit run to their Unverifiable-equivalent verdict AHEAD of the Divergent arm). Verified by the `discriminator` unit tests here (each phrase detected; `postcondition not satisfied`/`assertion failed` NOT detected) + the per-phase `divergent_teeth` (rlimit → not Divergent) + `forge/tests/divergence_rlimit_phrase_drift.rs` (phrase parity + the z3 `max. resource limit exceeded` literal caught). |

/// `true` iff the combined verus output carries a Verus/Z3 RESOURCE-LIMIT (rlimit)
/// exhaustion / timeout signal (case-insensitive). Such an error run is a DISCHARGE
/// failure (the solver ran out of budget), NOT a meaning mismatch / value
/// counterexample, so the TV phases route it to their Unverifiable-equivalent
/// verdict, NEVER Divergent (exec-stmt-tv.md REQ-5 four-way; R-HONEST-3 / R-CODE-4 —
/// a timeout degrades + is reported, never treated as a counterexample).
///
/// The FULL phrase set (covering verus's two phrasings + the distributed z3 binary's
/// own resourceout literal):
/// - `rlimit exceeded` — verus's bare phrasing;
/// - `rlimit) exceeded` — verus's `Resource limit (rlimit) exceeded`;
/// - `resource limit exceeded` — which ALSO catches z3's `max. resource limit
///   exceeded` (the bundled binary's `:reason-unknown` text — the #166-dropped,
///   #189-missing phrase).
pub(crate) fn is_rlimit_signal(output: &str) -> bool {
    let lower = output.to_ascii_lowercase();
    lower.contains("rlimit exceeded")
        || lower.contains("rlimit) exceeded")
        || lower.contains("resource limit exceeded")
}

// ---- the discriminator teeth (the SHARED-helper unit coverage) --------------
//
// Both directions, hand-derived (R-CHAR-3): each rlimit phrase is detected; a
// genuine counterexample diagnostic (`postcondition not satisfied` / `assertion
// failed`) is NOT. The expected values come from the verus/air rlimit literals + the
// distributed z3 binary's `max. resource limit exceeded` literal — NEVER from the
// helper's own output.
#[cfg(test)]
mod discriminator {
    use super::is_rlimit_signal;

    /// EACH rlimit phrase is detected (case-insensitively), including the z3-phrased
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
        // The distributed z3 binary's OWN resourceout diagnostic — contains
        // `resource limit exceeded` but NEITHER `rlimit exceeded` NOR `rlimit) exceeded`
        // (no `rlimit` token), so ONLY the third phrase catches it (the #166-dropped,
        // #189-missing load-bearing clause).
        assert!(
            is_rlimit_signal("unknown: max. resource limit exceeded\n0 verified, 1 errors"),
            "z3's own `max. resource limit exceeded` resourceout literal must be detected \
             (the third phrase is load-bearing: it carries `resource limit exceeded` but no \
             `rlimit` token)"
        );
    }

    /// A GENUINE counterexample diagnostic is NOT detected — it must stay in the
    /// Divergent class (the discriminator's NEGATIVE direction).
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
