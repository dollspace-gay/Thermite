# Effect-Primitive Standard Library (Basis Stage 3)
<!--
tier: 3-component
status: draft
governs: thermite-stdlib/src/effect/read.rs
governs: thermite-stdlib/src/effect/write.rs
governs: thermite-stdlib/src/effect/net.rs
governs: thermite-stdlib/src/effect/alloc.rs
governs: thermite-stdlib/src/effect/time.rs
governs: thermite-stdlib/src/effect/rand.rs
thesis-refs:
  - thermite-design.md §1
  - thermite-design.md §4.1
  - thermite-design.md §8
  - thermite-design.md §9
  - thermite-design.md §6
-->

## Summary

Stage 3 of the universal-verified-basis buildout (crosslink epic **#62**) is the
**EFFECT half of "program anything, verified."** Stage 1 (`01-adts.md`) gives the
DATA basis (every finite algebraic type); Stage 3 gives the EFFECT basis: each
atom of the §4.1 effect lattice (`Effect::Read`/`Write`/`Net`/`Alloc`/`Time`/
`Rand` in `enum Effect in ast.rs`) is instantiated as a contracted,
seccomp-confined **`#[boundary]` effect primitive** — a verified-effect-primitive
whose BODY is the real syscall (trusted by fiat, because you cannot prove the
kernel), whose CONTRACT states the assumed behavior of that syscall, whose effect
is TYPED (a `fx` row) and RUNTIME-SANDBOXED (the #57 seccomp filter confines the
primitive to exactly the syscalls its effect implies). The pure logic that
orchestrates these primitives is FULLY verified (L3); the trusted base is exactly
this small, enumerated, contracted, confined set.

This is the §1 trust-relocation thesis discharged for I/O: **"verify anything" =
"verify everything except this small, contracted, sandboxed, enumerated set."** A
program that reads input via `read_file`, computes via verified pure logic, and
writes via `write_file` is *verified-to-the-boundary* (the §9 / `#17` scope): its
pure logic SMT-proves against the primitives' assumed contracts, the audit
manifest (`#15`) enumerates the primitives as the TCB, and `forge build`
(`#57`) confines each primitive to exactly its effect's syscalls — a `read_file`
that tried to `write` or `connect` would be killed by the kernel at the syscall
boundary. This is the theoretical maximum: you cannot prove the disk, so you
*contract* and *confine* your dependence on it, and you enumerate it honestly.

This doc is **GREENFIELD / FORWARD-LOOKING.** There is no effect-primitive stdlib
anywhere in the toolchain today — no `thermite-stdlib` crate, no `read_file` /
`now` / `random` primitive, and the corpus exercises only `fx pure`. **Every REQ
below is NOT-STARTED**, tracked under epic **#62** (no separate blocker is filed —
#62 owns this stage; a gap needing an independent blocker is noted with a fresh
`#`). Stage 3 depends on three SHIPPED mechanisms it composes (it invents none):
the `#[boundary]` form + L1-enforced contract (`#16`,
`.design/boundary/ffi-boundary.md`), the verify-through-the-contract composition
lowering (`#52`, `.design/lower/boundary-composition.md`), and the seccomp runtime
sandbox + fx→syscall table (`#57`, `.design/forge/runtime-sandbox.md`).

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
  failure/EOF outcome cannot be silently dropped.
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

## The verus mechanism (GROUNDED — `verus 0.2026.05.24`)

An effect primitive is a `#[verifier::external_body]` fn carrying a real
`requires`/`ensures` contract: Verus ASSUMES the contract at every call site and
NEVER checks the foreign body (the body is the real syscall). A pure caller then
verifies THROUGH that assumed contract. Authoring harnesses (run against the real
`verus` binary; scratch removed):

**(1) The effect primitive + the compose-through proof.** Three primitives — `now`
(`Time`), `read_byte -> Option<u8>` (`Read`, a read that can short/EOF), and
`read_small -> u64` with a non-trivial assumed `ensures r < 256` — plus pure
callers that prove REAL properties using ONLY the assumed `ensures`:

```rust
#[verifier::external_body]
fn read_small() -> (r: u64) ensures r < 256, { unimplemented!() }   // the syscall, by fiat

fn doubled_fits() -> (out: u64)
    ensures out < 512,
{ let b = read_small(); b + b }      // proves out < 512 THROUGH read_small's assumed ensures
```

→ `verus compose.rs`: **`2 verified, 0 errors`** (exit 0, default mode). The pure
caller `doubled_fits` reaches a real L3 proof using only the primitive's assumed
`ensures r < 256` — the §9 / `#52` verify-through-the-contract. The full harness
(`now`, `read_byte` with a `match` on the EOF case, `read_small`, two callers)
verified **`3 verified, 0 errors`**.

**(2) Soundness — the caller cannot manufacture a guarantee the contract does not
deliver.** A caller `fn bad() -> (out: u64) ensures out <= 50, { now() }` (claims
`<= 50`, which `now`'s `ensures t >= 0` does not deliver):

```
7 | fn bad() -> (out: u64) ensures out <= 50, { now() }
  |                                ^^^^^^^^^   failed this postcondition
verification results:: 1 verified, 1 errors      (exit 1)
```

A COUNTEREXAMPLE, not a false L3. The external_body assumes ONLY the primitive's
`ensures`; the caller still proves its own postcondition. This is the `#52`
soundness property instantiated for effect primitives.

### THE LEGITIMATE-`external_body` DISTINCTION (load-bearing — `#52`/`#60` honesty gate)

`--no-cheating` (the flag that proves the CORE has no proof cheat) **bans
`external_body` entirely** — GROUNDED, the same `compose.rs` under `--no-cheating`:

```
error: external_body/assume_specification not allowed with --no-cheating
  --> compose.rs ...   fn read_small() -> (r: u64)
```

This is the distinction the doc must pin, exactly: `--no-cheating` is for CORE
logic, where `external_body` WOULD be a proof-dodge (the `#60`-style cheat
R-DEFER-9 forbids — never `external_body`/`#[verifier::external]`/`assume(false)`
to dodge a proof of code we wrote). An effect primitive is NOT core logic: it is a
**declared trust boundary** — a `#[boundary]` fn whose body is genuinely foreign
(the syscall), with no Thermite body to prove. For a boundary, `external_body` is
the HONEST modeling of a foreign function (`#52` honesty argument, pinned hard):

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
   disk): `write_file` ens "the bytes were handed to the OS" (a status `result`,
   not a claim about durability); `read_file` ens the SHAPE of the result
   (`Result<bytes, Error>`, Stage 1 ADT) — never WHICH bytes. See OQ-1 on
   inherently-uncertain syscalls.
2. **The effect (`fx`)** — the §4.1 effect atom this primitive carries
   (`Effect::Read`/`Write`/`Net`/`Alloc`/`Time`/`Rand`). This is the TYPED effect;
   the §4.1 row-subsumption check (`.design/lower/effect-subsumption.md`, SHIPPED)
   makes every transitive caller declare it.
3. **The lowering (`#[verifier::external_body]`)** — the primitive is woven into a
   caller's sub-program as an external_body signature (`#52` REQ-1,
   `lower_external_body_fn` in `lower.rs`): the assumed `requires`/`ensures` with
   NO checked body. Verus assumes the contract; the foreign body is never examined.
4. **The sandbox confinement (`#57`)** — `forge build --entry <fn>` derives the
   transitive `fx` row and installs a seccomp-bpf allowlist
   (`sandbox::syscall_allowlist` over `transitive_fx`, `.design/forge/runtime-sandbox.md`),
   so a primitive declared `fx read(_)` is confined to the `read(_)` syscall set
   (`openat`/`read`/`close`/`statx`/`newfstatat`) — a `write`/`socket` attempt is
   killed by the kernel (`SIGSYS`).

The body is trusted-by-fiat but effect-CONFINED and contract-STATED. That triple
(stated + typed + confined) is what makes a one-line foreign syscall an honest,
auditable member of the TCB rather than an opaque trust hole.

### The stdlib (one primitive family per effect atom)

Each family maps to an `Effect` atom and the §57 fx→syscall allowlist it implies
(the `.design/forge/runtime-sandbox.md` mapping table is the authority — this doc
does not redefine it):

| Effect atom (`enum Effect`) | Primitive family | Sketch contract (assumed) | `fx` row | Sandbox allowlist (the #57 table) |
|---|---|---|---|---|
| `Read(path)` | `read_file`, `read_stdin` | `ens` shape only: `Result<bytes, Error>` (Stage 1 ADT) — never WHICH bytes | `fx read(path)` | `openat`, `read`, `close`, `lseek`, `statx`, `newfstatat` |
| `Write(path)` | `write_file`, `print` | `ens` the bytes were handed to the OS (a status `result`), not durability | `fx write(path)` | `openat`, `write`, `fsync`, `newfstatat` |
| `Net(domain)` | `net_connect`, `net_send`, `net_recv` | `ens` shape of the connection/transfer result; `recv` may short/EOF (`Option`/`Result`) | `fx net(domain)` | `socket`, `connect`, `sendto`, `recvfrom`, `setsockopt`, `getsockopt` |
| `Alloc` | `allocate`, `box` (ties to Stage 1 `Box`) | `ens` a live, distinct allocation (the Stage 1 `Box<T>` heap primitive) | `fx alloc` | baseline (`mmap`/`munmap`/`brk`/`mprotect`) |
| `Time` | `now` | `ens` shape only: a `u64` timestamp — never a specific instant | `fx time` | `clock_gettime`, `clock_nanosleep` |
| `Rand` | `random` | `ens` shape only: a `u64` — explicitly NO distribution/unpredictability claim | `fx rand` | `getrandom` |

`Panic` and `Diverge` are effect atoms but NOT data-returning syscall primitives:
`panic` rides the baseline (`write`+`exit_group`, the L1 contract-violation path),
and `diverge` adds no syscall (it is the non-termination effect — see the
interactive-program note). They appear in the lattice and the row but are not
members of THIS stdlib's syscall-primitive families.

### GROUNDED — handled-or-loud on every arm (`verus 0.2026.05`)

The OQ-1 resolution — a boundary contract is honest iff its outcome space is
totally covered and the caller's `ens` holds on EVERY arm — was run against the
real `verus 0.2026.05.24` binary during authoring (scratch removed). The harness:
a `read` boundary returning a closed outcome space `Result<u64, ReadErr>`
(`external_body`, the syscall by fiat), a PURE caller that `match`es BOTH arms, and
the caller's own `ensures` proven to hold on EACH arm (the Ok/handle path AND the
Err/scream path):

```rust
pub enum ReadErr { Eof, Io }

#[verifier::external_body]
fn read_small() -> (r: Result<u64, ReadErr>)
    ensures match r { Result::Ok(v) => v < 256, Result::Err(_) => true },  // closes the SET
{ unimplemented!() }                                                       // the syscall, by fiat

fn read_capped() -> (out: u64)
    ensures out < 256,                       // holds on the Ok arm AND the Err arm
{
    match read_small() {
        Result::Ok(v)  => v,                 // HANDLE: proven correct (v < 256 from the ens)
        Result::Err(_) => 0,                 // SCREAM-and-recover: typed Err arm, also < 256
    }
}
```

→ `verus`: **`2 verified, 0 errors`** (exit 0, default mode — `read_capped` plus a
second-order consumer `doubled` that proves `out < 512` through `read_capped`'s
contract). The caller proves its postcondition on EVERY arm using ONLY the
primitive's assumed outcome-set `ensures`. **Negative control (outcome-coverage is
load-bearing, not vacuous):** make the Err/scream arm return a wrong value
(`Result::Err(_) => 999` under `ensures out < 256`) — verus FAILS
`0 verified, 1 errors` (the unhandled-correctly arm is caught). Cheat-token grep
(`assume`/`admit`/`verifier::external`/`--no-cheating`): NONE — the lone
`external_body` is the LEGITIMATE boundary model (the §52/§60 honesty gate:
`external_body` iff a declared boundary; banned under `--no-cheating`, which guards
the CORE). This proves "handled-or-loud-on-every-arm" is real: the boundary is
strong on the outcome SET (closed + each arm constrained + each arm forced handled),
silent on WHICH outcome — and a caller that mishandles the scream arm does NOT
verify.

### The TCB / honesty story (the load-bearing point — §1, §9, R-DEFER-9)

§9 states the trusted computing base is **exactly (slag blocks ∪ boundary
contracts ∪ the toolchain itself)**, and it is *enumerable*. The effect-primitive
stdlib is the boundary half of that union for I/O: a program = **verified pure
logic (L3) + this enumerated, contracted, confined effect base**. The audit
manifest (`#15`, `.design/forge/audit-manifest.md`, `AuditManifest` `tcb` section)
enumerates each primitive a program reaches — name, assumed contract, foreign
target, effect — so a skeptical third party reads the *entire* fiat-trusted base
in minutes (§1). The honesty chain, end to end:

- A program using `read_file` is **verified-to-the-boundary** (`#17`,
  `AssuranceScope::ToBoundary { via: read_file }`): its pure logic is L3-proved,
  but the whole-program guarantee depends on `read_file` honoring its assumed
  contract. The manifest marks this honestly — it never claims "verified, period"
  when an effect primitive is reached (`goal.md` R-DEFER-9, R-CHAR-3).
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

### Interactive / server programs (the `fx diverge` composition note)

A long-running server — `loop { let req = net_recv(); let resp = handle(req);
net_send(resp); }` — composes the effect primitives with `fx diverge` (the loop
never halts: §4.1, divergence requires `fx diverge` in the row) into a real
program that is STILL verified. The composition: `handle` (the per-request pure
logic) is L3-proved END-TO-END (`#17`); each crossing through `net_recv`/`net_send`
is verified-to-the-boundary against those primitives' contracts; the loop carries
`fx diverge` (a partial-correctness program — each request handled correctly, the
non-termination declared, not proved away). `forge build --entry` confines the
whole binary to the `net(_)` ∪ `diverge` allowlist (`diverge` adds no syscall,
`#57` table). So "verify anything" extends to a never-halting server: every
request is correctly handled and verified, the I/O is contracted + confined, and
the only non-termination is the explicitly-declared `diverge` of the accept loop.

## Requirements

- **REQ-1 (the effect-primitive declaration form):** each effect primitive is a
  `#[boundary("<syscall-target>")] fn NAME(params) -> ret req … ens … fx <atom> ;`
  — the `#16` bodyless-boundary surface form (`FnItem { boundary: Some(_), body:
  None }` in `ast.rs`), a mandatory contract, a declared effect atom, and a `;`
  body. Derived from `thermite-design.md` §9 (the boundary module is a foreign fn
  given a Thermite signature) + §4.1 (the effect row) + `#16`
  (`.design/boundary/ffi-boundary.md` REQ-1/REQ-2). No new grammar — the stdlib
  reuses the boundary form verbatim.

- **REQ-2 (one primitive family per effect atom — the stdlib):** the stdlib
  defines, for each non-`Panic`/`Diverge` atom of `enum Effect`
  (`Read`/`Write`/`Net`/`Alloc`/`Time`/`Rand`), the primitive family of the
  [stdlib table](#the-stdlib-one-primitive-family-per-effect-atom): `read_file`/
  `read_stdin` (`Read`), `write_file`/`print` (`Write`), `net_connect`/`net_send`/
  `net_recv` (`Net`), `allocate`/`box` (`Alloc`, ties to Stage 1 `Box`), `now`
  (`Time`), `random` (`Rand`). Each carries its assumed contract + the effect atom
  + the `#57` syscall allowlist its effect implies. Derived from §4.1 (the effect
  lattice this stdlib instantiates) + the §1 "verify anything" thesis + `01-adts.md`
  (the `Alloc`/`Box` tie).

- **REQ-3 (a boundary contract is honest iff it TOTALLY COVERS its outcome space
  — the resolved honesty seam):** a boundary/effect-primitive contract is HONEST
  not by making a strong claim about the world, and not by a blanket
  vacuity-exemption, but by **closing its outcome SET and forcing the caller to
  handle every arm.** The primitive's return type is a Stage-1 ADT `Result<T, E>` /
  `Option<T>` (the closed outcome space — Ok/value OR a typed Err/scream); its `ens`
  constrains the SHAPE of each arm (e.g. the Ok value's bound), never WHICH arm the
  world produces; and the caller's exhaustive `match` (`01-adts.md` REQ-5/REQ-12) is
  FORCED to resolve EVERY arm — handle-or-scream on each. The honesty test is
  therefore OUTCOME-COVERAGE — *is every arm of the returned sum type resolved by
  the caller, and does the caller's own `ens` hold on EACH arm (the Ok/handle path
  AND the Err/scream path)?* — NOT "is the value-promise strong?". The §7 vacuity
  battery's weak-`ens` rule, applied to a `#[boundary]` contract, checks
  OUTCOME-COVERAGE (REQ-6 / OQ-1 resolution), it does NOT fire merely because the
  value-promise is weak: a foreign syscall's honest contract genuinely IS weak about
  the world, and that weakness is legitimate precisely because the outcome SET is
  closed and totally handled. **GROUNDED** (`verus 0.2026.05.24`, [grounding
  below](#grounded-handled-or-loud-on-every-arm-verus-0202605)): a `read_small ->
  Result<u64, ReadErr>` external_body whose `ens` closes the outcome set, a pure
  caller that `match`es BOTH arms with its own `ens` holding on EACH
  (`2 verified, 0 errors`), and a negative control where the Err/scream arm returns
  a wrong value FAILS (`0 verified, 1 errors`). The fiat line is a KNOB: model more
  failure variants → more arms forced handled → more verified; the unmodeled
  remainder is manifest-enumerated (§9 TCB). Derived from §9 (the contract is the
  interface) + `goal.md` R-DEFER-9 (honest — neither vacuously-strong nor a free
  weak pass) + `01-adts.md` REQ-5/REQ-12 (exhaustive `match` = handled-or-loud) +
  the **#62** outcome-coverage resolution. (Resolves the honesty-seam OQ.)

- **REQ-4 (the effect is typed + the primitive lowers via `external_body`):** each
  primitive's `fx` atom is the §4.1 typed effect, checked by the SHIPPED
  row-subsumption (`.design/lower/effect-subsumption.md`) so every transitive
  caller declares it; and the primitive lowers into a caller's sub-program as a
  `#[verifier::external_body]` signature (`#52` REQ-1, `lower_external_body_fn` in
  `lower.rs`) — assumed `requires`/`ensures`, no checked body — so the caller
  proves THROUGH the contract. Derived from §4.1 (the typed row) + §9/§52 (the
  verify-through-the-contract composition) + the GROUNDED `2 verified, 0 errors`.

- **REQ-5 (the effect is runtime-sandboxed — confined to its syscalls):** a
  `forge build --entry` of a program reaching an effect primitive installs the
  `#57` seccomp allowlist for the primitive's `fx` atom (the
  [stdlib table](#the-stdlib-one-primitive-family-per-effect-atom) / the `#57`
  fx→syscall table), so the primitive is confined to EXACTLY its effect's
  syscalls — a primitive that attempts a syscall outside its atom's allowlist is
  killed by the kernel (`SIGSYS`). Derived from §4.1 ("killed at the syscall
  boundary, not trusted at the type level alone") + `#57`
  (`.design/forge/runtime-sandbox.md` REQ-1/REQ-3).

- **REQ-6 (the TCB / verified-to-the-boundary honesty story):** a program reaching
  an effect primitive certifies its pure logic at `Level::L3` while recording
  `AssuranceScope::ToBoundary { via: <primitive> }` (`#17`), the audit manifest
  (`#15`) enumerates each reached primitive (name + assumed contract + foreign
  target + effect) as a member of the TCB (slag ∪ boundary ∪ toolchain), and the
  manifest NEVER claims "verified, period" for such a program. The effect base is
  the enumerated, contracted, confined trusted set. Derived from §1 (trust
  relocation / the auditable residue) + §9 (the enumerable TCB) + `#15`/`#17` +
  `goal.md` R-DEFER-9 (honest enumeration of the entire fiat-trusted base).

- **REQ-7 (the legitimate-`external_body` distinction):** `external_body` is
  emitted for an effect primitive ONLY because it is a declared `#[boundary]` fn
  (`FnItem.boundary.is_some()`) — the honest foreign model, NOT a `#60`-style
  core-logic cheat. `--no-cheating` (which guards the core) BANS `external_body`;
  the effect-primitive boundary is verified in default mode (where the boundary is
  honest). A regular Thermite fn is always fully proved; no `external_body` is
  emitted for it. Derived from `#52`/`#60` honesty gate (`external_body iff a
  declared boundary/slag`) + `goal.md` R-DEFER-9 + the GROUNDED `--no-cheating`
  ban.

## Acceptance criteria

ACs tie to a NEW `conformance/effect-stdlib/` oracle the ORCHESTRATOR authors (a
hand-derived cases file, the `conformance/composition/cases.json` /
`conformance/sandbox/cases.json` precedents — R-CHAR-3, expected values hand-
derived from the contracts + verus/seccomp semantics, NEVER copied from toolchain
output). The centerpiece corpus program — a pure computation reading input via
`read_file`, computing, and writing the result via `write_file`:

```thermite
#[boundary("os::read_file")]
fn read_file(path: &[u32]) -> Result<Bytes, IoError>   // Stage 1 ADT return
  req true
  ens true                  // honest: shape only, never WHICH bytes
  fx  read(path)
;

#[boundary("os::write_file")]
fn write_file(path: &[u32], data: &[u32]) -> Result<(), IoError>
  req true
  ens true                  // honest: handed to the OS, never durability
  fx  write(path)
;

fn process(path: &[u32]) -> u64    // the VERIFIED PURE LOGIC (L3)
  req path.len() <= 4096
  ens result <= 1_000_000
  fx  read(path), write(path)      // row subsumes both primitives
{
  match read_file(path) {
    Result::Ok(bytes) => { let n = compute(bytes); /* write_file(path, ...) */ n }
    Result::Err(_)    => 0,        // FORCED to handle the failure case
  }
}
```

- **AC-1 (verified-to-the-boundary):** `forge check` of the program — the pure
  logic (`process`/`compute`) certifies `Level::L3` (its body SMT-proves against
  `read_file`/`write_file`'s assumed contracts via the `#52` external_body weave)
  AND `assurance_scope == ToBoundary { via: "read_file" }` (`#17`); `read_file` /
  `write_file` themselves certify `Level::L1`, `boundary == true`, with their
  syscall targets. The pure logic is verified to the boundary, the boundary is the
  primitives' assumed contracts. (Oracle: a `conformance/effect-stdlib/cases.json`
  entry; the `#16` `boundary.cert.json` precedent for the L1 primitive certs.)

- **AC-2 (the audit manifest enumerates the primitives as the TCB):** `forge audit`
  of the program emits an `AuditManifest` (`#15`) whose `tcb` section enumerates
  `read_file` and `write_file` (name + assumed contract + foreign target + effect)
  as `boundary_contracts` members; the pure logic appears in `functions` as L3 +
  `to_boundary`; nothing fiat-trusted is omitted (R-DEFER-9). (Oracle: the
  `conformance/effect-stdlib` audit oracle; the `#15` audit-manifest precedent.)

- **AC-3 (`forge build` sandbox-confines each primitive to its effect):** `forge
  build --entry process` derives the transitive `fx` row (`read(path)` ∪
  `write(path)`) and installs the union seccomp allowlist (`#57`); a
  `--sandbox-self-test` probe of a syscall OUTSIDE that union (e.g. `socket` —
  `Net` is not in the row) is KILLED (`SIGSYS`, exit 159), while the `read`/`write`
  syscalls the primitives need are allowed. This proves the confinement is fx-
  DERIVED and per-effect. (Oracle: a `conformance/effect-stdlib` sandbox case;
  the `#57` `sandbox/cases.json` precedent — exit 159 = 128+SIGSYS(31).)

- **AC-4 (honest-contract / soundness — no manufactured guarantee):** a caller that
  asserts an `ens` STRONGER than a primitive's assumed contract delivers (e.g.
  claims `read_file` returns specific bytes) FAILS verification with a `postcondition
  not satisfied` counterexample, NOT a false L3 (the GROUNDED harness (2)). The
  assumed `ensures` is a floor the caller cannot exceed. This is the anti-cheat AC
  (R-DEFER-9). (Oracle: a `conformance/effect-stdlib` soundness case.)

- **AC-5 (`external_body` iff boundary — the honesty gate):** the lowered verus for
  the pure logic contains an `external_body` signature for `read_file`/`write_file`
  (woven boundary deps) and contains NO `external_body` for any regular fn
  (`process`/`compute` are fully proved); and the pure existing corpus
  (`sum`/`binary_search`) emits NO `external_body` at all (every dependency is a
  `spec fn` / combinator). This is the load-bearing `#52` OQ-1 gate. (Oracle: the
  `#52` `composition.verus.rs` golden precedent + the unchanged pure corpus.)

- **AC-6 (corpus unaffected):** the existing pure corpus (`sum`, `binary_search`)
  certifies an IDENTICAL cert before and after Stage 3 — `Level::L3`, `assurance_scope`
  END-TO-END, no `external_body`, no sandbox kill — and the frozen golden
  `conformance/sum.cert.json` is byte-stable. (Oracle: the unchanged existing
  golden certs.)

## Architecture

Stage 3 owns NO new mechanism — it is a stdlib of `#[boundary]` fns plus the
`conformance` oracle that exercises the SHIPPED `#16`/`#52`/`#57` machinery for the
syscall families. The expected layout is a NEW `thermite-stdlib` crate of effect
primitive declarations (the route the orchestrator must add — see
[Routes to add](#routes-to-add-orchestrator)):

```text
forge check <program using read_file>
  │
  ├─ gate_fn: read_file (#[boundary]) -> BoundaryL1 cert (L1 + boundary flag)  [§16/§9, SHIPPED]
  │
  ├─ for the pure logic `process` (ProceedToL3):
  │     item_subprogram weaves read_file/write_file as #[verifier::external_body]  [#52, SHIPPED]
  │        ▼
  │     run_verus: `process` PROVES through the primitives' assumed ensures -> L3   [#52, GROUNDED]
  │        ▼
  │     closure::classify -> process.assurance_scope = ToBoundary { via: read_file }  [#17, SHIPPED]
  │
forge audit <program>
  │     AuditManifest.tcb enumerates read_file/write_file as boundary_contracts     [#15, SHIPPED]
  │
forge build --entry process <program>
  │     sandbox: transitive_fx (read(path) ∪ write(path)) -> seccomp allowlist      [#57, SHIPPED]
  │     a syscall outside the union -> SIGSYS kill                                    [#57, GROUNDED]
```

- **The primitives** are `#[boundary("os::…")]` fns in `thermite-stdlib`. The
  surface form is `#16`'s verbatim (`parse_attribute` + the `Semi`-body path in
  `parser.rs`; `FnItem { boundary: Some, body: None }` in `ast.rs`). The
  syscall-target string (`"os::read_file"`) is the foreign-target datum the L1
  wrapper calls and the audit manifest enumerates.
- **The compose-through** is `#52`'s `lower_external_body_fn` (in `lower.rs`) woven
  by `check::item_subprogram` — the pure logic resolves the primitive and proves
  against its assumed `ensures` (GROUNDED).
- **The confinement** is `#57`'s `sandbox::syscall_allowlist` over
  `sandbox::transitive_fx` (in `forge/src/sandbox.rs`), keyed on the primitive's
  `fx` atom via the `#57` fx→syscall table.
- **The honesty surface** is `#17`'s `AssuranceScope::ToBoundary` (in `closure.rs` /
  `manifest.rs`) + `#15`'s `AuditManifest.tcb` (in `forge/src/audit.rs`).

Stage 3's only NEW artifacts are the primitive `.th` declarations (the
`thermite-stdlib` crate) and the `conformance/effect-stdlib` oracle. The Stage 1
hook: a richer primitive contract returns a Stage 1 ADT `Result`/`Option`
(`read_file -> Result<Bytes, IoError>`), so the caller is forced to handle failure
(REQ-3). The Stage 5 hook: the composition-aggregation law (`05-…`, OUT of scope
here) AGGREGATES assurance across exactly these boundaries — Stage 3 supplies the
per-boundary `to_boundary` certs Stage 5's law sums into a project-level claim.

## Verification

- **Routes to add (orchestrator, not this doc):** add `[[route]]` entries mapping
  each `thermite-stdlib/src/effect/<atom>.rs` (or the chosen layout) → this doc,
  with `reference = ["conformance/effect-stdlib"]`. The spec-discipline hook
  (R-XLATE-2/R-XLATE-3) blocks the builder until both the route and this doc exist
  (this doc satisfies the latter). See [Routes to add](#routes-to-add-orchestrator).
- **Oracle (orchestrator-authored):** a `conformance/effect-stdlib/cases.json` hand-
  derived fixture file (the `conformance/composition/cases.json` /
  `conformance/sandbox/cases.json` precedents) carrying AC-1..AC-6's programs and
  their expected per-fn `level` + `assurance_scope` + (for the primitives)
  `boundary`/`boundary_target`/`effects`, the audit `tcb` enumeration, and the
  sandbox exit/signal. The cert-oracle / audit-oracle / sandbox-oracle tests
  (`forge/tests/`) run `forge check`/`audit`/`build` over each and assert the
  emitted subset against this golden file.
- **Golden lowering (R-CHAR-3):** a `tests/golden/lower/effect-stdlib.verus.rs`
  hand-authored from THIS design — the pure-logic program lowered, showing the
  `#[verifier::external_body] fn read_file(...) requires …, ensures …, {
  unimplemented!() }` signatures woven before the pure logic — which MUST itself
  pass the real `verus` with 0 errors (the load-bearing external truth; the
  GROUNDED `2 verified, 0 errors` / `3 verified, 0 errors` is the existence proof).
- **Soundness test (AC-4):** a `forge` test asserting an `ens`-overclaiming caller
  emits a NON-L3 cert with a `postcondition not satisfied` counterexample (GROUNDED
  harness (2)), never a false L3 (R-DEFER-9 anti-cheat).
- **Honesty-gate test (AC-5):** assert the lowered string contains `external_body`
  IFF the woven dep carries `#[boundary]`, and the pure corpus emits none (the
  `#52` OQ-1 load-bearing invariant).
- **Crate gauntlets (`goal.md` R-DEFER-6):** `cargo test -p forge`, `cargo test -p
  thermite-lower`, `cargo test -p thermite-stdlib`, `cargo clippy -p <crate>
  --all-targets -- -D warnings`, `cargo fmt --check`, plus the conformance corpus
  (`sum`/`binary_search` stay L3 + END-TO-END, AC-6; the effect-stdlib fixtures
  reach L3 + to_boundary, are enumerated in the TCB, and are sandbox-confined).

## Open questions

- **OQ-1 (the honesty seam — RESOLVED via OUTCOME-COVERAGE; #62
  design-refinement).** *(This is the "OQ-2 honesty seam" the #62 pass names — in
  this doc it has always been OQ-1.)* The question was: how does the assumed
  `ensures` of an inherently-uncertain syscall (read can fail/short/EOF; write can
  partial; connect can refuse) stay HONEST without being vacuous? **RESOLVED — a
  boundary contract is honest iff it TOTALLY COVERS its outcome space, NOT by a
  strong world-claim and NOT by a blanket vacuity exemption** (REQ-3, reframed). The
  uncertainty lives in the RETURN TYPE — a Stage-1 ADT `Result<T, E>` / `Option<T>`,
  a CLOSED outcome set; the `ens` constrains the SHAPE of each arm, never WHICH arm;
  and the caller's exhaustive `match` (`01-adts.md` REQ-5/REQ-12) FORCES every arm
  resolved with the caller's own `ens` proven on EACH (the Ok/handle path AND the
  Err/scream path). **The §7 weak-`ens` vacuity rule, applied to a `#[boundary]`
  contract, checks OUTCOME-COVERAGE — is every arm of the returned sum type
  resolved? — it does NOT fire merely because the value-promise is weak** (REQ-3).
  This REPLACES the old "exempt boundaries from the weak-`ens` rejection" framing: a
  boundary is not exempt from honesty, it is held to a DIFFERENT honesty test (the
  outcome SET is closed + must be handled) appropriate to a foreign function (REQ-3 is the honesty test; REQ-6 keeps the TCB enumeration honest). The
  builder + critic MUST pin that the §7 boundary path checks outcome-coverage (every
  arm handled), and that this does NOT leak the value-strength exemption to regular
  fns (which are held to the full §7 battery). GROUNDED (`verus 0.2026.05.24`,
  [grounding above](#grounded--handled-or-loud-on-every-arm-verus-0202605)):
  `read_small -> Result<u64, ReadErr>` + a both-arms-handled caller `2 verified, 0
  errors`; the Err-arm-returns-wrong-value negative FAILS `0 verified, 1 errors`.
  The fiat line is a KNOB — model more failure variants → more arms forced handled →
  more verified; the unmodeled remainder is manifest-enumerated (§9 TCB).

- **OQ-2 (stdlib crate layout + skill budget):** where do the primitives live —
  a `thermite-stdlib` crate of `.th` declarations, a built-in module the skill
  ships, or `conformance/effect-stdlib/stdlib.th`? The §10 skill is budgeted at
  ≤6,000 tokens and "the stdlib surface is in the skill too"; the effect-primitive
  families must fit (one attribute + one contract each, the `#16` minimal form).
  LEANING: a `thermite-stdlib` crate whose primitive signatures the skill generator
  embeds. The orchestrator decides the crate/route shape; this doc governs the
  effect-primitive contract regardless.

- **OQ-3 (the `Alloc`/`box` primitive vs Stage 1 `Box`):** Stage 1 (`01-adts.md`
  Decision) already ties `Box<T>` construction to `fx alloc` and the baseline
  `mmap`/`brk` syscalls. Does Stage 3 add a SEPARATE `allocate`/`box` boundary
  primitive, or is `Box<T>` construction itself the `Alloc` primitive (no boundary
  fn, the `alloc` effect carried by any constructing fn)? LEANING: `Box<T>`
  construction IS the `Alloc` primitive — it needs no `#[boundary]` syscall wrapper
  (the Rust allocator is the foreign body, confined by the baseline allowlist), so
  `Alloc` is the one atom whose "primitive" is a language construct, not a
  `#[boundary]` fn. Confirm against `01-adts.md` REQ-3 when Stage 4 (collections)
  generalizes the heap primitive.

- **OQ-4 (foreign body execution + cross-platform):** `#57` notes that compiling
  the foreign/boundary BODIES so they actually run + are confined is OUT of #57
  (the foreign target is external), and the sandbox is x86_64-Linux only. Stage 3
  declares the primitives + their contracts + their fx→syscall mappings; whether
  the v0.1 demonstration runs a REAL `read_file` syscall or the `#57` probe
  (`--sandbox-self-test`) stands in for the I/O attempt is the orchestrator's
  demonstrability call. The CONTRACT + the TYPED effect + the ENUMERATED TCB are
  fully specifiable in v0.1 regardless; the live foreign-body run is forward work.

## Routes to add (orchestrator)

```toml
# Effect-primitive standard library — verified, sandboxed #[boundary] syscall
# primitives, one family per effect atom (basis Stage 3, epic #62)
[[route]]
crate_pattern = "thermite-stdlib/src/effect/read.rs"
design = ".design/basis/03-effect-stdlib.md"
reference = ["conformance/effect-stdlib"]
conformance_ops = ["read_file_to_boundary", "process_l3", "audit_enumerates_tcb"]

[[route]]
crate_pattern = "thermite-stdlib/src/effect/write.rs"
design = ".design/basis/03-effect-stdlib.md"
reference = ["conformance/effect-stdlib"]
conformance_ops = ["write_file_to_boundary"]

[[route]]
crate_pattern = "thermite-stdlib/src/effect/net.rs"
design = ".design/basis/03-effect-stdlib.md"
reference = ["conformance/effect-stdlib"]
conformance_ops = ["net_recv_short_read", "server_loop_diverge"]

[[route]]
crate_pattern = "thermite-stdlib/src/effect/alloc.rs"
design = ".design/basis/03-effect-stdlib.md"
reference = ["conformance/effect-stdlib"]
conformance_ops = ["alloc_box"]

[[route]]
crate_pattern = "thermite-stdlib/src/effect/time.rs"
design = ".design/basis/03-effect-stdlib.md"
reference = ["conformance/effect-stdlib"]
conformance_ops = ["now_to_boundary"]

[[route]]
crate_pattern = "thermite-stdlib/src/effect/rand.rs"
design = ".design/basis/03-effect-stdlib.md"
reference = ["conformance/effect-stdlib"]
conformance_ops = ["random_to_boundary"]
```

The orchestrator authors `conformance/effect-stdlib/cases.json` (the oracle this
doc's ACs cite), the `tests/golden/lower/effect-stdlib.verus.rs` golden, and the
routes above + the `thermite-stdlib` crate scaffold. This doc does NOT author the
oracle, the golden, or the routes (R-DOC-1). The crate/file layout above is a
LEANING (OQ-2); the orchestrator settles it.

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (effect-primitive declaration form) | NOT-STARTED | epic #62, Stage 3. No effect-primitive stdlib exists: there is no `thermite-stdlib` crate and no `#[boundary("os::…")]` syscall primitive anywhere in the tree. The SHIPPED prerequisite form (`#16` `FnItem { boundary: Some, body: None }` in `ast.rs`, `parse_attribute` + the `Semi`-body path in `parser.rs`) is the substrate this stage instantiates, but no syscall primitive is declared against it. |
| REQ-2 (one primitive family per effect atom — the stdlib) | NOT-STARTED | epic #62, Stage 3. The `enum Effect` atoms (`Read`/`Write`/`Net`/`Alloc`/`Time`/`Rand` in `ast.rs`) exist and are unexercised by the corpus (`fx pure` only); no `read_file`/`write_file`/`net_*`/`now`/`random` primitive family is defined. The §57 fx→syscall table (`runtime-sandbox.md`) ships, but no primitive maps onto it. |
| REQ-3 (boundary contract honest iff TOTAL OUTCOME-COVERAGE — honesty seam RESOLVED) | NOT-STARTED | epic #62, Stage 3 (design-refinement: honesty seam resolved via outcome-coverage, OQ-1). No primitive contract exists yet to cover. The Stage-1 ADT `Result`/`Option` returns + exhaustive `match` (`01-adts.md` REQ-5/REQ-12) this REQ requires are themselves NOT-STARTED. The RESOLUTION is GROUNDED (`verus 0.2026.05.24`: both-arms-handled caller `2 verified, 0 errors`; Err-arm-wrong-value negative `0 verified, 1 errors`), not implemented. Prereq: Stage 1 ADTs for the closed-outcome-set return shape. |
| REQ-4 (typed effect + `external_body` lowering) | NOT-STARTED | epic #62, Stage 3. The SHIPPED `#52` `lower_external_body_fn` (in `lower.rs`) + `check::item_subprogram` weave and the SHIPPED row-subsumption (`effect-subsumption.md`) are the mechanism, GROUNDED here (`verus 0.2026.05.24`: a compose-through proof `2 verified, 0 errors`), but no effect primitive is lowered through them — there is no primitive to weave. |
| REQ-5 (runtime-sandboxed — confined to its syscalls) | NOT-STARTED | epic #62, Stage 3. The SHIPPED `#57` `sandbox::syscall_allowlist` over `sandbox::transitive_fx` (in `forge/src/sandbox.rs`) + the fx→syscall table are the confinement mechanism, but no effect primitive declares an `fx` atom for them to confine; no `conformance/effect-stdlib` sandbox case exists. |
| REQ-6 (TCB / verified-to-the-boundary honesty story) | NOT-STARTED | epic #62, Stage 3. The SHIPPED `#17` `AssuranceScope::ToBoundary` (in `closure.rs`/`manifest.rs`) + `#15` `AuditManifest.tcb` (in `forge/src/audit.rs`) are the honesty surface, but no program reaches an effect primitive, so no `to_boundary` scope is recorded for one and no primitive is enumerated in a TCB. |
| REQ-7 (legitimate-`external_body` distinction) | NOT-STARTED | epic #62, Stage 3. The distinction is GROUNDED (`verus 0.2026.05.24`: `external_body` verifies `2 verified, 0 errors` in default mode, but `--no-cheating` errors `external_body/assume_specification not allowed with --no-cheating`) and pinned by the SHIPPED `#52`/`#60` honesty gate (`external_body iff a declared boundary/slag`), but no effect primitive exercises it — there is no `#[boundary]` syscall primitive to lower. |
