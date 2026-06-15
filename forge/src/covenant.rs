//! `forge/src/covenant.rs` — the covenant record (REQ-4; `.design/stage1-forge-tier.md`).
//!
//! A covenant (RFC-1 §5) is the author's `witness { inhabit (…); falsify N; }` block: a
//! set of author-stated witnesses that must inhabit the precondition + a falsification
//! budget the generator drives at the executable semantics before any proof search is
//! allowed to "burn". The proof-search path must not start without a covenant record —
//! covenant-before-burn — which is why the record is a NON-OPTIONAL parameter of
//! [`crate::engine::Engine::discharge`] (a type-level seam, not a runtime convention).
//!
//! ## What the FOUNDATION ships vs what 2b builds
//!
//! This increment (the foundation) introduces the record TYPE and threads it through
//! the `Engine::discharge` signature and every call site, so the cross-cutting signature
//! ripple happens ONCE. It does NOT build the covenant LOGIC (the `inhabit` type-check +
//! execute, the `falsify` generator run, the covenant-before-burn enforcement) — that is
//! increment 2b. Today the `witness` SURFACE SYNTAX is REQ-3 and not present in the
//! parser, so every program legitimately carries a [`CovenantRecord::none`] — a truthful
//! "no covenant declared" record, NOT a stub: there is nothing to declare yet, and 2b
//! both adds the syntax and fills in the producing logic at the seam this record marks.
//!
//! The Q3 defaults (a fixed-seed `falsify 50_000` when a covenant is declared without an
//! explicit budget) are recorded on the type so 2b's producer and the cert's covenant
//! block (Q-ORACLE: witness count + falsify counts + seed join the forge-tier oracle)
//! read one shape.

use serde::{Deserialize, Serialize};

/// The Q3 default falsification budget (`falsify 50_000`) when a covenant is declared
/// without an explicit budget (`.design/stage1-forge-tier.md` REQ-4, Q3).
#[allow(
    dead_code,
    reason = "REQ-4/Q3 seam: the 2b covenant engine reads this default when a declared \
              covenant omits an explicit `falsify N`; the foundation defines it (and the \
              round-trip test exercises it) ahead of the producer"
)]
pub const DEFAULT_FALSIFY_BUDGET: u64 = 50_000;

/// The Q3 fixed falsification seed — the generator (`thermite-tv`'s SplitMix64, clock-free)
/// is seeded deterministically so a covenant's evidence is reproducible and cannot drift
/// silently (Q-ORACLE: the seed joins the forge-tier cert oracle).
pub const DEFAULT_FALSIFY_SEED: u64 = 0x5EED_0000_0000_C0DE;

/// The covenant record threaded into every discharge (REQ-4). A program with no declared
/// `witness` block carries [`CovenantRecord::none`] (truthful today: the surface syntax is
/// REQ-3, not present). When 2b lands the `witness` syntax + the covenant engine, the
/// parser populates `witnesses`/`falsify_budget` and the discharge enforces
/// covenant-before-burn — no call site changes, because the seam already threads this.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CovenantRecord {
    /// Whether a `witness` block was DECLARED for this item. `false` for every program
    /// today (the surface syntax is REQ-3, not yet present), so [`CovenantRecord::none`]
    /// is the truthful record, not a placeholder.
    pub declared: bool,
    /// The author-stated `inhabit` witnesses (REQ-4: at least one must be author-stated
    /// when a covenant is declared). Empty when none declared.
    pub witnesses: Vec<String>,
    /// The `falsify` budget (Q3 default [`DEFAULT_FALSIFY_BUDGET`] when declared without
    /// one). `0` when no covenant is declared (no falsification is owed).
    pub falsify_budget: u64,
    /// The deterministic SplitMix64 seed for the `falsify` run (Q3 fixed seed).
    pub falsify_seed: u64,
}

impl CovenantRecord {
    /// The truthful "no covenant declared" record (REQ-4): the record every program
    /// carries today, because the `witness` surface syntax (REQ-3) is not present yet.
    /// NOT a stub — there is nothing to declare, so no witnesses, no falsification owed.
    #[must_use]
    pub fn none() -> Self {
        CovenantRecord {
            declared: false,
            witnesses: Vec::new(),
            falsify_budget: 0,
            falsify_seed: DEFAULT_FALSIFY_SEED,
        }
    }

    /// `true` iff a covenant was declared (always `false` today — the seam 2b flips when
    /// the `witness` syntax lands).
    #[allow(
        dead_code,
        reason = "REQ-4 seam: the 2b covenant-before-burn enforcement reads this; the \
                  foundation defines the accessor (exercised by the round-trip test) \
                  ahead of the enforcement"
    )]
    #[must_use]
    pub fn is_declared(&self) -> bool {
        self.declared
    }
}

impl Default for CovenantRecord {
    fn default() -> Self {
        CovenantRecord::none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The no-covenant-declared record is truthful: not declared, no witnesses, no
    /// falsification owed, but the fixed seed is recorded (the Q3 deterministic default).
    #[test]
    fn none_is_a_truthful_no_covenant_record() {
        let c = CovenantRecord::none();
        assert!(!c.is_declared());
        assert!(c.witnesses.is_empty());
        assert_eq!(c.falsify_budget, 0);
        assert_eq!(c.falsify_seed, DEFAULT_FALSIFY_SEED);
        assert_eq!(c, CovenantRecord::default());
    }

    /// The record round-trips through serde (it joins the forge-tier cert covenant block,
    /// Q-ORACLE).
    #[test]
    fn covenant_record_round_trips() {
        let c = CovenantRecord {
            declared: true,
            witnesses: vec!["xs = [1, 2, 3]".to_string()],
            falsify_budget: DEFAULT_FALSIFY_BUDGET,
            falsify_seed: DEFAULT_FALSIFY_SEED,
        };
        let json = serde_json::to_string(&c).expect("serialize");
        let back: CovenantRecord = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, c);
    }
}
