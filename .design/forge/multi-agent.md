# Multi-Agent Forge Sessions
<!--
tier: 3-component
status: draft
governs: forge/src/session.rs  (NOT YET CREATED — greenfield; see "Route" below)
also-hardens: forge/src/cache.rs  (the concurrency-safe primitives this contract demonstrates)
thesis-refs:
  - thermite-design.md §9   (composition — trust invariant under composition)
  - thermite-design.md §1.5 (locality — an edit's blast radius is its block)
  - thermite-design.md §5.3 (content-addressed per-item cache — an edit to f cannot invalidate g's certificate unless g references f)
  - thermite-design.md §13  (v0.5 — multi-agent Forge sessions)
crosslink: #20 (the final kernel/roadmap issue; v0.5). builds on #8 (per-item proof cache).
status-note: ALL REQs NOT-STARTED, blocked on #20. The cache PRIMITIVES this contract
  rests on (atomic store, miss-on-torn load, per-item locality keys) ALREADY ship under #8;
  what is NOT-STARTED is the multi-agent SESSION MODEL — the concurrency oracle, the
  demonstration tests, and (optionally) a `forge session` surface — that turns those
  primitives into a stated, tested multi-agent guarantee.
-->

## Summary

A **multi-agent Forge session** is N agents each running `forge check` / `forge repair`
on their own items against a **shared project proof cache** (`target/thermite-proof-cache/`),
with **no central coordinator**. The filesystem cache (concurrency-safe) plus
content-addressed per-item locality (§5.3) *is* the coordination: concurrent `forge`
processes never corrupt the cache nor read a torn entry, and an edit by agent A to item
`f` never invalidates agent B's cached certificate for an unrelated `g`. This component
states and *mechanically demonstrates* that guarantee — the v0.5 §13 deliverable that makes
Forge safe for unsupervised agents building one project in parallel (§9: "the property
that matters once unsupervised agents start building large systems").

## Scope and boundaries

IN (#20):
1. **Concurrency-safe proof cache** — concurrent `forge` *processes* never corrupt an
   entry, never read a torn/partial one, never crash on a damaged cache; concurrent
   writers of the *same* key converge (content-addressed: both wrote the same bytes);
   concurrent writers of *different* keys never interfere.
2. **Multi-agent locality** — agent A editing `f` changes only `f`'s content-addressed
   key; agent B's cached cert for an unrelated `g` (whose lowered sub-program is unchanged)
   stays a HIT. No cross-invalidation, no cross-eviction.
3. **The session model** — the *semantics* of concurrent `forge` invocations over a shared
   cache, with no central daemon. Optionally a thin read-only `forge session` status surface
   (see OQ-1) — DECIDED minimal deliverable below.

OUT:
- Central agent-assignment / work-partition orchestration — that is **crosslink-level**, not
  forge (a daemon that hands items to agents is out of the toolchain's scope).
- Composition-verification of boundary callers — that is **#52** (the §9 caller-contract
  check), already partly served by `closure::classify` (#17). This doc consumes the §9
  *locality* consequence, not the caller-verification machinery.
- Any change to a *verdict*. The session model is a SAFETY + PERFORMANCE property over the
  existing per-item verdict pipeline; a concurrent run produces the same per-item
  certificates a serial run would (R-CODE-5 determinism), never a different one.

## Requirements

- **REQ-1 (atomic cache store).** `cache::store` must publish an entry **atomically** —
  serialize to a unique temp sibling, then `rename` over the final path — so a concurrent
  `cache::load` in another `forge` process never observes a half-written `<key>.json`, and a
  crash mid-write leaves either the prior entry or nothing, never a corrupt one. Derived from
  §5.3 (the cache is a correctness-preserving optimization) + `goal.md` R-CODE-2 (no
  corruption / no panic under the IO path).
- **REQ-2 (torn/corrupt load → MISS, never crash, never wrong verdict).** `cache::load` must
  treat an absent, unreadable, partial, or unparseable entry as a **MISS** (return `None`),
  degrading to a fresh re-verify — never an `Err`, never a panic, never a stale/torn read
  served as a verdict. Derived from §5.3 + R-CODE-2 + the #8/#49 load-time soundness
  precedent.
- **REQ-3 (concurrent same-key convergence).** Concurrent stores of the **same** key from
  different `forge` processes converge to a single consistent entry. Because the key is the
  content address of a deterministic verdict (§5.3, R-CODE-5), every concurrent writer
  serializes the *same* certificate bytes; whichever atomic `rename` lands last wins, and the
  winner is byte-equal to every loser. No torn merge is possible (a `rename` is all-or-nothing).
  Derived from §5.3 + REQ-1 + the content-addressed key (`cache::cache_key`).
- **REQ-4 (concurrent different-key non-interference).** Concurrent stores/loads of
  **different** keys never interfere: each entry is a distinct `<key>.json` file with a
  per-(pid, counter) temp sibling, so two `forge` processes checking different items touch
  disjoint paths. Derived from §1.5 (an edit's blast radius is its block) + REQ-1's
  per-key temp naming.
- **REQ-5 (multi-agent locality — no cross-invalidation).** An edit by one agent to item `f`
  changes **only** `f`'s cache key (re-verify `f`); the cache key of any unrelated item `g`
  — whose lowered sub-program is byte-unchanged — is byte-identical, so `g`'s cached cert
  stays a HIT. Cross-invalidation occurs **only** when `g`'s contract references `f`'s
  contract (then `g`'s lowered sub-program contains `f`'s contract and its key moves). Derived
  from §5.3 ("an edit to `f` cannot invalidate `g`'s certificate unless `g`'s contract
  references `f`'s contract") + §1.5 + §9 (composition independence).
- **REQ-6 (session semantics — no central coordinator).** A multi-agent session is *defined*
  as N independent `forge` invocations over the shared `target/thermite-proof-cache/`
  (resolved by `check::resolve_cache_dir`, overridable via `FORGE_CACHE_DIR`). The
  coordination substrate is the filesystem cache (REQ-1..4) + content-addressed locality
  (REQ-5); there is no forge daemon, no lock server, no agent registry. Concurrent
  invocations are safe by construction, and results are shared via the cache (a cert one
  agent stored is a HIT for the next agent to check the same lowered item). Derived from §13
  (v0.5 multi-agent sessions) + §5.3.
- **REQ-7 (determinism under concurrency — concurrent == serial).** The per-item certificate
  an agent observes is a pure function of the item's lowered sub-program + the pinned seed +
  the toolchain/verus versions + the check-schema (`cache::cache_key`'s inputs), independent
  of interleaving. N concurrent `forge check` runs over the corpus produce, per item, the
  *same oracle-stable certificate* a single serial run produces (`goal.md` R-CODE-5;
  `manifest::Certificate::oracle_subset` excludes `solver_time_ms` and `cached`). Derived
  from §5.3 + R-CODE-5.

## Acceptance criteria

Each AC is discharged by a `forge`-crate concurrency test (the builder/orchestrator authors
the test under `forge/tests/` or `forge/src/session.rs` `#[cfg(test)]`) and, where a corpus
oracle is the external truth, by a new `conformance/concurrency/` fixture set. Tests use a
per-run temp cache via `FORGE_CACHE_DIR` (the existing hermetic seam in
`check::resolve_cache_dir`) so they never pollute the shared `target/` cache and are
order-independent.

- **AC-1 (N concurrent checks → correct certs, uncorrupted cache).** Spawn N (≥ 8) concurrent
  `forge check` invocations over the corpus (`conformance/sum.th`, `conformance/binary_search.th`)
  sharing one `FORGE_CACHE_DIR`. After all join: every emitted certificate matches its golden
  (`conformance/sum.cert.json` and the `binary_search` oracle) under `oracle_subset`; every
  `<key>.json` in the cache dir is fully parseable (no torn entry); no process panicked or
  errored on the cache path. Discharges REQ-1, REQ-2, REQ-7.
- **AC-2 (concurrent same-key convergence).** Spawn N concurrent `cache::store` of the *same*
  key with byte-equal certs (the same lowered item) from threads/processes; afterward
  `cache::load` returns a single consistent cert byte-equal to the input under
  `oracle_subset`, and the cache dir contains exactly one `<key>.json` plus zero leftover
  `.tmp` files. Discharges REQ-1, REQ-3.
- **AC-3 (concurrent different-key non-interference).** Spawn N concurrent `cache::store` of
  *distinct* keys; afterward each key loads back its own cert (no entry clobbers another), and
  the dir contains exactly N entries. Discharges REQ-4.
- **AC-4 (torn/partial entry → MISS, never crash).** Write a truncated/partial `<key>.json`
  (simulating an interrupted non-atomic write) into the cache dir; `cache::load` returns
  `None` (a MISS), not an `Err` and not a panic; a subsequent `forge check` re-verifies and
  overwrites it atomically. Discharges REQ-2 (extends `cache::tests::corrupt_entry_is_a_miss`
  to the *torn* shape).
- **AC-5 (multi-agent locality — edit to A does not move B's key).** For two distinct corpus
  items, compute `cache::cache_key` for item B; mutate item A's body (a real edit) and
  recompute B's key from B's unchanged lowered sub-program — the two B keys are byte-identical
  (`assert_eq`). Negative control: when B's contract references A's contract, editing A's
  *contract* DOES move B's key. Discharges REQ-5. (Strengthens the existing
  `check::tests::cache_key_is_local_to_the_item` to the multi-agent framing.)
- **AC-6 (no cross-eviction under concurrency).** Pre-populate the shared cache with B's cert;
  while a concurrent agent re-checks A (forcing A's store), B's `<key>.json` is never removed
  or overwritten, and a `cache::load` of B's key remains a HIT byte-equal to the
  pre-populated cert throughout. Discharges REQ-5, REQ-4.
- **AC-7 (determinism — concurrent == serial).** The certificate set from AC-1's N concurrent
  runs equals, per item under `oracle_subset`, the set from a single serial `forge check` of
  the same corpus. Discharges REQ-7.

## Architecture

### The session is the shared cache + locality, not a daemon

There is no session object in v0.1's running state — a "session" is a *property* of the
filesystem. Each agent runs an ordinary `forge check` (`check::check_file` →
`check::check_file_with_options`). That pipeline already:

- resolves a single shared cache dir per run (`resolve_cache_dir in check.rs`,
  `target/thermite-proof-cache/` via `default_cache_dir in cache.rs`, overridable with
  `FORGE_CACHE_DIR`); and
- per item (§5.3), computes a content-address key over the item's **own** isolated lowered
  sub-program (`item_subprogram in check.rs` → `thermite_lower::lower`), consults the cache
  (`load in cache.rs`) before spawning verus, and stores the verdict (`store in cache.rs`)
  on a miss.

This is the entire coordination mechanism. Two agents checking the **same** lowered item read
each other's stored verdict (a HIT, verus skipped); two agents checking **different** items
never touch each other's entries. No daemon is needed because the verdict is a *pure function*
of the key inputs (§5.3, R-CODE-5), so a cache entry is authoritative regardless of which
process wrote it.

### Concurrency safety lives in `cache.rs`

The cache primitives are **already concurrency-shaped** under #8 — this doc states the
multi-agent contract over them and demands the demonstration tests that #20 owns:

- **Atomic publish** (`store in cache.rs`, REQ-1): `store` serializes to a unique temp
  sibling (`temp_sibling in cache.rs`, named by `process::id()` + an `AtomicU64` counter — NOT
  wall-clock, R-CODE-5) and then `std::fs::rename`s it over `<key>.json`. `rename` over the
  same filesystem is atomic, so a concurrent reader sees either the old entry or the new one,
  never a half-written file. On a `rename` error the orphan temp is cleaned up and the error
  surfaced for the caller to *degrade* on (`check::check_file` ignores the store result — the
  verdict already stands, R-CODE-2).
- **Miss-on-torn load** (`load in cache.rs`, REQ-2): `load` returns `None` for a missing,
  unreadable, or unparseable entry (`serde_json::from_str::<Certificate>(...).ok()?`), and
  additionally `None` for an internally-inconsistent entry (the #49 load-time soundness guard
  via `is_internally_consistent in cache.rs`). A torn/partial JSON file fails to parse →
  MISS → fresh re-verify. No panic, no `Err`, no stale read.
- **Per-key isolation** (REQ-4): each entry is `<key>.json` (`entry_path in cache.rs`); temp
  siblings carry the pid + counter, so concurrent stores of even the same key get distinct
  temp files and only collide at the final atomic `rename` (the convergence point, REQ-3).

### Why locality holds for the multi-agent case (§5.3 / §1.5 / §9)

`cache::cache_key` hashes exactly `(lowered_src, seed, verus_version, thermite_version)` plus
the check-schema version — and `lowered_src` is the item's **own** isolated sub-program from
`item_subprogram in check.rs`. For a `fn`, that sub-program is `[the file's spec fns] + this
fn` — it does **not** contain a sibling `fn`'s body. So:

- Editing `sum`'s body changes `sum`'s lowered sub-program → `sum`'s key moves → `sum` is
  re-verified. `binary_search`'s lowered sub-program is byte-unchanged → its key is identical
  → its cached cert stays a HIT. (Grounded: `conformance/binary_search.th` contains neither
  `sum` nor any `spec fn`, so its per-item sub-program is exactly `binary_search` — wholly
  independent of `sum.th`.)
- The **only** way A's edit can move B's key is if B's contract references A's contract — then
  A's contract lowers into B's sub-program and B's `lowered_src` (hence key) changes. This is
  exactly §5.3's stated exception and §9's composition rule: trust is invariant under
  composition because B keys on A's *contract*, not A's *body*. This is the multi-agent
  no-cross-invalidation guarantee, restated and tested for N concurrent agents.

### Minimal concrete deliverable (DECIDED)

The minimal #20 deliverable is **the stated guarantee + the demonstration tests** (REQ-1..7,
AC-1..7), hardening/verifying `cache.rs`'s already-atomic store and miss-on-torn load. A
`forge session` *command* is **not** required for the guarantee (concurrent `forge check`
invocations are already safe). A thin read-only `forge session status` surface (enumerate the
cache dir: entry count, total size, parseable/torn counts) is OPTIONAL and tracked as OQ-1; if
shipped, it lives in `forge/src/session.rs` and is read-only (it must not mutate or lock the
cache — that would reintroduce a coordinator). The session *semantics* (REQ-6) ship as
documentation + the demonstration tests regardless of whether the optional command ships.

## Verification

- `cargo test -p forge` — the concurrency suite (AC-1..7) under `forge/tests/concurrency.rs`
  or `forge/src/session.rs` `#[cfg(test)]`, spawning concurrent `forge check` *processes*
  (`std::process::Command` on the built `forge` binary) for AC-1/AC-7 and concurrent threads
  calling `cache::store`/`cache::load` for AC-2/AC-3/AC-6, all under a per-run
  `FORGE_CACHE_DIR` temp dir.
- `conformance/concurrency/` — the oracle fixtures (the corpus programs re-used + the expected
  per-item cert set), so AC-1/AC-7 assert against an EXTERNAL truth (the golden certs), never
  the toolchain's own output (`goal.md` R-CHAR-3). The expected certs trace to
  `conformance/sum.cert.json` and the `binary_search` oracle.
- The existing `cache.rs` tests (`corrupt_entry_is_a_miss`, `round_trip_load_store`) and
  `check.rs`'s `cache_key_is_local_to_the_item` are the seeds AC-4/AC-2/AC-5 extend.
- `cargo clippy -p forge --all-targets -- -D warnings` and `cargo fmt --check` (the gauntlet).

### Test approach (spawn concurrent forge processes / threads)

- **Process-level (AC-1, AC-7):** build `forge`, then spawn N `Command::new(forge_bin)
  .args(["check", "conformance/sum.th", "--json"]).env("FORGE_CACHE_DIR", tmp)` children,
  join them, and diff each child's emitted cert JSON against the golden under `oracle_subset`.
  Compare the concurrent set to a single serial run's set for AC-7.
- **Thread-level (AC-2, AC-3, AC-6):** spawn N `std::thread`s each calling
  `cache::store(&dir, &key, &cert)` (same key for AC-2, distinct keys for AC-3), join, then
  assert the loaded entries and the on-disk file count.
- **Fault injection (AC-4):** write a deliberately truncated `<key>.json` and assert
  `cache::load` returns `None`.

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (atomic cache store) | NOT-STARTED | open issue #20 (v0.5). The PRIMITIVE already ships — `pub fn store in cache.rs` writes a `temp_sibling` then `std::fs::rename`s over `entry_path` (atomic publish), consumed by `check::check_file_with_options`. What is NOT-STARTED is the multi-agent contract over it: no concurrency test asserts the atomicity holds across concurrent `forge` PROCESSES, and there is no `conformance/concurrency/` oracle. The stated guarantee is unverified for the N-agent case until #20's AC-1/AC-2 land. |
| REQ-2 (torn/corrupt load → MISS) | NOT-STARTED | open issue #20. The PRIMITIVE ships — `pub fn load in cache.rs` returns `None` on unreadable/unparseable/inconsistent entries (`is_internally_consistent in cache.rs`), tested by `cache::tests::corrupt_entry_is_a_miss`. NOT-STARTED: no test covers the *torn/partial-write* shape specifically under concurrency (AC-4), the multi-agent failure mode #20 must demonstrate. |
| REQ-3 (concurrent same-key convergence) | NOT-STARTED | open issue #20. No test spawns concurrent same-key `cache::store` and asserts convergence to one consistent entry (AC-2). The mechanism exists (`rename` is all-or-nothing over a content-addressed key) but the guarantee is unstated and untested. |
| REQ-4 (different-key non-interference) | NOT-STARTED | open issue #20. No test spawns concurrent distinct-key stores and asserts disjoint, non-clobbering entries (AC-3, AC-6). The mechanism exists (`entry_path in cache.rs` + per-pid `temp_sibling`) but is undemonstrated for the multi-agent case. |
| REQ-5 (multi-agent locality — no cross-invalidation) | NOT-STARTED | open issue #20. Per-item locality holds via `cache::cache_key` over `item_subprogram`'s own lowered source (single-item locality is tested by `check::tests::cache_key_is_local_to_the_item`), but the multi-agent framing — edit by agent A leaves agent B's key/HIT untouched, with the §9 contract-reference exception as a negative control — has no AC-5/AC-6 test and no session-level statement. |
| REQ-6 (session semantics — no central coordinator) | NOT-STARTED | open issue #20. There is no `forge/src/session.rs` (`ls forge/src/session.rs` → not found) and no documented session model in the toolchain. The semantics (concurrent invocations safe; shared cache; no daemon) are stated only here; #20 must land them as tested guarantees (AC-1) and, optionally (OQ-1), a read-only `forge session status` surface. |
| REQ-7 (determinism under concurrency — concurrent == serial) | NOT-STARTED | open issue #20. `cache::cache_key` is a pure function of its inputs (`cache::tests::cache_key_is_pure`) and the per-item verdict is interleaving-independent, but no test asserts that N concurrent `forge check` runs produce the same oracle-stable cert set as a serial run (AC-7). |

## Open questions

- **OQ-1 (is a `forge session` command needed?).** DECIDED minimal: no — concurrent
  `forge check` invocations are safe without any new command, so the guarantee + tests are the
  deliverable. OPEN: whether to ship a thin **read-only** `forge session status` (enumerate
  the shared cache: entry count, total bytes, parseable vs torn) as an ergonomic surface. If
  shipped it MUST NOT lock or mutate the cache (that reintroduces a coordinator and breaks
  REQ-6). Surfaced for the orchestrator's call; not required for #20's core guarantee.
- **OQ-2 (route ownership).** Two viable shapes: (a) add a `forge/src/session.rs` route → this
  doc (needed if the optional `forge session status` command ships); or (b) if #20 is purely
  hardening + demonstrating `cache.rs`, no new source file is created and BOTH `cache.rs`'s
  existing `.design/forge/proof-cache.md` route AND this doc govern the concurrency contract
  (this doc is then a cross-cutting contract with no single `governs:` file). The route choice
  is the orchestrator's; the spec-discipline hook needs the route added before any
  `session.rs` edit. Leaning (b) for the minimal deliverable.
- **OQ-3 (same-filesystem assumption for atomic rename).** `std::fs::rename` is atomic only
  within one filesystem. The cache lives under `target/` (same FS as the project), and the
  temp sibling is created *in the cache dir* (`temp_sibling in cache.rs` joins `cache_dir`), so
  rename is same-dir, same-FS by construction — the assumption holds for the default and for a
  `FORGE_CACHE_DIR` pointing at a normal directory. A cache dir straddling a bind/overlay mount
  boundary is out of scope (R-CODE-2 still holds: a cross-FS rename error degrades to "uncached,"
  never a corrupt entry).

## Route

This doc's route is **not yet in `tooling/spec-routes.toml`** (it is the contract being
authored ahead of the builder). The orchestrator adds, per OQ-2:
- minimal/hardening (OQ-2 b): no new route file; this doc + `proof-cache.md` jointly govern
  the concurrency hardening of `forge/src/cache.rs`; OR
- if the optional command ships (OQ-2 a):
  `crate_pattern = "forge/src/session.rs"`, `design = ".design/forge/multi-agent.md"`,
  `reference = ["conformance/concurrency"]`, `conformance_ops = ["sum", "binary_search"]`.
