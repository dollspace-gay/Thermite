//! `forge/src/cache.rs` — the per-item, content-addressed proof cache and the
//! home of the bit-reproducible-verification contract (`thermite-design.md`
//! §5.3: "Proof results are content-addressed and cached per item").
//!
//! For each `.th` item, `check::check_file` computes a STABLE cache key from the
//! four inputs that determine that item's verdict — the item's LOWERED Verus
//! source, the pinned solver seed, the verus version, and the thermite toolchain
//! version — consults the cache BEFORE spawning verus, returns the stored
//! [`Certificate`] on a HIT (skipping the solver), and stores the result on a
//! MISS. The cache is a PERFORMANCE optimization that NEVER changes a verdict: a
//! hit is indistinguishable from a fresh verify (`goal.md` R-DEFER-9 — no proof
//! cheats; the cache cannot fabricate a verdict).
//!
//! Governing design: `.design/forge/proof-cache.md`.
//!
//! This module is a thin, deterministic, content-addressed store with NO
//! verification logic of its own — it sits BETWEEN `check::item_subprogram` /
//! `thermite_lower::lower` (which produce the lowered source it content-addresses)
//! and `check::run_verus` (the solver invocation it lets `forge` skip on a hit).
//! IO failures DEGRADE to a MISS, never a panic (R-CODE-2): a damaged cache is
//! "slower," never "wrong" or "crashes."
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (cache-key composition — verdict-determining inputs) | SHIPPED | `pub fn cache_key(lowered_src, seed, verus_version, thermite_version) -> String` hashes the four args PLUS the `CHECK_SCHEMA_VERSION` check-logic version (blocker #49), each DOMAIN-TAGGED + LENGTH-PREFIXED (`field`), into a lowercase-hex sha256 content address. Consumer: `check::check_file`. |
//! | REQ-2 (soundness-completeness invariant — hit == fresh verify) | SHIPPED | `cache_key` captures every verdict-determining input (the four args); `store` clears `cached` before persisting and `load` returns the stored cert unchanged, so `check::check_file`'s `with_cached(true)` HIT is oracle-equal to the fresh verify it was stored from. Verified by `key_changes_when_any_input_changes` + `check::check_file`'s `with_cached` wiring. |
//! | REQ-3 (lookup-then-store flow, per item) | SHIPPED | `pub fn load(cache_dir, key) -> Option<Certificate>` consulted BEFORE `run_verus`; `pub fn store(cache_dir, key, cert)` after a MISS. Consumer: `check::check_file`'s per-item L3 path. |
//! | REQ-4 (locality — per-item) | SHIPPED | the key is over the item's OWN `item_subprogram` lowered source (`check.rs`), so an edit to a sibling leaves this item's key byte-identical. Verified by `check::tests::cache_key_is_local_to_the_item`. |
//! | REQ-5 (version-keyed invalidation) | SHIPPED | `verus_version` + `thermite_version` (`env!("CARGO_PKG_VERSION")`, sourced in `check.rs`) are key inputs; a version change forces a universal MISS. Verified by `key_changes_when_any_input_changes`. |
//! | REQ-6 (cache location + format — gitignore-able) | SHIPPED | `pub fn default_cache_dir() -> PathBuf` = `target/thermite-proof-cache/` (under the already-ignored `target/`); one `<hex-key>.json` per key; `store` writes atomically (temp + rename); a corrupt/unreadable entry → `load` returns `None` (MISS, never an error). Consumer: `check::check_file`. |
//! | REQ-7 (additive `cached: bool` field) | SHIPPED | `manifest::Certificate::cached` (`#[serde(default)]`, oracle-excluded); `store` persists `cached: false` (a stored cert is the canonical fresh verify), `check::check_file` sets `with_cached(true)` on the HIT it returns. |
//! | REQ-8 (bit-reproducible deterministic cert) | SHIPPED | `cache_key` is a PURE function of its four inputs (no wall-clock, no ambient state, R-CODE-5); `load`/`store` round-trip the cert's deterministic fields byte-for-byte. Verified by `cache_key_is_pure` + `round_trip_load_store`. |

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::manifest::{Certificate, ObligationStatus};

/// Domain-separation tag prefixed to the WHOLE keyed stream, so a `forge` proof
/// cache key can never collide with an unrelated sha256 use of the same bytes
/// (`.design/forge/proof-cache.md` REQ-1 — domain separation).
const DOMAIN: &[u8] = b"thermite.forge.proof-cache.v1";

/// The version of forge's VERDICT-AFFECTING CHECK LOGIC — the set of gates a
/// cached certificate was produced under (`.design/forge/proof-cache.md` REQ-2,
/// the soundness-completeness invariant). It is a FIFTH cache-key input
/// (domain-tagged + length-prefixed like the other four) so that a certificate
/// stored under one set of gates can NEVER be re-served once the gate set
/// changes: a different schema ⇒ a different key ⇒ a MISS ⇒ a full re-check
/// under the CURRENT gates. This closes the bypass where a cert cached by a
/// forge BEFORE a gate existed — under an IDENTICAL (lowered_src, seed,
/// verus_version, thermite_version) key, because `forge`'s crate version did not
/// move — was served on a HIT and skipped the now-required gate.
///
/// MAINTENANCE CONTRACT (blocker #49): BUMP this constant WHENEVER the set of
/// verdict-affecting checks/gates changes — a gate added, removed, or its
/// pass/fail semantics altered (e.g. the §7 mutation floor, the vacuity battery,
/// the triage rejects). The `thermite_version` input does NOT suffice: the
/// toolchain ships gate changes WITHOUT a crate-version bump (issue #12's
/// mutation gate landed at 0.1.0), so the check-logic version must move
/// independently. Forgetting to bump it re-opens the stale-verdict bypass; this
/// is the contract that prevents it.
///
/// History:
///   1 — pre-mutation-gate check logic (the original four-input key era).
///   2 — issue #12 §7 mutation floor added (blocker #49: invalidates every
///       pre-gate cert so a weak contract is re-checked through the gate).
///   3 — blocker #74: the §7 early-return mutant is now SYNTHESIZED for a
///       `Vec<T>` return (an empty-Vec `TVec<Suffix> { data: Vec::new() }`,
///       mirroring the #48 `&[]` slice synthesis), so a `Vec`-returning fn is
///       SCORED instead of 0/0-gated to `WeakContract`. This CHANGES the gate
///       verdict for `Vec`-return fns (a genuinely-proved `push_one` now
///       certifies L3 instead of the spurious mutation-gated L0), so every cert
///       stored under schema 2 MUST be re-checked under schema 3 — the
///       maintenance contract above (a gate-semantics change ⇒ bump, or a
///       stale L0 is served on an identical lowered-source key, REQ-2).
///   4 — blocker #80: the §7 early-return mutant is now SYNTHESIZED for a
///       `String` return (an empty `TString { data: Vec::new() }`, mirroring the
///       #74 empty-`Vec` synthesis) in `mutation::early_return_value`'s
///       `Type::String` arm, so a `String`-returning fn is SCORED instead of
///       0/0-gated to `WeakContract`. This CHANGES the gate verdict for
///       `String`-return fns (the genuinely-proved `join`/`concat` now certifies
///       L3 instead of the spurious mutation-gated L0), so every cert stored
///       under schema 3 MUST be re-checked under schema 4 — the maintenance
///       contract above (else `forge check` serves the stale L0 cached on an
///       identical lowered-source key, REQ-2: a HIT must equal a fresh verify).
const CHECK_SCHEMA_VERSION: u32 = 4;

/// The project-local proof-cache directory (`.design/forge/proof-cache.md`
/// REQ-6, OQ-1): `target/thermite-proof-cache/`. It is BUILD OUTPUT under the
/// already-git-ignored `target/`, so it is never committed and `cargo clean`
/// clears it. The path is relative to the current working directory (the project
/// root), matching where `target/` lives. Consumed by `check::check_file`.
pub fn default_cache_dir() -> PathBuf {
    PathBuf::from("target").join("thermite-proof-cache")
}

/// Compute the STABLE content-address cache key for ONE item (REQ-1) — a
/// lowercase-hex sha256 over EXACTLY the four verdict-determining inputs:
///
/// 1. `lowered_src` — the item's LOWERED Verus source (what verus actually
///    checks; the §5.3 isolated sub-program). REQ-1a.
/// 2. `seed` — the pinned SMT solver seed (`check::resolve_seed`, §5.3). REQ-1b.
/// 3. `verus_version` — the verus binary version (`verus --version`). REQ-1d/REQ-5.
/// 4. `thermite_version` — the `forge` toolchain version
///    (`env!("CARGO_PKG_VERSION")`). REQ-1c/REQ-5.
///
/// Each field is DOMAIN-TAGGED and LENGTH-PREFIXED (`field`), so two distinct
/// input tuples cannot collide by concatenation ambiguity — the hash is
/// injective on the structured tuple, not merely on a flat byte concatenation
/// (the soundness argument, REQ-2). This function is PURE: no wall-clock, no
/// environment beyond the explicitly-passed arguments (R-CODE-5). Identical
/// inputs ⇒ identical key; ANY differing input ⇒ a different key ⇒ a MISS ⇒
/// re-verify.
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
    // The FIFTH input (blocker #49): the verdict-affecting check-logic version, so
    // a cert cached under one set of gates cannot be re-served once the gate set
    // changes (a different schema ⇒ a different key ⇒ a MISS ⇒ re-check under the
    // CURRENT gates). Captures what `thermite_version` cannot — gate changes that
    // ship without a crate-version bump (see `CHECK_SCHEMA_VERSION`).
    field(
        &mut hasher,
        b"check-schema",
        &CHECK_SCHEMA_VERSION.to_le_bytes(),
    );
    let digest = hasher.finalize();
    hex_lower(&digest)
}

/// Feed one DOMAIN-TAGGED, LENGTH-PREFIXED field into the hasher (REQ-1). The
/// layout is `len(tag):u32-le || tag || len(value):u64-le || value`, so neither
/// the tag nor the value can be re-split into a different (tag, value) pair —
/// the boundaries are unambiguous. This is what makes the four-input hash
/// injective on the tuple (the no-collision-by-concatenation guarantee).
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
/// Returns `Some(cert)` on a HIT (a present, readable, parseable entry whose key
/// filename matches), and `None` on a MISS. A MISS includes: no file, an
/// unreadable file, and a CORRUPT/unparseable file — a damaged cache degrades to
/// "re-verify," NEVER to an error and NEVER to a stale read (REQ-6, R-CODE-2: no
/// panic on the IO error path). The returned cert carries whatever `cached`
/// value was stored (`store` persists `false`); `check::check_file` sets the
/// observable `cached: true` via `Certificate::with_cached` on the hit it serves.
pub fn load(cache_dir: &Path, key: &str) -> Option<Certificate> {
    let path = entry_path(cache_dir, key);
    let src = std::fs::read_to_string(&path).ok()?;
    // A corrupt/unparseable entry is a MISS (not an error): re-verify + overwrite.
    let cert = serde_json::from_str::<Certificate>(&src).ok()?;
    // An INTERNALLY-INCONSISTENT entry is also a MISS (blocker #49). A cert
    // produced under a DIFFERENT set of gates than the current `forge` — e.g.
    // stored by a forge BEFORE the §7 mutation floor existed — can land under the
    // same content-address as a current key (the gate change shipped without a
    // verdict-input change) and would otherwise be re-served on a HIT, BYPASSING
    // the now-required gate. The tell is self-contradiction: the stored cert
    // claims CLEAN (`reject: None`) yet still carries a FAILED obligation in its
    // own `obligations` array (the gate that failed it under the old logic). A
    // genuinely-clean cert produced by the current logic NEVER carries a failed
    // obligation while `reject` is `None`. Treating such an entry as a MISS forces
    // a full re-check under the CURRENT gates (REQ-2: a hit must equal a fresh
    // verify; `goal.md` R-DEFER-9: the cache cannot launder a stale clean verdict
    // past a gate). This is the load-time half of the soundness guard; the
    // `CHECK_SCHEMA_VERSION` cache-key input is the on-disk-key half.
    if is_internally_consistent(&cert) {
        Some(cert)
    } else {
        None
    }
}

/// A stored [`Certificate`] is internally consistent iff a CLEAN verdict
/// (`reject.is_none()`) carries NO failed obligation (blocker #49). A cert that
/// claims clean while still recording a failed obligation was produced under a
/// different gate set than the current `forge` (a stale verdict that predates a
/// gate); serving it would bypass that gate. The check is conservative — it only
/// rejects the self-contradictory shape, so every cert the current logic itself
/// stores (clean ⇒ all obligations discharged; rejected ⇒ `reject.is_some()`)
/// round-trips as a HIT (no regression to the warm-hit path, REQ-2/AC-1).
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
/// CANONICAL fresh-verify form: `cached` is forced to `false` before writing, so
/// a future `load` + `with_cached(true)` HIT is oracle-equal to this fresh
/// verify (REQ-2/REQ-7 — provenance is set at serve time, never baked into the
/// stored verdict).
///
/// The write is ATOMIC: serialize to a sibling temp file, then rename over the
/// final path, so a concurrent `load` never observes a half-written entry (and a
/// crash mid-write leaves either the old entry or nothing, never a corrupt one).
/// An IO failure (including a missing cache dir that cannot be created) is
/// returned as an [`std::io::Error`] for the caller to DEGRADE on — a cache that
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
/// process id + a monotonic counter — NOT wall-clock — so concurrent stores of
/// the SAME key do not collide on the temp file while staying R-CODE-5-clean
/// (determinism is a property of the stored CERTIFICATE bytes, not the scratch
/// path; mirrors `check::unique_temp_path`).
fn temp_sibling(cache_dir: &Path, key: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    cache_dir.join(format!("{key}.{pid}.{n}.tmp"))
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

    // AC-4 / REQ-8: the key is a PURE function of its four inputs — same inputs,
    // same hex key, deterministically.
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

    // AC-2 / REQ-1 / REQ-5: changing ANY single input changes the key (the
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

    // REQ-1: domain-tagged length-prefixing prevents a concatenation collision —
    // moving the boundary between two adjacent fields yields a DIFFERENT key
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
        // A MISS before any store.
        assert!(load(&dir, &key).is_none(), "empty cache is a MISS");
        // Store a HIT-flagged cert; the stored form must be canonical false.
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

    // REQ-6: a corrupt/unparseable entry is a MISS, never an error and never a
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
}
