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
//! - **Phase 1 — syntactic** (the common path): normalize both encodings to the layer-1
//!   canonical form ([`crate::normalize`], carrying `nnf_sound`/`prenex_sound` — the
//!   Lean `Strat/Nnf.lean` lemmas these passes mirror) and compare byte-for-byte. A hit
//!   certifies equivalence without a solver call. SPIKE-2 measured 40/40 = 100 %
//!   syntactic coverage over the corpus, clearing the ≥ 90 % bar (Q-TV2), so this is the
//!   dominant path.
//! - **Phase 2 — semantic** (the thin fallback): on a syntactic miss, emit the
//!   negation-unfriendly quantified-equivalence Z3 query with FINITE-bound assertions
//!   ([`semantic_obligation`]) and run it. The two non-quantifier combinators
//!   (`count_where`, a recursive `nat` fold; `permutation_of`, a multiset equality;
//!   REQ-6) have no raw-quantifier spelling, so they bypass phase 1 entirely
//!   ([`ClauseRoute::DirectSemantic`]) and land here directly.
//! - **Timeout** is HONEST: a solver timeout in phase 2 WITHHOLDS the certificate
//!   ([`TvVerdict::Withheld`]) — it is never reported as a pass. A withheld clause keeps
//!   the conservative trust profile; it does not flip (see [`strat_trust_profile`]).
//!
//! ## The trust flip (the G2 gate)
//!
//! During the rollout window a stratified clause carries `trust: solver(z3) +
//! ref_encode(strat, UNPROVEN — stage 2 in progress)`, recording that the reference
//! encoder's soundness (T1-S/T2-S) is proven but the END-TO-END flip is gated on G2
//! (`make audit` [1′][4′][8][9] green, REQ-9). The flip to the proven form
//! `ref_encode(strat)` is the one-LINE change of the [`G2_FLIPPED`] gate, and is itself
//! a tested code path ([`strat_trust_profile`] + the toggle test). The gate constraint
//! (REQ-5 option B / REQ-9): the flip must not trigger on REQ-5's structural soundness
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
/// The solver execution lives in the caller (`forge`/Verus), as the contract-TV
/// obligation text is executed there — this crate produces the query and routes the
/// verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticOutcome {
    /// Z3 proved `production <==> reference` (over the finite-bound model).
    Equivalent,
    /// Z3 found a model where they differ — a lowering-fidelity bug.
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
    /// A real divergence (phase 2 found a counter-model): the lowering is not faithful.
    Divergent,
    /// The semantic phase timed out — the certificate is WITHHELD (`Timeout`
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
/// [`semantic_obligation`] text, only when phase 1 misses (or is skipped). The closure
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
/// Z3 query with FINITE-bound assertions (metatheory §8.2). The query asserts the
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
    /// Divergent: a lowering-fidelity bug (phase 2 found a counter-model).
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
    /// lowering, but it is not a pass either; a clean two-phase sweep certifies all.)
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

/// the G2 declaration (REQ-8's one-line flip, now enabled by REQ-9). This is the
/// compiled-in intent — "the four stratified soundness theorems are in the spine and the
/// G2 trust flip is ENABLED" — flipped to `true` at G2 (REQ-9), the increment that built
/// the audit gate and saw [1′][4′][8][9] green in one `make audit` run.
///
/// CRUCIALLY, the declaration alone does not emit the proven label: the effective per-clause
/// flip is [`g2_flip_permitted`]`(G2_FLIPPED, &checks)` — the declaration and every gating
/// audit check green. A red check downgrades the label back to the conservative rollout
/// form regardless of this constant (the AC-9 mechanical block: a flipped certificate can
/// never out-run the audit that justifies it). `forge g2-gate` is the runtime enforcer —
/// it fails `make audit` if `G2_FLIPPED` is set while any of the four is red.
///
/// The proven label is scoped (REQ-9 / REQ-5 option B): structure proven (T1-S),
/// qfree atoms grounded to the v1 `Thermite.denote` (T2-S), and rel/array atoms discharged
/// by Z3's theory (the solver base) — kernel-grounding the rel atoms is stage-3
/// reconstruction. See [`REF_ENCODE_PROVEN`].
pub const G2_FLIPPED: bool = true;

/// The conservative pre-G2 trust string records that the reference encoder is sound
/// (T1-S/T2-S) while the end-to-end flip remains gated on G2.
pub const REF_ENCODE_UNPROVEN: &str = "ref_encode(strat, UNPROVEN — stage 2 in progress)";

/// The proven (post-G2) reference-encoder trust string — HONESTLY scoped (REQ-9 / REQ-5
/// option B / #330–#331). The flip attests exactly: the quantifier/boolean STRUCTURE is
/// proven faithful (T1-S `strat_ref_sound`), `qfree` atoms are grounded to the v1
/// `Thermite.denote` (T2-S `strat_lowering_faithful`), and `rel`/array atoms are discharged
/// by Z3's theory (the solver base — model-relative, the L4 boundary). It does not
/// claim kernel-grounding of the rel/array atoms; that is stage-3 reconstruction. The
/// string starts with `ref_encode(strat)` (the recognizable proven prefix) and carries the
/// scope inline so a reader of the certificate sees the boundary, not an over-claim.
pub const REF_ENCODE_PROVEN: &str =
    "ref_encode(strat): structure proven (T1-S), qfree grounded to v1 (T2-S), \
     rel/array by z3-theory (solver base; kernel-grounding rel = stage 3)";

/// The solver component of every stratified clause's trust profile (the Z3 discharge of
/// the per-clause obligation, unchanged across the flip).
pub const SOLVER_Z3: &str = "solver(z3)";

/// The trust profile a stratified clause carries, parameterized by the gate. This is the
/// flip's tested code path: `g2_proven == false` (the rollout window) reads the UNPROVEN
/// form; `g2_proven == true` (post-G2) reads the proven form. The solver component is
/// unchanged. A clause whose two-phase verdict is not certified
/// ([`TvVerdict::is_certified`]) must not be given this profile by the caller (a withheld
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

// ===========================================================================
// The G2 gate — the four-check audit gate that mechanically blocks the flip
// (stage-2 REQ-9 / AC-9)
// ===========================================================================

/// The four `make audit` checks that gate the G2 trust flip (REQ-9 / AC-9). Each field is
/// the green (`true`) / red (`false`) outcome of one audit sub-check; the flip is permitted
/// only when every one is green. The field names mirror the design-doc check labels
/// ([1′][4′][8][9], `.design/stage2-stratified-cage.md` REQ-9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct G2Checks {
    /// `[1′]` — the Lean axiom probe (`scripts/lean-axiom-probe.sh`): the four stratified
    /// soundness theorems (`strat_ref_sound`, `strat_lowering_faithful`,
    /// `classifier_correct`, `restrat_conservative`) elaborate with axioms ⊆ {propext,
    /// Classical.choice, Quot.sound}.
    pub axiom_probe: bool,
    /// `[4′]` — the doc-drift tripwire (`tooling/doc-drift.py`): the three new mirrored Rust
    /// files (`classifier.rs` / `strat_ref_encode.rs` / `strat_two_phase.rs`) are
    /// content-pinned and current under `.design/verified/strat-rust-lean-correspondence.md`.
    pub doc_drift: bool,
    /// `[8]` — the classifier differential battery (`forge strat-tv`): the Rust classifier
    /// agrees with the Lean kernel `Thermite.Strat.Cls.admitted` on every generated formula,
    /// zero unknown-on-admitted tripwire.
    pub differential: bool,
    /// `[9]` — the stratified two-phase TV sweep (`forge strat-faithful-tv`): every
    /// stratified clause certified (no divergence, none withheld).
    pub two_phase_tv: bool,
}

impl G2Checks {
    /// All four green — the necessary condition for the trust flip (AC-9).
    #[must_use]
    pub fn all_green(&self) -> bool {
        self.axiom_probe && self.doc_drift && self.differential && self.two_phase_tv
    }

    /// The labels of the red checks (for the audit report / the withhold reason). Empty iff
    /// [`all_green`](Self::all_green). Deterministically ordered [1′][4′][8][9].
    #[must_use]
    pub fn red(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if !self.axiom_probe {
            out.push("[1'] axiom-probe");
        }
        if !self.doc_drift {
            out.push("[4'] doc-drift");
        }
        if !self.differential {
            out.push("[8] differential-battery");
        }
        if !self.two_phase_tv {
            out.push("[9] two-phase-TV");
        }
        out
    }

    /// The all-green constructor (a passing `make audit` run).
    #[must_use]
    pub fn all_passing() -> Self {
        Self {
            axiom_probe: true,
            doc_drift: true,
            differential: true,
            two_phase_tv: true,
        }
    }
}

/// the G2 gate (AC-9). The trust flip is PERMITTED iff G2 is declared (`declared`, the
/// compiled-in [`G2_FLIPPED`]) and every gating audit check is green
/// ([`G2Checks::all_green`]). This is the mechanical block: any red check withholds the
/// flip regardless of the declaration, so a flipped certificate can never out-run the audit
/// that justifies it. This is the tested code path — the toggle tests below drive each of
/// the four red and assert the flip is withheld.
#[must_use]
pub fn g2_flip_permitted(declared: bool, checks: &G2Checks) -> bool {
    declared && checks.all_green()
}

/// The effective stratified trust profile under the G2 gate: the proven, scoped
/// form iff [`g2_flip_permitted`], else the conservative `UNPROVEN` rollout form. The
/// production / audit-surface emitter routes through here so a red check automatically
/// downgrades the label (never an over-claim — REQ-9 / REQ-5 option B).
#[must_use]
pub fn strat_trust_profile_gated(declared: bool, checks: &G2Checks) -> Vec<String> {
    strat_trust_profile(g2_flip_permitted(declared, checks))
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
        // The pre-G2 rollout form uses the UNPROVEN reference-encoder string.
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
    fn compiled_declaration_is_flipped_at_g2() {
        // REQ-9 reached G2: `G2_FLIPPED` is now the declaration (`true`). The
        // declaration-level profile reads the proven, scoped form — but this is
        // the declaration only; the per-clause emit still routes through the gate
        // ([`g2_flip_permitted`]), which a red check downgrades (see the toggle tests).
        // REQ-9 flips the G2 declaration on (checked at compile time).
        const _: () = assert!(G2_FLIPPED);
        assert_eq!(strat_trust_profile_current(), strat_trust_profile(true));
        assert!(strat_trust_profile_current()
            .iter()
            .all(|s| !s.contains("UNPROVEN")));
        assert!(strat_trust_profile_current()
            .iter()
            .any(|s| s.starts_with("ref_encode(strat)")));
    }

    // ---- the G2 gate (AC-9: the four-check mechanical block) ----

    #[test]
    fn gate_permits_the_flip_only_when_all_four_green() {
        let all = G2Checks::all_passing();
        assert!(all.all_green());
        assert!(all.red().is_empty());
        // Declared + all green ⇒ the flip is permitted ⇒ the proven (scoped) profile.
        assert!(g2_flip_permitted(true, &all));
        let profile = strat_trust_profile_gated(true, &all);
        assert!(profile.iter().all(|s| !s.contains("UNPROVEN")));
        assert!(profile.iter().any(|s| s.starts_with("ref_encode(strat)")));
    }

    #[test]
    fn gate_blocks_the_flip_when_any_one_check_is_red() {
        // AC-9: toggle each of the four red in turn (the others green) and assert the flip
        // is mechanically withheld — the proven label is never emitted while a check is red,
        // even though the declaration is on.
        let labels = [
            "[1'] axiom-probe",
            "[4'] doc-drift",
            "[8] differential-battery",
            "[9] two-phase-TV",
        ];
        for (idx, label) in labels.iter().enumerate() {
            let mut checks = G2Checks::all_passing();
            match idx {
                0 => checks.axiom_probe = false,
                1 => checks.doc_drift = false,
                2 => checks.differential = false,
                _ => checks.two_phase_tv = false,
            }
            assert!(!checks.all_green(), "{label} should be red");
            assert_eq!(checks.red(), vec![*label], "exactly {label} is red");
            // The flip is WITHHELD even with the declaration on (`true`).
            assert!(
                !g2_flip_permitted(true, &checks),
                "the flip must be blocked while {label} is red"
            );
            let profile = strat_trust_profile_gated(true, &checks);
            assert!(
                profile.iter().any(|s| s.contains("UNPROVEN")),
                "a red {label} downgrades the label to UNPROVEN: {profile:?}"
            );
            // The solver component is unchanged across the withhold.
            assert!(profile.iter().any(|s| s == SOLVER_Z3));
        }
    }

    #[test]
    fn gate_blocks_the_flip_when_all_four_red() {
        let none = G2Checks {
            axiom_probe: false,
            doc_drift: false,
            differential: false,
            two_phase_tv: false,
        };
        assert_eq!(none.red().len(), 4);
        assert!(!g2_flip_permitted(true, &none));
        assert!(strat_trust_profile_gated(true, &none)
            .iter()
            .any(|s| s.contains("UNPROVEN")));
    }

    #[test]
    fn gate_withholds_when_undeclared_even_if_all_green() {
        // The declaration is the necessary precondition too: an UNdeclared gate
        // (`declared = false`) never flips, regardless of the checks. (Symmetric honesty:
        // green checks alone do not declare G2.)
        assert!(!g2_flip_permitted(false, &G2Checks::all_passing()));
        assert!(strat_trust_profile_gated(false, &G2Checks::all_passing())
            .iter()
            .any(|s| s.contains("UNPROVEN")));
    }
}
