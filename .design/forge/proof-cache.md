# Forge proof cache + bit-reproducible verification

<!--
tier: 3-component
status: draft
audited-sha: 1dc7c5492dea73d66e3d8ede3423e0339d741433 (bootstrap pin: decision 4 — doc-last-touch, NOT verified-current; backlog #262)
governs: forge/src/cache.rs
thesis-refs:
  - thermite-design.md §5.3
  - thermite-design.md §6
  - thermite-design.md §11
  - thermite-design.md §12
  - thermite-design.md Appendix A
-->

## Summary

`forge/src/cache.rs` is the per-item, content-addressed proof cache and the
home of the bit-reproducible-verification contract (`thermite-design.md` §5.3:
"Proof results are content-addressed and cached per item"). For each `.th`
item, `forge check` computes a STABLE cache key from everything that determines
the verdict (the item's lowered Verus source + the pinned solver seed + the
verus version + the thermite toolchain version), consults the cache BEFORE
spawning verus, returns the cached certificate on a HIT (skipping the solver),
and stores the result on a MISS. The cache is a PERFORMANCE optimization that
NEVER changes a verdict: a hit is indistinguishable from a fresh verify.

GREENFIELD — no `cache.rs` exists. `forge check` (issues #5/#6) ships and
already verifies PER ITEM (`check::check_file`'s per-item loop, `item_subprogram`,
`run_verus`, `resolve_seed`/`DEFAULT_SOLVER_SEED`). All REQs below are
NOT-STARTED, blocked on issue #8 (the last v0.1-kernel leaf).

## Requirements

- REQ-1 (cache-key composition — the four verdict-determining inputs): the
  cache key for one item is a STABLE sha256 hash over EXACTLY the four inputs
  that determine that item's verdict: (a) the item's LOWERED Verus source — the
  byte string `thermite_lower::lower(&sub)` produces for that item's isolated
  sub-program (`check::item_subprogram`), which is what verus actually checks;
  (b) the pinned solver seed (`check::resolve_seed` / `DEFAULT_SOLVER_SEED`,
  §5.3); (c) the verus version; (d) the thermite toolchain version. The hash is
  domain-separated and length-prefixed per field so no two distinct input tuples
  collide by concatenation ambiguity. The key is the cache's content address.
  Source: `thermite-design.md` §5.3 ("proof results are content-addressed and
  cached per item"); §12 ("certificates are per-item and content-addressed").
- REQ-2 (the soundness-completeness invariant — a hit equals a fresh verify):
  the key MUST capture everything that affects the proof outcome, so that any
  change to a verdict-determining input changes the key → MISS → re-verify. A
  cache HIT therefore returns the SAME verdict a fresh verus run would, by
  construction. A stale-cache false-L3 (returning L3 when a fresh run would not)
  is a SOUNDNESS HOLE; the completeness of the four-input key (REQ-1) is the
  argument that this cannot occur. The cache is never a way to fabricate a
  verdict (`goal.md` R-DEFER-9 — no proof cheats).
  Source: `thermite-design.md` §5.3 ("a proof that passed yesterday passes today
  unless something semantically relevant changed"); §11 ("never by weakening the
  gate"); `goal.md` R-DEFER-9.
- REQ-3 (lookup-then-store flow in `forge check`, per item): for each item,
  `check::check_file` consults the cache BEFORE `run_verus`: on a HIT it returns
  the stored certificate and does NOT spawn verus; on a MISS it runs verus,
  assembles the certificate, and STORES it under the key. The cache is consulted
  only on the L3 (verus) path — the `#[slag]` L1 short-circuit and a triage
  reject never reach verus (`check::gate_fn`), so they are not cached (no solver
  work to skip). A `spec fn` item, which has no contract obligation, is cached on
  the same lowered-source key as any other item.
  Source: `thermite-design.md` §5.3; the existing `check::check_file` per-item
  loop (`run_verus` is the cacheable seam).
- REQ-4 (locality — per-item, not per-crate): the key is computed from the
  item's OWN isolated lowered sub-program (`check::item_subprogram`: the item
  plus the file's `spec fn` dependencies), NOT the whole crate. So an edit to
  item `f` does not change item `g`'s lowered source → `g`'s key is unchanged →
  `g`'s cache entry stays valid (`thermite-design.md` §5.3: "an edit to `f`
  cannot invalidate `g`'s certificate unless `g`'s contract references `f`'s
  contract"). Because a contract reference to a `spec fn` IS part of `g`'s
  sub-program, editing a `spec fn` that `g` references DOES change `g`'s lowered
  source and correctly invalidates `g` — locality and soundness agree.
  Source: `thermite-design.md` §5.3 (locality / the per-item invalidation rule);
  §12 ("cross-item invalidation only through contract references").
- REQ-5 (version-keyed invalidation): the verus version and the thermite
  toolchain version are key inputs (REQ-1). A toolchain or verus upgrade changes
  the key for EVERY item → universal MISS → everything re-verifies under the new
  versions. The thermite version is sourced deterministically (the `forge` crate
  `CARGO_PKG_VERSION`); the verus version is sourced from the verus binary
  (`verus --version`) — captured once per run, never wall-clock-derived
  (R-CODE-5). A missing/unreadable verus version is an environment error, not a
  silent empty-string key.
  Source: `thermite-design.md` §5.3 ("given the same toolchain version and solver
  seeds"); `goal.md` R-CODE-5.
- REQ-6 (cache location + format — gitignore-able, never committed): cache
  entries live under a project-local cache DIRECTORY,
  `target/thermite-proof-cache/`, one JSON file per key (`<hex-key>.json`)
  holding the stored `Certificate`. The directory MUST be git-ignorable and is
  NOT committed (it is build output, like `target/`). A corrupt/unparseable entry
  is treated as a MISS (re-verify + overwrite), never an error and never a stale
  read. The cache add does NOT alter the golden cert or the conformance corpus.
  Source: `thermite-design.md` §11 ("mitigated by per-item caching"); §5.3;
  `goal.md` R-CODE-2 (no panics — IO failures degrade to a MISS, surfaced).
- REQ-7 (the additive `cached: bool` certificate field — observability): the
  `Certificate` schema (`manifest.rs`,
  `.design/forge/certificate-manifest.md`) gains an ADDITIVE `cached: bool`
  field (`#[serde(default)]`, defaulting `false`) recording whether THIS
  certificate came from a cache hit. It follows the #6 `slag_meta`/`reject`
  additive-field pattern: the frozen golden `conformance/sum.cert.json` (which
  omits it) still deserializes, and `cached` is EXCLUDED from the deterministic
  cert-oracle subset (`Certificate::oracle_subset`) — a hit and a fresh verify
  must be oracle-equal, so `cached` cannot be an oracle field (REQ-2). It exists
  so a hit is observable and the soundness test (AC-1) can assert it.
  Source: `thermite-design.md` §5.3; `goal.md` R-SPEC-2 (the cert schema is a
  contract; a field add is additive, not a rename); the #6 `slag_meta` precedent
  in `manifest.rs`.
- REQ-8 (bit-reproducible deterministic certificate): given the same lowered
  source + same toolchain + same seed, the certificate's DETERMINISTIC fields
  (`oracle_subset`: `item`, `level`, `effects`, `slag`, plus the per-obligation
  `status`/`name`/`location`) are byte-identical across runs, whether produced
  fresh or from the cache. The only non-deterministic field, `solver_time_ms`,
  is oracle-excluded already (`manifest::Certificate::oracle_subset`). On a HIT,
  `solver_time_ms` reports the STORED value (the cache does not re-measure
  wall-clock; reporting `0` is the alternative — see OQ-2) and `cached` is
  `true`; the deterministic fields match the stored cert exactly.
  Source: `thermite-design.md` §5.3 (bit-reproducible); `goal.md` R-CODE-5;
  `.design/forge/check.md` REQ-7 / `certificate-manifest.md` REQ-6
  (`solver_time_ms` excluded).

## Acceptance criteria

- AC-1 (cache HIT — same verdict, observable, solver skipped): running
  `forge check conformance/sum.th` twice — the first run populates the cache
  (`cached: false`), the second run is a HIT (`cached: true`) with a
  DETERMINISTICALLY EQUAL certificate (`oracle_subset` identical). The strongest
  form: after the first run populates the cache, a second run with the `verus`
  binary UNAVAILABLE (off `PATH`) STILL returns the cached L3 certificate — proof
  that the solver was skipped on the hit (if it were not, REQ-6 of
  `.design/forge/check.md` would make it `ForgeError::VerusAbsent`). This is the
  decisive solver-skip evidence.
- AC-2 (INVALIDATION — any key input change → MISS → re-verify): changing any of
  the four key inputs produces `cached: false` and a fresh verus run. Mechanically
  checkable per input: (a) edit the item's body/contract → different lowered
  source → MISS; (b) change the seed (`resolve_seed`) → MISS; (c) change the
  thermite version component → MISS; (d) change the verus version component →
  MISS. A unit test on the key function asserts each single-input perturbation
  changes the key.
- AC-3 (LOCALITY — editing `f` does not invalidate `g`): in a two-item file
  `f`,`g` (where `g`'s contract does not reference `f`), editing `f`'s body leaves
  `g`'s cache key byte-identical (because `g`'s `item_subprogram` lowered source
  is unchanged) — `g` stays a HIT while `f` becomes a MISS. A unit test computes
  `g`'s key before and after an `f`-only edit and asserts equality, and computes
  `f`'s key and asserts inequality.
- AC-4 (DETERMINISM — identical cert bytes): two fresh `forge check` runs over
  the same `.th` with the same toolchain + seed produce byte-identical
  certificates on the deterministic subset (excluding `solver_time_ms`); and a
  HIT's deterministic subset equals the stored MISS's. A unit test asserts the
  key function is a pure function of its four inputs (same inputs → same hex
  key).
- AC-5 (corpus still certifies; golden still deserializes): `forge check`
  on `conformance/sum.th` and `conformance/binary_search.th` still certify L3
  (the cache changes performance, not verdict — AC-1/REQ-2), and the golden
  `conformance/sum.cert.json` STILL deserializes into the `Certificate` struct
  with the new `cached` field (`#[serde(default)]`; the golden omits it). The
  cert-oracle deterministic-subset comparison is unchanged (`cached` and
  `solver_time_ms` excluded).

## Architecture

`forge/src/cache.rs` is a thin, deterministic, content-addressed store with no
verification logic of its own — it sits BETWEEN `check::item_subprogram` /
`thermite_lower::lower` (which produce the lowered source) and `check::run_verus`
(the solver invocation it lets `forge` skip on a hit). It depends on `sha2` for
the stable hash (see "Dependency note" below).

**The cache key (REQ-1).** The content address for one item is

```text
key = sha256(
        len-prefix( lowered_verus_source )  ||   // what verus checks (REQ-1a)
        len-prefix( seed_bytes )            ||   // pinned solver seed (REQ-1b, §5.3)
        len-prefix( verus_version )         ||   // REQ-1d / REQ-5
        len-prefix( thermite_version )           // REQ-1c / REQ-5
      )
```

Each field is length-prefixed (and domain-tagged) before hashing so that, e.g.,
two distinct (source, version) splits cannot produce the same byte stream — the
hash is injective on the structured tuple, not merely on a flat concatenation.
The key is rendered as lowercase hex for the on-disk filename. The function is a
PURE function of the four inputs: no wall-clock, no environment beyond the
explicitly-passed version strings (R-CODE-5).

**Why the key is COMPLETE — the soundness argument (REQ-2).** A cache hit must
be indistinguishable from a fresh verus run. The verdict for an item is a
deterministic function of exactly: what verus is asked to prove (the lowered
Verus source — the §5.3 isolated sub-program, including every `spec fn` the
item's contract references), the seed the SMT solver is pinned to
(`smt.random_seed`, §5.3), and the prover itself (the verus version, which fixes
Z3/the encoding) plus the thermite lowering that produced the source (the
thermite version). Nothing else influences the verdict: there is no wall-clock,
no un-seeded randomness (`goal.md` R-CODE-5), no ambient state. Therefore if all
four key inputs are equal, the verdict is equal, and the cached certificate is
exactly what a fresh run would produce. Conversely, if ANY of them differs, the
key differs and `forge` re-verifies. The argument's load-bearing claim is the
ENUMERATION: these four are ALL the verdict-determining inputs. The lowered
source is the right content-address (not the surface AST) precisely because it
is *what verus checks* — two surface programs that lower to the same Verus bytes
have the same verdict, and a surface change that does not alter the lowered bytes
cannot change the verdict (see OQ-3).

**The lookup/store flow (REQ-3).** Inside `check::check_file`'s existing per-item
loop, for an item on the L3 path (i.e. not a `#[slag]` L1 short-circuit and not a
triage reject — those never reach verus, so there is nothing to cache):

```text
sub      = item_subprogram(item, &spec_items)        // §5.3 isolated sub-program
lowered  = thermite_lower::lower(&sub)
key      = cache::key(&lowered, seed, &verus_version, &thermite_version)   // REQ-1
match cache::load(&key) {
  Some(stored) => stored.with_cached(true),          // HIT: skip verus (REQ-3, AC-1)
  None => {
    verus = run_verus(&sub, &lowered, seed)           // MISS: the solver runs
    cert  = assemble_certificate(item, &verus)
    cache::store(&key, &cert)                          // store for next time
    cert.with_cached(false)
  }
}
```

`cache.rs` exposes the boundary surface `pub fn key(...) -> String`,
`pub fn load(key) -> Option<Certificate>`, `pub fn store(key, cert)`; its sole
non-test production consumer is `check::check_file`. The verus-version string is
captured once per `check_file` invocation (`verus --version`, REQ-5) and the
thermite version is `env!("CARGO_PKG_VERSION")` of the `forge` crate.

**Cache location + format (REQ-6).** Entries live under
`target/thermite-proof-cache/<hex-key>.json`, each a `serde_json` serialization
of the stored `Certificate`. The directory is build output — git-ignorable and
NEVER committed (a `target/` entry already exists in workspace ignores; the
proof-cache dir is under `target/`, so it inherits the ignore — the builder
confirms/adds the `.gitignore` line). A corrupt or unparseable entry is a MISS:
`load` returns `None` (re-verify + overwrite on store), never an error and never
a stale read — a damaged cache degrades to "slower," never to "wrong" or
"crashes" (R-CODE-2: no panic; the IO error path returns `None`/`Ok(())`).

**The additive `cached` field (REQ-7).** `manifest::Certificate` gains
`#[serde(default)] pub cached: bool`, mirroring the #6 `slag_meta`/`reject`
additive precedent (`manifest.rs`): the frozen golden `sum.cert.json` (no
`cached` key) still deserializes (defaulting `false`), and `cached` is NOT in
`Certificate::oracle_subset` — because a hit and a fresh verify MUST compare
oracle-equal (REQ-2), `cached` describes provenance, never verdict. A
`Certificate::with_cached(bool)` builder sets it. `cli::render_human` may surface
it for observability; `--json` carries it for the soundness test.

**Determinism + version-keying (REQ-8, REQ-5).** The certificate is
bit-reproducible on its deterministic subset given identical (lowered source,
seed, verus version, thermite version) — the same tuple that forms the key. A
HIT returns the stored cert's deterministic fields unchanged and sets
`cached: true`; `solver_time_ms` reports the stored value (oracle-excluded
either way — see OQ-2). A verus or thermite upgrade changes the version inputs,
hence the key, hence forces a universal re-verify — the cache cannot serve a
certificate proved by a different prover than the one now installed (REQ-5,
guarding against a stale-prover false-L3).

**Boundaries (documented, attributed).**
- Compiled-BINARY bit-reproducibility (the rustc/Cargo machine-code output) is
  OUT of scope — that is the Rust toolchain's concern, not `forge`'s. Issue #8 is
  VERIFICATION determinism + the proof cache, not codegen reproducibility
  (`thermite-design.md` §5.3 names "builds, formatting, codegen, and check
  results" as a family, but the codegen-byte guarantee is inherited from rustc,
  not implemented here).
- Cross-run proof-repair / background re-verify (driving L1/L2 back to L3
  unattended) is issue #18 / `thermite-design.md` §5.2, §13 v0.5 (`forge repair`)
  — NOT this component. The cache stores and returns the verdict it was given; it
  does not attempt to improve a stored non-L3 result.
- The full L3→L2→L1 degrade ladder + solver portfolio (issue #10) is OUT — the
  cache keys on whatever verdict `run_verus` produced under v0.1's binary
  level logic (`.design/forge/check.md` REQ-5).

## Verification

- `cargo test -p forge` — unit tests in `cache.rs`:
  - key purity / determinism (AC-4): `key(inputs) == key(inputs)`.
  - key completeness / invalidation (AC-2): each single-input perturbation
    (lowered source, seed, verus version, thermite version) changes the key.
  - locality (AC-3): `g`'s key is invariant under an `f`-only edit; `f`'s key
    changes. Computed from two `item_subprogram` lowerings of a two-item program.
  - round-trip store/load of a `Certificate`; a corrupt entry loads as `None`
    (REQ-6).
  - `cached` additivity (REQ-7 / AC-5): a `Certificate` with `cached: true`
    serializes the field; the golden `conformance/sum.cert.json` still
    deserializes with `cached` defaulting `false`; `oracle_subset` ignores
    `cached`.
- Conformance integration (`goal.md` model (B); the `conformance` route
  reference): `forge check conformance/sum.th` twice → 1st `cached:false`,
  2nd `cached:true` with an oracle-equal cert (AC-1); the verus-unavailable
  HIT test (AC-1, decisive solver-skip evidence); `sum`/`binary_search` still
  certify L3 (AC-5). Expected verdicts trace to `conformance/sum.cert.json` /
  `thermite-design.md`, never copied from `forge`'s own output (R-CHAR-3).
- `cargo clippy -p forge --all-targets -- -D warnings`, `cargo fmt --check`,
  anti-pattern gate.

These conformance checks are the `goal.md` R-DEFER-6 gate that runs whenever a
commit touches `forge`.

**Dependency note.** This component adds `sha2 = "0.10"` to `forge/Cargo.toml`
(confirmed present in the cargo registry cache: `sha2-0.10.9`). The builder adds
the dependency; the doc-author does not edit `Cargo.toml`.

## Open questions

- OQ-1 (cache location): `target/thermite-proof-cache/` is the DECIDED location
  (build output, git-ignored via `target/`, project-local so a key collision
  across unrelated projects is impossible). The alternative `.thermite/cache/`
  (a dedicated dotdir, survives `cargo clean`) is recorded as the rejected
  option — `target/` was chosen so `cargo clean` clears the cache and the
  existing `target/` ignore covers it without a new `.gitignore` rule beyond a
  confirmation. Revisit if `forge` needs a cache that outlives `cargo clean`.
- OQ-2 (what `solver_time_ms` reports on a HIT): DECIDED — report the STORED
  value (the time the original proof took), not `0`. Rationale: it is
  oracle-excluded either way, so the choice does not affect any assertion; the
  stored value is the more honest datum ("this proof costs ~612ms when run") and
  `cached: true` already signals the time was not just incurred. `0` is the
  rejected alternative (it would understate the real proof cost). Either is
  sound; flagged because it is a visible field.
- OQ-3 (lowered-source hash vs surface-AST hash as the content address):
  DECIDED — hash the LOWERED Verus source, because that is *what verus checks*
  and therefore the true determinant of the verdict (REQ-1a / the soundness
  argument). A surface-AST hash would be UNSOUND if two distinct ASTs lower to
  identical Verus (they have the same verdict but different keys — merely a
  missed-hit, harmless) and, worse, risks being INCOMPLETE if the lowerer's
  output depends on anything not captured by the AST hash (e.g. a `spec fn`
  dependency pulled into the sub-program). Keying on the lowered bytes makes the
  key exactly track the verus input. This is the design's LEAST-settled call: it
  assumes `thermite_lower::lower` is itself deterministic given the AST
  (`.design/forge/check.md` REQ-7 asserts the pipeline is, and `lower` emits "in
  source order") — if that assumption ever fails, the cache key inherits the
  non-determinism and the invalidation tests (AC-4) would catch it.

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (cache-key composition) | SHIPPED | `cache::cache_key(lowered_src, seed, verus_version, thermite_version) -> String` (`cache.rs`) sha256-hashes the four inputs, each DOMAIN-tagged + LENGTH-prefixed (`cache::field`); `sha2 = "0.10"` added to `forge/Cargo.toml`. Consumer: `check::check_file` in `check.rs`. |
| REQ-2 (soundness-completeness invariant) | SHIPPED | the key captures all four verdict-determining inputs; `cache::store` persists the canonical `cached: false` and `cache::load` returns it unchanged, so `check::check_file`'s HIT (`Certificate::with_cached(true)`) is oracle-equal to the fresh verify. Verified by `cache::tests::key_changes_when_any_input_changes` + `cache_conformance::second_run_is_a_cache_hit_with_equal_deterministic_fields`. |
| REQ-3 (lookup-then-store flow, per item) | SHIPPED | `check::check_file`'s L3 path calls `cache::load` BEFORE `run_verus` (HIT → return + skip verus + `continue`); on a MISS it runs verus, assembles + graduates the cert, `cache::store`s it, and returns `with_cached(false)`. |
| REQ-4 (locality — per-item) | SHIPPED | the key is over the item's OWN `item_subprogram` lowered source; `check::tests::cache_key_is_local_to_the_item` asserts `g`'s key is invariant under an `f`-only edit while `f`'s key changes. |
| REQ-5 (version-keyed invalidation) | SHIPPED | `check::resolve_verus_version` captures the verus version once per run (the `VERUS_VERSION` pin, else `verus --version`; a missing version is `ForgeError::VerusAbsent`, never an empty-string key) and `check::THERMITE_VERSION = env!("CARGO_PKG_VERSION")` feed the key. Verified by `cache::tests::key_changes_when_any_input_changes`. |
| REQ-6 (cache location + format) | SHIPPED | `cache::default_cache_dir()` = `target/thermite-proof-cache/` (under the already-ignored `target/`); one `<hex-key>.json` per key; `cache::store` writes atomically (temp + rename); a corrupt entry → `cache::load` returns `None` (`cache::tests::corrupt_entry_is_a_miss`). |
| REQ-7 (additive `cached: bool` field) | SHIPPED | `manifest::Certificate::cached` (`#[serde(default)]`, EXCLUDED from `oracle_subset`); `Certificate::with_cached` builder; the golden `sum.cert.json` still deserializes (`manifest::tests::cached_field_is_additive_and_oracle_excluded`). |
| REQ-8 (bit-reproducible cert) | SHIPPED | `cache::cache_key` is a PURE function of its four inputs (`cache::tests::cache_key_is_pure`); `cache::store`/`load` round-trip the deterministic fields (`cache::tests::round_trip_load_store`). |
