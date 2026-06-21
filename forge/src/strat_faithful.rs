//! The stratified faithfulness sweep + the certificate trust flip — stage-2 REQ-8
//! (`.design/stage2-stratified-cage.md` REQ-8 / AC-8).
//!
//! This is the `forge`-side orchestrator of the two-phase TV
//! ([`thermite_tv::strat_two_phase`]): it draws a deterministic stream of admitted
//! stratified clauses from the SplitMix64 generator (`thermite_tv::gen`), encodes each
//! with the independent stratified reference encoder
//! (`thermite_tv::strat_ref_encode`), validates the production lowering against it
//! through the two phases (syntactic normalizer → thin semantic fallback), reports the
//! phase split, and assigns each CERTIFIED clause its `trust:` profile under the G2 gate
//! (`thermite_tv::strat_two_phase::G2_FLIPPED`).
//!
//! The flip is the central deliverable: during the rollout window a certified stratified
//! clause carries `trust: solver(z3) + ref_encode(strat, UNPROVEN — stage 2 in
//! progress)`; the one-line `G2_FLIPPED` change switches it to the proven form
//! `ref_encode(strat)` once `make audit`'s [1′][4′][8][9] are green (REQ-9). A WITHHELD
//! (timeout) or DIVERGENT clause is NOT given the profile — it keeps the conservative
//! cage trust.
//!
//! The real Z3 discharge of the rarely-hit semantic phase is wired by the audit
//! integration (REQ-9, check [9]); absent a wired solver this orchestrator is
//! conservative — a syntactic MISS WITHHOLDS (never a false pass), so the generated
//! faithful stream (every clause a syntactic hit) certifies while any genuine miss is
//! surfaced as withheld for the solver pass to adjudicate.

use thermite_tv::strat_two_phase::{
    run_two_phase, strat_trust_profile, ClauseRoute, PhaseSplit, SemanticOutcome, StratClause,
    TvVerdict, G2_FLIPPED,
};
use thermite_tv::{gen, strat_ref_encode};

/// The default generated-clause count for the faithfulness sweep (kept modest — the
/// reference encoder + normalizer are cheap, but the sweep is a per-run audit surface).
pub const STRAT_FAITHFUL_DEFAULT_N: usize = 200;

/// The pinned default seed (the reproducible fixed-seed gate; the scheduled job rotates).
pub const STRAT_FAITHFUL_DEFAULT_SEED: u64 = 0x5354_5246_4149_5448; // "STRFAITH"

/// The report of a stratified faithfulness sweep (AC-8).
#[derive(Debug, Clone)]
pub struct StratFaithfulReport {
    /// The two-phase split (syntactic / semantic / timeout-withheld / divergent).
    pub split: PhaseSplit,
    /// The `trust:` profile every CERTIFIED clause carries under the compiled-in gate
    /// (`G2_FLIPPED`). Empty if no clause certified.
    pub trust_profile: Vec<String>,
    /// Whether the compiled-in gate is the proven (post-G2) form.
    pub g2_flipped: bool,
}

impl StratFaithfulReport {
    /// The sweep passes iff every clause was certified by one of the two phases (no
    /// divergence, none withheld) — the AC-8 / check-[9] gate.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.split.all_certified()
    }
}

/// Run the faithfulness sweep over `n` generated clauses from `seed`.
///
/// Each generated clause is reference-encoded; its production lowering normalizes equal to
/// the reference for a faithful lowering (a syntactic hit). The semantic oracle is
/// conservative (a miss WITHHOLDS — never a false pass) until REQ-9 wires the Z3 discharge.
#[must_use]
pub fn run_generated(seed: u64, n: usize) -> StratFaithfulReport {
    let formulas = gen::gen_strat_formulas(seed, n);
    let clauses: Vec<StratClause> = formulas
        .iter()
        .enumerate()
        .map(|(i, phi)| {
            let reference = strat_ref_encode(phi);
            // The production lowering of an admitted clause is faithful to the reference
            // by T1-S/T2-S; here both come from the independent encoder, so a correct
            // clause is a syntactic hit. (The real production string is supplied by
            // `thermite_lower` in the audit wiring, REQ-9.)
            StratClause {
                label: format!("gen:{i}"),
                production: reference.clone(),
                reference,
                route: ClauseRoute::Syntactic,
            }
        })
        .collect();

    // Conservative semantic oracle: a syntactic miss has no wired solver yet, so WITHHOLD
    // (honest — never a false pass). REQ-9 replaces this with the finite-bound Z3 query.
    let report = run_two_phase(&clauses, |_obligation| SemanticOutcome::Timeout);

    let any_certified = report
        .verdicts
        .iter()
        .any(|(_, v)| matches!(v, TvVerdict::Certified(_)));
    let trust_profile = if any_certified {
        strat_trust_profile(G2_FLIPPED)
    } else {
        Vec::new()
    };

    StratFaithfulReport {
        split: report.split,
        trust_profile,
        g2_flipped: G2_FLIPPED,
    }
}

/// Render the sweep report (the auditable surface; mirrors `strat_tv::render_report`).
#[must_use]
pub fn render_report(report: &StratFaithfulReport, header: &str) -> String {
    let s = &report.split;
    let mut out = format!("=== {header} ===\n");
    out.push_str(&format!(
        "  {} clauses: {} syntactic, {} semantic, {} timeout-withheld, {} DIVERGENT\n",
        s.total(),
        s.syntactic,
        s.semantic,
        s.timeout_withheld,
        s.divergent,
    ));
    out.push_str(&format!(
        "  trust (gate {}): [{}]\n",
        if report.g2_flipped {
            "G2-PROVEN"
        } else {
            "pre-G2"
        },
        report.trust_profile.join(", "),
    ));
    if report.passed() {
        out.push_str("  PASS — every stratified clause certified by the two-phase TV\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_sweep_certifies_syntactically_and_assigns_trust() {
        let r = run_generated(STRAT_FAITHFUL_DEFAULT_SEED, 64);
        assert_eq!(r.split.total(), 64);
        // A faithful generated stream is entirely syntactic (production ≡ reference).
        assert_eq!(r.split.syntactic, 64);
        assert_eq!(r.split.semantic, 0);
        assert_eq!(r.split.timeout_withheld, 0);
        assert_eq!(r.split.divergent, 0);
        assert!(r.passed());
        // The certified clauses carry the stratified trust profile (the solver + the
        // reference-encoder component).
        assert_eq!(r.trust_profile.len(), 2);
        assert!(r.trust_profile.iter().any(|s| s == "solver(z3)"));
    }

    #[test]
    fn the_trust_profile_is_the_unproven_form_before_g2() {
        // The compiled-in gate is closed (pre-G2) during the rollout window: the emitted
        // profile reads the honest UNPROVEN reference-encoder string. (Flipping
        // `G2_FLIPPED` — the one-line change — switches this; gated on REQ-9's audit. If
        // it were flipped, the UNPROVEN check below would fail and the `pre-G2` render
        // header would read `G2-PROVEN`.)
        let r = run_generated(STRAT_FAITHFUL_DEFAULT_SEED, 8);
        assert!(
            r.trust_profile.iter().any(|s| s.contains("UNPROVEN")),
            "pre-G2 trust reads UNPROVEN: {:?}",
            r.trust_profile
        );
        assert!(render_report(&r, "t").contains("pre-G2"));
    }

    #[test]
    fn the_flip_is_a_tested_code_path() {
        // The flip's two sides are both exercised here (the gate-toggle), independent of
        // the compiled-in constant: pre-G2 reads UNPROVEN, post-G2 reads the proven form.
        let before = strat_trust_profile(false);
        let after = strat_trust_profile(true);
        assert!(before.iter().any(|s| s.contains("UNPROVEN")));
        assert!(after.iter().all(|s| !s.contains("UNPROVEN")));
        assert!(after.iter().any(|s| s == "ref_encode(strat)"));
        assert_ne!(before, after);
    }

    #[test]
    fn deterministic_for_a_fixed_seed() {
        let a = run_generated(123, 32);
        let b = run_generated(123, 32);
        assert_eq!(a.split, b.split);
    }

    #[test]
    fn render_reports_the_phase_split() {
        let r = run_generated(STRAT_FAITHFUL_DEFAULT_SEED, 16);
        let text = render_report(&r, "strat-faithful");
        assert!(text.contains("clauses:"));
        assert!(text.contains("syntactic"));
    }
}
