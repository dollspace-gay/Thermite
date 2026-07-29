//! `forge/src/metrics.rs` — the §6 metrics dashboard (umbrella
//! `.design/thermite2-program.md` REQ-7 / AC-12: "From M1, `forge` emits the
//! routing-reason and verdict telemetry fields the §6 dashboard needs — cage-vs-forge
//! share by reason, verdict counts, TV phase split — and the audit prints them").
//!
//! This module is a **read-only projection** of telemetry `forge` already emits: the
//! per-clause `{engine, verdict}` attribution the schema-v2 certificate carries
//! (`manifest::ObligationResult`) and the translation-validation (TV) phase verdicts
//! (`contract_tv::TvCounts`). It aggregates three §6 metrics:
//!
//! 1. **cage-vs-forge share BY routing reason** — every discharged clause was routed
//!    either to the Verus/Z3 *cage* (the default push-button path) or escalated UP to
//!    the *forge* (the nlsat relax route or the Lean lemma route). The routing reason
//!    is a total, deterministic projection of the per-clause `engine` tag
//!    ([`RoutingReason::from_engine`]): `verus`/absent ⇒ in-cage, `nlsat` ⇒ relaxable,
//!    `lean-*` ⇒ lemma. No new certificate field is introduced — `engine` IS the
//!    routing telemetry, so the v1 and forge-tier conformance goldens stay
//!    byte-identical (the metrics are a projection, never part of the cert oracle).
//! 2. **the seven-verdict counts** — the tally over the closed forge-tier vocabulary
//!    [`crate::verdict::CertVerdict`] (the per-clause `verdict` field).
//! 3. **the TV phase split** — the contract-TV outcome split mapped to the §6
//!    taxonomy: faithful (verified faithful), syntactic (a `Skipped` clause outside
//!    the framed sublanguage), semantic (a `Divergent` lowering infidelity), and
//!    timeout (an `Unverifiable` rlimit / verus-absent run).
//!
//! ## Gates nothing (#274, `.design/forge/audit-manifest.md` REQ-10)
//!
//! The dashboard is printed by `forge audit --metrics` as an informational section
//! after the manifest. It changes no exit code and alters no verdict — like
//! the `--meaning` companion. Its output is not part of the certificate
//! `oracle_subset`, so it never perturbs a golden.
//!
//! ## REQ status
//!
//! <!-- generated:reqs view=forge-metrics-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-FORGE-METRICS-DASHBOARD | shipped | `forge/src/metrics.rs` | §6 metrics dashboard projection |  |
//! <!-- /generated:reqs -->

use crate::contract_tv::TvCounts;
use crate::manifest::Certificate;

/// The closed seven-verdict vocabulary, in [`crate::verdict::CertVerdict`] declaration
/// order (the stable dashboard column order, R-CODE-5 deterministic). The dashboard
/// tallies the per-clause `verdict` field into these seven buckets.
pub const VERDICT_KINDS: [&str; 7] = [
    "Proved",
    "Counterexample",
    "RealWitness",
    "CovenantRefuted",
    "Stuck",
    "KernelBudget",
    "Timeout",
];

/// The per-clause routing reason — WHY a clause was discharged where it was (REQ-7).
/// A total, deterministic projection of the schema-v2 per-clause `engine` tag, not a
/// new certificate field (the cert schema is untouched, so goldens stay byte-identical).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingReason {
    /// The clause stayed in the Verus/Z3 **cage** — the default push-button path
    /// (engine `verus`, or a clause with no per-clause attribution: the v1 Verus
    /// corpus representation, which is the cage by construction).
    InCage,
    /// The clause was **relaxable** — a polynomial side-condition escalated UP to the
    /// forge's nlsat (QF_NRA) real-relaxation route (engine `nlsat`).
    Relaxable,
    /// The clause was discharged by a forge **lemma** — the Lean route (an
    /// author-authored frozen-battery proof / lemma; engine `lean-auto`/
    /// `lean-interactive`).
    Lemma,
}

/// Whether a routing reason kept the clause in the cage or escalated it to the forge
/// (REQ-7 — the cage-vs-forge share).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Share {
    /// The Verus/Z3 cage (the default push-button path).
    Cage,
    /// The forge (nlsat relax or Lean lemma — escalated UP, not degraded DOWN).
    Forge,
}

impl RoutingReason {
    /// Project the per-clause `engine` tag to its routing reason (REQ-7) — a total map
    /// over the closed engine-tag set ([`crate::engine::EngineName::tag`]):
    /// `nlsat` ⇒ [`RoutingReason::Relaxable`], any `lean*` tag ⇒ [`RoutingReason::Lemma`],
    /// and `verus` / `None` (the v1 Verus corpus clause, which carries no per-clause
    /// attribution) / any other tag ⇒ [`RoutingReason::InCage`] (the cage is the
    /// default route). Deterministic (R-CODE-5).
    #[must_use]
    pub fn from_engine(engine: Option<&str>) -> Self {
        match engine {
            Some("nlsat") => RoutingReason::Relaxable,
            Some(e) if e.starts_with("lean") => RoutingReason::Lemma,
            // `verus`, the absent v1-corpus clause, or any other tag is the cage.
            _ => RoutingReason::InCage,
        }
    }

    /// The cage-vs-forge classification (REQ-7): in-cage is the cage; relaxable and
    /// lemma are both forge escalations.
    #[must_use]
    pub fn share(self) -> Share {
        match self {
            RoutingReason::InCage => Share::Cage,
            RoutingReason::Relaxable | RoutingReason::Lemma => Share::Forge,
        }
    }

    /// The stable dashboard label for this routing reason (deterministic).
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            RoutingReason::InCage => "in-cage",
            RoutingReason::Relaxable => "relaxable",
            RoutingReason::Lemma => "lemma",
        }
    }
}

/// The cage-vs-forge routing share BY reason (REQ-7) — the per-reason clause counts.
/// `in_cage` is the whole cage share; `relaxable` + `lemma` is the whole forge share.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RoutingShare {
    /// Clauses discharged in the Verus/Z3 cage (the default path).
    pub in_cage: usize,
    /// Clauses escalated to the forge nlsat relax route (relaxable side-conditions).
    pub relaxable: usize,
    /// Clauses discharged by a forge Lean lemma (author-authored proofs).
    pub lemma: usize,
}

impl RoutingShare {
    /// The total cage share (REQ-7).
    #[must_use]
    pub fn cage(&self) -> usize {
        self.in_cage
    }

    /// The total forge share — relaxable + lemma (REQ-7).
    #[must_use]
    pub fn forge(&self) -> usize {
        self.relaxable + self.lemma
    }

    /// The total routed clauses (cage + forge).
    #[must_use]
    pub fn total(&self) -> usize {
        self.cage() + self.forge()
    }

    /// The clause count for a given routing reason (REQ-7).
    #[must_use]
    pub fn count(&self, reason: RoutingReason) -> usize {
        match reason {
            RoutingReason::InCage => self.in_cage,
            RoutingReason::Relaxable => self.relaxable,
            RoutingReason::Lemma => self.lemma,
        }
    }

    /// Tally one clause by its routing reason.
    fn bump(&mut self, reason: RoutingReason) {
        match reason {
            RoutingReason::InCage => self.in_cage += 1,
            RoutingReason::Relaxable => self.relaxable += 1,
            RoutingReason::Lemma => self.lemma += 1,
        }
    }
}

/// The seven-verdict counts (REQ-7) — the tally over [`VERDICT_KINDS`] (the closed
/// [`crate::verdict::CertVerdict`] vocabulary), indexed in declaration order. A clause
/// with no recorded seven-verdict (the v1 Verus corpus, which records `status`/`level`)
/// is counted in [`VerdictCounts::unattributed`] rather than dropped — so the columns
/// always sum to the clause total.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct VerdictCounts {
    /// Counts indexed by [`VERDICT_KINDS`] position (declaration order).
    pub by_kind: [usize; 7],
    /// Clauses carrying no seven-verdict (the v1 Verus corpus representation).
    pub unattributed: usize,
}

impl VerdictCounts {
    /// The count for a verdict kind string, or 0 for an unknown kind.
    #[must_use]
    pub fn get(&self, kind: &str) -> usize {
        verdict_index(kind).map_or(0, |i| self.by_kind[i])
    }

    /// Tally one clause's recorded verdict (by its [`crate::verdict::CertVerdict::kind`]).
    fn bump(&mut self, kind: &str) {
        match verdict_index(kind) {
            Some(i) => self.by_kind[i] += 1,
            // The closed seven-element set is exhaustive over CertVerdict::kind(), so
            // this arm is unreachable in practice; an unrecognized kind is counted as
            // unattributed rather than silently dropped (R-DEFER-9).
            None => self.unattributed += 1,
        }
    }
}

/// The index of a verdict kind in [`VERDICT_KINDS`] (declaration order), or `None` for
/// an unrecognized kind.
fn verdict_index(kind: &str) -> Option<usize> {
    VERDICT_KINDS.iter().position(|k| *k == kind)
}

/// The TV phase split (REQ-7) — the contract-TV outcome split mapped to the §6
/// taxonomy. The mapping over [`crate::contract_tv::ClauseVerdict`]:
/// - `faithful` ← `Faithful` (the lowering verified faithful — the baseline);
/// - `syntactic` ← `Skipped` (a clause outside the framed sublanguage — a syntactic
///   coverage gap, reported not-checked rather than a false faithful);
/// - `semantic` ← `Divergent` (verus found a counterexample — a semantic lowering
///   infidelity);
/// - `timeout` ← `Unverifiable` (a Verus/Z3 rlimit exhaustion or verus-absent run —
///   surfaced, never fabricated into a Divergent).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TvPhaseSplit {
    /// Clauses verified faithful (the baseline).
    pub faithful: usize,
    /// Clauses outside the framed sublanguage (`Skipped` — a syntactic gap).
    pub syntactic: usize,
    /// Clauses with a semantic lowering infidelity (`Divergent`).
    pub semantic: usize,
    /// Clauses the solver could not decide (`Unverifiable` — rlimit / verus-absent).
    pub timeout: usize,
}

impl TvPhaseSplit {
    /// Project the contract-TV per-verdict tally to the §6 phase split (REQ-7).
    #[must_use]
    pub fn from_tv_counts(c: &TvCounts) -> Self {
        TvPhaseSplit {
            faithful: c.faithful,
            syntactic: c.skipped,
            semantic: c.divergent,
            timeout: c.unverifiable,
        }
    }

    /// The total TV clauses across the four buckets.
    #[must_use]
    pub fn total(&self) -> usize {
        self.faithful + self.syntactic + self.semantic + self.timeout
    }
}

/// The aggregated §6 metrics dashboard (REQ-7 / AC-12) — a read-only projection of the
/// per-clause certificate telemetry (routing + verdicts) and the TV phase split. Gates
/// nothing; never part of the cert oracle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetricsDashboard {
    /// The cage-vs-forge share BY routing reason (REQ-7).
    pub routing: RoutingShare,
    /// The seven-verdict counts (REQ-7).
    pub verdicts: VerdictCounts,
    /// The contract-TV phase split — `None` when the TV phase was not run (e.g. a
    /// forge-tier-only file the contract-TV phase treats as inert), so the dashboard
    /// reports "not run" rather than a misleading all-zero split.
    pub tv: Option<TvPhaseSplit>,
    /// The total per-clause obligations projected (the routing/verdict denominator).
    pub total_clauses: usize,
}

impl MetricsDashboard {
    /// Project the dashboard from a settled certificate collection + an optional
    /// contract-TV phase tally (REQ-7) — a pure aggregation, no re-derivation. Every
    /// per-clause obligation across `certs` contributes its routing reason (from the
    /// `engine` tag) and its seven-verdict (from the `verdict` field, if any).
    #[must_use]
    pub fn from_certificates(certs: &[Certificate], tv: Option<&TvCounts>) -> Self {
        let mut routing = RoutingShare::default();
        let mut verdicts = VerdictCounts::default();
        let mut total_clauses = 0usize;
        for cert in certs {
            for obl in &cert.obligations {
                total_clauses += 1;
                routing.bump(RoutingReason::from_engine(obl.engine.as_deref()));
                match &obl.verdict {
                    Some(v) => verdicts.bump(v.kind()),
                    None => verdicts.unattributed += 1,
                }
            }
        }
        MetricsDashboard {
            routing,
            verdicts,
            tv: tv.map(TvPhaseSplit::from_tv_counts),
            total_clauses,
        }
    }

    /// Render the dashboard as the `forge audit --metrics` human-readable section
    /// (REQ-7 / AC-12). A pure function of the projected counts — deterministic
    /// (R-CODE-5) and gating nothing.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::from(
            "\n=== §6 metrics dashboard (REQ-7; read-only projection, gates nothing) ===\n",
        );

        // (1) cage-vs-forge share BY routing reason.
        out.push_str(&format!(
            "routing (cage vs forge, by reason) — {} clause(s):\n",
            self.routing.total()
        ));
        out.push_str(&format!("  cage  : {}\n", self.routing.cage()));
        out.push_str(&format!("  forge : {}\n", self.routing.forge()));
        for reason in [
            RoutingReason::InCage,
            RoutingReason::Relaxable,
            RoutingReason::Lemma,
        ] {
            let share = match reason.share() {
                Share::Cage => "cage",
                Share::Forge => "forge",
            };
            out.push_str(&format!(
                "    {} [{share}]: {}\n",
                reason.label(),
                self.routing.count(reason)
            ));
        }

        // (2) the seven-verdict counts.
        out.push_str(&format!(
            "verdicts (the seven CertVerdict) — {} attributed clause(s):\n",
            self.total_clauses - self.verdicts.unattributed
        ));
        for kind in VERDICT_KINDS {
            out.push_str(&format!("  {kind}: {}\n", self.verdicts.get(kind)));
        }
        if self.verdicts.unattributed > 0 {
            out.push_str(&format!(
                "  (unattributed: {} — v1 Verus-corpus clauses recorded by status/level)\n",
                self.verdicts.unattributed
            ));
        }

        // (3) the TV phase split.
        match &self.tv {
            Some(tv) => {
                out.push_str(&format!(
                    "tv phase split (syntactic / semantic / timeout) — {} clause(s):\n",
                    tv.total()
                ));
                out.push_str(&format!("  faithful : {}\n", tv.faithful));
                out.push_str(&format!(
                    "  syntactic: {} (outside the framed sublanguage)\n",
                    tv.syntactic
                ));
                out.push_str(&format!(
                    "  semantic : {} (lowering infidelity)\n",
                    tv.semantic
                ));
                out.push_str(&format!(
                    "  timeout  : {} (rlimit / verus-absent)\n",
                    tv.timeout
                ));
            }
            None => {
                out.push_str("tv phase split: not run (no contract-TV clauses for this file)\n");
            }
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{Certificate, Level, ObligationResult};
    use crate::verdict::{CertVerdict, CovenantCounterexample, RealPoint};

    /// REQ-7: the routing-reason projection is a total map over the closed engine-tag
    /// set — `nlsat` ⇒ relaxable, every `lean*` tag ⇒ lemma, `verus`/absent ⇒ in-cage,
    /// and the cage-vs-forge `share()` splits relaxable+lemma to the forge.
    #[test]
    fn routing_reason_from_engine_is_total() {
        assert_eq!(
            RoutingReason::from_engine(Some("nlsat")),
            RoutingReason::Relaxable
        );
        assert_eq!(
            RoutingReason::from_engine(Some("lean-auto")),
            RoutingReason::Lemma
        );
        assert_eq!(
            RoutingReason::from_engine(Some("lean-interactive")),
            RoutingReason::Lemma
        );
        assert_eq!(
            RoutingReason::from_engine(Some("verus")),
            RoutingReason::InCage
        );
        // The v1 Verus-corpus clause carries no per-clause attribution → the cage.
        assert_eq!(RoutingReason::from_engine(None), RoutingReason::InCage);

        assert_eq!(RoutingReason::InCage.share(), Share::Cage);
        assert_eq!(RoutingReason::Relaxable.share(), Share::Forge);
        assert_eq!(RoutingReason::Lemma.share(), Share::Forge);
    }

    /// REQ-7: the verdict-index map covers exactly the seven kinds in declaration
    /// order and rejects an unknown kind (so an off-vocabulary string is counted
    /// unattributed, never silently bucketed).
    #[test]
    fn verdict_index_covers_the_seven_kinds() {
        for (i, kind) in VERDICT_KINDS.iter().enumerate() {
            assert_eq!(verdict_index(kind), Some(i));
        }
        assert_eq!(verdict_index("NotAVerdict"), None);
        assert_eq!(VERDICT_KINDS.len(), 7);
    }

    /// A clause discharged by an engine + verdict (the schema-v2 forge-tier block).
    fn clause(engine: &str, verdict: CertVerdict) -> ObligationResult {
        ObligationResult::discharged("c").with_clause_attribution(engine, vec![], verdict)
    }

    /// REQ-7 / AC-12: the dashboard aggregates the per-clause routing + the seven
    /// verdicts across a forge-tier cert collection. Two nlsat (relaxable) `Proved`
    /// clauses + one lean (lemma) `Proved` clause ⇒ forge share 3 (relaxable 2,
    /// lemma 0... ), cage 0, and 3 `Proved` verdicts.
    #[test]
    fn dashboard_aggregates_routing_and_verdicts() {
        let cert = Certificate::new(
            "isqrt_class",
            Level::L3,
            vec!["pure".to_string()],
            0,
            vec![
                clause("nlsat", CertVerdict::Proved),
                clause("nlsat", CertVerdict::Proved),
                clause("lean-auto", CertVerdict::Proved),
            ],
        );
        let d = MetricsDashboard::from_certificates(&[cert], None);
        assert_eq!(d.total_clauses, 3);
        // Routing: 2 relaxable (nlsat) + 1 lemma (lean) → forge 3, cage 0.
        assert_eq!(d.routing.cage(), 0);
        assert_eq!(d.routing.forge(), 3);
        assert_eq!(d.routing.relaxable, 2);
        assert_eq!(d.routing.lemma, 1);
        // Verdicts: 3 Proved, no other kind, none unattributed.
        assert_eq!(d.verdicts.get("Proved"), 3);
        assert_eq!(d.verdicts.get("Counterexample"), 0);
        assert_eq!(d.verdicts.unattributed, 0);
        // The TV split is absent (not run) → "not run" in the render.
        assert!(d.tv.is_none());
        assert!(d.render().contains("tv phase split: not run"));
    }

    /// REQ-7: a v1 Verus-corpus clause (no per-clause attribution) projects to the
    /// cage share and the unattributed verdict bucket — never dropped, never a false
    /// seven-verdict.
    #[test]
    fn v1_corpus_clause_is_cage_and_unattributed() {
        let cert = Certificate::new(
            "sum",
            Level::L3,
            vec!["pure".to_string()],
            0,
            vec![ObligationResult::discharged("sum")],
        );
        let d = MetricsDashboard::from_certificates(&[cert], None);
        assert_eq!(d.routing.cage(), 1);
        assert_eq!(d.routing.forge(), 0);
        assert_eq!(d.verdicts.unattributed, 1);
        assert_eq!(d.verdicts.by_kind.iter().sum::<usize>(), 0);
    }

    /// REQ-7 / AC-12: every one of the seven verdicts is tallied into its own column
    /// (the closed vocabulary maps onto the seven dashboard buckets, none collide).
    #[test]
    fn all_seven_verdicts_are_tallied_distinctly() {
        let all = vec![
            CertVerdict::Proved,
            CertVerdict::Counterexample {
                obligations: vec![],
            },
            CertVerdict::RealWitness {
                point: RealPoint { assignment: vec![] },
            },
            CertVerdict::CovenantRefuted {
                counterexample: CovenantCounterexample {
                    input: "n=1".to_string(),
                    seed: 1,
                },
            },
            CertVerdict::Stuck {
                goals: vec![],
                hint: None,
            },
            CertVerdict::KernelBudget {
                detail: "x".to_string(),
            },
            CertVerdict::Timeout {
                detail: "x".to_string(),
            },
        ];
        let obls: Vec<ObligationResult> = all.into_iter().map(|v| clause("lean-auto", v)).collect();
        let cert = Certificate::new("f", Level::L3, vec!["pure".to_string()], 0, obls);
        let d = MetricsDashboard::from_certificates(&[cert], None);
        for kind in VERDICT_KINDS {
            assert_eq!(d.verdicts.get(kind), 1, "verdict {kind} tallied once");
        }
        assert_eq!(d.verdicts.unattributed, 0);
        assert_eq!(d.total_clauses, 7);
    }

    /// REQ-7: the TV phase split maps the contract-TV four-way onto the §6 taxonomy
    /// (faithful / syntactic=Skipped / semantic=Divergent / timeout=Unverifiable).
    #[test]
    fn tv_phase_split_maps_the_four_way() {
        let counts = TvCounts {
            faithful: 5,
            divergent: 2,
            skipped: 3,
            unverifiable: 1,
        };
        let split = TvPhaseSplit::from_tv_counts(&counts);
        assert_eq!(split.faithful, 5);
        assert_eq!(split.semantic, 2);
        assert_eq!(split.syntactic, 3);
        assert_eq!(split.timeout, 1);
        assert_eq!(split.total(), 11);

        let d = MetricsDashboard::from_certificates(&[], Some(&counts));
        let rendered = d.render();
        assert!(rendered.contains("tv phase split"));
        assert!(rendered.contains("semantic : 2"));
        assert!(rendered.contains("syntactic: 3"));
        assert!(rendered.contains("timeout  : 1"));
    }

    /// REQ-7 (determinism, R-CODE-5): the same inputs render byte-identically.
    #[test]
    fn render_is_deterministic() {
        let cert = Certificate::new(
            "f",
            Level::L3,
            vec!["pure".to_string()],
            0,
            vec![clause("nlsat", CertVerdict::Proved)],
        );
        let counts = TvCounts {
            faithful: 1,
            divergent: 0,
            skipped: 0,
            unverifiable: 0,
        };
        let a = MetricsDashboard::from_certificates(std::slice::from_ref(&cert), Some(&counts))
            .render();
        let b = MetricsDashboard::from_certificates(std::slice::from_ref(&cert), Some(&counts))
            .render();
        assert_eq!(a, b);
    }
}
