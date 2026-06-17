//! `forge/src/accessibility.rs` — the Stage-1 forge-tier **`dec wf` accessibility cache
//! consumer** (`.design/stage1-forge-tier.md` REQ-9 / Q7 / AC-13, increment 3).
//!
//! A `dec wf <rel>` termination measure (Q-DECWF: ASCII, normalized by the parser to the
//! registry-free call `wf(<rel>)`) carries an ACCESSIBILITY obligation: the relation must
//! be well-founded on the recursion's carrier for the recursion to be admitted. That proof
//! is expensive and re-derivable, and — crucially — it is shared across every item that
//! recurses under the SAME (relation, carrier): the well-foundedness of `<` on `u32` is one
//! fact, not one-per-item. So it is content-addressed by the (relation, carrier) pair and
//! cached ([`crate::cache::AccessibilityProof`]) under the same `CHECK_SCHEMA_VERSION` gate
//! key as the per-item proof cache.
//!
//! This module is the CONSUMER that populates that cache from the check path: it extracts
//! the (relation, carrier) of an item's `dec wf` measure ([`dec_wf_relation_and_carrier`])
//! and discharges-or-serves the accessibility proof through the cache
//! ([`discharge_accessibility`]). A re-check on an unchanged (relation, carrier) HITS the
//! cache (AC-13) — exactly the cross-invocation hit the per-item proof cache uses (`forge
//! check` writes, a re-check reads), observable through the cache layer.
//!
//! Governing design: `.design/stage1-forge-tier.md` REQ-9 / Q7 / AC-13; the cache
//! conventions live in `.design/forge/proof-cache.md` (the home of `cache.rs`).

use std::path::Path;

use thermite_syntax::ast::{Clause, Expr, ForgeItem, Item, Param};

use crate::cache::{self, AccessibilityProof};

/// Extract the `(relation, carrier)` pair of an item's `dec wf <rel>` measure, or `None`
/// if the item carries no `dec wf` measure (REQ-9 / Q7). The carrier is the recursing
/// item's FIRST parameter type (the measure ranges over the structurally-decreasing
/// argument), rendered span-free so the same carrier caches identically regardless of
/// source position (reuses [`crate::lemma_library::render_type`]). An item with no params
/// has the unit carrier `()`.
#[must_use]
pub fn dec_wf_relation_and_carrier(item: &Item) -> Option<(String, String)> {
    let (dec, params): (&Clause, &[Param]) = match item {
        Item::Fn(f) => (f.dec.as_ref()?, &f.params),
        Item::SpecFn(s) => (&s.dec, &s.params),
        Item::Forge(ForgeItem::PropFn(p)) => (p.dec.as_ref()?, &p.params),
        _ => return None,
    };
    let relation = wf_relation(dec)?;
    let carrier = params.first().map_or_else(
        || "()".to_string(),
        |p| crate::lemma_library::render_type(&p.ty),
    );
    Some((relation, carrier))
}

/// The well-founded relation named by a `dec wf <rel>` clause, or `None` if the clause is
/// not a `wf(<rel>)` measure (REQ-9). The parser normalizes `dec wf <rel>` to the
/// registry-free call `wf(<rel>)` (Q-DECWF), so the clause expr is an [`Expr::Call`] whose
/// callee is the single-segment path `wf`; the relation is recovered from the clause's
/// verbatim `text` (the `wf`-prefixed span) so it is a stable, position-independent key.
fn wf_relation(clause: &Clause) -> Option<String> {
    let Expr::Call { callee, .. } = &clause.expr else {
        return None;
    };
    let Expr::Path(segs) = callee.as_ref() else {
        return None;
    };
    if segs.len() != 1 || segs[0] != "wf" {
        return None;
    }
    // The clause text is the `wf <rel>` span (the `dec` keyword is consumed before the
    // span starts, Q-DECWF). Strip the leading `wf` marker to leave the relation text.
    let text = clause.text.trim();
    let rel = text.strip_prefix("wf").unwrap_or(text).trim();
    if rel.is_empty() {
        None
    } else {
        Some(rel.to_string())
    }
}

/// Discharge OR serve the accessibility proof for a `(relation, carrier)` through the cache
/// (REQ-9 / AC-13). On a cache HIT the stored proof is returned (flagged `cached: true`) —
/// a re-check on an unchanged (relation, carrier) does NOT re-derive (the observable
/// cache-layer hit). On a MISS, `derive()` computes the well-founded verdict, which is
/// stored (best-effort — a write failure does not fail the check, R-CODE-2) and returned.
///
/// `derive` is the caller's accessibility decision (on the check path: whether the prover
/// admitted the recursion under this measure). Keeping it a closure means this module owns
/// the CACHE protocol (load → derive-on-miss → store), not the proof itself — the cache
/// discipline REQ-9 specifies, decoupled from the discharge.
pub fn discharge_accessibility(
    cache_dir: &Path,
    relation: &str,
    carrier: &str,
    derive: impl FnOnce() -> bool,
) -> AccessibilityProof {
    let key = cache::accessibility_cache_key(relation, carrier);
    if let Some(hit) = cache::load_accessibility(cache_dir, &key) {
        return hit;
    }
    let proof = AccessibilityProof::new(relation, carrier, derive());
    // Best-effort store: a cache that cannot be written is slower, never wrong.
    let _ = cache::store_accessibility(cache_dir, &key, &proof);
    proof
}

/// Cache the `dec wf` accessibility proofs for every item in `items` that carries a `dec
/// wf` measure (REQ-9 / AC-13), keyed by (relation, carrier). `derive(name)` supplies the
/// accessibility verdict for the item named `name` (on the check path: whether its cert
/// admitted the recursion). A side-effecting write-through pass: the cache is populated so
/// a SUBSEQUENT `forge check` re-check hits it (the cross-invocation hit the per-item proof
/// cache uses). Items with no `dec wf` measure are skipped — a no-op on the v1 corpus (no
/// v1 item uses `dec wf`).
pub fn cache_dec_wf_accessibility(
    cache_dir: &Path,
    items: &[Item],
    mut derive: impl FnMut(&str) -> bool,
) {
    for item in items {
        if let Some((relation, carrier)) = dec_wf_relation_and_carrier(item) {
            let name = item.name().to_string();
            let _ = discharge_accessibility(cache_dir, &relation, &carrier, || derive(&name));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_ok(src: &str) -> thermite_syntax::ast::Program {
        let parsed = thermite_syntax::parse(src);
        assert!(parsed.is_clean(), "parse errors: {:?}", parsed.errors);
        parsed.program
    }

    fn unique_dir(tag: &str) -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static C: AtomicU64 = AtomicU64::new(0);
        let n = C.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "forge_wf_test_{}_{}_{}",
            tag,
            std::process::id(),
            n
        ))
    }

    // REQ-9 / Q7: the (relation, carrier) of a `dec wf <rel>` measure is extracted — the
    // relation from the normalized `wf(<rel>)` clause, the carrier from the first param type.
    #[test]
    fn extracts_dec_wf_relation_and_carrier() {
        let prog = parse_ok(
            "spec fn rank(n: u32) -> u32 dec wf lt_rel { n }\n\
             fn plain(n: u32) -> u32 req true ens result == n fx pure { n }",
        );
        let rank = &prog.items[0];
        assert_eq!(
            dec_wf_relation_and_carrier(rank),
            Some(("lt_rel".to_string(), "Prim(U32)".to_string())),
            "the wf relation + the first-param carrier are extracted"
        );
        // A non-`dec wf` item yields None (a plain fn with no dec measure).
        let plain = &prog.items[1];
        assert_eq!(dec_wf_relation_and_carrier(plain), None);
    }

    // A `dec lex(...)` / plain `dec <expr>` measure is NOT a `dec wf` — no accessibility key.
    #[test]
    fn non_wf_measures_are_not_accessibility_keyed() {
        let prog = parse_ok("spec fn rank(n: u32) -> u32 dec n { n }");
        assert_eq!(dec_wf_relation_and_carrier(&prog.items[0]), None);
    }

    // REQ-9 / AC-13: the FIRST discharge derives + stores; a re-check on the same
    // (relation, carrier) HITS the cache (no re-derivation), observable via the cache layer.
    #[test]
    fn recheck_hits_without_rederiving() {
        let dir = unique_dir("discharge");
        let _ = std::fs::remove_dir_all(&dir);
        use std::cell::Cell;
        let derivations = Cell::new(0u32);
        let first = discharge_accessibility(&dir, "lt_rel", "Prim(U32)", || {
            derivations.set(derivations.get() + 1);
            true
        });
        assert!(!first.cached, "the first discharge is a fresh derivation");
        assert!(first.well_founded);
        assert_eq!(derivations.get(), 1, "derived once on the miss");
        // The re-check HITS — the derive closure must NOT run again.
        let second = discharge_accessibility(&dir, "lt_rel", "Prim(U32)", || {
            derivations.set(derivations.get() + 1);
            panic!("a cache hit must not re-derive");
        });
        assert!(
            second.cached,
            "the re-check is served from the cache (AC-13)"
        );
        assert_eq!(second.relation, "lt_rel");
        assert_eq!(second.carrier, "Prim(U32)");
        assert_eq!(derivations.get(), 1, "no re-derivation on the hit");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // REQ-9: the write-through pass populates the cache for `dec wf` items only; a
    // subsequent direct lookup hits (the cross-invocation re-check model).
    #[test]
    fn cache_pass_populates_dec_wf_items_only() {
        let dir = unique_dir("pass");
        let _ = std::fs::remove_dir_all(&dir);
        let prog = parse_ok(
            "spec fn rank(n: u32) -> u32 dec wf lt_rel { n }\n\
             fn plain(n: u32) -> u32 req true ens result == n fx pure { n }",
        );
        cache_dec_wf_accessibility(&dir, &prog.items, |_name| true);
        // The `dec wf` item is cached; a re-check hits.
        let key = cache::accessibility_cache_key("lt_rel", "Prim(U32)");
        assert!(
            cache::load_accessibility(&dir, &key).is_some(),
            "the dec wf item's accessibility proof is cached (a re-check would hit)"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
