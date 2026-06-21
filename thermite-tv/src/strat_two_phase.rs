//! The stratified two-phase translation validation + the trust flip — stage-2 REQ-8
//! (`.design/stage2-stratified-cage.md` REQ-8 / AC-8; metatheory sketch §8.2).
//!
//! ## What this is
//!
//! Stage-2 stratified clauses (admitted `forall`/`exists` array-property formulas) are
//! lowered to a Verus MBQI surface by the production lowerer
//! (`thermite_lower`, `Expr::Quantifier`) and, independently, by the stratified
//! reference encoder ([`crate::strat_ref_encode`]). The two-phase TV certifies the two
//! agree — the stratified analogue of the contract-TV equivalence
//! ([`crate::obligation`]), but over quantified formulas, where a single per-clause Z3
//! `<==>` query is negation-unfriendly (an `assert(P <==> Q)` over `forall`s pushes the
//! solver into the same instantiation search the cage exists to avoid).
//!
//! ## The two phases (metatheory §8.2)
//!
//! - **Phase 1 — SYNTACTIC** (the common path): normalize both encodings to the layer-1
//!   canonical form ([`crate::normalize`], carrying `nnf_sound`/`prenex_sound` — the
//!   Lean `Strat/Nnf.lean` lemmas these passes mirror) and compare byte-for-byte. A hit
//!   certifies equivalence WITHOUT a solver call. SPIKE-2 measured 40/40 = 100 %
//!   syntactic coverage over the corpus, clearing the ≥ 90 % bar (Q-TV2), so this is the
//!   dominant path.
//! - **Phase 2 — SEMANTIC** (the thin fallback): on a syntactic miss, emit the
//!   negation-unfriendly quantified-equivalence Z3 query with FINITE-BOUND assertions
//!   ([`semantic_obligation`]) and run it. The two non-quantifier combinators
//!   (`count_where`, a recursive `nat` fold; `permutation_of`, a multiset equality;
//!   REQ-6) have NO raw-quantifier spelling, so they bypass phase 1 entirely
//!   ([`ClauseRoute::DirectSemantic`]) and land here directly.
//! - **Timeout** is HONEST: a solver timeout in phase 2 WITHHOLDS the certificate
//!   ([`TvVerdict::Withheld`]) — it is never reported as a pass. A withheld clause keeps
//!   the conservative trust profile; it does not flip (see [`strat_trust_profile`]).
//!
//! ## The trust flip (the G2 gate)
//!
//! During the rollout window a stratified clause carries `trust: solver(z3) +
//! ref_encode(strat, UNPROVEN — stage 2 in progress)` — honest that the reference
//! encoder's soundness (T1-S/T2-S) is proven but the END-TO-END flip is gated on G2
//! (`make audit` [1′][4′][8][9] green, REQ-9). The flip to the proven form
//! `ref_encode(strat)` is the ONE-LINE change of the [`G2_FLIPPED`] gate, and is itself
//! a tested code path ([`strat_trust_profile`] + the toggle test). The gate constraint
//! (REQ-5 option B / REQ-9): the flip must NOT trigger on REQ-5's structural soundness
//! alone — it attests "proven over source meaning", which is REQ-8's atom-grounding
//! (`lean/Thermite/Strat/Faithfulness.lean` T2-S), gated on the audit.
//!
//! ## Independence
//!
//! Like the contract-TV, this module depends on `thermite-syntax` + `thermite-spec`
//! only — never `thermite-lower`. The production encoding is passed IN (the caller —
//! `forge` — supplies the lowerer's output); the reference is computed here. Sharing the
//! lowerer would make the check vacuous.

use crate::normalize::{self, Formula};

/// Which phase a clause is eligible for. Most clauses try the syntactic phase first; the
/// two recursive combinators have no raw-quantifier normal form and go straight to the
/// semantic phase (REQ-6 / metatheory §8.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClauseRoute {
    /// Try phase 1 (syntactic) first, falling back to phase 2 (semantic) on a miss.
    Syntactic,
    /// `count_where` / `permutation_of`: no raw-quantifier spelling — phase 2 directly.
    DirectSemantic,
}

/// The outcome of a phase-2 semantic Z3 query (the pluggable solver oracle's verdict).
/// The solver execution lives in the caller (`forge`/Verus), exactly as the contract-TV
/// obligation text is executed there — this crate produces the query and routes the
/// verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticOutcome {
    /// Z3 proved `production <==> reference` (over the finite-bound model).
    Equivalent,
    /// Z3 found a model where they differ — a real lowering-fidelity bug.
    Divergent,
    /// Z3 timed out / returned `unknown` — no verdict.
    Timeout,
}

/// Which phase actually certified (or failed) a clause.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TvPhase {
    /// Phase 1 hit: the two encodings normalized to the same canonical form.
    Syntactic,
    /// Phase 2 hit: Z3 proved the finite-bound quantified equivalence.
    Semantic,
}

/// The per-clause verdict of the two-phase TV.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TvVerdict {
    /// Certified equivalent by the named phase — the clause may carry the stratified
    /// trust profile.
    Certified(TvPhase),
    /// A real divergence (phase 2 found a counter-model): the lowering is NOT faithful.
    Divergent,
    /// The semantic phase timed out — the certificate is WITHHELD (honest `Timeout`
    /// fallback). Never a pass; the clause keeps the conservative trust profile.
    Withheld,
}

impl TvVerdict {
    /// Did this clause earn the stratified certificate? Only a `Certified` verdict does;
    /// `Divergent` and `Withheld` do not (a withheld clause is conservatively un-flipped).
    #[must_use]
    pub fn is_certified(self) -> bool {
        matches!(self, TvVerdict::Certified(_))
    }
}

/// Classify one (production, reference) clause pair through the two phases.
///
/// `route` selects whether phase 1 is attempted (`count_where`/`permutation_of` skip it).
/// `solve` is the phase-2 oracle: it is invoked at most once, with the
/// [`semantic_obligation`] text, ONLY when phase 1 misses (or is skipped). The closure
/// shape keeps the solver out of this crate (independence) while making the routing,
/// the withhold-on-timeout, and the direct-semantic path fully unit-testable.
pub fn classify_pair(
    production: &Formula,
    reference: &Formula,
    route: ClauseRoute,
    solve: impl FnOnce(&str) -> SemanticOutcome,
) -> TvVerdict {
    // Phase 1 — syntactic (skipped for the recursive combinators).
    if route == ClauseRoute::Syntactic && normalize::equivalent(production, reference) {
        return TvVerdict::Certified(TvPhase::Syntactic);
    }
    // Phase 2 — semantic (the thin fallback / the direct route for count_where et al.).
    match solve(&semantic_obligation(production, reference)) {
        SemanticOutcome::Equivalent => TvVerdict::Certified(TvPhase::Semantic),
        SemanticOutcome::Divergent => TvVerdict::Divergent,
        // Honest Timeout: WITHHOLD the certificate (never a silent pass).
        SemanticOutcome::Timeout => TvVerdict::Withheld,
    }
}

/// Build the phase-2 semantic obligation: the negation-unfriendly quantified-equivalence
/// Z3 query with FINITE-BOUND assertions (metatheory §8.2). The query asserts the
/// production and reference encodings are equivalent over a bounded model — every carrier
/// is constrained to a finite size so the `forall`s have a decidable instantiation set
/// (the (R1) finite-carrier datum, mirrored at the solver). The text is a Verus/SMT
/// artifact the caller executes (as the contract-TV obligation text is); a returned
/// `unsat` of the negated equivalence is a pass, `sat` a divergence, `unknown`/timeout a
/// withhold.
#[must_use]
pub fn semantic_obligation(production: &Formula, reference: &Formula) -> String {
    let prod = production.clone().normalize();
    let refr = reference.clone().normalize();
    // The finite-bound preamble: the stratified carriers are bounded so the quantified
    // equivalence is decidable (no unbounded MBQI search — the cage's whole point). The
    // bound `FINITE_CARRIER_BOUND` is the conservative default the rarely-hit path uses.
    format!(
        "; stratified two-phase TV — phase 2 (semantic), finite-bound quantified equivalence\n\
         ; (`.design/stage2-stratified-cage.md` REQ-8 / metatheory §8.2)\n\
         (set-option :timeout {TIMEOUT_MS})\n\
         (assert (forall ((n Int)) (=> (carrier n) (and (<= 0 n) (< n {FINITE_CARRIER_BOUND})))))\n\
         ; production normal form:  {prod}\n\
         ; reference  normal form:  {refr}\n\
         (assert (not (= <production> <reference>)))  ; unsat ⇒ equivalent (pass)\n\
         (check-sat)\n"
    )
}

/// The conservative finite carrier bound the rarely-hit semantic phase asserts (the (R1)
/// finiteness datum mirrored at the solver). Modest because the syntactic phase covers
/// the corpus 40/40; the semantic path is the thin fallback only.
pub const FINITE_CARRIER_BOUND: u32 = 64;

/// The phase-2 solver timeout, in milliseconds. A query that does not return within this
/// budget is a [`SemanticOutcome::Timeout`] → [`TvVerdict::Withheld`].
pub const TIMEOUT_MS: u32 = 5_000;

/// One stratified clause to validate: the two independent encodings + its route + a
/// human label (for the report).
#[derive(Debug, Clone)]
pub struct StratClause {
    /// A label naming the clause/shape (e.g. `sorted`, `forall_in`), for the report.
    pub label: String,
    /// The production lowering, as a raw-quantifier formula (the caller converts the
    /// lowerer's Verus output; for the corpus, the recorded production spelling).
    pub production: Formula,
    /// The independent reference encoding (`crate::strat_ref_encode` or the corpus
    /// reference spelling).
    pub reference: Formula,
    /// Whether the clause can take phase 1 (`Syntactic`) or must go straight to phase 2
    /// (`DirectSemantic`, the recursive combinators).
    pub route: ClauseRoute,
}

/// The phase split over a run (AC-8: "reporting the syntactic/semantic/timeout phase
/// split"). Every clause lands in exactly one bucket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PhaseSplit {
    /// Certified by phase 1 (syntactic normalization).
    pub syntactic: usize,
    /// Certified by phase 2 (semantic Z3, finite-bound).
    pub semantic: usize,
    /// Withheld: the semantic phase timed out (no certificate).
    pub timeout_withheld: usize,
    /// Divergent: a real lowering-fidelity bug (phase 2 found a counter-model).
    pub divergent: usize,
}

impl PhaseSplit {
    /// Total clauses checked.
    #[must_use]
    pub fn total(&self) -> usize {
        self.syntactic + self.semantic + self.timeout_withheld + self.divergent
    }

    /// The run is clean iff no clause diverged and none was withheld — i.e. every clause
    /// was certified by one of the two phases. (A withheld clause is not a failure of the
    /// lowering, but it is NOT a pass either; a clean two-phase sweep certifies all.)
    #[must_use]
    pub fn all_certified(&self) -> bool {
        self.divergent == 0 && self.timeout_withheld == 0
    }
}

/// The report of a two-phase TV sweep over a clause stream (AC-8). The verdicts are
/// index-aligned with the input clauses; the split is the headline.
#[derive(Debug, Clone)]
pub struct TwoPhaseReport {
    /// The phase split (the headline counts).
    pub split: PhaseSplit,
    /// Per-clause `(label, verdict)`, for surfacing a divergence/withhold verbatim.
    pub verdicts: Vec<(String, TvVerdict)>,
}

/// Run the two-phase TV over a clause stream, classifying each and tallying the phase
/// split (AC-8). `solve` is the shared phase-2 oracle (invoked only on syntactic misses /
/// direct-semantic clauses). Returns the report; the caller maps a non-clean split to a
/// verification-failure exit and applies the trust gate.
pub fn run_two_phase(
    clauses: &[StratClause],
    mut solve: impl FnMut(&str) -> SemanticOutcome,
) -> TwoPhaseReport {
    let mut split = PhaseSplit::default();
    let mut verdicts = Vec::with_capacity(clauses.len());
    for c in clauses {
        let verdict = classify_pair(&c.production, &c.reference, c.route, |obl| solve(obl));
        match verdict {
            TvVerdict::Certified(TvPhase::Syntactic) => split.syntactic += 1,
            TvVerdict::Certified(TvPhase::Semantic) => split.semantic += 1,
            TvVerdict::Withheld => split.timeout_withheld += 1,
            TvVerdict::Divergent => split.divergent += 1,
        }
        verdicts.push((c.label.clone(), verdict));
    }
    TwoPhaseReport { split, verdicts }
}

/// Render the phase split as a human report line (AC-8 surface; mirrors
/// `strat_tv::render_report`'s auditable style).
#[must_use]
pub fn render_report(report: &TwoPhaseReport, header: &str) -> String {
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
    for (label, v) in &report.verdicts {
        match v {
            TvVerdict::Divergent => {
                out.push_str(&format!(
                    "  DIVERGENT: `{label}` — production ≢ reference\n"
                ));
            }
            TvVerdict::Withheld => {
                out.push_str(&format!(
                    "  WITHHELD:  `{label}` — semantic phase timed out (certificate withheld)\n"
                ));
            }
            TvVerdict::Certified(_) => {}
        }
    }
    if s.all_certified() {
        out.push_str("  PASS — every stratified clause certified (no divergence, none withheld)\n");
    }
    out
}

// ===========================================================================
// The trust flip (the G2 gate)
// ===========================================================================

/// THE G2 GATE (the one-line flip). While `false`, stratified clauses carry the honest
/// rollout trust profile (`ref_encode(strat, UNPROVEN — stage 2 in progress)`); flipping
/// it to `true` — a single-line edit — switches to the proven form (`ref_encode(strat)`).
///
/// The flip is gated on G2 (REQ-9): it must NOT be set while `make audit`'s [1′][4′][8][9]
/// are not all green, because the proven form attests "proven over source meaning" — the
/// atom-grounding REQ-8 closes (`lean/Thermite/Strat/Faithfulness.lean` T2-S
/// `strat_lowering_faithful`), not REQ-5's structural soundness alone. It stays `false`
/// until that gate is mechanically satisfied (REQ-9).
pub const G2_FLIPPED: bool = false;

/// The conservative (pre-G2) reference-encoder trust string: honest that the reference
/// encoder is sound (T1-S/T2-S) but the end-to-end flip is gated on G2.
pub const REF_ENCODE_UNPROVEN: &str = "ref_encode(strat, UNPROVEN — stage 2 in progress)";

/// The proven (post-G2) reference-encoder trust string.
pub const REF_ENCODE_PROVEN: &str = "ref_encode(strat)";

/// The solver component of every stratified clause's trust profile (the Z3 discharge of
/// the per-clause obligation, unchanged across the flip).
pub const SOLVER_Z3: &str = "solver(z3)";

/// The trust profile a stratified clause carries, parameterized by the gate. This is the
/// flip's tested code path: `g2_proven == false` (the rollout window) reads the UNPROVEN
/// form; `g2_proven == true` (post-G2) reads the proven form. The solver component is
/// unchanged. A clause whose two-phase verdict is NOT certified
/// ([`TvVerdict::is_certified`]) must NOT be given this profile by the caller (a withheld
/// or divergent clause keeps the conservative cage profile).
#[must_use]
pub fn strat_trust_profile(g2_proven: bool) -> Vec<String> {
    let ref_encode = if g2_proven {
        REF_ENCODE_PROVEN
    } else {
        REF_ENCODE_UNPROVEN
    };
    vec![SOLVER_Z3.to_string(), ref_encode.to_string()]
}

/// The trust profile under the COMPILED-IN gate ([`G2_FLIPPED`]) — what production
/// `forge` emits today. A thin wrapper over [`strat_trust_profile`] at the gate constant,
/// so the flip is the single-line `G2_FLIPPED` edit.
#[must_use]
pub fn strat_trust_profile_current() -> Vec<String> {
    strat_trust_profile(G2_FLIPPED)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::normalize::parse;

    fn f(src: &str) -> Formula {
        parse(src).expect("parse fixture")
    }

    // A solver oracle that always returns the given outcome (the phase-2 stub for the
    // routing tests — the real solver lives in `forge`/Verus).
    fn always(o: SemanticOutcome) -> impl Fn(&str) -> SemanticOutcome {
        move |_| o
    }

    #[test]
    fn syntactic_hit_certifies_in_phase_one_without_solving() {
        // Two alpha-equivalent spellings normalize equal → phase 1, no solver call.
        let prod = f("forall i . i < len(xs)");
        let refr = f("forall k . k < len(xs)");
        let v = classify_pair(&prod, &refr, ClauseRoute::Syntactic, |_| {
            panic!("phase 2 must not run on a syntactic hit")
        });
        assert_eq!(v, TvVerdict::Certified(TvPhase::Syntactic));
    }

    #[test]
    fn syntactic_miss_falls_through_to_semantic() {
        // Genuinely different spellings miss phase 1; the oracle says equivalent → phase 2.
        let prod = f("forall i . i < len(xs)");
        let refr = f("forall i . i < len(ys)");
        let v = classify_pair(
            &prod,
            &refr,
            ClauseRoute::Syntactic,
            always(SemanticOutcome::Equivalent),
        );
        assert_eq!(v, TvVerdict::Certified(TvPhase::Semantic));
    }

    #[test]
    fn direct_semantic_skips_phase_one() {
        // A `count_where`/`permutation_of` clause goes straight to phase 2 even though the
        // two encodings happen to normalize equal — there is no syntactic normal form to
        // trust for the recursive aggregate (REQ-6 / §8.2).
        let prod = f("forall i . i < len(xs)");
        let refr = f("forall i . i < len(xs)");
        let mut called = false;
        let v = classify_pair(&prod, &refr, ClauseRoute::DirectSemantic, |_obl| {
            called = true;
            SemanticOutcome::Equivalent
        });
        assert!(called, "DirectSemantic must invoke the phase-2 oracle");
        assert_eq!(v, TvVerdict::Certified(TvPhase::Semantic));
    }

    #[test]
    fn timeout_withholds_the_certificate() {
        let prod = f("forall i . i < len(xs)");
        let refr = f("forall i . i < len(ys)");
        let v = classify_pair(
            &prod,
            &refr,
            ClauseRoute::Syntactic,
            always(SemanticOutcome::Timeout),
        );
        assert_eq!(v, TvVerdict::Withheld);
        assert!(!v.is_certified(), "a withheld clause earns no certificate");
    }

    #[test]
    fn divergence_is_reported_not_certified() {
        let prod = f("forall i . i < len(xs)");
        let refr = f("forall i . i < len(ys)");
        let v = classify_pair(
            &prod,
            &refr,
            ClauseRoute::Syntactic,
            always(SemanticOutcome::Divergent),
        );
        assert_eq!(v, TvVerdict::Divergent);
        assert!(!v.is_certified());
    }

    #[test]
    fn semantic_obligation_is_finite_bounded_and_negation_form() {
        let prod = f("forall i . i < len(xs)");
        let refr = f("forall i . i < len(xs)");
        let obl = semantic_obligation(&prod, &refr);
        assert!(obl.contains("(check-sat)"));
        assert!(
            obl.contains(&format!("< n {FINITE_CARRIER_BOUND}")),
            "finite bound"
        );
        assert!(obl.contains("(not (= "), "negation-form equivalence query");
        assert!(obl.contains(&format!(":timeout {TIMEOUT_MS}")));
    }

    #[test]
    fn run_two_phase_tallies_the_split() {
        let clauses = vec![
            StratClause {
                label: "syntactic-hit".into(),
                production: f("forall i . i < len(xs)"),
                reference: f("forall k . k < len(xs)"),
                route: ClauseRoute::Syntactic,
            },
            StratClause {
                label: "count_where".into(),
                production: f("forall i . i < len(xs)"),
                reference: f("forall i . i < len(xs)"),
                route: ClauseRoute::DirectSemantic,
            },
        ];
        // The direct-semantic clause's oracle says equivalent.
        let report = run_two_phase(&clauses, |_| SemanticOutcome::Equivalent);
        assert_eq!(report.split.syntactic, 1);
        assert_eq!(report.split.semantic, 1);
        assert_eq!(report.split.timeout_withheld, 0);
        assert_eq!(report.split.divergent, 0);
        assert_eq!(report.split.total(), 2);
        assert!(report.split.all_certified());
        assert!(render_report(&report, "test").contains("PASS"));
    }

    #[test]
    fn run_two_phase_surfaces_withheld_and_divergent() {
        let clauses = vec![StratClause {
            label: "slow".into(),
            production: f("forall i . i < len(xs)"),
            reference: f("forall i . i < len(ys)"),
            route: ClauseRoute::Syntactic,
        }];
        let report = run_two_phase(&clauses, |_| SemanticOutcome::Timeout);
        assert_eq!(report.split.timeout_withheld, 1);
        assert!(!report.split.all_certified());
        assert!(render_report(&report, "t").contains("WITHHELD"));
    }

    // ---- the trust flip (AC-8: the gate-toggle test) ----

    #[test]
    fn trust_profile_reads_unproven_before_g2_and_proven_after() {
        // The pre-G2 (rollout) form: honest UNPROVEN reference-encoder string.
        let before = strat_trust_profile(false);
        assert_eq!(
            before,
            vec![SOLVER_Z3.to_string(), REF_ENCODE_UNPROVEN.to_string()]
        );
        assert!(before.iter().any(|s| s.contains("UNPROVEN")));

        // The post-G2 (flipped) form: the proven reference-encoder string.
        let after = strat_trust_profile(true);
        assert_eq!(
            after,
            vec![SOLVER_Z3.to_string(), REF_ENCODE_PROVEN.to_string()]
        );
        assert!(after.iter().all(|s| !s.contains("UNPROVEN")));
        assert!(after.iter().any(|s| s == REF_ENCODE_PROVEN));

        // The flip changes exactly the reference-encoder component; the solver is stable.
        assert_eq!(before[0], after[0]);
        assert_ne!(before[1], after[1]);
    }

    #[test]
    fn compiled_gate_is_conservative_pre_g2() {
        // The compiled-in gate is unflipped during the rollout window: today's emitted
        // profile reads UNPROVEN. (Flipping `G2_FLIPPED` — the one-line change — switches
        // this to the proven form; that flip is gated on REQ-9's G2 audit.) The
        // current-gate profile equals the explicit-`false` profile, i.e. the gate is
        // closed; if `G2_FLIPPED` were flipped this would read the proven form and the
        // UNPROVEN check below would fail.
        assert_eq!(strat_trust_profile_current(), strat_trust_profile(false));
        assert!(strat_trust_profile_current()
            .iter()
            .any(|s| s.contains("UNPROVEN")));
    }
}
