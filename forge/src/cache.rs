//! `forge/src/cache.rs` — the per-item, content-addressed proof cache and the
//! home of the bit-reproducible-verification contract (`thermite-design.md`
//! §5.3: "Proof results are content-addressed and cached per item").
//!
//! For each `.th` item, `check::check_file` computes a stable cache key from the
//! four inputs that determine that item's verdict (the item's lowered Verus
//! source, the pinned solver seed, the verus version, and the thermite toolchain
//! version), consults the cache before spawning verus, returns the stored
//! [`Certificate`] on a hit (skipping the solver), and stores the result on a
//! miss. The cache is a performance optimization that does not change a verdict: a
//! hit is indistinguishable from a fresh verify (`goal.md` R-DEFER-9: the cache
//! does not affect a verdict).
//!
//! Governing design: `.design/forge/proof-cache.md`.
//!
//! This module is a thin, deterministic, content-addressed store with no
//! verification logic of its own. It sits between `check::item_subprogram` /
//! `thermite_lower::lower` (which produce the lowered source it content-addresses)
//! and `check::run_verus` (the solver invocation it lets `forge` skip on a hit).
//! IO failures degrade to a miss, not a panic (R-CODE-2): a damaged cache is
//! slower, never wrong.
//!
//! ## REQ status
//!
//! <!-- generated:reqs view=forge-cache-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-FORGE-CACHE-CACHED-FIELD | shipped | `forge/src/cache.rs` | Additive cached certificate field |  |
//! | REQ-FORGE-CACHE-DETERMINISM | shipped | `forge/src/cache.rs` | Deterministic proof cache round trip |  |
//! | REQ-FORGE-CACHE-HIT-FRESH-INVARIANT | shipped | `forge/src/cache.rs` | Proof cache hit equals fresh verify |  |
//! | REQ-FORGE-CACHE-KEY | shipped | `forge/src/cache.rs` | Proof cache key composition |  |
//! | REQ-FORGE-CACHE-LOCALITY | shipped | `forge/src/cache.rs` | Proof cache per-item locality |  |
//! | REQ-FORGE-CACHE-LOOKUP-STORE | shipped | `forge/src/cache.rs` | Proof cache lookup then store flow |  |
//! | REQ-FORGE-CACHE-STORAGE | shipped | `forge/src/cache.rs` | Proof cache location and JSON storage |  |
//! | REQ-FORGE-CACHE-VERSION-KEY | shipped | `forge/src/cache.rs` | Proof cache version-keyed invalidation |  |
//! <!-- /generated:reqs -->

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::manifest::{Certificate, ObligationStatus};

/// Domain-separation tag prefixed to the whole keyed stream, so a `forge` proof
/// cache key does not collide with an unrelated sha256 use of the same bytes
/// (`.design/forge/proof-cache.md` REQ-1, domain separation).
const DOMAIN: &[u8] = b"thermite.forge.proof-cache.v1";

/// The version of forge's verdict-affecting check logic — the set of gates a
/// cached certificate was produced under (`.design/forge/proof-cache.md` REQ-2,
/// the soundness-completeness invariant). It is a fifth cache-key input
/// (domain-tagged + length-prefixed like the other four) so that a certificate
/// stored under one set of gates is not re-served once the gate set changes: a
/// different schema ⇒ a different key ⇒ a miss ⇒ a full re-check under the current
/// gates. This closes the bypass where a cert cached by a forge before a gate
/// existed (under an identical (lowered_src, seed, verus_version,
/// thermite_version) key, because `forge`'s crate version did not move) was served
/// on a hit and skipped the now-required gate.
///
/// Maintenance contract (blocker #49): bump this constant whenever the set of
/// verdict-affecting checks/gates changes (a gate added, removed, or its
/// pass/fail semantics altered, e.g. the §7 mutation floor, the vacuity battery,
/// the triage rejects). The `thermite_version` input does not suffice: the
/// toolchain ships gate changes without a crate-version bump (issue #12's
/// mutation gate landed at 0.1.0), so the check-logic version must move
/// independently. Without a bump the stale-verdict bypass re-opens.
///
/// History:
///   1 — pre-mutation-gate check logic (the original four-input key era).
///   2 — issue #12 §7 mutation floor added (blocker #49: invalidates every
///       pre-gate cert so a weak contract is re-checked through the gate).
///   3 — blocker #74: the §7 early-return mutant is now synthesized for a
///       `Vec<T>` return (an empty-Vec `TVec<Suffix> { data: Vec::new() }`,
///       mirroring the #48 `&[]` slice synthesis), so a `Vec`-returning fn is
///       scored instead of 0/0-gated to `WeakContract`. This changes the gate
///       verdict for `Vec`-return fns (a proved `push_one` now certifies L3
///       instead of the mutation-gated L0), so every cert stored under schema 2
///       must be re-checked under schema 3 (the maintenance contract above: a
///       gate-semantics change ⇒ bump, or a stale L0 is served on an identical
///       lowered-source key, REQ-2).
///   4 — blocker #80: the §7 early-return mutant is now synthesized for a
///       `String` return (an empty `TString { data: Vec::new() }`, mirroring the
///       #74 empty-`Vec` synthesis) in `mutation::early_return_value`'s
///       `Type::String` arm, so a `String`-returning fn is scored instead of
///       0/0-gated to `WeakContract`. This changes the gate verdict for
///       `String`-return fns (the proved `join`/`concat` now certifies L3 instead
///       of the mutation-gated L0), so every cert stored under schema 3 must be
///       re-checked under schema 4 (the maintenance contract above: else `forge
///       check` serves the stale L0 cached on an identical lowered-source key,
///       REQ-2: a hit must equal a fresh verify).
///   5 — blocker #101 (`.design/forge/equivalent-mutants.md` REQ-5): the §7
///       mutation gate now excludes a survivor Verus proves observably equivalent
///       to the real body under `req` from the kill-ratio denominator
///       (`check::mutation_score` → `equivalence_proves_equal`). This changes the
///       gate verdict for forced-output fns (a `1/3` `WeakContract` `clamp_zero`
///       becomes a certifying `1/1` once its two proved-equivalent survivors
///       drop), so the check logic is no longer the same function of its inputs:
///       every cert stored under schema 4 must be re-checked under schema 5
///       (else `forge check` serves the stale `WeakContract` cached on an
///       identical lowered-source key, REQ-2: a hit must equal a fresh verify).
///   6 — blocker #269 (`.design/forge/mutation-scoring.md` REQ-9/REQ-10/REQ-12):
///       the §7 early-return family now also synthesizes the F-IDENT identity
///       returns (`return <param>` for every param whose type equals the return)
///       and the F-STRUCT-ZERO named-struct field-zero literal. Both are
///       verdict-changing widenings of the frozen mutant set (an item's `K/N`
///       and even its certify/gate verdict can change, e.g. `move_up` gains a
///       surviving `return b` identity mutant), so the check logic is no longer
///       the same function of its inputs: every cert stored under schema 5 must
///       be re-checked under schema 6 (else `forge check` serves a stale
///       pre-#269 tally / verdict on an identical lowered-source key, REQ-2: a
///       hit must equal a fresh verify).
///   7 — blocker #269 (`.design/forge/equivalent-mutants.md` REQ-7/REQ-9): the
///       per-survivor equivalence probe now handles call-bearing bodies. A §9
///       composition caller's F-IDENT identity survivor (`return <param>`) that
///       is proved equivalent through its callees' contracts (the exec-harness
///       arm) drops from the denominator. This is verdict-changing for
///       call-bearing fns: `caller(x) { ext_id(x) }`'s identity mutant flips from
///       a counted survivor (gating `WeakContract` at schema 6, the schema-6
///       Arc-1 build cached this) to an excluded equivalent (certifying L3). The
///       check logic is no longer the same function of its inputs, so every cert
///       stored under schema 6 must be re-checked under schema 7 (else
///       `forge check` serves a stale schema-6 `WeakContract` on an identical
///       lowered-source key, REQ-2: a hit must equal a fresh verify).
const CHECK_SCHEMA_VERSION: u32 = 7;

/// The project-local proof-cache directory (`.design/forge/proof-cache.md`
/// REQ-6, OQ-1): `target/thermite-proof-cache/`. It is build output under the
/// already-git-ignored `target/`, so it is never committed and `cargo clean`
/// clears it. The path is relative to the current working directory (the project
/// root), matching where `target/` lives. Consumed by `check::check_file`.
pub fn default_cache_dir() -> PathBuf {
    PathBuf::from("target").join("thermite-proof-cache")
}

/// Compute the stable content-address cache key for one item (REQ-1) — a
/// lowercase-hex sha256 over the four verdict-determining inputs:
///
/// 1. `lowered_src` — the item's lowered Verus source (what verus checks; the
///    §5.3 isolated sub-program). REQ-1a.
/// 2. `seed` — the pinned SMT solver seed (`check::resolve_seed`, §5.3). REQ-1b.
/// 3. `verus_version` — the verus binary version (`verus --version`). REQ-1d/REQ-5.
/// 4. `thermite_version` — the `forge` toolchain version
///    (`env!("CARGO_PKG_VERSION")`). REQ-1c/REQ-5.
///
/// Each field is domain-tagged and length-prefixed (`field`), so two distinct
/// input tuples do not collide by concatenation ambiguity: the hash is injective
/// on the structured tuple, not on a flat byte concatenation (the soundness
/// argument, REQ-2). This function is pure: no wall-clock, no environment beyond
/// the explicitly-passed arguments (R-CODE-5). Identical inputs ⇒ identical key;
/// a differing input ⇒ a different key ⇒ a miss ⇒ re-verify.
pub fn cache_key(
    lowered_src: &str,
    seed: u64,
    verus_version: &str,
    thermite_version: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(DOMAIN);
    field(&mut hasher, b"lowered", lowered_src.as_bytes());
    field(&mut hasher, b"seed", &seed.to_le_bytes());
    field(&mut hasher, b"verus", verus_version.as_bytes());
    field(&mut hasher, b"thermite", thermite_version.as_bytes());
    // The fifth input (blocker #49): the verdict-affecting check-logic version, so
    // a cert cached under one set of gates is not re-served once the gate set
    // changes (a different schema ⇒ a different key ⇒ a miss ⇒ re-check under the
    // current gates). Captures what `thermite_version` cannot — gate changes that
    // ship without a crate-version bump (see `CHECK_SCHEMA_VERSION`).
    field(
        &mut hasher,
        b"check-schema",
        &CHECK_SCHEMA_VERSION.to_le_bytes(),
    );
    let digest = hasher.finalize();
    hex_lower(&digest)
}

/// Feed one domain-tagged, length-prefixed field into the hasher (REQ-1). The
/// layout is `len(tag):u32-le || tag || len(value):u64-le || value`, so neither
/// the tag nor the value can be re-split into a different (tag, value) pair: the
/// boundaries are unambiguous. This makes the four-input hash
/// injective on the tuple (the no-collision-by-concatenation property).
fn field(hasher: &mut Sha256, tag: &[u8], value: &[u8]) {
    hasher.update((tag.len() as u32).to_le_bytes());
    hasher.update(tag);
    hasher.update((value.len() as u64).to_le_bytes());
    hasher.update(value);
}

/// Render a byte digest as lowercase hex (the on-disk filename form). Pure and
/// deterministic (R-CODE-5).
fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        // Two lowercase hex nibbles per byte.
        const HEX: &[u8; 16] = b"0123456789abcdef";
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

/// The on-disk path for one cache entry under `cache_dir` (REQ-6):
/// `<cache_dir>/<hex-key>.json`.
fn entry_path(cache_dir: &Path, key: &str) -> PathBuf {
    cache_dir.join(format!("{key}.json"))
}

/// Look up a cached [`Certificate`] by `key` under `cache_dir` (REQ-3/REQ-6).
///
/// Returns `Some(cert)` on a hit (a present, readable, parseable entry whose key
/// filename matches), and `None` on a miss. A miss includes: no file, an
/// unreadable file, and a corrupt/unparseable file. A damaged cache degrades to
/// re-verify, not to an error and not to a stale read (REQ-6, R-CODE-2: no
/// panic on the IO error path). The returned cert carries whatever `cached`
/// value was stored (`store` persists `false`); `check::check_file` sets the
/// observable `cached: true` via `Certificate::with_cached` on the hit it serves.
pub fn load(cache_dir: &Path, key: &str) -> Option<Certificate> {
    let path = entry_path(cache_dir, key);
    let src = std::fs::read_to_string(&path).ok()?;
    // A corrupt/unparseable entry is a miss (not an error): re-verify + overwrite.
    let cert = serde_json::from_str::<Certificate>(&src).ok()?;
    // An internally-inconsistent entry is also a miss (blocker #49). A cert
    // produced under a different set of gates than the current `forge` (e.g.
    // stored by a forge before the §7 mutation floor existed) can land under the
    // same content-address as a current key (the gate change shipped without a
    // verdict-input change) and would otherwise be re-served on a hit, bypassing
    // the now-required gate. The tell is self-contradiction: the stored cert
    // claims clean (`reject: None`) yet still carries a failed obligation in its
    // own `obligations` array (the gate that failed it under the old logic). A
    // clean cert produced by the current logic never carries a failed obligation
    // while `reject` is `None`. Treating such an entry as a miss forces a full
    // re-check under the current gates (REQ-2: a hit must equal a fresh verify;
    // `goal.md` R-DEFER-9: the cache does not serve a stale clean verdict past a
    // gate). This is the load-time half of the soundness guard; the
    // `CHECK_SCHEMA_VERSION` cache-key input is the on-disk-key half.
    if is_internally_consistent(&cert) {
        Some(cert)
    } else {
        None
    }
}

/// A stored [`Certificate`] is internally consistent iff a clean verdict
/// (`reject.is_none()`) carries no failed obligation (blocker #49). A cert that
/// claims clean while still recording a failed obligation was produced under a
/// different gate set than the current `forge` (a stale verdict that predates a
/// gate); serving it would bypass that gate. The check is conservative: it only
/// rejects the self-contradictory shape, so every cert the current logic itself
/// stores (clean ⇒ all obligations discharged; rejected ⇒ `reject.is_some()`)
/// round-trips as a hit (no regression to the warm-hit path, REQ-2/AC-1).
fn is_internally_consistent(cert: &Certificate) -> bool {
    if cert.reject.is_some() {
        return true;
    }
    !cert
        .obligations
        .iter()
        .any(|o| o.status == ObligationStatus::Failed)
}

/// Store `cert` under `key` in `cache_dir` (REQ-3/REQ-6), persisting the
/// canonical fresh-verify form: `cached` is forced to `false` before writing, so
/// a future `load` + `with_cached(true)` hit is oracle-equal to this fresh
/// verify (REQ-2/REQ-7 — provenance is set at serve time, never baked into the
/// stored verdict).
///
/// The write is atomic: serialize to a sibling temp file, then rename over the
/// final path, so a concurrent `load` never observes a half-written entry (and a
/// crash mid-write leaves either the old entry or nothing, never a corrupt one).
/// An IO failure (including a missing cache dir that cannot be created) is
/// returned as an [`std::io::Error`] for the caller to degrade on — a cache that
/// cannot be written must not fail the verification (`check::check_file` ignores
/// the result: the verdict already stands, the cache is best-effort).
pub fn store(cache_dir: &Path, key: &str, cert: &Certificate) -> std::io::Result<()> {
    std::fs::create_dir_all(cache_dir)?;
    let canonical = cert.clone().with_cached(false);
    let json = serde_json::to_string_pretty(&canonical)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    // Atomic publish: write a unique temp sibling, then rename over the target.
    let tmp = temp_sibling(cache_dir, key);
    std::fs::write(&tmp, json.as_bytes())?;
    match std::fs::rename(&tmp, entry_path(cache_dir, key)) {
        Ok(()) => Ok(()),
        Err(e) => {
            // Best-effort cleanup of the orphaned temp; surface the rename error.
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

/// A unique temp-sibling path for an atomic `store` (REQ-6). Uniqueness uses the
/// process id + a monotonic counter, not wall-clock, so concurrent stores of
/// the same key do not collide on the temp file while staying R-CODE-5-clean
/// (determinism is a property of the stored certificate bytes, not the scratch
/// path; mirrors `check::unique_temp_path`).
fn temp_sibling(cache_dir: &Path, key: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    cache_dir.join(format!("{key}.{pid}.{n}.tmp"))
}

// ---- REQ-9 `dec wf` accessibility-proof cache (increment 3) -------------------------
//
// Stage-1 forge tier (`.design/stage1-forge-tier.md` REQ-9 / Q7 / AC-13): a `dec wf <rel>`
// termination measure carries an ACCESSIBILITY obligation — the relation must be
// well-founded on the recursion's carrier for the recursion to be admitted. That proof is
// expensive and re-derivable, so — exactly like the per-item proof cache above — it is
// content-addressed and cached by the (relation, carrier) pair, invalidated by the same
// `CHECK_SCHEMA_VERSION` gate-set key (a gate change ⇒ a new key ⇒ a miss ⇒ a re-check). A
// `dec wf` re-check on an unchanged (relation, carrier) HITS the cache (AC-13), skipping the
// re-derivation — observable through [`load_accessibility`] returning the stored proof.

/// Domain-separation tag for the accessibility cache key (REQ-9), distinct from the
/// per-item proof cache's `DOMAIN`, so an accessibility key never collides with a proof
/// key over the same bytes.
const DOMAIN_ACCESSIBILITY: &[u8] = b"thermite.forge.wf-accessibility.v1";

/// A cached `dec wf` accessibility proof (REQ-9 / Q7 / AC-13): the well-founded relation,
/// the carrier it ranges over, and the discharged verdict (whether the relation was proved
/// well-founded on the carrier — the accessibility obligation that admits the recursion).
/// Content-addressed by [`accessibility_cache_key`] over (relation, carrier), schema-keyed
/// like a [`Certificate`]. Additive + cache-only: never part of any cert's oracle subset.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AccessibilityProof {
    /// The well-founded relation (the normalized `dec wf <rel>` relation text).
    pub relation: String,
    /// The carrier the relation ranges over — the recursing parameter's type (the
    /// rendered type string), so the same relation over different carriers caches apart.
    pub carrier: String,
    /// The discharged accessibility verdict: `true` iff the relation was proved
    /// well-founded on the carrier (the recursion is admitted). A `false` records a
    /// settled non-accessibility (still cacheable — the obligation was decided).
    pub well_founded: bool,
    /// Whether this proof was served from the cache (REQ-9, mirroring `Certificate::cached`):
    /// `true` on a hit, `false` on a fresh derivation. Set at serve time
    /// ([`load_accessibility`] flags the hit); the stored form is canonical `false`.
    #[serde(default)]
    pub cached: bool,
}

impl AccessibilityProof {
    /// A fresh accessibility proof for `(relation, carrier)` with verdict `well_founded`
    /// (REQ-9). `cached` is `false` (a fresh derivation); a cache hit sets it `true` at
    /// serve time.
    #[must_use]
    pub fn new(
        relation: impl Into<String>,
        carrier: impl Into<String>,
        well_founded: bool,
    ) -> Self {
        AccessibilityProof {
            relation: relation.into(),
            carrier: carrier.into(),
            well_founded,
            cached: false,
        }
    }

    /// Return this proof flagged as cache-served (REQ-9): the serve-time provenance flip,
    /// mirroring `Certificate::with_cached`.
    #[must_use]
    pub fn with_cached(mut self, cached: bool) -> Self {
        self.cached = cached;
        self
    }
}

/// The content-address cache key for a `dec wf` accessibility proof (REQ-9 / Q7): a
/// lowercase-hex sha256 over the (relation, carrier) pair, domain-tagged + length-prefixed
/// like [`cache_key`], plus the `CHECK_SCHEMA_VERSION` gate-set input (so an accessibility
/// proof cached under one gate set is not re-served once the gate set changes — the same
/// soundness guard the per-item cache uses). Pure (R-CODE-5): identical (relation, carrier)
/// ⇒ identical key.
#[must_use]
pub fn accessibility_cache_key(relation: &str, carrier: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(DOMAIN_ACCESSIBILITY);
    field(&mut hasher, b"relation", relation.as_bytes());
    field(&mut hasher, b"carrier", carrier.as_bytes());
    field(
        &mut hasher,
        b"check-schema",
        &CHECK_SCHEMA_VERSION.to_le_bytes(),
    );
    hex_lower(&hasher.finalize())
}

/// The on-disk path for one accessibility cache entry (REQ-9): `<cache_dir>/wf-<key>.json`.
/// The `wf-` prefix keeps the accessibility namespace visibly distinct from the per-item
/// proof entries (`<key>.json`) in the same directory.
fn accessibility_entry_path(cache_dir: &Path, key: &str) -> PathBuf {
    cache_dir.join(format!("wf-{key}.json"))
}

/// Look up a cached [`AccessibilityProof`] by `key` under `cache_dir` (REQ-9 / AC-13).
/// Returns `Some(proof)` (flagged `cached: true`) on a hit, `None` on a miss. A miss
/// includes no file, an unreadable file, and a corrupt/unparseable file — a damaged cache
/// degrades to re-derive, never to an error or a stale read (R-CODE-2), exactly like
/// [`load`].
#[must_use]
pub fn load_accessibility(cache_dir: &Path, key: &str) -> Option<AccessibilityProof> {
    let path = accessibility_entry_path(cache_dir, key);
    let src = std::fs::read_to_string(&path).ok()?;
    let proof = serde_json::from_str::<AccessibilityProof>(&src).ok()?;
    // Provenance is set at serve time: the stored form is canonical `cached: false`, the
    // served hit is `cached: true` (so a re-check can observe the hit, AC-13).
    Some(proof.with_cached(true))
}

/// Store `proof` under `key` in `cache_dir` (REQ-9 / AC-13), persisting the canonical
/// `cached: false` form so a future [`load_accessibility`] hit is equal to a fresh derive
/// but for the serve-time `cached` flag. Atomic publish (temp sibling + rename), mirroring
/// [`store`]; an IO failure is returned for the caller to degrade on (the cache is
/// best-effort — a derivation that cannot be cached is still valid).
pub fn store_accessibility(
    cache_dir: &Path,
    key: &str,
    proof: &AccessibilityProof,
) -> std::io::Result<()> {
    std::fs::create_dir_all(cache_dir)?;
    let canonical = proof.clone().with_cached(false);
    let json = serde_json::to_string_pretty(&canonical)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let tmp = temp_sibling(cache_dir, &format!("wf-{key}"));
    std::fs::write(&tmp, json.as_bytes())?;
    match std::fs::rename(&tmp, accessibility_entry_path(cache_dir, key)) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{Certificate, Level, ObligationResult};

    const VERUS: &str = "verus 0.2024.01.01";
    const THERMITE: &str = "0.1.0";

    fn sample_cert(item: &str, level: Level) -> Certificate {
        Certificate::new(
            item,
            level,
            vec!["pure".to_string()],
            612,
            vec![ObligationResult::discharged(format!(
                "{item}_check::{item}"
            ))],
        )
    }

    fn unique_test_dir(tag: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static C: AtomicU64 = AtomicU64::new(0);
        let n = C.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "forge_cache_test_{}_{}_{}",
            tag,
            std::process::id(),
            n
        ))
    }

    // AC-4 / REQ-8: the key is a pure function of its four inputs — same inputs,
    // same hex key.
    #[test]
    fn cache_key_is_pure() {
        let a = cache_key("fn f() {}", 0, VERUS, THERMITE);
        let b = cache_key("fn f() {}", 0, VERUS, THERMITE);
        assert_eq!(a, b, "same inputs must yield the same key");
        // The key is lowercase hex of a 32-byte sha256 digest.
        assert_eq!(a.len(), 64, "sha256 hex is 64 chars");
        assert!(
            a.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
            "key must be lowercase hex: {a}"
        );
    }

    // AC-2 / REQ-1 / REQ-5: changing any single input changes the key (the
    // completeness side of the soundness invariant). Each perturbation is a
    // single-input change from the same baseline.
    #[test]
    fn key_changes_when_any_input_changes() {
        let base = cache_key("fn f() {}", 0, VERUS, THERMITE);
        // (a) lowered source.
        assert_ne!(base, cache_key("fn g() {}", 0, VERUS, THERMITE));
        // (b) seed.
        assert_ne!(base, cache_key("fn f() {}", 1, VERUS, THERMITE));
        // (c) thermite version.
        assert_ne!(base, cache_key("fn f() {}", 0, VERUS, "0.2.0"));
        // (d) verus version.
        assert_ne!(
            base,
            cache_key("fn f() {}", 0, "verus 0.2024.02.02", THERMITE)
        );
    }

    // REQ-1: domain-tagged length-prefixing prevents a concatenation collision.
    // Moving the boundary between two adjacent fields yields a different key
    // (a flat concatenation would collide here).
    #[test]
    fn length_prefixing_prevents_boundary_collision() {
        // ("ab","") vs ("a","b") on (source, verus_version): a flat concat of the
        // bytes would be identical; length-prefixing keeps them distinct.
        let x = cache_key("ab", 0, "", THERMITE);
        let y = cache_key("a", 0, "b", THERMITE);
        assert_ne!(
            x, y,
            "field boundaries must be unambiguous (no concat collision)"
        );
    }

    // REQ-3 / REQ-6 / AC-4: a stored cert round-trips through load on its
    // deterministic fields, and the stored form is the canonical `cached: false`.
    #[test]
    fn round_trip_load_store() {
        let dir = unique_test_dir("roundtrip");
        let _ = std::fs::remove_dir_all(&dir);
        let key = cache_key("fn f() {}", 0, VERUS, THERMITE);
        // A miss before any store.
        assert!(load(&dir, &key).is_none(), "empty cache is a MISS");
        // Store a hit-flagged cert; the stored form must be canonical false.
        let cert = sample_cert("f", Level::L3).with_cached(true);
        store(&dir, &key, &cert).expect("store");
        let loaded = load(&dir, &key).expect("HIT after store");
        assert_eq!(
            loaded.oracle_subset(),
            cert.oracle_subset(),
            "oracle fields round-trip"
        );
        assert!(
            !loaded.cached,
            "stored cert is canonical fresh-verify (cached:false); provenance is set at serve time"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    // REQ-6: a corrupt/unparseable entry is a miss, never an error and never a
    // stale read.
    #[test]
    fn corrupt_entry_is_a_miss() {
        let dir = unique_test_dir("corrupt");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        let key = cache_key("fn f() {}", 0, VERUS, THERMITE);
        std::fs::write(entry_path(&dir, &key), b"{ this is not valid json").expect("write garbage");
        assert!(
            load(&dir, &key).is_none(),
            "a corrupt entry degrades to a MISS"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    // REQ-6: the default cache dir is under the git-ignored `target/`.
    #[test]
    fn default_cache_dir_is_under_target() {
        let dir = default_cache_dir();
        assert!(
            dir.starts_with("target"),
            "the proof cache lives under the ignored `target/`: {dir:?}"
        );
        assert!(dir.ends_with("thermite-proof-cache"));
    }

    // ---- REQ-9 `dec wf` accessibility cache (increment 3) ----------------------------

    // REQ-9 / Q7: the accessibility key is a pure function of (relation, carrier); the same
    // pair yields the same 64-char lowercase-hex key, a different pair a different key.
    #[test]
    fn accessibility_key_is_pure_and_pair_sensitive() {
        let a = accessibility_cache_key("lt", "u32");
        assert_eq!(
            a,
            accessibility_cache_key("lt", "u32"),
            "pure: same pair, same key"
        );
        assert_eq!(a.len(), 64);
        assert!(a
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
        // Either component changing changes the key.
        assert_ne!(
            a,
            accessibility_cache_key("gt", "u32"),
            "relation participates"
        );
        assert_ne!(
            a,
            accessibility_cache_key("lt", "u64"),
            "carrier participates"
        );
        // The accessibility key never collides with a same-text proof key (distinct domain).
        assert_ne!(a, cache_key("lt", 0, "u32", "0.1.0"));
    }

    // REQ-9 / AC-13: a `dec wf` re-check HITS the cache — a stored accessibility proof is
    // served (flagged `cached: true`) on the second look, observable via the cache layer;
    // the stored form is canonical `cached: false`.
    #[test]
    fn accessibility_recheck_hits_the_cache() {
        let dir = unique_test_dir("wf_hit");
        let _ = std::fs::remove_dir_all(&dir);
        let key = accessibility_cache_key("lt", "u32");
        // A miss before any store (the first check derives).
        assert!(
            load_accessibility(&dir, &key).is_none(),
            "empty cache is a MISS"
        );
        let proof = AccessibilityProof::new("lt", "u32", true);
        store_accessibility(&dir, &key, &proof).expect("store");
        // The re-check HITS — same (relation, carrier), served from the cache.
        let hit = load_accessibility(&dir, &key).expect("HIT after store");
        assert_eq!(hit.relation, "lt");
        assert_eq!(hit.carrier, "u32");
        assert!(hit.well_founded);
        assert!(
            hit.cached,
            "a served hit is flagged cached:true (observable re-check)"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    // REQ-9: the accessibility cache is invalidated by the same `CHECK_SCHEMA_VERSION` gate
    // key as the per-item cache — a schema bump changes the key, so a proof cached under the
    // old schema is a MISS under the new (no stale accessibility past a gate change).
    #[test]
    fn accessibility_is_schema_invalidated() {
        // The schema input is folded into the key, so a different schema ⇒ a different key.
        // Re-derive the key bytes with the schema participating: two different relations
        // sharing a schema differ, and the same pair across schemas would differ — pinned
        // structurally by `accessibility_cache_key` feeding `CHECK_SCHEMA_VERSION`.
        let k = accessibility_cache_key("lt", "u32");
        // A hand-built key WITHOUT the schema field must differ from the real key (proving
        // the schema participates — the invalidation lever).
        let mut bare = Sha256::new();
        bare.update(DOMAIN_ACCESSIBILITY);
        field(&mut bare, b"relation", b"lt");
        field(&mut bare, b"carrier", b"u32");
        let bare = hex_lower(&bare.finalize());
        assert_ne!(
            k, bare,
            "the schema version is a cache-key input (invalidation lever)"
        );
    }

    // REQ-9 / R-CODE-2: a corrupt accessibility entry degrades to a MISS, never a stale read.
    #[test]
    fn corrupt_accessibility_entry_is_a_miss() {
        let dir = unique_test_dir("wf_corrupt");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        let key = accessibility_cache_key("lt", "u32");
        std::fs::write(accessibility_entry_path(&dir, &key), b"{ not json").expect("write garbage");
        assert!(
            load_accessibility(&dir, &key).is_none(),
            "corrupt entry is a MISS"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
