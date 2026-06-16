//! `forge/src/burn.rs` — the L3 **burn receipt** (`.design/stage1-forge-tier.md`
//! REQ-7, increment 2e; the RFC-1 §9 L3 certificate shape; Q-BURN resolved). The
//! thesis's "burn the cheap resource" made auditable: when a forge-tier proof
//! closes a goal, the certificate records how much proof text was committed and
//! which lemmas it cited, so a reader can weigh the burned proof against the claim
//! it discharges (the program plan §6 "tokens per discharged L3 clause" metric).
//!
//! ## The two token fields (Q-BURN, resolved)
//!
//! - [`BurnReceipt::proof_tokens`] — ALWAYS present: the **lexer-token count of the
//!   committed proof text**, the project lexer ([`thermite_syntax::tokenize`]) run
//!   over the verbatim proof block. Deterministic and re-derivable by a skeptic with
//!   the same lexer (a pure function of the committed text — no wall-clock, no
//!   randomness), so it can join the certificate without making it non-reproducible.
//! - [`BurnReceipt::authoring_tokens`] — OPTIONAL: the LLM tokens the authoring
//!   harness spent producing the proof, recorded only when the harness supplies them
//!   (absent otherwise). The burn-economics dashboard consumes this where present and
//!   falls back to `proof_tokens` as a proxy (Q-BURN).
//!
//! BOTH fields are **oracle-EXCLUDED** (Q-ORACLE / Q-BURN): re-authoring a proof
//! legitimately changes the committed token count and the authoring spend without
//! changing what was proven, so the burn receipt is NOT part of
//! [`crate::manifest::Certificate::oracle_subset`] — exactly like `solver_time_ms`.
//! Adding it leaves the v1 golden certs byte-identical (a v1 item never burns a
//! forge-tier proof, so its `burn` stays `None`).
//!
//! Governing design: `.design/stage1-forge-tier.md` REQ-7 / AC-11.

use serde::{Deserialize, Serialize};

use crate::battery::{self, Citation};

/// The L3 burn receipt attached to a forge-tier certificate (REQ-7, RFC-1 §9). The
/// committed-proof token count, the optional authoring spend, and the lemmas the
/// proof cited. Additive + oracle-excluded (see the module docs): a v1 item never
/// carries one, so the golden certs are unperturbed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BurnReceipt {
    /// The lexer-token count of the committed proof text — the project lexer
    /// ([`thermite_syntax::tokenize`]) run over the verbatim proof block (Q-BURN).
    /// Deterministic and re-derivable: a pure function of the committed text.
    pub proof_tokens: usize,
    /// The LLM authoring tokens the harness spent, when it supplies them (Q-BURN).
    /// `None` unless the authoring harness records it; the dashboard falls back to
    /// `proof_tokens` as a proxy where absent. Oracle-excluded (re-authoring changes
    /// it without changing the claim). `#[serde(default, skip_serializing_if)]` so a
    /// receipt without it serializes without the key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authoring_tokens: Option<u64>,
    /// The lemmas the committed proof cited — the simp-lemma citations the frozen
    /// battery's scanner ([`battery::scan_citations`]) extracts from the proof text,
    /// deduplicated in document order (RFC-1 §9 "lemmas cited"). A proof that cites no
    /// lemma carries an empty list; `#[serde(default, skip_serializing_if)]` so it is
    /// omitted then (mirroring `strengthening`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cited_lemmas: Vec<String>,
}

impl BurnReceipt {
    /// Build the burn receipt for a committed proof block from its verbatim source
    /// text (REQ-7 / AC-11). `proof_tokens` is the project lexer's token count over
    /// the text; `cited_lemmas` are the simp-lemma citations the frozen-battery
    /// scanner extracts (deduped, document order); `authoring_tokens` is `None` (the
    /// authoring harness attaches it with [`BurnReceipt::with_authoring_tokens`] when
    /// it has the figure). A pure, deterministic function of `proof_text` (R-CODE-5).
    #[must_use]
    pub fn for_proof_text(proof_text: &str) -> Self {
        BurnReceipt {
            proof_tokens: proof_token_count(proof_text),
            authoring_tokens: None,
            cited_lemmas: cited_lemmas(proof_text),
        }
    }

    /// Attach the optional LLM authoring-token count (Q-BURN), returning the receipt
    /// with `authoring_tokens` set. Called by an authoring harness that tracked the
    /// spend; absent on a receipt minted purely from the committed text.
    #[allow(
        dead_code,
        reason = "Q-BURN authoring-token setter: production `forge` mints the receipt \
                  from the committed proof text (`for_proof_text`); this opt-in setter is \
                  for an authoring harness that supplies the LLM spend (absent until one \
                  does), exercised by the burn::tests round-trip."
    )]
    #[must_use]
    pub fn with_authoring_tokens(mut self, authoring_tokens: u64) -> Self {
        self.authoring_tokens = Some(authoring_tokens);
        self
    }
}

/// The lexer-token count of the committed proof text (Q-BURN): the number of tokens
/// the project lexer ([`thermite_syntax::tokenize`]) emits over the verbatim text,
/// EXCLUDING the trailing `Eof` sentinel (the lexer appends one `TokKind::Eof` to
/// every stream — it is a parser marker, not committed proof content, so an empty
/// proof counts 0 tokens and `omega` counts 1). Lex errors (the proof text is Lean
/// tactic syntax, not Thermite source, so unknown glyphs like `⊢`/`∀` are not
/// lexable) do not perturb determinism — the emitted token sequence is a pure
/// function of the bytes, so a skeptic re-running the same lexer gets the same count.
/// Deterministic (R-CODE-5).
fn proof_token_count(proof_text: &str) -> usize {
    let (tokens, _errors) = thermite_syntax::tokenize(proof_text);
    tokens
        .iter()
        .filter(|t| t.kind != thermite_syntax::TokKind::Eof)
        .count()
}

/// The lemmas the proof cites — the [`Citation::SimpLemma`] names the frozen-battery
/// scanner extracts from `simp [ … ]` arguments, deduplicated while preserving
/// document order (RFC-1 §9 "lemmas cited"). Tactic heads (`omega`, `simp`, …) are
/// tactics, not lemmas, so they are not cited lemmas; the only lemma citations the
/// scanner surfaces are the simp-set members the proof named. Deterministic.
fn cited_lemmas(proof_text: &str) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for citation in battery::scan_citations(proof_text) {
        if let Citation::SimpLemma(name) = citation {
            if seen.insert(name.clone()) {
                out.push(name);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // REQ-7 / Q-BURN: `proof_tokens` is the project lexer's token count over the
    // committed proof text — deterministic and re-derivable. A simple `omega` proof
    // lexes to one token.
    #[test]
    fn proof_tokens_is_the_lexer_token_count() {
        let receipt = BurnReceipt::for_proof_text("omega");
        assert_eq!(receipt.proof_tokens, 1, "`omega` is one lexer token");
        // Re-deriving from the same text yields the same count (determinism).
        assert_eq!(
            BurnReceipt::for_proof_text("omega").proof_tokens,
            receipt.proof_tokens
        );
    }

    // REQ-7 / RFC-1 §9: `cited_lemmas` are the simp-lemma citations, deduped in
    // document order; tactic heads are not lemmas.
    #[test]
    fn cited_lemmas_are_the_simp_citations_deduped() {
        let receipt = BurnReceipt::for_proof_text(
            "simp [Thermite.bindInt, Thermite.intVal]; simp [Thermite.bindInt]; omega",
        );
        assert_eq!(
            receipt.cited_lemmas,
            vec![
                "Thermite.bindInt".to_string(),
                "Thermite.intVal".to_string()
            ],
            "simp lemmas in document order, deduped; `omega`/`simp` heads excluded"
        );
    }

    // Q-BURN: `authoring_tokens` is absent unless the harness supplies it; attaching
    // it is opt-in and does not touch the deterministic fields.
    #[test]
    fn authoring_tokens_is_opt_in() {
        let bare = BurnReceipt::for_proof_text("omega");
        assert_eq!(bare.authoring_tokens, None);
        let with = bare.clone().with_authoring_tokens(4096);
        assert_eq!(with.authoring_tokens, Some(4096));
        assert_eq!(
            with.proof_tokens, bare.proof_tokens,
            "deterministic fields untouched"
        );
    }

    // Q-BURN / Q-ORACLE: the burn receipt is ORACLE-EXCLUDED — a cert carrying a burn
    // receipt and the same cert without it compare oracle-EQUAL, so re-authoring a proof
    // (which changes the receipt) never perturbs the cert oracle / breaks golden stability.
    #[test]
    fn burn_is_oracle_excluded() {
        use crate::manifest::{Certificate, Level, ObligationResult};
        let base = Certificate::new(
            "merge_advance",
            Level::L3,
            vec!["pure".to_string()],
            0,
            vec![ObligationResult::discharged("x")],
        );
        let with_burn = base
            .clone()
            .with_burn(BurnReceipt::for_proof_text("simp [Thermite.denote]; omega"));
        assert_ne!(base.burn, with_burn.burn, "the burn field itself differs");
        assert_eq!(
            base.oracle_subset(),
            with_burn.oracle_subset(),
            "the burn receipt is oracle-excluded (Q-BURN): the oracle subset is unchanged"
        );
    }

    // The receipt serde round-trips; an empty `cited_lemmas` + absent
    // `authoring_tokens` omit their keys (additive, mirrors `strengthening`).
    #[test]
    fn serde_round_trips_and_omits_empty_fields() {
        let receipt = BurnReceipt::for_proof_text("omega");
        let json = serde_json::to_string(&receipt).expect("serialize");
        assert!(json.contains("\"proof_tokens\":1"), "{json}");
        assert!(
            !json.contains("authoring_tokens"),
            "absent key omitted: {json}"
        );
        assert!(!json.contains("cited_lemmas"), "empty list omitted: {json}");
        let back: BurnReceipt = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, receipt);
    }
}
