//! `forge/src/verdict.rs` — the seven-variant certificate-level verdict (REQ-1 /
//! AC-1; `.design/stage1-forge-tier.md`). The forge tier's closed outcome
//! vocabulary, a **separate cert-level enum**, not arms of [`crate::engine::Verdict`]
//! (the three-arm engine type `Proven`/`Refuted`/`Unknown`).
//!
//! ## The split (the §gap-analysis "verdict architecture" decision)
//!
//! The engine layer decides three things and only three: it `Proven`, it `Refuted`
//! (with a witness), or it could not decide (`Unknown`). Three of the seven cert
//! verdicts come from that, by a total map with no wildcard arm ([`CertVerdict::
//! from_engine_verdict`]): `Proven → Proved`, `Refuted → Counterexample`, `Unknown →
//! Timeout`. No `Unknown` survives into a certificate (R-VERDICT-1): the map is total,
//! so every engine verdict becomes one of the seven.
//!
//! The other four have **no engine-level source** — they are produced upstream at the
//! forge orchestration layer, each by a different increment's machinery:
//! - `RealWitness` — the relax route (REQ-8/2f): a real countermodel of a clause true
//!   over ℤ but false over ℝ, carrying the raw real point (the 3-arm engine `Verdict`
//!   cannot carry it). Escalates UP to the forge, never down to `Counterexample`.
//! - `CovenantRefuted` — the covenant check (REQ-4/2b): a `falsify` hit, a hard fail
//!   in the [`crate::degrade`] ladder with the same never-degrades treatment as
//!   `Counterexample`.
//! - `Stuck` — the frozen battery (REQ-5/2c): the proof did not close; carries the
//!   residual goal(s) + a missing-bridge hint.
//! - `KernelBudget` — the budget wrapper (REQ-1b): a Lean elaboration/kernel-budget
//!   exhaustion (Q4 30s/clause), detected upstream via the textually-distinct signal
//!   [`crate::tv_signal::is_kernel_budget_signal`] (Q-KBSIGNAL — a distinct Lean signal
//!   does exist: `(deterministic) timeout … maximum number of heartbeats` /
//!   `maximum recursion depth has been reached`, never confusable with the Z3 rlimit
//!   text), so a budget exhaustion is `KernelBudget`, never mis-mapped to `Timeout`.
//!
//! For the foundation (this increment) the variants, their construction sites, and the
//! serde are defined; the *producing logic* for `RealWitness`/`CovenantRefuted`/`Stuck`
//! is built by 2b/2c/2f, so those are constructed only in tests here (and at the
//! pinned construction sites the later increments fill).

use serde::{Deserialize, Serialize};

use crate::engine::{Reason, Verdict};
use crate::manifest::ObligationResult;

/// A raw real countermodel point (REQ-8/2f): the per-variable real assignment the
/// nlsat (QF_NRA) query returned, as textual rationals/decimals. Carried by
/// [`CertVerdict::RealWitness`] — a clause true over ℤ but false over ℝ; the 3-arm
/// engine `Verdict`/`Counterexample` cannot carry a non-integer point. The producer is
/// the relax engine route (2f); the foundation defines the shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealPoint {
    /// `(variable, value)` pairs — the raw real point as nlsat reports it (a textual
    /// rational/decimal, e.g. `("n", "1.41421356…")` or `("n", "1414/1000")`).
    pub assignment: Vec<(String, String)>,
}

/// A covenant `falsify` counterexample (REQ-4/2b): the concrete input that refuted the
/// declared covenant, plus the deterministic SplitMix64 seed (Q3 default: a fixed-seed
/// `falsify 50_000`). Carried by [`CertVerdict::CovenantRefuted`]. The producer is the
/// covenant engine (2b); the foundation defines the shape so the degrade ladder can
/// treat it as a `Counterexample`-class hard fail today.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CovenantCounterexample {
    /// The concrete falsifying input (textual, the generator's rendered assignment).
    pub input: String,
    /// The deterministic SplitMix64 seed that produced the hit (Q3 fixed-seed).
    pub seed: u64,
}

/// The seven-variant certificate-level verdict (REQ-1 / AC-1). A closed set: every
/// path that yields a certificate yields exactly one of these, and no
/// [`crate::engine::Verdict::Unknown`] survives (the map [`CertVerdict::
/// from_engine_verdict`] is total). Serialized internally-tagged on `kind` (the stable
/// per-clause verdict string the schema-v2 block records, Q-ORACLE).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum CertVerdict {
    /// Proven for all inputs at the engine's level → certify. The only verdict
    /// [`from_engine_verdict`](CertVerdict::from_engine_verdict) constructs from a
    /// `Proven`; no other path constructs `Proved` from a non-`Proven` value
    /// (R-VERDICT-1, the never-converts-silently invariant).
    Proved,
    /// A witnessed countermodel → hard fail, never degrades. Carries the per-obligation
    /// witnesses (the engine `Counterexample`'s `obligations`).
    Counterexample {
        /// The witnessing per-obligation failure results (§5.1 "counterexamples, not
        /// adjectives").
        obligations: Vec<ObligationResult>,
    },
    /// A real countermodel of a clause true over ℤ, false over ℝ (REQ-8/2f) → escalate
    /// UP to the forge, never `Counterexample`. Carries the raw real point.
    RealWitness {
        /// The raw real point the nlsat query returned.
        point: RealPoint,
    },
    /// A covenant `falsify` hit (REQ-4/2b) → hard fail, the same never-degrades
    /// treatment as `Counterexample`. Carries the falsifying input + seed.
    CovenantRefuted {
        /// The concrete falsifying input + the deterministic seed.
        counterexample: CovenantCounterexample,
    },
    /// The proof did not close (REQ-5/2c) → not certified. Carries the residual
    /// goal(s) and an optional missing-bridge hint (the RFC-1 §8 transcript heuristic).
    Stuck {
        /// The residual proof goal(s) left open.
        goals: Vec<String>,
        /// An optional "missing simp bridge" hint (the battery heuristic).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hint: Option<String>,
    },
    /// A Lean elaboration/kernel-budget exhaustion (REQ-1b, Q4 30s/clause) → not
    /// certified. Distinct from a solver `Timeout` (Q-KBSIGNAL): detected upstream via
    /// the textually-distinct Lean signal, never the Z3 rlimit text.
    KernelBudget {
        /// The budget-exhaustion detail (which budget, the captured signal head).
        detail: String,
    },
    /// A solver resource-limit (rlimit) exhaustion → not certified. The
    /// [`Verdict::Unknown`] image under the total map (no `Unknown` survives).
    Timeout {
        /// The rlimit/timeout detail (the `VerusTimeout` / incompleteness detail).
        detail: String,
    },
}

impl CertVerdict {
    /// The stable verdict string (the per-clause `verdict` the schema-v2 block records,
    /// Q-ORACLE; also the cert-level diagnostic tag). Deterministic (R-CODE-5).
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            CertVerdict::Proved => "Proved",
            CertVerdict::Counterexample { .. } => "Counterexample",
            CertVerdict::RealWitness { .. } => "RealWitness",
            CertVerdict::CovenantRefuted { .. } => "CovenantRefuted",
            CertVerdict::Stuck { .. } => "Stuck",
            CertVerdict::KernelBudget { .. } => "KernelBudget",
            CertVerdict::Timeout { .. } => "Timeout",
        }
    }

    /// `true` iff this is the certifying verdict. The single predicate the cert
    /// assembly gates the certified level on (no other verdict certifies).
    #[allow(
        dead_code,
        reason = "REQ-1 certify-gate predicate: the forge-tier L3 cert assembly (2b+) \
                  gates certification on it; the foundation defines it and the never-\
                  converts-silently tests (degrade::tests) exercise it"
    )]
    #[must_use]
    pub fn is_proved(&self) -> bool {
        matches!(self, CertVerdict::Proved)
    }

    /// The total map from the three-arm engine [`Verdict`] into the cert vocabulary
    /// (REQ-1 / AC-1), by an exhaustive match with no wildcard arm: `Proven → Proved`,
    /// `Refuted → Counterexample`, `Unknown → Timeout`. Only these three of the seven
    /// have an engine-level source; the other four are produced upstream (see the
    /// module docs). The map is total, so no `engine::Verdict::Unknown` survives into a
    /// certificate, and — the never-converts-silently invariant (R-VERDICT-1) — `Proved`
    /// is constructed only from `Proven`.
    #[must_use]
    pub fn from_engine_verdict(v: &Verdict) -> Self {
        match v {
            Verdict::Proven(_) => CertVerdict::Proved,
            Verdict::Refuted(cx) => CertVerdict::Counterexample {
                obligations: cx.obligations.clone(),
            },
            Verdict::Unknown(reason) => CertVerdict::Timeout {
                detail: reason_detail(reason),
            },
        }
    }
}

/// The human detail carried by an engine [`Reason`] (exhaustive, no wildcard): both
/// non-`Proven`/`Refuted` reasons are solver-budget/incompleteness events that the cert
/// vocabulary classes as `Timeout` (a Lean KERNEL budget is discriminated upstream
/// before this map, by [`cert_verdict_for_lean`], so it never reaches here as a
/// `Timeout`).
fn reason_detail(reason: &Reason) -> String {
    match reason {
        Reason::VerusTimeout(detail) => detail.clone(),
        Reason::IncompleteUnknown(detail) => detail.clone(),
    }
}

/// Produce the cert verdict for a Lean discharge (REQ-1b/REQ-5, the KernelBudget + Stuck
/// upstream construction sites). The classification order, each distinct from the others:
///
/// 1. A Lean elaboration/kernel-budget exhaustion carries a textually-distinct signal
///    ([`crate::tv_signal::is_kernel_budget_signal`]) that the Z3 rlimit text never
///    matches and vice-versa (the negative test in `tv_signal`), so a budget exhaustion
///    is classed `KernelBudget` upstream — never routed through the 3-arm
///    [`CertVerdict::from_engine_verdict`] map (which would mis-call it `Timeout`).
/// 2. A proof that ELABORATED but left a residual goal ("unsolved goals", REQ-5/2c) is
///    [`CertVerdict::Stuck`] — the residual goal(s) + the frozen-battery missing-bridge
///    hint ([`crate::battery::stuck_from_lake_output`]) — never silently `Proved` and
///    never the solver `Timeout` the 3-arm map would assign (a tactic that ran but did
///    not close is not an rlimit event).
/// 3. Any other Lean outcome maps through the total engine map.
#[must_use]
pub fn cert_verdict_for_lean(lake_output: &str, engine_verdict: &Verdict) -> CertVerdict {
    if crate::tv_signal::is_kernel_budget_signal(lake_output) {
        let head: String = lake_output
            .lines()
            .find(|l| {
                let lower = l.to_ascii_lowercase();
                lower.contains("(deterministic) timeout")
                    || lower.contains("maximum number of heartbeats")
                    || lower.contains("maximum recursion depth")
            })
            .unwrap_or("Lean elaboration/kernel budget exhausted")
            .chars()
            .take(300)
            .collect();
        return CertVerdict::KernelBudget { detail: head };
    }
    // REQ-5/2c: a residual-goal failure is `Stuck` (it elaborated, the battery did not
    // close it), carrying the residual + the missing-bridge hint — never `Proved`, never
    // mapped to the solver `Timeout`.
    if let Some(stuck) = crate::battery::stuck_from_lake_output(lake_output) {
        return CertVerdict::Stuck {
            goals: stuck.goals,
            hint: stuck.hint,
        };
    }
    CertVerdict::from_engine_verdict(engine_verdict)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{CacheKey, Counterexample, EngineName, Evidence};
    use crate::manifest::ObligationResult;

    fn proven() -> Verdict {
        Verdict::Proven(Evidence {
            verified: 1,
            key: CacheKey {
                engine: EngineName::Verus,
                content_address: "k".to_string(),
            },
        })
    }

    fn refuted() -> Verdict {
        Verdict::Refuted(Counterexample {
            obligations: vec![ObligationResult::failed(
                "ens#0",
                Some("f.rs:1:1".to_string()),
                Some("postcondition not satisfied".to_string()),
            )],
        })
    }

    /// The total map is exhaustive with no wildcard: each of the three engine arms maps
    /// to exactly its cert image; `Unknown` (both reasons) becomes `Timeout`, so no
    /// `Unknown` survives (REQ-1 / AC-1).
    #[test]
    fn engine_verdict_maps_totally_and_no_unknown_survives() {
        assert_eq!(CertVerdict::from_engine_verdict(&proven()).kind(), "Proved");
        assert_eq!(
            CertVerdict::from_engine_verdict(&refuted()).kind(),
            "Counterexample"
        );
        assert_eq!(
            CertVerdict::from_engine_verdict(&Verdict::Unknown(Reason::VerusTimeout(
                "rlimit exceeded".to_string()
            )))
            .kind(),
            "Timeout"
        );
        assert_eq!(
            CertVerdict::from_engine_verdict(&Verdict::Unknown(Reason::IncompleteUnknown(
                "smt unknown".to_string()
            )))
            .kind(),
            "Timeout"
        );
    }

    /// The never-converts-silently invariant (R-VERDICT-1 / AC-3): `Proved` is
    /// constructed only from `Proven`. A `Refuted`/`Unknown` never yields `Proved`.
    #[test]
    fn proved_is_constructed_only_from_proven() {
        assert!(CertVerdict::from_engine_verdict(&proven()).is_proved());
        assert!(!CertVerdict::from_engine_verdict(&refuted()).is_proved());
        assert!(
            !CertVerdict::from_engine_verdict(&Verdict::Unknown(Reason::VerusTimeout(
                "t".to_string()
            )))
            .is_proved()
        );
    }

    /// A Lean kernel/elaboration-budget exhaustion is produced upstream as
    /// `KernelBudget`, not mapped to `Timeout` (Q-KBSIGNAL). A residual-goal failure is
    /// `Stuck` (REQ-5/2c), and only a budget-less, residual-less incompleteness falls
    /// through to the total engine map as `Timeout`.
    #[test]
    fn lean_kernel_budget_is_upstream_not_timeout() {
        let budget_out = "error: (deterministic) timeout at `isDefEq`, maximum number of \
                          heartbeats (200000) has been reached";
        let v = cert_verdict_for_lean(
            budget_out,
            &Verdict::Unknown(Reason::IncompleteUnknown("lake nonzero".to_string())),
        );
        assert_eq!(v.kind(), "KernelBudget");

        // A tactic that elaborated but left a residual goal is `Stuck` (REQ-5), not the
        // solver `Timeout` the 3-arm map would assign.
        let residual = "error: unsolved goals\n  ⊢ a ≤ b";
        let v2 = cert_verdict_for_lean(
            residual,
            &Verdict::Unknown(Reason::IncompleteUnknown("unsolved goals".to_string())),
        );
        assert_eq!(v2.kind(), "Stuck");

        // A budget-less, residual-less incompleteness (a rlimit/`unknown`) is the
        // engine map's `Timeout` image.
        let rlimit = "error: rlimit exceeded; resource limit reached";
        let v3 = cert_verdict_for_lean(
            rlimit,
            &Verdict::Unknown(Reason::VerusTimeout("rlimit exceeded".to_string())),
        );
        assert_eq!(v3.kind(), "Timeout");
    }

    /// serde round-trips for all seven variants (REQ-1 / AC-1): each serializes to a
    /// `"kind"`-tagged object and deserializes back to an equal value.
    #[test]
    fn all_seven_variants_round_trip() {
        let all = vec![
            CertVerdict::Proved,
            CertVerdict::Counterexample {
                obligations: vec![ObligationResult::discharged("f")],
            },
            CertVerdict::RealWitness {
                point: RealPoint {
                    assignment: vec![("n".to_string(), "1414/1000".to_string())],
                },
            },
            CertVerdict::CovenantRefuted {
                counterexample: CovenantCounterexample {
                    input: "n = 7".to_string(),
                    seed: 50_000,
                },
            },
            CertVerdict::Stuck {
                goals: vec!["⊢ a ≤ b".to_string()],
                hint: Some("missing simp bridge: melems_cons".to_string()),
            },
            CertVerdict::KernelBudget {
                detail: "(deterministic) timeout".to_string(),
            },
            CertVerdict::Timeout {
                detail: "rlimit exceeded".to_string(),
            },
        ];
        // Each kind tag is distinct (the closed seven-element set).
        let kinds: Vec<&str> = all.iter().map(CertVerdict::kind).collect();
        assert_eq!(kinds.len(), 7);
        let mut uniq = kinds.clone();
        uniq.sort_unstable();
        uniq.dedup();
        assert_eq!(uniq.len(), 7, "the seven kinds are distinct");

        for v in all {
            let json = serde_json::to_string(&v).expect("serialize CertVerdict");
            assert!(
                json.contains(&format!("\"kind\":\"{}\"", v.kind())),
                "the serialized form is tagged on `kind`: {json}"
            );
            let back: CertVerdict = serde_json::from_str(&json).expect("deserialize CertVerdict");
            assert_eq!(back, v, "round-trip preserves the verdict");
        }
    }
}
