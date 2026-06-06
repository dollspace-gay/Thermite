# Effect-Primitive Standard Library (Basis Stage 3)
<!--
tier: 3-component
status: draft
governs: thermite-stdlib/src/effect/read.rs
governs: thermite-stdlib/src/effect/write.rs
governs: thermite-stdlib/src/effect/time.rs
thesis-refs:
  - thermite-design.md §1
  - thermite-design.md §4.1
  - thermite-design.md §8
  - thermite-design.md §9
  - thermite-design.md §6
-->

## Summary

Stage 3 of the universal-verified-basis buildout (crosslink epic **#62**, issue
**#72**) is the **EFFECT half of "program anything, verified."** Stage 1
(`01-adts.md`) gives the DATA basis (every finite algebraic type); Stage 3 gives
the EFFECT basis: each atom of the §4.1 effect lattice (`Effect::Read`/`Write`/
`Net`/`Alloc`/`Time`/`Rand` in `enum Effect in ast.rs`) is instantiated as a
contracted, seccomp-confined **`#[boundary]` effect primitive** — a
verified-effect-primitive whose BODY is the real syscall (trusted by fiat, because
you cannot prove the kernel), whose CONTRACT states the assumed behavior of that
syscall, whose effect is TYPED (a `fx` row) and RUNTIME-SANDBOXED (the #57 seccomp
filter confines the primitive to exactly the syscalls its effect implies). The pure
logic that orchestrates these primitives is FULLY verified (L3); the trusted base
is exactly this small, enumerated, contracted, confined set.

This is the §1 trust-relocation thesis discharged for I/O: **"verify anything" =
"verify everything except this small, contracted, sandboxed, enumerated set."**

**Stage 3 invents NO new validator, lowering, or sandbox mechanism.** The
build-resolution pass (#62 / #72) GROUNDED this against the real toolchain
(`verus 0.2026.05.24`, `forge` built from this tree): every machine part the stdlib
needs is already SHIPPED, and the centerpiece path runs end-to-end TODAY. Stage 3's
deliverable is therefore (a) the `thermite-stdlib` crate of `#[boundary]` primitive
DECLARATIONS, (b) a `conformance/effect-stdlib` oracle pinning the COMPOSITION of
the shipped pieces, and (c) — the one open question this pass RESOLVES — a precise,
buildable v1 scope. See [v1 scope (PINNED)](#v1-scope-pinned) and the grounded
forge output throughout.

## v1 scope (PINNED — the #72 build-resolution pass resolved the three ambiguities)

The three BUILD ambiguities the prior draft left open are now resolved
EMPIRICALLY (real `forge` runs reproduced under [Grounding the full
path](#grounding-the-full-path-real-forge-output)):

### Resolution 1 — OUTCOME-COVERAGE is EMERGENT, not new validator code.

**No new validator rule is needed.** The honesty seam — "a boundary contract is
honest iff its outcome space is totally covered AND the caller resolves every arm"
— falls OUT of two already-SHIPPED mechanisms composing:

- **The boundary's contract is held to the SAME §7.1 structural triage as any fn,
  MINUS rule (d).** `vacuity::triage` (in `forge/src/vacuity.rs`) runs rules (a)
  `ens-is-trivial`, (b) `ens-omits-result`, (c) `ens-implied-by-req` on a
  `#[boundary]` fn; only rule (d) (maximal-fx) is exempt (`fx_maximal_without_slag`
  reads `item.boundary` — a boundary is slag-adjacent, §9/OQ-4). **A boundary fn
  also short-circuits to `BoundaryL1` at `gate_fn` BEFORE the #13 SOLVER-vacuity
  gate and the #12 mutation gate ever run** (`check::gate_fn`'s
  `GateOutcome::BoundaryL1`, `forge/src/check.rs`), so the boundary's assumed `ens`
  is NEVER mutation-scored or solver-vacuity-rejected — only structurally triaged.
- **GROUNDED — a boundary with `ens true` is REJECTED (rule (a)), a boundary with a
  CLOSED-outcome-set `ens` PASSES to L1.** The shipped `#16` boundary oracle already
  pins this (`conformance/boundary/cases.json` reject `boundary_vacuous_contract`:
  `#[boundary("ext::g")] … ens true …` → `EnsIsTrivial`), and I reproduced it: an
  `ens true` boundary → `EnsIsTrivial` reject; the SAME fn with
  `ens match result { Some(v) => v < 256, None => true }` (the outcome set CLOSED via
  a Stage-1 ADT return) → certifies **`L1`, `boundary: true`, `boundary_target`**.
- **The caller's exhaustive `match` is the Stage-1b validator + verus.** A caller
  that drops an arm of the primitive's `Option`/`Result`/user-enum return is
  REJECTED: for a USER enum at the validator (`NonExhaustiveMatch { missing }`,
  `thermite-spec`), for built-in `Option` at verus (`E0004: non-exhaustive
  patterns: None not covered`). Both are LOUD compile-time rejects (GROUNDED below).

So outcome-coverage = (#16 boundary triage admits a closed-set `ens` but rejects a
trivial one) ∘ (Stage-1b exhaustive-match forces every arm handled) ∘ (#52
verify-through proves the caller's own `ens` on EACH arm). **The one thing the
builder MUST do is the `conformance/effect-stdlib` test that PINS this composition**
— not write a new "outcome-coverage validator." **NO new vacuity-exemption fix is
needed:** the feared "a boundary's weak `ens` is wrongly vacuity-rejected" does NOT
occur — a weak-but-closed `ens` (`match result { … }`, which mentions `result` and
is not `BoolLit(true)`) passes (a)/(b)/(c) cleanly, and the value-strength gates
(#12 mutation, #13 solver) never reach a boundary fn (it L1-short-circuits first).
The honest claim about a syscall is a TOTALLY-COVERED outcome SET, not a strong
world-promise — and the toolchain already checks exactly that.

### Resolution 2 — `thermite-stdlib` structure + the v1 forge-build-link decision.

**v1 = the verification + sandbox-derivation layer; the runnable foreign-body LINK
is DEFERRED (OQ-4).** GROUNDED: `forge build --entry` of a program that actually
CALLS a boundary lowers the call to the foreign Rust path (`os::now()`) and invokes
real `rustc`, which FAILS — `error[E0433]: cannot find module or crate \`os\``.
There is no `os::` crate to link, and writing real seccomp-confined syscall wrappers
is forward work (the `#57` design itself scopes "compiling the foreign BODIES so
they run + are confined" as OUT, x86_64-Linux only). So:

- **`thermite-stdlib/src/effect/{read,write,time}.rs`** hold the v1 effect-primitive
  **`#[boundary("os::…")]` DECLARATIONS** (Thermite source the crate exports —
  `.th` text or a `const &str` the skill embeds; the orchestrator settles the exact
  packaging, OQ-2). They are NOT yet the executable Rust syscall wrappers; the
  `"os::read_file"` target string is the foreign-target DATUM the L1 wrapper names
  and the audit manifest enumerates, with no live link in v1.
- **`forge check` / `forge audit` of a program using the primitives works TODAY**
  (no link needed): the boundary certifies L1, the pure caller composes through to
  L3 + `to_boundary`, the audit manifest enumerates the primitive in the TCB
  (all GROUNDED below).
- **`forge build`'s SANDBOX-CONFINEMENT is demonstrated via the SHIPPED
  `fx`-declaring-body pattern + `--sandbox-self-test`** (the `#57`
  `conformance/sandbox/cases.json` precedent), NOT a real `os::` call: a fn that
  DECLARES `fx read(src)` (with a pure body) installs the `read`-widened seccomp
  allowlist (GROUNDED: 27 syscalls incl. `openat`), and the `--sandbox-self-test`
  probe confirms the confinement is `fx`-derived. The kill of an off-allowlist
  syscall (`SIGSYS`, exit 159) is the shipped `#57` `pure_probe_killed` case. This
  proves the per-effect confinement WITHOUT the live foreign-body link.

The live foreign-body run (a real `read_file` syscall confined by the filter) is
**v1.1 / OQ-4** — it needs the `thermite-stdlib` Rust wrappers + a `forge build`
link path, both forward work. v1 delivers the full VERIFICATION + ENUMERATION +
CONFINEMENT-DERIVATION story, which is the §1/§9 honesty claim in its entirety.

### Resolution 3 — the v1 primitive set: Read / Write / Time.

**v1 ships three families that establish the pattern; Net/Rand/Alloc are v1.1.**

- **`read_file` / `read_stdin` (`Read`)** — return a Stage-1 ADT `Option`/`Result`
  (the closed outcome set: a read can EOF/fail), the centerpiece of outcome-coverage.
- **`write_file` / `print` (`Write`)** — return `Result<(), IoError>` (write can
  fail); `ens` the bytes were handed to the OS, never durability.
- **`now` (`Time`)** — the simplest primitive (`-> u64`, no failure arm, `fx time`),
  the minimal compose-through + sandbox-derivation case.

`Net` (`net_connect`/`net_send`/`net_recv`, ties to the `fx diverge` server note),
`Rand` (`random`), and `Alloc` (`Box<T>` construction — OQ-3) follow the IDENTICAL
shape and are **v1.1** (each is one declaration + one `cases.json` entry on the same
machinery). The `governs` routes are trimmed to the v1 three (read/write/time); the
orchestrator adds net/rand/alloc routes when v1.1 starts.

### Layer map (the build order — small enough for one cohesive builder)

| Layer | Deliverable | Mechanism (all SHIPPED) |
|---|---|---|
| **3a** | `thermite-stdlib` crate of the v1 `#[boundary("os::…")]` declarations (read/write/time) | `#16` boundary form (`parser.rs` `Semi`-body path; `ast.rs` `FnItem { boundary: Some, body: None }`) |
| **3b** | `conformance/effect-stdlib/cases.json` pinning OUTCOME-COVERAGE: L1 boundary cert; compose-through to L3 + `to_boundary`; the missing-arm reject; the wrong-arm soundness reject; the TCB enumeration | `#16` `gate_fn` L1; `#52` `lower_external_body_fn` weave; Stage-1b exhaustive-match; `#17` `ToBoundary`; `#15` `AuditManifest.tcb` |
| **3c** | the `forge build` sandbox-confinement DEMO (the `fx read(src)`-body + `--sandbox-self-test` pattern; exit 159 kill / allow-on-widen) | `#57` `sandbox::syscall_allowlist` over `transitive_fx` |

No layer adds production `.rs` to forge/thermite-lower/thermite-spec — Stage 3 is a
stdlib + an oracle over the shipped pipeline.

## The unifying principle — handled-or-loud, on every OUTCOME (the EFFECT seam)

Stage 3 is where the toolchain's unifying law (crosslink **#62** design-refinement
pass) meets the genuinely-uncertain world: **for every outcome a program MODELS it
must either HANDLE it (a path proven L3 or checked L1) or SCREAM (an explicit,
typed, greppable refusal); silently doing the wrong thing is structurally
impossible.** An effect primitive interacts with a world the prover cannot
model — but it can still CLOSE its outcome SET and force every arm to be handled.
The three escalating teeth all show up here:

- **Compile-time scream.** A primitive returning a Stage-1 ADT `Result<T, E>` /
  `Option<T>` models its outcome space as a closed sum type; the caller's exhaustive
  `match` (`01-adts.md` REQ-5/REQ-12) makes a missed arm a VALIDATION reject — the
  failure/EOF outcome cannot be silently dropped. GROUNDED: a user-enum missing arm
  → `NonExhaustiveMatch { missing: ["…"] }`; a built-in `Option` missing arm →
  verus `E0004: non-exhaustive patterns`.
- **Runtime scream.** Each primitive's contract is L1-enforced on EVERY crossing
  (the `#16` `lower_boundary_fn_l1` wrapper: `req`-check → foreign call → `ens`-
  check, §6 L1): a primitive that violates its assumed contract is caught at the
  boundary, exit 101, never a wrong value. And `fx panic` makes "I can scream here"
  FIRST-CLASS — a function that may abort declares `panic` in its effect row (§4.1),
  so the refusal is in the row and in the manifest, greppable.
- **Kill scream.** The #57 seccomp sandbox confines each primitive to exactly its
  effect's syscalls (REQ-5): a `read_file` that tries to `write`/`connect` is
  `SIGSYS`-killed by the kernel — the trusted-by-fiat body cannot exceed its
  declared `fx`.

The fiat/verified line is a KNOB (the load-bearing reframing OQ-1 resolves): the
honest claim about a syscall is NOT a strong promise about the world, it is a
TOTALLY-COVERED outcome SET. You model MORE failure variants → MORE arms the caller
is forced to handle → MORE of the program verified; whatever you leave UNMODELED is
the enumerated trusted remainder the manifest reports (the §9 TCB). The boundary is
strong where it CAN be (the outcome set is closed and must be handled) and silent
where it MUST be (WHICH outcome the world produces). That is handled-or-loud for
effects.

## Grounding the full path (REAL `forge` output — the #72 build-resolution pass)

Reproduced against `forge` built from this tree + `verus 0.2026.05.24` (scratch +
forge temp removed per #53). The centerpiece source (`effect_demo.th`):

```thermite
#[boundary("os::read_small")]
fn read_small() -> Option<u64>
  req true
  ens match result {
        Some(v) => v < 256,        // closes the SET; constrains the Ok arm's SHAPE
        None    => true,           // the EOF/scream arm
      }
  fx  read(input)
;

fn read_doubled() -> u64
  req true
  ens result < 512                 // holds on the Some arm (v<256 ⇒ v+v<512) AND the None arm
  fx  read(input)
{
  match read_small() {             // exhaustive: BOTH arms FORCED handled
    Some(v) => v + v,              // HANDLE: proven via the assumed ens
    None    => 0,                  // SCREAM-and-recover: typed None arm, also < 512
  }
}
```

**`forge check effect_demo.th --mutation-floor 0`** (see the mutation note below):

```
item: read_small
level: L1
boundary: true
boundary_target: os::read_small
assurance_scope: to-the-boundary (via read_small)
item: read_doubled
level: L3
assurance_scope: to-the-boundary (via read_small)
```

The primitive certifies **L1 + boundary + target** (`#16`); the pure caller composes
THROUGH the assumed `ens` to **L3 + `to-the-boundary (via read_small)`** (`#52` +
`#17`). This is verified-to-the-boundary, GROUNDED end-to-end on the shipped path.

**Negative control (outcome-coverage is load-bearing).** Make the None/scream arm
return a value violating the caller's `ens` (`None => 999` under `ens result < 512`):

```
item: read_doubled
level: L0
  [FAIL] postcondition not satisfied @ read_small_check.rs:18:9
         error: postcondition not satisfied
```

A COUNTEREXAMPLE — the mishandled scream arm does NOT verify (no false L3). The
caller proves its postcondition on EVERY arm using ONLY the primitive's assumed
outcome-set `ens`, or it fails.

**`ens true` on a boundary is REJECTED (the vacuity-exemption question, settled).**
`#[boundary("os::now")] fn now() -> u64 req true ens true fx time ;` →

```
reject: EnsIsTrivial — §7.1 (a): ens#0 is syntactically `true` (literal or identity)
```

…while `… ens result < 4000000000 fx time ;` → **`L1`, `boundary: true`,
`boundary_target: os::now`**. So the boundary is NOT exempt from honesty — a
trivially-true `ens` is still rejected by §7.1 (a)/(b)/(c); only the value-STRENGTH
gates (#12/#13) are bypassed (the boundary L1-short-circuits before them). **This is
why no new vacuity-exemption fix is needed:** a closed-outcome-set `ens`
(`match result { … }`, which mentions `result` and is not `BoolLit(true)`) passes
the structural battery as-is.

**The audit manifest enumerates the primitive in the TCB.** `forge audit` of the
`now` program:

```
tcb (trusted computing base):
  boundary: now -> os::now (req="true" ens=[result < 4000000000] fx=[time])
  toolchain: verus=Verus  Version: 0.2026.05.24…
```

**The sandbox confinement is `fx`-derived (the `#57` self-test pattern).**
`forge build --entry rf --sandbox-self-test` over
`fn rf(x: u32) -> u32 req x < 100 ens result == x fx read(src) { x }`:

```
sandbox: seccomp installed (transitive fx=[read(src)]; 27 syscalls allowlisted)
```

vs the pure `sum` (`fx pure`): **23 syscalls**, vs `fx time`: **25 syscalls**
(baseline 23 + `clock_gettime` 228 + `clock_nanosleep`). The `read`-widened filter
ALLOWS the `--sandbox-self-test` `openat` probe; under the pure filter the same
probe is `SIGSYS`-KILLED (exit 159 = 128+31, the shipped `pure_probe_killed`). The
allowlist is `fx`-DERIVED and per-effect, GROUNDED — no live `os::` link required.

**Honest grounding caveat — the #12 mutation gate vs bound-style effect `ens`.** A
pure caller whose RESULT flows from the uncertain read necessarily has a BOUND-style
postcondition (`result < N`), and a `return 0`-at-head mutant SURVIVES `result < N`
→ the #12 mutation gate reports `WeakContract` at the DEFAULT floor (GROUNDED:
`read_doubled` → `WeakContract` mutation kill ratio 0/3). This is intrinsic: a
program reading the world cannot pin an EXACT result `ens` the way pure
`binary_search` does. The v1 corpus therefore certifies the bound-style effect
caller with **`--mutation-floor 0`** (the documented `forge check` relaxation),
exactly as a real effect-reading program would; the L3 + `to_boundary` claim is
otherwise identical. The builder + critic MUST pin the corpus's `--mutation-floor 0`
flag and NOT let it leak to the pure corpus (which stays at the default floor).
(NOTE: the user-enum match-on-`result`-in-`ens` variant currently hits a verus
obligation failure on lowering even at floor 0 — the built-in `Option` return is the
reliable L3 demo. If the builder needs a user-enum effect primitive, file a fresh
`#` against the match-in-ens lowering; v1 uses `Option`/`Result`.)

## The verus mechanism (GROUNDED — `verus 0.2026.05.24`)

An effect primitive is a `#[verifier::external_body]` fn carrying a real
`requires`/`ensures` contract: Verus ASSUMES the contract at every call site and
NEVER checks the foreign body (the body is the real syscall). A pure caller then
verifies THROUGH that assumed contract. Authoring harnesses (run against the real
`verus` binary; scratch removed):

**(1) The effect primitive + the compose-through proof.** A `read_small ->
Option<u64>` whose `ens` closes the outcome set, plus a pure caller that `match`es
BOTH arms — `forge check … --mutation-floor 0`: `read_small` **L1 boundary**,
`read_doubled` **L3 + to-the-boundary** (the §9 / `#52` verify-through-the-contract,
GROUNDED above).

**(2) Soundness — the caller cannot manufacture a guarantee the contract does not
deliver.** A caller claiming an `ens` STRONGER than the primitive delivers (the
`None => 999` negative under `ens result < 512`) FAILS with `postcondition not
satisfied` (GROUNDED above) — a COUNTEREXAMPLE, not a false L3. The external_body
assumes ONLY the primitive's `ensures`; the caller still proves its own
postcondition. This is the `#52` soundness property instantiated for effect
primitives.

### THE LEGITIMATE-`external_body` DISTINCTION (load-bearing — `#52`/`#60` honesty gate)

`--no-cheating` (the flag that proves the CORE has no proof cheat) **bans
`external_body` entirely** (the `#52`/`#60` honesty gate, GROUNDED in the prior
authoring pass: `error: external_body/assume_specification not allowed with
--no-cheating`). The distinction the doc pins, exactly: `--no-cheating` is for CORE
logic, where `external_body` WOULD be a proof-dodge (the `#60`-style cheat R-DEFER-9
forbids). An effect primitive is NOT core logic: it is a **declared trust
boundary** — a `#[boundary]` fn whose body is genuinely foreign (the syscall), with
no Thermite body to prove. For a boundary, `external_body` is the HONEST modeling of
a foreign function (`#52` honesty argument, pinned hard):

- It is emitted ONLY for a fn carrying the syntactic `#[boundary]` flag
  (`FnItem.boundary.is_some()` in `ast.rs`), already certified `Level::L1` +
  `boundary: true` by the §16 path (`Certificate::boundary_l1` in `manifest.rs`).
  A regular Thermite fn is ALWAYS fully proved (`#52` REQ-1 / OQ-1 honesty gate).
- The contract is L1-ENFORCED at runtime on every crossing (`#16` REQ-4, the
  `lower_boundary_fn_l1` wrapper in `l1.rs`: `req`-check → foreign call → `ens`-
  check), so a primitive that violates its assumed contract is caught at the
  boundary — the assumed `ensures` is not an unchecked free pass.
- The effect is RUNTIME-SANDBOXED (`#57`): the primitive is confined to exactly
  its effect's syscalls, so even the trusted-by-fiat body cannot exceed its
  declared `fx`.

So `external_body iff a declared boundary/slag` is the `#52`/`#60` honesty gate,
and the effect-primitive stdlib is the canonical *legitimate* use of it: the
honest, enumerated trusted base, NOT a core-logic cheat. **Verified in default
mode (where the boundary is honest), banned under `--no-cheating` (which guards
the core).** The two modes encode the distinction mechanically.

## The effect-primitive pattern (the unit this stage instantiates)

Each effect primitive is a `#[boundary("…")]` fn — the SAME surface form `#16`
ships (`.design/boundary/ffi-boundary.md`, "the surface form"), specialized to a
syscall target rather than a crates.io target. Four parts, all on SHIPPED
machinery:

1. **The CONTRACT (`req`/`ens`)** — the *assumed* behavior of the syscall, stated
   in SpecTherm. The honest claim is the MINIMAL true one (you cannot prove the
   disk), and it CLOSES the outcome set: `read_file` ens the SHAPE of each arm of a
   `Result<bytes, Error>` / `Option` (Stage 1 ADT) — never WHICH arm the world
   produces; `write_file` ens "the bytes were handed to the OS" (a status `result`),
   not durability. A trivially-`true` `ens` is REJECTED by §7.1 (a) (GROUNDED).
2. **The effect (`fx`)** — the §4.1 effect atom this primitive carries
   (`Effect::Read(path)`/`Write(path)`/`Time`/…). This is the TYPED effect; the §4.1
   row-subsumption check (`.design/lower/effect-subsumption.md`, SHIPPED) makes every
   transitive caller declare it. (Grammar: `read`/`write`/`net` take a path arg —
   `fx read(input)`; `time`/`rand`/`alloc` are bare — `parse_effect` in `parser.rs`.)
3. **The lowering (`#[verifier::external_body]`)** — the primitive is woven into a
   caller's sub-program as an external_body signature (`#52` REQ-1,
   `lower_external_body_fn` in `lower.rs`): the assumed `requires`/`ensures` with
   NO checked body. Verus assumes the contract; the foreign body is never examined.
4. **The sandbox confinement (`#57`)** — `forge build --entry <fn>` derives the
   transitive `fx` row and installs a seccomp-bpf allowlist
   (`sandbox::syscall_allowlist` over `transitive_fx`, `.design/forge/runtime-sandbox.md`),
   so a primitive declared `fx read(_)` is confined to the `read(_)` syscall set
   (`openat`/`read`/`close`/`statx`/…) — a `write`/`socket` attempt is killed by the
   kernel (`SIGSYS`). In v1 the confinement is GROUNDED via the `fx`-declaring-body
   + `--sandbox-self-test` pattern (the live foreign-body call is v1.1, OQ-4).

The body is trusted-by-fiat but effect-CONFINED and contract-STATED. That triple
(stated + typed + confined) is what makes a one-line foreign syscall an honest,
auditable member of the TCB rather than an opaque trust hole.

### The stdlib (one primitive family per effect atom)

Each family maps to an `Effect` atom and the §57 fx→syscall allowlist it implies
(the `.design/forge/runtime-sandbox.md` mapping table is the authority — this doc
does not redefine it). **v1 = Read/Write/Time (top three rows); Net/Alloc/Rand =
v1.1 (same shape).**

| Effect atom (`enum Effect`) | v1? | Primitive family | Sketch contract (assumed) | `fx` row | Sandbox allowlist (the #57 table) |
|---|---|---|---|---|---|
| `Read(path)` | **v1** | `read_file`, `read_stdin` | `ens` shape only: a closed `Option`/`Result` (Stage 1 ADT) — never WHICH bytes | `fx read(path)` | `openat`, `read`, `close`, `lseek`, `statx`, `newfstatat` |
| `Write(path)` | **v1** | `write_file`, `print` | `ens` the bytes were handed to the OS (a `Result` status), not durability | `fx write(path)` | `openat`, `write`, `fsync`, `newfstatat` |
| `Time` | **v1** | `now` | `ens` shape only: a `u64` timestamp — never a specific instant | `fx time` | `clock_gettime` (228), `clock_nanosleep` |
| `Net(domain)` | v1.1 | `net_connect`, `net_send`, `net_recv` | `ens` shape of the connection/transfer; `recv` may short/EOF (`Option`/`Result`) | `fx net(domain)` | `socket`, `connect`, `sendto`, `recvfrom`, `setsockopt`, `getsockopt` |
| `Alloc` | v1.1 (OQ-3) | `Box<T>` construction (a language construct, no `#[boundary]` fn) | `ens` a live, distinct allocation | `fx alloc` | baseline (`mmap`/`munmap`/`brk`/`mprotect`) |
| `Rand` | v1.1 | `random` | `ens` shape only: a `u64` — explicitly NO distribution/unpredictability claim | `fx rand` | `getrandom` (318) |

`Panic` and `Diverge` are effect atoms but NOT data-returning syscall primitives:
`panic` rides the baseline (`write`+`exit_group`, the L1 contract-violation path),
and `diverge` adds no syscall (the non-termination effect). They appear in the
lattice and the row but are not members of THIS stdlib's syscall-primitive families.

### The TCB / honesty story (the load-bearing point — §1, §9, R-DEFER-9)

§9 states the trusted computing base is **exactly (slag blocks ∪ boundary
contracts ∪ the toolchain itself)**, and it is *enumerable*. The effect-primitive
stdlib is the boundary half of that union for I/O: a program = **verified pure
logic (L3) + this enumerated, contracted, confined effect base**. The audit
manifest (`#15`, `.design/forge/audit-manifest.md`, `AuditManifest` `tcb` section)
enumerates each primitive a program reaches — name, assumed contract, foreign
target, effect (GROUNDED: `boundary: now -> os::now (req=… ens=… fx=[time])`) — so a
skeptical third party reads the *entire* fiat-trusted base in minutes (§1). The
honesty chain, end to end:

- A program using `read_file` is **verified-to-the-boundary** (`#17`,
  `AssuranceScope::ToBoundary { via: read_file }`, GROUNDED): its pure logic is
  L3-proved, but the whole-program guarantee depends on `read_file` honoring its
  assumed contract. The manifest marks this honestly — it never claims "verified,
  period" when an effect primitive is reached (`goal.md` R-DEFER-9, R-CHAR-3).
- The boundary IS the primitive's assumed contract (§9: the contract, not the
  body, is the interface). The §52 composition keeps the pure logic's L3 cert
  valid independent of the syscall's body.
- The sandbox guarantees the primitive can ONLY do its declared effect: a
  `read_file` confined to the `read` allowlist is KILLED if it tries to `write` /
  `connect` (`#57` AC-2, the `SIGSYS` kill). The confinement is the second half of
  honesty — the assumed contract says "this only reads," and the kernel enforces it.

This is the theoretical maximum the §1 thesis targets: you cannot prove the
kernel, the disk, or the network, so you reduce your dependence on them to a small,
enumerated, contract-stated, syscall-confined set, and verify EVERYTHING else.

### Interactive / server programs (the `fx diverge` composition note — v1.1)

A long-running server — `loop { let req = net_recv(); let resp = handle(req);
net_send(resp); }` — composes the (v1.1) net primitives with `fx diverge` into a
real program that is STILL verified: `handle` (the per-request pure logic) is
L3-proved END-TO-END (`#17`); each crossing through `net_recv`/`net_send` is
verified-to-the-boundary; the loop carries `fx diverge` (partial correctness — each
request handled correctly, non-termination declared, not proved away). `forge build
--entry` confines the binary to the `net(_)` ∪ `diverge` allowlist (`diverge` adds
no syscall, `#57` table). This extends "verify anything" to a never-halting server;
it lands with the `Net` family in v1.1.

## Requirements

- **REQ-1 (the effect-primitive declaration form):** each effect primitive is a
  `#[boundary("<syscall-target>")] fn NAME(params) -> ret req … ens … fx <atom> ;`
  — the `#16` bodyless-boundary surface form (`FnItem { boundary: Some(_), body:
  None }` in `ast.rs`), a mandatory contract, a declared effect atom, and a `;`
  body. Derived from `thermite-design.md` §9 + §4.1 + `#16`
  (`.design/boundary/ffi-boundary.md` REQ-1/REQ-2). No new grammar — the stdlib
  reuses the boundary form verbatim. v1: read/write/time families.

- **REQ-2 (the v1 primitive families — the stdlib):** the `thermite-stdlib` crate
  declares the v1 families `read_file`/`read_stdin` (`Read`), `write_file`/`print`
  (`Write`), `now` (`Time`), each carrying its assumed contract + the effect atom +
  (via the `#57` table) its syscall allowlist. `Net`/`Alloc`/`Rand` are v1.1 (the
  IDENTICAL shape). Derived from §4.1 (the effect lattice this stdlib instantiates) +
  the §1 "verify anything" thesis + `01-adts.md` (the `Alloc`/`Box` tie, OQ-3).

- **REQ-3 (a boundary contract is honest iff it TOTALLY COVERS its outcome space —
  EMERGENT, no new code):** a boundary/effect-primitive contract is HONEST by
  **closing its outcome SET and forcing the caller to handle every arm**, NOT by a
  strong world-claim and NOT by a blanket vacuity-exemption. The primitive's return
  type is a Stage-1 ADT `Option`/`Result` (closed outcome set); its `ens` constrains
  the SHAPE of each arm, never WHICH arm; the caller's exhaustive `match`
  (`01-adts.md` REQ-5/REQ-12) FORCES every arm resolved with the caller's own `ens`
  proven on EACH. **This is EMERGENT from shipped pieces** (v1-scope Resolution 1):
  §7.1 (a)/(b)/(c) reject a trivially-`true` boundary `ens` but ADMIT a closed-set
  `ens`; the boundary L1-short-circuits before #12/#13 (so its weak-but-honest `ens`
  is never value-strength-rejected); Stage-1b exhaustive-match rejects a dropped arm;
  #52 verifies the caller's `ens` on each arm. **No new validator rule and no
  vacuity-exemption fix is required.** GROUNDED (`verus 0.2026.05.24`): `ens true`
  boundary → `EnsIsTrivial`; closed-set `ens` → L1; both-arms-handled caller → L3 +
  `to_boundary`; the wrong-arm negative → `postcondition not satisfied`; the
  missing-arm → `NonExhaustiveMatch` / `E0004`. Derived from §9 + `goal.md`
  R-DEFER-9 + `01-adts.md` REQ-5/REQ-12 + the **#62/#72** resolution. The builder +
  critic MUST pin that this honesty test does NOT leak the value-strength bypass to
  REGULAR fns (which never carry `#[boundary]`, so never short-circuit, so face the
  full §7 battery + #12/#13).

- **REQ-4 (the effect is typed + the primitive lowers via `external_body`):** each
  primitive's `fx` atom is the §4.1 typed effect, checked by the SHIPPED
  row-subsumption (`.design/lower/effect-subsumption.md`) so every transitive
  caller declares it; and the primitive lowers into a caller's sub-program as a
  `#[verifier::external_body]` signature (`#52` REQ-1, `lower_external_body_fn` in
  `lower.rs`) — assumed `requires`/`ensures`, no checked body — so the caller proves
  THROUGH the contract. Derived from §4.1 + §9/§52 + the GROUNDED L3 + `to_boundary`.

- **REQ-5 (the effect is runtime-sandbox-DERIVED — confined to its syscalls):** a
  `forge build --entry` of a program declaring an effect atom installs the `#57`
  seccomp allowlist for that `fx` (the [stdlib table](#the-stdlib-one-primitive-family-per-effect-atom) /
  the `#57` fx→syscall table), so the program is confined to EXACTLY its effect's
  syscalls — a syscall outside the allowlist is `SIGSYS`-killed (exit 159). In v1
  the derivation + kill are GROUNDED via the `fx`-declaring-body +
  `--sandbox-self-test` pattern (the live foreign-body call linking `os::…` is v1.1,
  OQ-4). Derived from §4.1 ("killed at the syscall boundary") + `#57` REQ-1/REQ-3.

- **REQ-6 (the TCB / verified-to-the-boundary honesty story):** a program reaching
  an effect primitive certifies its pure logic at `Level::L3` while recording
  `AssuranceScope::ToBoundary { via: <primitive> }` (`#17`, GROUNDED); the audit
  manifest (`#15`) enumerates each reached primitive (name + assumed contract +
  foreign target + effect) as a `tcb.boundary` member (GROUNDED: `now -> os::now`);
  the manifest NEVER claims "verified, period." Derived from §1 + §9 + `#15`/`#17` +
  `goal.md` R-DEFER-9.

- **REQ-7 (the legitimate-`external_body` distinction):** `external_body` is
  emitted for an effect primitive ONLY because it is a declared `#[boundary]` fn
  (`FnItem.boundary.is_some()`) — the honest foreign model, NOT a `#60`-style
  core-logic cheat. `--no-cheating` (which guards the core) BANS `external_body`;
  the effect-primitive boundary is verified in default mode. A regular Thermite fn is
  always fully proved; no `external_body` is emitted for it. Derived from `#52`/`#60`
  honesty gate (`external_body iff a declared boundary/slag`) + `goal.md` R-DEFER-9.

## Acceptance criteria

ACs tie to a NEW `conformance/effect-stdlib/cases.json` oracle the ORCHESTRATOR
authors (hand-derived per R-CHAR-3, the `conformance/boundary/cases.json` /
`conformance/sandbox/cases.json` precedents). The centerpiece corpus
(`effect_demo.th`, GROUNDED above) — a pure caller reading via `read_small ->
Option<u64>`, computing, both arms handled:

- **AC-1 (verified-to-the-boundary, GROUNDED):** `forge check effect_demo.th
  --mutation-floor 0` certifies `read_small` at **`Level::L1`, `boundary == true`,
  `boundary_target == "os::read_small"`** and the pure caller `read_doubled` at
  **`Level::L3`, `assurance_scope == ToBoundary { via: "read_small" }`** (the `#52`
  external_body weave + `#17` classification). EXACT expected output reproduced under
  [Grounding the full path](#grounding-the-full-path-real-forge-output). The
  `--mutation-floor 0` flag is REQUIRED for the bound-style effect caller (the #12
  note) and MUST NOT leak to the pure corpus.

- **AC-2 (the audit manifest enumerates the primitive as the TCB, GROUNDED):**
  `forge audit` of the program emits an `AuditManifest` (`#15`) whose `tcb` section
  enumerates `read_small` (name + assumed contract + foreign target + effect) as a
  `boundary` member (GROUNDED form: `boundary: now -> os::now (req=… ens=… fx=…)`);
  the pure logic appears as L3 + `to-the-boundary`; nothing fiat-trusted is omitted
  (R-DEFER-9).

- **AC-3 (`forge build` sandbox-confinement is `fx`-DERIVED, GROUNDED):** `forge
  build --entry <fn> --sandbox-self-test` over a fn declaring `fx read(src)` installs
  the `read`-widened allowlist (GROUNDED: 27 syscalls incl. `openat`) and the
  `--sandbox-self-test` probe is ALLOWED; the same probe under a `fx pure` filter (23
  syscalls) is `SIGSYS`-KILLED (exit 159 = 128+31, the `#57` `pure_probe_killed`
  case). A `fx time` program installs 25 syscalls (baseline + `clock_gettime` 228).
  This proves the confinement is `fx`-derived and per-effect WITHOUT a live `os::`
  link (v1; OQ-4).

- **AC-4 (honest-contract / soundness — no manufactured guarantee, GROUNDED):** a
  caller whose handled/scream arm asserts a value VIOLATING the caller's own `ens`
  (the `None => 999` under `ens result < 512` negative) FAILS verification with
  `postcondition not satisfied` (`Level::L0`), NOT a false L3. The assumed `ensures`
  is a floor the caller cannot exceed. This is the anti-cheat AC (R-DEFER-9).

- **AC-5 (handled-or-loud — the missing arm SCREAMS, GROUNDED):** a caller that drops
  an arm of the primitive's return is REJECTED: a USER-enum return → the Stage-1b
  validator `NonExhaustiveMatch { missing: [...] }`; a built-in `Option` return →
  verus `E0004: non-exhaustive patterns: None not covered`. Either way the
  failure/EOF outcome cannot be silently dropped (`01-adts.md` REQ-5).

- **AC-6 (`external_body` iff boundary — the honesty gate):** the lowered verus for
  the pure logic contains an `external_body` signature for the primitive (woven
  boundary dep) and NO `external_body` for any regular fn (`read_doubled` is fully
  proved); the pure existing corpus (`sum`/`binary_search`) emits NO `external_body`
  at all. The `#52` OQ-1 gate.

- **AC-7 (corpus unaffected):** the existing pure corpus (`sum`, `binary_search`)
  certifies an IDENTICAL cert before and after Stage 3 — `Level::L3`,
  `assurance_scope` END-TO-END, no `external_body`, no sandbox kill, the DEFAULT
  mutation floor, and the frozen golden `conformance/sum.cert.json` byte-stable.

## Architecture

Stage 3 owns NO new mechanism — it is a `thermite-stdlib` crate of `#[boundary]`
declarations plus a `conformance/effect-stdlib` oracle over the SHIPPED
`#16`/`#52`/`#57` pipeline. The full path, GROUNDED:

```text
forge check <program using read_small> --mutation-floor 0
  │
  ├─ gate_fn: read_small (#[boundary]) -> BoundaryL1 cert (L1 + boundary flag)   [§16/§9, GROUNDED]
  │     (short-circuits BEFORE #13 solver-vacuity + #12 mutation — only §7.1
  │      (a)/(b)/(c) structural triage runs on the boundary's ens)
  │
  ├─ for the pure logic `read_doubled` (ProceedToL3):
  │     item_subprogram weaves read_small as #[verifier::external_body]           [#52, SHIPPED]
  │        ▼
  │     run_verus: `read_doubled` PROVES through the assumed ens on EVERY arm     [#52, GROUNDED]
  │        ▼
  │     closure::classify -> assurance_scope = ToBoundary { via: read_small }      [#17, GROUNDED]
  │
forge audit <program>
  │     AuditManifest.tcb enumerates read_small as a boundary member              [#15, GROUNDED]
  │
forge build --entry <fn> --sandbox-self-test
  │     sandbox: transitive_fx (read(_)) -> seccomp allowlist (27 syscalls)        [#57, GROUNDED]
  │     a syscall outside the allowlist -> SIGSYS kill (exit 159)                   [#57, GROUNDED]
```

- **The primitives** are `#[boundary("os::…")]` fns in `thermite-stdlib` (the v1
  read/write/time families). The surface form is `#16`'s verbatim (`parse_attribute`
  + the `Semi`-body path in `parser.rs`; `FnItem { boundary: Some, body: None }` in
  `ast.rs`). The syscall-target string (`"os::read_file"`) is the foreign-target
  datum the L1 wrapper names and the audit manifest enumerates — NOT yet a live link
  (v1; OQ-4).
- **The compose-through** is `#52`'s `lower_external_body_fn` (in `lower.rs`) woven
  by `check::item_subprogram`.
- **The confinement** is `#57`'s `sandbox::syscall_allowlist` over
  `sandbox::transitive_fx` (in `forge/src/sandbox.rs`), keyed on the `fx` atom via
  the `#57` fx→syscall table.
- **The honesty surface** is `#17`'s `AssuranceScope::ToBoundary` (in `closure.rs` /
  `manifest.rs`) + `#15`'s `AuditManifest.tcb` (in `forge/src/audit.rs`).

Stage 3's only NEW artifacts are the primitive declarations (`thermite-stdlib`) and
the `conformance/effect-stdlib` oracle. The Stage 5 hook: the
composition-aggregation law (`05-composition.md`, OUT of scope here) aggregates
assurance across exactly these boundaries.

## Verification

- **Routes (orchestrator):** the v1 routes (read/write/time) are below; net/rand/
  alloc routes land with v1.1. The spec-discipline hook (R-XLATE-2/R-XLATE-3) blocks
  the builder until both the route and this doc exist.
- **Oracle (orchestrator-authored):** `conformance/effect-stdlib/cases.json` —
  hand-derived (R-CHAR-3) carrying AC-1..AC-7's programs and their expected per-fn
  `level` + `assurance_scope` + (primitives) `boundary`/`boundary_target`/`effects`,
  the audit `tcb` enumeration, and the sandbox exit/signal. The EXACT expected
  `forge` output is pinned under [Grounding the full
  path](#grounding-the-full-path-real-forge-output) (reproduce it, do not copy
  toolchain output blindly).
- **Golden lowering (R-CHAR-3):** a `tests/golden/lower/effect-stdlib.verus.rs`
  hand-authored from THIS design — the pure-logic program lowered, showing the
  `#[verifier::external_body] fn read_small(...) requires …, ensures …, {
  unimplemented!() }` signature woven before the pure logic — which MUST itself pass
  the real `verus` with 0 errors.
- **Soundness test (AC-4):** assert the `ens`-violating-arm caller emits a NON-L3
  cert with `postcondition not satisfied` (GROUNDED), never a false L3.
- **Handled-or-loud test (AC-5):** assert the missing-arm caller is rejected
  (`NonExhaustiveMatch` for a user enum, `E0004` for `Option`).
- **Honesty-gate test (AC-6):** assert the lowered string contains `external_body`
  IFF the woven dep carries `#[boundary]`, and the pure corpus emits none.
- **Crate gauntlets (`goal.md` R-DEFER-6):** `cargo test -p forge`, `cargo test -p
  thermite-lower`, `cargo test -p thermite-stdlib`, `cargo clippy -p <crate>
  --all-targets -- -D warnings`, `cargo fmt --check`, plus the conformance corpus
  (`sum`/`binary_search` stay L3 + END-TO-END at the DEFAULT floor, AC-7).

## Open questions

- **OQ-1 (the honesty seam — RESOLVED via OUTCOME-COVERAGE, EMERGENT; #62/#72).**
  How does the assumed `ensures` of an inherently-uncertain syscall stay HONEST
  without being vacuous? **RESOLVED — a boundary contract is honest iff it TOTALLY
  COVERS its outcome space, and this is EMERGENT from shipped pieces (no new code)**:
  the uncertainty lives in the RETURN TYPE (a Stage-1 ADT `Option`/`Result`, a closed
  outcome set); the `ens` constrains each arm's SHAPE; the caller's exhaustive
  `match` forces every arm resolved with the caller's own `ens` proven on EACH. The
  §7.1 structural battery already rejects a trivially-`true` boundary `ens` (a) and
  admits a closed-set `ens`; the boundary L1-short-circuits before the
  value-strength gates (#12/#13), so a weak-but-honest `ens` is never wrongly
  value-rejected. **No new vacuity-exemption fix is needed** (the feared wrong-reject
  does not occur — GROUNDED). The builder + critic MUST pin that the value-strength
  bypass does NOT leak to regular fns. GROUNDED end-to-end (see [Grounding](#grounding-the-full-path-real-forge-output)).

- **OQ-2 (stdlib crate layout + skill budget):** where do the primitives live — a
  `thermite-stdlib` crate of `.th` declarations the skill generator embeds (the
  LEANING), a built-in module, or `conformance/effect-stdlib/stdlib.th`? The §10
  skill is budgeted ≤6,000 tokens; the v1 three families (one attribute + one
  contract each, the `#16` minimal form) fit. The orchestrator settles the
  crate/route shape; this doc governs the contract regardless.

- **OQ-3 (the `Alloc`/`box` primitive vs Stage 1 `Box` — v1.1):** Stage 1 ties
  `Box<T>` construction to `fx alloc` + the baseline `mmap`/`brk` syscalls. LEANING:
  `Box<T>` construction IS the `Alloc` primitive (no `#[boundary]` syscall wrapper —
  the Rust allocator is the foreign body, confined by the baseline allowlist), so
  `Alloc` is the one atom whose "primitive" is a language construct. Confirm against
  `01-adts.md` REQ-3 when Stage 4 (collections) generalizes the heap primitive. Out
  of v1 scope.

- **OQ-4 (foreign body execution + cross-platform — DEFERRED to v1.1, GROUNDED):**
  `forge build` of a program CALLING a boundary lowers to the foreign Rust path
  (`os::now()`) and `rustc`-FAILS (`E0433: cannot find module or crate \`os\``) — no
  `os::` crate exists. v1 DELIVERS the verification + enumeration +
  confinement-derivation (all GROUNDED) and demonstrates confinement via the
  `fx`-declaring-body + `--sandbox-self-test` pattern; the LIVE foreign-body run
  (real syscall wrappers in `thermite-stdlib` + a `forge build` link path,
  x86_64-Linux only) is v1.1. The CONTRACT + the TYPED effect + the ENUMERATED TCB
  are fully specifiable + verifiable in v1 regardless.

- **OQ-5 (user-enum match-on-`result` lowering — file a fresh `#` if needed):** the
  built-in `Option`/`Result` return is the reliable L3 demo; a USER-enum return whose
  primitive `ens` is `match result { Good(v) => …, Bad => … }` currently hits a verus
  obligation failure on lowering even at `--mutation-floor 0` (GROUNDED). v1 uses
  `Option`/`Result`. If a user-enum effect primitive is needed, the builder files a
  separate blocker against the match-in-ens lowering (NOT under #72, which owns the
  stdlib + composition).

## Routes to add (orchestrator) — v1 (read/write/time)

```toml
# Effect-primitive standard library — verified, sandboxed #[boundary] syscall
# primitives (basis Stage 3 v1, epic #62 / issue #72). Net/Rand/Alloc = v1.1.
[[route]]
crate_pattern = "thermite-stdlib/src/effect/read.rs"
design = ".design/basis/03-effect-stdlib.md"
reference = ["conformance/effect-stdlib"]
conformance_ops = ["read_small_to_boundary", "read_doubled_l3", "audit_enumerates_tcb"]

[[route]]
crate_pattern = "thermite-stdlib/src/effect/write.rs"
design = ".design/basis/03-effect-stdlib.md"
reference = ["conformance/effect-stdlib"]
conformance_ops = ["write_file_to_boundary"]

[[route]]
crate_pattern = "thermite-stdlib/src/effect/time.rs"
design = ".design/basis/03-effect-stdlib.md"
reference = ["conformance/effect-stdlib"]
conformance_ops = ["now_to_boundary", "now_sandbox_clock_gettime"]
```

The orchestrator authors `conformance/effect-stdlib/cases.json`, the
`tests/golden/lower/effect-stdlib.verus.rs` golden, the routes above, and the
`thermite-stdlib` crate scaffold. This doc does NOT author the oracle, the golden,
or the routes (R-DOC-1). The crate/file layout is a LEANING (OQ-2); the orchestrator
settles it.

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (effect-primitive declaration form) | NOT-STARTED | epic #62 / issue #72, Stage 3 v1. No `thermite-stdlib` crate and no `#[boundary("os::…")]` syscall primitive exists in the tree. The SHIPPED prerequisite form (`#16` `FnItem { boundary: Some, body: None }` in `ast.rs`; `parse_attribute` + the `Semi`-body path in `parser.rs`) parses + certifies a `#[boundary("os::now")]` decl to L1 TODAY (GROUNDED: `forge check` → `L1, boundary: true, boundary_target: os::now`), but no syscall primitive is declared against it. |
| REQ-2 (v1 primitive families — read/write/time) | NOT-STARTED | epic #62 / issue #72. The `enum Effect` atoms (`Read(path)`/`Write(path)`/`Time` in `ast.rs`) exist and parse (`fx read(input)`/`fx time`, `parse_effect` in `parser.rs`), and the §57 fx→syscall table maps each (GROUNDED: `read(src)` → 27 syscalls, `time` → 25 incl. `clock_gettime` 228), but no `read_file`/`write_file`/`now` primitive family is declared in a `thermite-stdlib` crate. |
| REQ-3 (boundary honest iff TOTAL OUTCOME-COVERAGE — EMERGENT, no new code) | NOT-STARTED | epic #62 / issue #72. The RESOLUTION is EMERGENT + fully GROUNDED (`verus 0.2026.05.24`): a boundary `ens true` → `EnsIsTrivial` reject; a closed-set `ens match result { … }` → L1; a both-arms-handled caller → L3 + `to_boundary`; the wrong-arm negative → `postcondition not satisfied`; the missing-arm → `NonExhaustiveMatch`/`E0004`. NO new validator rule and NO vacuity-exemption fix is needed (the feared wrong-reject does not occur). But no primitive contract is declared and no `conformance/effect-stdlib` test PINS the composition yet. |
| REQ-4 (typed effect + `external_body` lowering) | NOT-STARTED | epic #62 / issue #72. The SHIPPED `#52` `lower_external_body_fn` (in `lower.rs`) + `check::item_subprogram` weave and the SHIPPED row-subsumption (`effect-subsumption.md`) compose a boundary into a caller's L3 proof TODAY (GROUNDED: `read_doubled` → `L3` + `to-the-boundary (via read_small)` at `--mutation-floor 0`), but no effect primitive is declared to be woven. |
| REQ-5 (runtime-sandbox-DERIVED — confined to its syscalls) | NOT-STARTED | epic #62 / issue #72. The SHIPPED `#57` `sandbox::syscall_allowlist` over `transitive_fx` (in `forge/src/sandbox.rs`) + the fx→syscall table DERIVE + enforce the confinement TODAY (GROUNDED: `fx read(src)` → 27 syscalls incl. `openat`, the `--sandbox-self-test` probe allowed; `fx pure` → 23, probe `SIGSYS`-killed exit 159), but no effect primitive program exercises it via the oracle. The live `os::` foreign-body link is DEFERRED (OQ-4; `forge build` of a real boundary CALL `rustc`-fails `E0433`). |
| REQ-6 (TCB / verified-to-the-boundary honesty story) | NOT-STARTED | epic #62 / issue #72. The SHIPPED `#17` `AssuranceScope::ToBoundary` (closure.rs/manifest.rs) + `#15` `AuditManifest.tcb` (forge/src/audit.rs) record + enumerate a reached boundary TODAY (GROUNDED: `forge audit` → `tcb … boundary: now -> os::now (req=… ens=… fx=[time])`; the pure caller carries `scope=to-the-boundary`), but no effect-primitive program is in the corpus to be enumerated. |
| REQ-7 (legitimate-`external_body` distinction) | NOT-STARTED | epic #62 / issue #72. The distinction is pinned by the SHIPPED `#52`/`#60` honesty gate (`external_body iff a declared boundary/slag`; `external_body` verifies in default mode, `--no-cheating` errors `external_body/assume_specification not allowed`), but no effect primitive exercises it — there is no `#[boundary]` syscall primitive declared to lower. |
