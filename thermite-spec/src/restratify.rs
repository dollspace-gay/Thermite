//! The restratification rewrite + its R-side-1 certification discipline
//! (`.design/stage2-stratified-cage.md` REQ-7 / AC-7), the Rust ops-half mirroring
//! `lean/Thermite/Strat/Restratify.lean`'s T4-R metatheory.
//!
//! `restrat` breaks an admission cycle by EXCISING the cycle-closing conjunct `B` and
//! replacing it with a fresh opaque abstraction `p` (an [`Atom::QFree`] leaf — opaque to
//! the classifier, contributing no sorts and no graph edges). On the motivating kv
//! alternation cycle
//!
//! ```text
//! φ  =  (∀k:Key. ∃v:Value. v = k)  ∧  (∀v:Value. ∃k:Key. k = v)
//!       └──────────  A  ──────────┘     └──────────  B  ──────────┘
//! ```
//!
//! whose sort graph has both `Key → Value` (from A) and `Value → Key` (from B) — a cycle,
//! so `classify(φ)` is [`Verdict::Rejected`] — the rewrite yields
//!
//! ```text
//! φ' = restrat(φ)  =  A ∧ p            (only Key → Value ⇒ acyclic ⇒ admitted)
//! Side(φ', φ)      =  p ⇒ B            (only Value → Key ⇒ acyclic ⇒ admitted)
//! ```
//!
//! **R-side-1 (the required discipline).** A certificate of φ' alone never counts for
//! φ: `p` is a fresh, unconstrained abstraction, so `A ∧ p` is satisfied trivially by
//! `p := true` without B holding. [`certify`] therefore WITHHOLDS the φ-certificate unless
//! the `Side` obligation (`p ⇒ B`, itself in-cage) is separately discharged. This mirrors
//! the Lean `restrat_conservative`, which consumes both φ' and `Side`; dropping `Side`
//! is exactly the mis-certification `Thermite.PinRestratDropSide` exhibits.

use crate::classifier::{classify, Atom, Frm, Sort2, Tm, Verdict};

/// The fresh opaque boolean abstraction leaf standing in for an excised sub-formula
/// (`Strat/Restratify.lean` `absLeaf`). A `qfree` atom is opaque to the classifier — it
/// contributes no sorts and no graph edges — so substituting it for a cycle-closing
/// conjunct deletes that conjunct's edges from the sort graph.
#[must_use]
pub fn abs_leaf() -> Frm {
    Frm::Atom(Atom::QFree)
}

/// The product of the restratify rewrite: the admissible φ' and the side obligation
/// `Side(φ', φ)` (`Strat/Restratify.lean` `restrat` + `Side`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestratResult {
    /// The original (rejected) formula φ.
    pub original: Frm,
    /// The rewritten formula φ' = `A ∧ p` (the cycle-closing conjunct excised).
    pub rewritten: Frm,
    /// The side obligation `Side(φ', φ)` = `p ⇒ B` (the excised conjunct recovered from
    /// its abstraction).
    pub side: Frm,
}

/// The restratify rewrite (metatheory §6). On a conjunction `A ∧ B` whose right conjunct
/// `B` closes the alternation cycle, excise `B`, replace it with the fresh abstraction
/// `p`, and emit `Side = p ⇒ B` (`Strat/Restratify.lean` `restrat`/`Side`). Returns
/// [`None`] when `φ` is not a conjunction — there is no cycle-closing conjunct to excise
/// (the kv repair is the §6 worked instance; the classifier's reported cycle selects the
/// split).
#[must_use]
pub fn restratify(phi: &Frm) -> Option<RestratResult> {
    match phi {
        Frm::Conj(a, b) => Some(RestratResult {
            original: phi.clone(),
            rewritten: Frm::Conj(a.clone(), Box::new(abs_leaf())),
            side: Frm::Imp(Box::new(abs_leaf()), b.clone()),
        }),
        _ => None,
    }
}

/// Why a restratify-based φ-certificate was WITHHELD (R-side-1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WithheldReason {
    /// The rewrite did not apply — φ is not a conjunction, so there is no cycle-closing
    /// conjunct to excise.
    NotRestratifiable,
    /// φ' is not admitted — the rewrite did not move φ into the cage (a malformed split).
    RewrittenNotAdmitted,
    /// `Side` is not itself in-cage, so it cannot be discharged in-cage — the split is
    /// not usable.
    SideNotInCage,
    /// φ' and `Side` are both in-cage, but `Side` was not discharged — the φ-certificate
    /// is withheld (R-side-1: a φ'-only certificate never counts for φ).
    SideUndischarged,
}

/// The certification verdict for restratifying φ.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Certification {
    /// φ is certified: φ' is admitted and `Side` was discharged in-cage. Carries the
    /// rewrite products for rendering / auditing.
    Certified(RestratResult),
    /// φ is not certified — see the reason. Carries the rewrite products when the rewrite
    /// applied (for rendering), else `None`.
    Withheld(WithheldReason, Option<RestratResult>),
}

impl Certification {
    /// `true` iff φ was certified (φ' admitted and `Side` discharged in-cage).
    #[must_use]
    pub fn is_certified(&self) -> bool {
        matches!(self, Certification::Certified(_))
    }
}

/// Certify φ through restratification, honouring R-side-1. `side_discharged` models
/// whether the caller separately discharged the `Side` obligation in-cage; when `false`,
/// the φ-certificate is WITHHELD even though φ' is admitted — exactly the discipline
/// `Thermite.Strat.Cls.restrat_conservative` enforces (it consumes the `Side` hypothesis)
/// and `Thermite.PinRestratDropSide` shows is required.
#[must_use]
pub fn certify(phi: &Frm, side_discharged: bool) -> Certification {
    let Some(result) = restratify(phi) else {
        return Certification::Withheld(WithheldReason::NotRestratifiable, None);
    };
    if !matches!(classify(&result.rewritten), Verdict::Admitted) {
        return Certification::Withheld(WithheldReason::RewrittenNotAdmitted, Some(result));
    }
    if !matches!(classify(&result.side), Verdict::Admitted) {
        return Certification::Withheld(WithheldReason::SideNotInCage, Some(result));
    }
    if !side_discharged {
        // R-side-1: φ' is admitted and Side is in-cage, but Side was not discharged.
        return Certification::Withheld(WithheldReason::SideUndischarged, Some(result));
    }
    Certification::Certified(result)
}

/// The §6 kv-alternation worked example (`Strat/Fragment.lean` `ex_kvCycle`): the
/// inadmissible formula `(∀k:Key. ∃v:Value. v = k) ∧ (∀v:Value. ∃k:Key. k = v)` whose
/// sort graph has the cycle `Key ⇄ Value` (Key = `opaque 0`, Value = `opaque 1`).
#[must_use]
pub fn kv_example() -> Frm {
    use crate::classifier::Rel;
    let key_s = Sort2::Opaque(0);
    let value_s = Sort2::Opaque(1);
    // ∀k:Key. ∃v:Value. v = k  (de Bruijn: v = var 0, k = var 1)
    let body1 = Frm::Atom(Atom::Rel(
        Rel::Eq,
        Tm::Var(value_s.clone(), 0),
        Tm::Var(key_s.clone(), 1),
    ));
    // ∀v:Value. ∃k:Key. k = v
    let body2 = Frm::Atom(Atom::Rel(
        Rel::Eq,
        Tm::Var(key_s.clone(), 0),
        Tm::Var(value_s.clone(), 1),
    ));
    Frm::Conj(
        Box::new(Frm::All(
            key_s.clone(),
            Box::new(Frm::Ex(value_s.clone(), Box::new(body1))),
        )),
        Box::new(Frm::All(value_s, Box::new(Frm::Ex(key_s, Box::new(body2))))),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classifier::RejectReason;

    /// The original kv formula is rejected with a sort-graph cycle (mirrors
    /// `ex_kvCycle_rejected`).
    #[test]
    fn kv_original_rejected() {
        assert!(matches!(
            classify(&kv_example()),
            Verdict::Rejected(RejectReason::SortGraphCycle { .. })
        ));
    }

    /// The rewrite applies and both products are admitted in-cage (mirrors the Lean
    /// `restrat_admits` + `side_admitted`).
    #[test]
    fn rewrite_and_side_both_admitted() {
        let r = restratify(&kv_example()).expect("kv is a conjunction");
        assert_eq!(
            classify(&r.rewritten),
            Verdict::Admitted,
            "φ' = A ∧ p is admitted (the Value → Key edge is gone with B)"
        );
        assert_eq!(
            classify(&r.side),
            Verdict::Admitted,
            "Side = p ⇒ B is admitted (only the Value → Key edge, no cycle)"
        );
    }

    /// AC-7 — certification is GRANTED end to end when `Side` is discharged in-cage.
    #[test]
    fn certified_when_side_discharged() {
        let cert = certify(&kv_example(), true);
        assert!(cert.is_certified(), "Side discharged in-cage ⇒ φ certified");
    }

    /// AC-7 — the withheld-certification discipline: certification is WITHHELD when `Side`
    /// is undischarged (R-side-1). This is the Rust mirror of `PinRestratDropSide`.
    #[test]
    fn withheld_when_side_undischarged() {
        let cert = certify(&kv_example(), false);
        assert!(
            !cert.is_certified(),
            "φ' admitted but Side undischarged ⇒ φ-certificate WITHHELD (R-SIDE-1)"
        );
        assert!(matches!(
            cert,
            Certification::Withheld(WithheldReason::SideUndischarged, Some(_))
        ));
    }

    /// A non-conjunction has no cycle-closing conjunct to excise — the rewrite does not
    /// apply, and certification is withheld.
    #[test]
    fn non_conjunction_not_restratifiable() {
        let phi = Frm::Atom(Atom::QFree);
        assert!(restratify(&phi).is_none());
        assert!(matches!(
            certify(&phi, true),
            Certification::Withheld(WithheldReason::NotRestratifiable, None)
        ));
    }
}
