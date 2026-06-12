# RATIONALE — the metaphor-to-mechanism resolution layer

The [README](README.md) uses plain terms: "a cage," "the ladder," "promises," "a
certificate," "kill the mutants." This file makes each of those terms precise. For
each, it records what established concept it *is* (with lineage), the concrete
mechanism that implements it (file + symbol pointers into this repo), the
engineering tradeoff that decided it over the alternatives, the limits (where it
does *not* protect), and the planned direction (with a tracking pointer).

Lineage attributions are to the published literature; they are *context*, not
project claims. The three pieces this project asserts as genuinely novel (the
caged quantifier fragment, the anti-Goodhart battery, and the effect-row→seccomp
derivation) are marked as such and cite the SOTA survey
([`.design/research/formal-methods-sota.md`](.design/research/formal-methods-sota.md)),
which records each as a **GENUINE EXTENSION** with the survey's *own* hedge on
each, rather than a flat claim:

- the **caged quantifier fragment**: "no *direct* analogue in the surveyed
  verified-compilation lit; **needs a targeted survey to confirm novelty**";
- the **anti-Goodhart battery**: "no analogue in the surveyed
  verified-compilation lit";
- the **effect-row→seccomp hybrid**: the surveyed effect lit "does not combine
  static effect typing with runtime syscall confinement," so the hybrid is
  asserted *by absence* and (survey gap #3) "**needs its own survey**" of the
  row-effect + seccomp/CHERI literature.

Each is "no analogue *in the surveyed literature*," a bounded, falsifiable claim
about a specific survey rather than an absolute novelty assertion.

No new claims are made here. This document only assembles, in standard
vocabulary, what the code and the `.design/` docs already establish.

---

## The contract (`req` / `ens` / `fx`)

**Definition.** Pre/postconditions plus an effect annotation: **design-by-contract**
(Meyer/Eiffel) realized as machine-checked **Hoare logic** (Hoare 1969;
modern push-button precedent: Dafny, Verus, F*). `req` is the precondition, `ens`
the postcondition (over a distinguished `result` binding and the function's
parameters; Thermite has no `old(_)` pre-state construct, and the spec grammar
exposes `result` plus the in-scope parameters and nothing more), and `fx` is an
**effect row** (its own entry below). Thermite's departure from the lineage is
that all three are **mandatory syntax**: omitting one is a compile error rather
than a lint.

**Mechanism.** The contract is parsed into `Contract { req, ens, fx }` clauses
(`thermite-syntax/src/ast.rs`); `thermite-spec`'s `validate` (`validator.rs`)
enforces that every contract-position expression stays inside the frozen
sublanguage; `thermite-lower` lowers `req`/`ens` to Verus `requires`/`ensures`
and `fx` to a compile-time subsumption check (`effects.rs`) plus a runtime
sandbox (`forge/src/sandbox.rs`). See
[`.design/spec/spectherm-combinators.md`](.design/spec/spectherm-combinators.md),
[`.design/lower/effect-subsumption.md`](.design/lower/effect-subsumption.md).

**Why this design.** The alternative, contracts as optional annotations, is what
every prior contract system chose, and it is why they did not change behaviour at
scale: the path of least resistance is to write no contract, so most code carries
none, so the guarantee is opt-in and therefore absent where it matters. Thermite's
bet (README "The problem") is that AI agents flip the keystroke economics, so the
tool can afford to make the strict path the *only* path. Mandatory contracts are
the anti-Goodhart precondition: a tool that grades contract strength (the battery,
below) is meaningless if a function can score perfectly by declaring no contract
at all. Making the contract mandatory closes that escape before the battery runs.

**Limits / failure modes.** A *stated* contract does not guarantee *correct
intent*: `ens` can be mechanically satisfiable yet say the wrong thing (`ens true`
is the degenerate case). Mandatoriness buys presence rather than meaning; the
battery (below) attacks the weak-but-present case, and the gap between the formal
spec and the human's actual intent is the irreducible residual (`thermite-design.md`
§1 "spec-intent alignment," never machine-closed).

**Direction.** Strengthening probes (§7 step 5,
[`.design/forge/strengthening-probes.md`](.design/forge/strengthening-probes.md))
*propose* a tighter `ens` that still proves against the unchanged body, moving a
present-but-weak contract toward a stronger one without authoring it for the user.

---

## The ladder (L3 / L2 / L1 / L0)

**Definition.** Four assurance tiers, each a distinct established verification
technique:

- **L3** = **SMT-discharged deductive verification**, proven for *all* inputs.
  Via Verus (the Rust verifier) emitting verification conditions to **Z3**
  (the SMT solver). Total correctness over the frozen fragment.
- **L2** = **bounded model checking** (Kani/CBMC lineage; Alive2 for the
  "sound-for-reported-violations, incomplete" framing), proven for all inputs up
  to a stated size bound.
- **L1** = **runtime contract monitoring**, the contract compiled to
  always-active assertions that abort on violation (design-by-contract's runtime
  half).
- **L0** = **a trusted-by-fiat annotation**, CompCert's "reduced trusted base"
  concept made per-function; the `#[slag]` escape hatch (own entry below).

**Mechanism.** `enum Level { L0, L1, L2, L3 }` (`forge/src/manifest.rs`, derived
`Ord` so `L0 < L1 < L2 < L3`). The default `forge check` path attempts L3 and
auto-degrades on a *timeout*: `forge::degrade::run_ladder` (`forge/src/degrade.rs`)
drives `L3Verdict::Proved → certify L3`; `Timeout → attempt L2 (lower_l2 →
run_kani) → … → L1 (lower_l1)`. The three-way verdict that drives it is
`classify_verus_outcome → VerusOutcome { Proved, Timeout, Counterexample }`
(`forge/src/check.rs`). See
[`.design/forge/degrade-ladder.md`](.design/forge/degrade-ladder.md).

**Why this design (the degrade-never-on-counterexample policy).** A solver gives
three answers rather than two: *proved*, *disproved* (with a counterexample), and
*could not decide in budget* (timeout / `unknown`). The ladder degrades **only the
third**. A `Counterexample` (the solver found an input where the contract is
false) is a hard failure that never degrades to a lower rung (`ladder_action_l3`
maps it to `LadderAction::HardFail`, `forge/src/degrade.rs`). The alternative,
softening a disproof into "it passes at L1," would hide a *known* bug behind a
lowered-assurance stamp, the worst outcome and the failure §12 ("bounded checks
oversold as proofs") exists to prevent. Degrading inconclusiveness keeps the gate
from *blocking* an agent on a hard SMT goal (§5.2 "the gate degrades, it never
blocks"); refusing to degrade falsity keeps it from reporting a known bug as
passing. The two rules pull in opposite directions, and the policy resolves every
ambiguity toward "never hide a bug" (R-DEFER-9; the L2 counterexample-vs-under-bound
split, the riskiest case, defaults to treating an ambiguous failure as a
counterexample, `.design/forge/degrade-ladder.md` OQ-2).

**Limits / failure modes.** L3 is total correctness *relative to Z3's soundness*
(see the Lean proof spine entry). L2 is bounded and can miss a bug above its size
bound (this is by design, the BMC tradeoff). L1 catches violations only at
runtime, on the inputs actually executed. L0 is trusted, with no further check. A
`fx diverge` function is *capped* at L1 (partial correctness) because it may not
terminate, so it cannot claim L3-total: a structural cap decided before the prover
runs, distinct from a timeout degrade (`.design/forge/degrade-ladder.md` REQ-9).

**Direction.** A standing background proof-repair loop (§5.2, #18) drives
degraded L1/L2 items back up toward L3; the proof-backend interface
([`.design/verified/proof-backends.md`](.design/verified/proof-backends.md))
generalizes the ladder so an L3 can be discharged by Lean as well as Verus, with
a smaller trusted base.

---

## The "cage" / seccomp sandbox

**Definition.** A **seccomp-BPF syscall filter**, the same in-kernel
system-call filtering mechanism Docker and Chrome use to confine processes. The
`fx` row is compiled to a syscall **allowlist**; a syscall outside it makes the
Linux kernel kill the process with `SIGSYS`. This is the README's "cage."

**Mechanism.** `forge/src/sandbox.rs`: `emit_sandbox_prelude` hand-builds a
classic `sock_filter[]` BPF program (arch-guard on `AUDIT_ARCH_X86_64`, a
`BPF_JEQ` per allowlisted syscall number → `SECCOMP_RET_ALLOW`, default
`SECCOMP_RET_KILL_PROCESS`) and installs it via raw `extern "C"`
`prctl(PR_SET_NO_NEW_PRIVS)` + `prctl(PR_SET_SECCOMP, SECCOMP_MODE_FILTER)`,
injected as the first statements of the generated `main` (`synthesize_entry_main`
in `build.rs`), *before* the entry function runs. The allowlist is derived by
`transitive_fx`: the union of `manifest::effects_of` over the entry plus its
transitive intra-file call closure (`closure::reachable_in_file_fns`), mapped
token→syscalls by `syscall_allowlist` (`pure` → a minimal run/print/exit
baseline that pointedly *excludes* `openat`/`socket`/`getrandom`/`clock_gettime`;
`read(_)` adds `openat`; `term` adds `ioctl`; etc.). See
[`.design/forge/runtime-sandbox.md`](.design/forge/runtime-sandbox.md).

**Why seccomp (and not ptrace / LSM / AppArmor / containers).** The requirement
is to enforce a *per-function, statically-derived* effect set at the syscall
boundary, with no trusted supervisor process and minimal runtime cost.

- **ptrace** needs a separate tracer process intercepting every syscall, which
  adds a supervisor to the trusted base and a context-switch per call.
- **LSM / AppArmor** are system-administrator policy, configured out-of-band and
  not derived from or pinned to the program's own declared effects; they would
  decouple the runtime grant from the compile-time `fx` row.
- **containers** confine at the wrong granularity (a whole filesystem/network
  namespace) rather than the per-function effect set.
- **seccomp-BPF** runs *in-kernel* with no supervisor, and the filter is a pure
  function of the verified `fx` row: the same `transitive_fx` walk the
  compile-time subsumption check uses, read forward. The grant *is* the
  verified effects. That direct derivation is why seccomp was chosen here.

**Limits / failure modes (the README's "tripwire").**

- **Linux-only, x86_64-only** in v0.1: the filter pins `AUDIT_ARCH_X86_64`;
  other platforms get the `--no-sandbox` no-op fallback
  (`.design/forge/runtime-sandbox.md` OQ-3).
- **Syscall-number granularity only *in v0.1*, a filter choice rather than a
  mechanism limit.** Classic seccomp-BPF exposes the syscall's scalar argument
  *values* in `seccomp_data.args[0..5]`, and a BPF program can `BPF_JEQ` on them
  (Chrome's sandbox filters `ioctl` by its `cmd` exactly this way). What classic
  seccomp-BPF *cannot* do is **dereference a pointer argument**: the kernel forbids
  reading pointed-to memory from a filter precisely to avoid TOCTOU (the argument
  could be rewritten between the filter check and the syscall). A `cmd` (a scalar)
  is therefore filterable, but a *path* or any other string behind a pointer is
  not inspectable at this layer. v0.1's filter matches on `nr` only, an
  implementation choice rather than a mechanism ceiling, so the `term`→`ioctl`
  grant is currently `ioctl`-*broad* (any cmd), documented as the v1 scope
  (`.design/forge/runtime-sandbox.md` OQ-5). **Path-scoping is enforced at the
  language level** (the `fx read(path)` row, compile-time) because seccomp
  *structurally cannot* read the path string; with seccomp as the coarse syscall
  backstop, the two layers are complementary rather than redundant.
- **Memory safety is not this layer's job**; that is Rust's borrow checker /
  LLVM (the target's responsibility, the RustBelt/Stacked Borrows boundary,
  SOTA finding #7).
- **Pure Thermite never *triggers* it.** A pure program issues no disallowed
  syscall, so the filter never fires; the cage's value is confining
  `#[boundary]`/`#[slag]` foreign bodies to their declared `fx`, plus a
  defense-in-depth backstop against a miscompilation. Demonstrated by an explicit
  `--sandbox-self-test` probe (a denied `openat` → exit 159 = 128+SIGSYS, versus a
  clean pure run).

**This is a genuine extension (asserted by absence, survey pending).** The
*hybrid* (a static effect row (`fx`) that is *both* the compile-time subsumption
lattice *and* the source of the runtime syscall allowlist) has no analogue in the
surveyed effect literature (Koka/Eff/Frank do row effects; seccomp/CHERI do
confinement; no surveyed system derives the second from the first). The survey is
explicit that this is **asserted by absence and still needs its own targeted
survey** of the row-effect + seccomp/CHERI literature to confirm (survey gap #3),
[`.design/research/formal-methods-sota.md`](.design/research/formal-methods-sota.md)
terminology-map row "static effect-rows (`fx`) + seccomp confinement."

**Direction.** A scalar-argument-filtering build (a `BPF_JEQ` on the `ioctl` cmd
register, a scalar, so this is within classic seccomp-BPF's ability, narrowing
`term` to `TCGETS`/`TCSETS`) and non-Linux backends are future refinements
(`.design/forge/runtime-sandbox.md` OQ-5/OQ-3). Pointer-behind arguments (paths)
stay at the language layer by mechanism, not by schedule.

---

## The certificate

**Definition.** A **JSON manifest**, the stable, versioned data contract
`forge check` emits per item; the deliverable's machine-readable trust statement
(§5.1, §6, Appendix A). It attests *what was proved and how*; it does **not**
attest *intent*.

**Mechanism.** `struct Certificate { item, level, solver_time_ms,
contract_quality, effects, slag, obligations, suggested_move, … }`
(`forge/src/manifest.rs`), serialized with `serde_json`. Key fields: `level`
(`L0..L3`, the ladder rung); `effects` (the `fx` row); `contract_quality`
(`tautology`, `vacuous_precondition`, `mutants_killed: String` e.g. `"17/18"`,
`survivor`), the battery scores; `obligations: Vec<ObligationResult>`, per
proof obligation, `Discharged` or `Failed` with a `location` + a concrete
`diagnostic` ("counterexamples, not adjectives," §5.1); `slag`/`slag_meta`
(fiat-trust flag + justification). See
[`.design/forge/certificate-manifest.md`](.design/forge/certificate-manifest.md).
The project-level aggregate (`AssuranceManifest`, min-over-functions) and the
full audit deliverable (`AuditManifest`, `forge/src/audit.rs`) build on it.

**Why this design.** The schema is *fixed at its full Appendix A shape now* even
though several producers (the battery, mutation) arrive over later issues:
forward-declared fields are present with non-asserted values rather than absent,
so a later component *fills* a field instead of *reshaping* the schema
(`certificate-manifest.md` "the two-speed schema"). A field add/rename is a design
amendment (R-SPEC-2) rather than a code-local choice, because the certificate *is*
the contract a downstream auditor pins. The non-deterministic `solver_time_ms` is
structurally excluded from the oracle comparison so the cert stays reproducible
(R-CODE-5).

**Limits / failure modes.** The certificate attests the *formal* result rather
than the *intended* one: a `level: L3` cert with `mutants_killed: "18/18"` says
"this body provably satisfies this contract for all inputs, and the contract is
strong enough to catch every mutant," and it says **nothing** about whether the
contract is the one the human wanted (the §1 intent gap, surfaced for human review,
never machine-closed). The version-sensitive battery ratio (`mutants_killed`) is
oracle-*excluded* because a stronger prover may shift it
(`mutation-scoring.md` REQ-8).

**Direction.** Per-obligation engine attribution
([`.design/verified/proof-backends.md`](.design/verified/proof-backends.md)
REQ-4): the cert gains, per discharged obligation, the engine that proved it and
that engine's trust profile, so an auditor sees that an L3-via-Lean enumerates a
smaller trusted base than an L3-via-Verus.

---

## The vacuity battery (the anti-Goodhart layer)

**Definition.** A mandatory anti-gaming layer combining **vacuity detection**
(model-checking lineage; a property that holds trivially is flagged) with
**mutation testing** (DeMillo/Lipton/Sayward 1978; Hamlet), which generates
deliberately-broken copies of a body and requires the contract to *catch* them,
plus a **kill-ratio floor** and a **prover-proved-equivalent-mutant exclusion**.
This is the README's "you can't cheat the grade."

**Mechanism.** Run as §7 steps 1–4 inside the `forge check` gate, after a
successful L3 proof:

- **structural triage** (`vacuity::triage`, `forge/src/vacuity.rs`): rejects
  `ens true`, result-omitting, req-implied, and unjustified-maximal-`fx`
  contracts;
- **solver vacuity** (`vacuity_solver::solver_vacuity_check`): two
  inverted-polarity Verus harnesses detect semantic tautology / unsatisfiable
  precondition (a *proved* degenerate-property harness is the failing case);
- **mutation scoring** (`forge/src/mutation.rs`): `generate` produces a frozen,
  deterministic mutant set (operator flips, off-by-ones, early returns, branch
  swaps; `MUTANT_CAP = 64`), each re-lowered and re-verified against the
  *unchanged* contract; a mutant Verus **rejects** is *killed* (the contract
  caught it), and a mutant Verus **proves** is a *survivor* (the contract is too
  weak to tell it from the real body). `MUTATION_FLOOR = 0.60` gates
  certification: `kill_ratio < floor` → does not certify,
  `RejectReason { cause: "WeakContract" }` + the surviving mutant as a precise
  strengthening prompt;
- **equivalent-mutant exclusion** (`equivalent-mutants.md`, `#101`): a survivor
  Verus proves *observably equivalent to the real body under `req`* is dropped
  from the denominator (it was never evidence of weakness). This is
  sound-but-incomplete: a survivor is excluded **only** on a Verus *proof* of
  equivalence, so a genuinely distinguishing survivor is never excluded.

See [`.design/forge/mutation-scoring.md`](.design/forge/mutation-scoring.md),
[`.design/forge/equivalent-mutants.md`](.design/forge/equivalent-mutants.md),
[`.design/forge/vacuity-triage.md`](.design/forge/vacuity-triage.md),
[`.design/forge/solver-vacuity.md`](.design/forge/solver-vacuity.md).

**Why this design.** A *mandatory* contract creates a Goodhart pressure: when the
metric is "has a proven contract," the cheapest way to score is a contract so
weak it proves trivially (`ens true`, or `ens result <= huge_bound`). Vacuity
detection catches the *syntactically/semantically* trivial cases; mutation
scoring catches the harder case, a contract that is non-vacuous yet still fails to
constrain the body, by checking whether it can distinguish the real body from a
sabotaged one. The kill-ratio *floor* (rather than 100%) is the concession that
some mutants are equivalent or unreachable; the *equivalent-mutant exclusion*
makes the floor fair (otherwise an honest forced-output function, `req x == 0,
ens result == 0`, is falsely flagged `WeakContract` because its `return 0` mutant
is *genuinely* indistinguishable). The polarity is inverted by design: a prover
*success* on a mutant is the failing case, which is why the battery is a real
adversary rather than a rubber stamp.

**Limits / failure modes.** Mutation testing is incomplete: the mutant set is
finite and frozen, so a weakness no mutant exercises is not caught (the floor is
a *floor* rather than a proof of contract completeness). The kill ratio is
verus-version-sensitive (a stronger prover proves one more mutant), so it is
asserted as a threshold + a run-equals-run determinism property rather than a
frozen golden string (`mutation-scoring.md` OQ-1). A timeout-on-a-mutant is
conservatively counted *killed* (an unproved mutant is not a survivor;
`mutation-scoring.md` OQ-4).

**This is a genuine extension (bounded to the survey).** The anti-Goodhart battery
(mutation-kill-ratio + vacuity/tautology detection applied to *contract quality*)
has "no analogue in the surveyed verified-compilation lit"
([`.design/research/formal-methods-sota.md`](.design/research/formal-methods-sota.md)
terminology-map row "anti-Goodhart battery" → GENUINE EXTENSION), a claim about
*that* survey rather than an absolute first.

**Direction.** The battery becomes engine-generic
([`.design/verified/proof-backends.md`](.design/verified/proof-backends.md)
REQ-9): a Lean-proven contract still faces mutation, with kill semantics
`Refuted ∪ Unknown-after-attempt` and an honest "untested against engine X" when
no engine's fragment admits a mutant.

---

## `#[slag]`

**Definition.** A **trusted-code annotation**, the explicit and visible escape
hatch for a function whose body is *not* machine-proved. Lineage: Rust's `unsafe`,
Dafny's `assume`, Coq/Lean's `axiom`/`admit`, CompCert's enumerated trusted base
made per-function (SOTA finding #3). It is the README's "L0 — trusted by fiat."

**Mechanism.** `forge/src/slag.rs`: `pub fn validate(&SlagAttr) -> Result<SlagMeta,
SlagError>` requires the three justification fields `reason` / `owner` / `review`
present *and* non-empty (`None` → `MissingField`, empty-after-`trim` →
`EmptyField`). A valid `#[slag]` item is **L3-exempt but L1-enforced**: `forge
check` does not invoke Verus on it; it certifies `Level::L1` with `slag: true`
and the metadata in the certificate, and its contract is still compiled to runtime
checks. It is the only construct that justifies a maximal `fx` row (the §7.1(d)
vacuity interaction), and it remains subject to the vacuity triage rules (a) /
(b) / (c): slag exempts a body from *proving*, never from *stating and checking*.
See [`.design/forge/slag.md`](.design/forge/slag.md).

**Why visible and greppable.** The escape hatch is deliberately less convenient
than the proven path (§8's "polarity inversion"): three mandatory justification
fields and a `grep`-able keyword. The alternative, a quiet pragma or a default-on
"trust me" mode, makes the trusted base invisible and therefore unbounded, and a
reviewer cannot audit what they cannot enumerate. Making non-verification cost
*more* keystrokes and visibility means `grep slag` over a codebase is the
**complete inventory** of fiat-trusted code (§8), and the certificate surfaces
every slag block with its justification.

**Limits / failure modes.** A slag body is genuinely unverified; its correctness
rests entirely on the human reviewer the `review` field names. Slag caps the item
at L1 (runtime contract enforcement), so a slag body's *contract* is still
checked at runtime, but the body's logic below that contract is trusted. A slag
block is a hole in the proof; the hole is *enumerated* rather than absent.

**Direction.** CI policy hooks that cap slag count or require second-party
sign-off (§8) are a later policy layer; the `forge audit` TCB section
([`.design/forge/audit-manifest.md`](.design/forge/audit-manifest.md)) already
enumerates every slag block as part of the trusted computing base.

---

## The combinator cage

**Definition.** A **fixed, closed quantifier fragment with hand-tuned, frozen SMT
triggers**, the deliberately-weak specification sublanguage (SpecTherm). Eight
bounded combinators stand in for raw `forall`/`exists`. Lineage: the broad idea
of restricting to a decidable/automatable fragment is standard (the "cage = a
decidability/automation lever," SOTA finding #4); the *specific* frozen-trigger
combinator set is a project extension (below).

**Mechanism.** `thermite-spec/src/combinators.rs` ships the frozen registry
(`sorted`, `forall_in`, `exists_in`, `count_where`, `permutation_of`, `disjoint`,
`forall_below`, `forall_from`), each with a name, arity, ordered `ArgKind`s
(`Slice`/`Index`/`Pred`/`Value`), result kind, and a frozen `verus_l3` quantifier
body carrying a pinned `#[trigger]` (e.g. `forall_in(s,p) == forall|i| 0 <= i <
s.len() ==> #[trigger] p(s[i])`). `thermite-spec/src/validator.rs` (`validate`)
enforces that contracts use *only* registered combinators (right name/arity/
arg-kinds), declared `spec fn` calls, and grammar built-ins, and that a
combinator's predicate-closure body is a **flat predicate** (no anonymous nested
combinator; named `spec fn` composition only; REQ-6, the `#40` fix). See
[`.design/spec/spectherm-combinators.md`](.design/spec/spectherm-combinators.md).

**Why no raw `forall`.** Unrestricted quantifiers in an SMT backend are the
primary source of proof *instability*: the solver instantiates quantifiers via
**trigger** (e-matching) heuristics, and small edits to a formula can flip a
proof to a timeout or matching loop as the solver's trigger inference changes
(§13 risk "small edits flip proofs to timeouts"). The alternatives,
(a) letting users write raw `forall` and rely on the solver coping, or (b)
requiring users to hand-author triggers, both expose the agent to that
instability and make proof success non-reproducible. The cage instead offers a
*fixed* library where every quantifier is bounded (eliminating unbounded-quantifier
blowup) and carries a *frozen, hand-tuned* trigger (removing the heuristic
variance). The restriction is what makes proof automation *predictable*, and
predictable automation is what makes the Lean proof spine and the mutation battery
*feasible* (see the frozen-subset entry). What is excluded: anonymous nested
quantification (`forall_in(xs, |x| exists_in(ys, |y| …))`), raw `forall`/`exists`,
and unbounded recursion without a `dec` measure.

**Limits / failure modes.** The fragment is *deliberately* less expressive than
full first-order logic: a property no combination of the eight combinators (plus
named `spec fn`s) can express simply cannot be stated. Adding a combinator is a
slow, budget-gated RFC rather than a user-level abstraction (§11), so
expressiveness grows only by deliberate design amendment. The invariant is "every
quantifier is a bounded combinator with a frozen trigger; composition is named
(`spec fn`, each `dec`-measured) and never anonymous"; depth is named and bounded
rather than zero (`spectherm-combinators.md` "Thesis-clarification note").

**This is a genuine extension (with the survey's hedge).** The caged quantifier
fragment (the specific bounded-combinator set + frozen triggers) is asserted
novel-by-absence: the SOTA survey records "no *direct* analogue in the surveyed
verified-compilation lit" and explicitly adds it "**needs a targeted survey to
confirm novelty**"
([`.design/research/formal-methods-sota.md`](.design/research/formal-methods-sota.md)
terminology-map row "caged quantifier fragment"). The claim is bounded to that
survey, not absolute.

**Direction.** New combinators arrive only through the RFC process (§11); the
flat-closure rule (REQ-6, `#40`) tightens the cage where the early
implementation over-permitted nested quantification.

---

## The effect row (`fx`)

**Definition.** An **effect system** (algebraic/row effects: Koka, Eff, Frank,
F* effects) realized as a static lattice over a fixed atom set, with both a
**compile-time subsumption** check and a **runtime derivation** (the seccomp
sandbox). The README's "what I'm allowed to touch."

**Mechanism.** `enum Effect { Read(p), Write(p), Net(d), Alloc, Time, Rand,
Panic, Diverge, Term }` and `enum EffectRow { Pure, Set(...) }`
(`thermite-syntax/src/ast.rs`). `thermite-lower/src/effects.rs` projects a row to
`EffectKind` atoms (`effects`), and `pub fn subsumes` enforces the rule *a
caller's row must subsume every callee's row* (`effects(callee) ⊆
effects(caller)`; `Pure` subsumes only `Pure`); `check_effects` walks every call
site, emitting `LowerError::EffectNotSubsumed { caller, callee, missing, span }`
on a violation. The subset test is delegated to the Verus-verified
`thermite_verified::subsumes_masks` (a 9-atom `u16` bitset, proved, then
cross-checked over all 512×512 mask pairs). At runtime the *same* row drives the
seccomp allowlist (`transitive_fx` → `syscall_allowlist`, the cage entry above).
See [`.design/lower/effect-subsumption.md`](.design/lower/effect-subsumption.md).

**Why this design.** Subsumption is checked *directly* (each call site against the
immediate callee's declared row) rather than via a transitive closure walk,
because every callee's declared row must *already* subsume its own callees
(checked when that callee is analyzed), so direct checking composes to transitive
correctness (`effect-subsumption.md` OQ-2). This is the §9 "trust is invariant
under composition" property: a caller reasons through a callee's *contract* (here
its `fx` row) rather than its body. Path granularity (`write("/tmp")` ⊄
`write("/etc")`) is deferred to a future path lattice; v0.1 is atom-kind level and
explicit about what it enforces (`effect-subsumption.md` OQ-1).

**Limits / failure modes.** v0.1 subsumption is path-*insensitive* (a `Write(_)`
caller subsumes any `Write(_)` callee); path-scoping is stated at the row level
but not yet ordered by path. Compile-time subsumption alone does not *enforce* at
runtime; the runtime half is the seccomp cage (a separate, Linux-only,
coarse-grained layer with its own limits).

**This is part of the genuine extension**: the *hybrid* of static effect typing
*and* runtime syscall confinement derived from the same row (see the cage entry).

**Direction.** A path lattice for path-granular subsumption matching the runtime
sandbox granularity (`effect-subsumption.md` OQ-1).

---

## Translation validation + the Lean proof spine

**Definition.** Two composed techniques answering whether the *translation* is
faithful:

- **Translation validation** (Pnueli/Siegel/Singerman 1998; Necula's GCC
  validator PLDI'00; Alive2 PLDI'21): a *per-run* check that an unverified
  translator produced an equivalent output, via an **independent reference
  encoder** and an SMT equivalence query. It gives *existential* evidence (for the
  programs that were run).
- **A verified validator** (Leroy/CompCert; the CACM "verified validator composed
  with an unverified compiler is as strong as a verified compiler, provided the
  validator is smaller and simpler"): the reference encoder is itself *proven
  sound* in **Lean 4**, lifting the per-run check to a *universal* **semantic
  preservation** guarantee (`S ≈ C`, stated as a forward simulation).

**Mechanism.** Per run: `thermite-tv`'s `equivalence_obligation` family
(`thermite-tv/src/obligation.rs`) emits a self-contained Verus program asserting
`(P_production) <==> (P_reference)`, where `P_reference` comes from the
independent reference encoders (`ref_encode.rs` / `exec_encode.rs` /
`exec_stmt_encode.rs`), forbidden by the build from sharing code with the
production lowerer; Z3 must prove both translations equivalent on every check.
Once and for all: the `lean/Thermite/` spine proves those reference encoders
denotation-faithful against a mechanized semantics `S` (`theorem ref_sound` /
`ref_sound_eq` (`Soundness.lean`), `exec_ref_sound` (`Exec.lean`),
`body_ref_sound` (`Exec/Stmt.lean`), the partial-correctness `while_rule`
(`Exec/Loop.lean`)), composed into `theorem lowering_faithful`
(`Faithfulness.lean`). The Rust encoders are tied to their Lean models by an
arm-by-arm inspection audit with pinned commits
([`.design/verified/rust-lean-correspondence.md`](.design/verified/rust-lean-correspondence.md)).
See [`.design/verified/thermite-semantics.md`](.design/verified/thermite-semantics.md).

**What is proven, precisely (and what is not).** The Lean spine proves (T1) the
reference encoder is sound against `S` (`∀ P, ⟦R(P)⟧ = ⟦P⟧_S`) and composes it
with the per-run Z3 check to get (T2) `⟦lower(P)⟧ = ⟦P⟧_S` for every `P` passing
TV: the lowering *preserves meaning*. This is the **verified-validator
architecture** (Leroy/CompCert lineage): the *small* reference encoder (≈ one
compiler-pass of effort) is verified rather than the thousands-of-lines production
lowerer. The five load-bearing theorems are kernel-checked depending on **only**
the standard Lean axiom set `{propext, Classical.choice, Quot.sound}`, with no
`sorry` and no custom axioms (`make audit` check [1] re-verifies this on the
skeptic's machine).

**The enumerated trust base (the residuals, stated rather than hidden).** Following
CompCert's reduced-trusted-base framing (SOTA finding #3, Leroy's "verification
never eliminates the trusted base, it reduces it to an enumerable set"), an L3
certificate currently trusts the following (this is `scripts/audit.sh` check [6]'s
five-item residual-trust block; the checker the spine itself leans on is item 1):

1. **The Lean kernel + its three standard axioms**, `{propext,
   Classical.choice, Quot.sound}`. The five load-bearing theorems are checked by
   the Lean kernel, so the kernel's soundness and that small, standard axiom set
   are themselves trusted; `make audit` check [1] re-runs the kernel on the
   skeptic's machine and parses exactly this axiom footprint (no `sorry`, no custom
   axiom).
2. **Z3 / Verus soundness**: the per-run TV equivalence is `Z3 ⊢ lower(P) ⟺
   R(P)`; if Z3 is unsound on a query, (T2) inherits it. This is the floor of any
   SMT-discharged result rather than a Thermite-specific gap.
3. **`S` agrees with the *intended* meaning of Thermite**: the most delicate
   item, an unprovable-from-within assumption (Gödel; the §1 spec-intent slot).
   `S` is human-audited; its auditability is the design goal.
4. **The Rust↔Lean encoder correspondence**: that the *Rust* encoders match the
   *Lean*-proved algorithm, discharged at the **inspection tier** (arm-by-arm
   audit + a SHA-pinned drift tripwire) rather than a mechanized extraction bridge.
5. **rustc / LLVM / the build chain**: the `Rust → machine code` link, inherited
   from the Rust toolchain (the RustBelt/Stacked Borrows boundary).

**Z3 demotion (the path to shrink residual 2).** The route to demote Z3 from
*trusted* to *kernel-checked* is **proof-producing SMT + reconstruction**
(Lean-SMT's cvc5 path, SOTA finding #8). `lean/Thermite/SmtDemo.lean` is a
**proven proof-of-concept**: two *real* per-run TV equivalence obligations over
the scalar/QF-linear-integer contract fragment were re-discharged by cvc5 and
**kernel-checked** with the standard axioms only (`#print axioms` → `{propext,
Classical.choice, Quot.sound}`, no `sorryAx`, no cvc5 oracle axiom; a partial-scope
demotion rather than laundering). The remaining obstacles to *full* demotion: an
upstream Lean-SMT `sorry` in the QF_BV (bitwise) reconstruction; ~30% cvc5
proof-rule coverage (quantified/recursive fragments); Verus/Z3 not emitting
reconstructable certificates (requiring the obligation to be re-solved through
cvc5); and a missing Rust→Lean predicate exporter. These are engineering and
upstream-maturation items rather than feasibility ones, documented in
[`.design/verified/z3-demotion.md`](.design/verified/z3-demotion.md).

**Why Lean 4 (and not Coq / Isabelle / Verus-native).** The live route to demote
Z3 is proof reconstruction, and Lean-SMT (cvc5 reconstruction into the Lean
kernel) is the maturing path to it; that TCB-shrink lever drove the choice over
Coq/Isabelle (`thermite-semantics.md` REQ-5). Verus-native meta-theory was
rejected because it would re-trust the very Z3 the spine exists to be auditable
*against* (the meaning of Thermite is defined by the Lean semantics rather than by
Verus; Verus is the first proof engine, proven faithful, rather than the
foundation).

**Limits / failure modes.** The theorem upgrades exactly *one* link (lowering)
from existential to universal; the Verus VC-generator + Z3, the borrow
checker/LLVM, and the source-semantics-agreement assumption stay inherited
(stated above). It is **not** a whole-toolchain or unconditional preservation
proof; the v1 `while` rule is *partial* correctness (termination is the per-run
Verus `decreases` residual). Loops beyond v1 `while`, `break`/`continue`,
multi-exit early `return`, nested loops, and non-scalar mutation `xs[i]=e` are
out of the proven fragment (`thermite-semantics.md` coverage section).

**Direction.** Full Z3 demotion (upstream-gated) and the Lean→Rust extraction
bridge (to upgrade residual 4, the Rust↔Lean correspondence, from inspection to
mechanization). Lean as proof engine #2 has shipped — the next section.

---

## Proof backends (the second engine)

**Definition.** The L3 discharge is routed through a backend-neutral **engine
interface** (`forge/src/engine.rs`,
[`.design/verified/proof-backends.md`](.design/verified/proof-backends.md)): an
engine declares its FRAGMENT (which obligations it can attempt), a DISCHARGE map
onto the three-verdict lattice `Proven / Unknown / Refuted`, a TRUST PROFILE
(the tools a `Proven` obliges you to trust), and a content-addressed EVIDENCE
key. Verus/Z3 is engine #1 (the default); **Lean is engine #2**
(`forge check --engine lean|auto`): obligations are serialized by the
Thermite→Lean exporter (`forge/src/lean_export.rs`) and discharged either by a
tactic battery (the auto tier) or by a replayed interactive proof.

**The discipline is engine-generic.** `Unknown` degrades down the ladder; a
witnessed `Refuted` hard-fails and never degrades — the anti-cheat is stated
once, independent of Verus. A certificate carries per-obligation attribution
`{engine, trust_profile}`, so "L3" names *which* engine proved the item under
*which* assumptions; project aggregation stays the honest min.

**Disagreement is an alarm, not a tiebreak.** Under `--engine auto` both
engines may attempt the same obligation. `Proven ⊕ Refuted` halts the run as a
structured soundness alarm naming both engines and the obligation; it is never
resolved by preference — one of the two provers (or the exporter between them)
is wrong, and surfacing exactly that event is the architecture's purpose.
`Proven ⊕ Unknown` is benign (one engine simply could not decide).

**Interactive proofs without an injection surface.** A hard obligation emits a
Lean skeleton (`<file>.lean-proofs/<item>.lean`) carrying an evidence-key
header; the replay reconstructs the file from the canonical generator, so the
*only* author-controlled text is the proof term after the obligation theorem's
`:=`. `sorry` and non-standard axioms are rejected from the obligation
theorem's own `#print axioms` report; a stale evidence key forces
re-derivation. This canonical-reconstruction design closed a five-generation
adversarial bypass arc (#248–#252) in which `notation`/`macro` poisons
re-elaborated the obligation's conclusion to `True` under a clean axiom report
— the lesson being that a blocklist over a Turing-complete elaborator is
unsoundable, so the author-helper surface was deleted rather than filtered.

**Limits / failure modes.** The Lean engine's exportable fragment today is pure
contracts plus straight-line executable bodies; loops and user-ADT `match` are
the named next increments (the spine's `while_rule` already exists, so the loop
extension is engineering rather than new theory). Non-exportable items are
honest `Unverifiable` skips, never false verdicts. Verus remains the sole
engine for the meta/battery queries (vacuity, mutation, strengthening) in v1;
the Lean-path mutation battery covers only the items Lean discharges, with
untested-against-Lean mutants reported and never counted as killed.

**Direction.** Widen the exporter past straight-line bodies (`while` first);
grow the auto-tier tactic coverage; converge with the Z3-demotion arc above.

---

## Typed holes (`?N`) + the goal REPL

**Definition.** **Typed holes** with an incremental **goal state**, the
Agda/Idris interactive-hole and Lean `sorry`-with-goals lineage. A `?N` marks an
unfinished body position; the REPL shows what is given and what must be achieved.

**Mechanism.** The lexer emits `TokKind::Hole(N)` for `?<digits>`
(`thermite-syntax/src/lexer.rs`); the parser accepts it *only* in fn-body
statement position (a `?N` in a spec clause / expression / `spec fn` is a
structured `SyntaxError`, never a panic), recording `FnItem.holes`. `forge
goal <item>` (`goal_repl::render_goal`) renders the §5.1 four-part view
(given / want / per-obligation status with concrete counterexamples / open
holes); `forge fill <addr> <code>` (`goal_repl::fill_hole`) splices code at the
hole's span, re-parses, re-checks, and prints the new goal state (which may
surface new holes); `forge edit`/`battery` are the sibling verbs. The
**never-certifies gate**: `forge check`'s per-item loop short-circuits any holed
`FnItem` to a non-certifying `Level::L0` cert with an `OpenHole` reject cause
**before** lowering or Verus, so a holed program cannot certify. See
[`.design/forge/goal-repl.md`](.design/forge/goal-repl.md).

**Why this design.** This realizes the README's "like a conversation": declare the
contract, leave the body a hole, let the agent fill it and immediately re-check,
with failures coming back as concrete counterexamples (`lo=3, hi=3, mid=3`) rather
than adjectives. The hole gate is the safety interlock: it makes "incomplete" a
*verdict* (non-certifying), so an unfinished program can never masquerade as a
proved one.

**Limits / failure modes.** v1 holes are fn-body-statement position only (no
holes in expressions, signatures, spec clauses); there is no incremental
hole-id stability across fills (holes re-number on re-parse); `fill`/`edit`
re-run the *whole-item* check (the proof cache makes unaffected items cheap), not
incremental obligation-level re-solving (`goal-repl.md` "v1 scope").

**Direction.** Incremental obligation-level re-checking and richer hole positions
are future work; the goal REPL is the v0.1 surface of the §5.1 dialogue.

---

## The frozen subset (the central design *why*)

**Definition.** Thermite is a deliberately **small, frozen language**: a fixed
sublanguage of constructs (the eight combinators, a bounded exec expression set,
straight-line bodies + v1 `while`, `dec`-measured spec functions, the nine effect
atoms). CakeML's "end-to-end verified compilation of a real language, but only
over a *fixed subset*" (POPL'14, SOTA finding #5) is the existence proof that a
frozen subset can be carried to a universal correctness theorem.

**Mechanism.** The freeze is enforced at every layer: `thermite-spec`'s validator
rejects out-of-cage contracts; the reference encoders return `Err`
(`RefEncodeError::Unsupported`) on any node outside the subset; the Lean `Expr` /
`Block` inductives (`lean/Thermite/Ast.lean`) model *exactly* the frozen subset;
the skill (`THERMITE.skill.md`) is regenerated from the registry and CI-gated so
a new construct without a skill entry is a compile error. The denotation domain
of `S` is precisely the union of the three encoders' admitted-node sets
(`thermite-semantics.md` AC-1).

**Why this is THE central design why.** The strong properties of Thermite are
*purchased* by the weakening. The direct answer to "what does the restriction add"
is that the machine-checked soundness proof was only completable because the
fragment is frozen and finite.

- The Lean proof spine (`lowering_faithful`) proves soundness by *structural
  induction over the AST*; that induction is finite, and the proof *closeable*,
  because the construct set is closed and frozen. An open or unbounded surface
  language would have no finite induction to perform.
- The mutation battery is feasible because the mutator families are a fixed,
  finite set over a fixed construct set (`MUTANT_CAP = 64`).
- The combinator cage is *predictable* (frozen triggers, bounded ranges) only
  because the combinator set is closed.
- Z3's automation is reproducible only over the decidable/automatable fragment
  the freeze defines.

The alternative, a large, evolving, general-purpose surface, is what every
unverified language is, and it is *why* their meta-properties are not
machine-checked: an induction over a moving target cannot be finished. The freeze
is the *lever* that makes the strong properties (the universal preservation
theorem, the feasible battery, the predictable solver) achievable. The restriction
purchases the proof.

**Limits / failure modes.** The frozen subset is genuinely less expressive than a
general language; constructs outside it (user-ADT `match`/`is` in the *proven*
fragment, multi-exit control flow, non-scalar mutation, nested loops) are out
(`thermite-semantics.md` coverage; README "Deferred (tracked)"). Growing the
language is a deliberate, RFC-gated, proof-extending act rather than a free
addition.

**Direction.** The proven fragment grows construct-by-construct, each addition
extending the Lean spine and the battery; the named residuals (user-ADT
`match`/`is` in the proven fragment, the basis v1.1 layer) are tracked in the
README's "Deferred" list and the verified-docs coverage sections.

---

## The kernel target

**Definition.** `forge build --target kernel`, a freestanding `no_std + alloc`,
OS-less library build profile, the road toward a verified microkernel (§13).

**Mechanism.** A codegen-profile fork of `forge build` (`forge/src/build.rs`,
`enum BuildTarget { Std, Kernel }`): emits a `#![no_std]` + `extern crate alloc;`
rlib (`--crate-type=rlib -C panic=abort`), reusing `thermite_lower::lower_l1`'s
output verbatim (the L1 checks + `TString`/`TVec`/`TMap` wrappers resolve against
`alloc`), with **no** `main` and **no** seccomp sandbox. It adds one new reject:
`reject_ambient_fx_for_kernel` scans every function's `transitive_fx` for
`KERNEL_REJECTED_FX = ["read","write","net","term","time","rand"]` and refuses
before codegen, because kernel code has no ambient userspace syscall surface (and
no ambient clock/entropy). The admit set is exactly `pure`/`alloc`/`panic`/`diverge`.
The L3 verification path is *target-independent* (untouched). See
[`.design/build/kernel-target.md`](.design/build/kernel-target.md).

**Why this design.** Because rustc is the codegen backend (§3), a "target" is a
rustc-invocation + crate-prelude choice rather than a compiler change, so the same
verified, L1-lowered program links into a kernel as into a userspace binary. The
ambient-`fx` reject is the principled boundary: a syscall mapping is a userspace
seccomp concept with no kernel analogue, and the #198 amendment moved
`time`/`rand` into the reject set after a real `fx time` boundary leaked a
std-bodied `SystemTime::now()` (`E0433`) into the `no_std` crate; the doc adapts
to the code (R-DOC-1), documenting the reject the code now enforces.

**Limits / failure modes.** v1 emits a *library* (no `main`, no panic
handler/global allocator; the kernel host supplies them, `kernel-target.md`
OQ-1), and a final `bin`/`staticlib` link is out of v1. L1 checks fire as `panic!`
→ the host's `#[panic_handler]` under `panic=abort`; whether the abort is
observable is the host's responsibility.

**Direction.** A `--target kernel-bin` profile (default abort handler) and the
verified-microkernel convergence (§13).

---

## `make audit`

**Definition.** A skeptic's one-command re-derivation of the entire trust chain on
their own machine, ending with the honest residual-trust statement. The README's
"don't trust us — audit it."

**Mechanism.** `Makefile` `audit` target → `scripts/audit.sh` runs six checks:
**[1]** builds the `lean/` spine from source and parses `#print axioms` for the
five load-bearing theorems (`lowering_faithful`, `ref_sound`, `exec_ref_sound`,
`body_ref_sound`, `while_rule`), passing only if each depends on nothing beyond
`{propext, Classical.choice, Quot.sound}` (no `sorry`, no custom axiom);
**[2]** full-corpus translation validation (`forge tv`/`exec-tv`/`body-tv` over
every `conformance/` program, zero divergences); **[3]** the falsification
battery (inject known translation-bug classes, assert Z3 catches each) + one
visible end-to-end mutant; **[4]** the Rust↔Lean correspondence drift tripwire
(pinned SHAs vs current); **[5]** the committed proof re-verified under
third-party Verus/Z3 with `forge` excluded; **[6]** the verdict + the residual
trust statement. Each guarantee-bearing check **skips visibly** (degrading the
verdict to `INCONCLUSIVE`, nonzero exit) when its tool is absent, so it cannot be
mistaken for a pass. `make audit-fast` is the 60-second A/B demo. See
[`.design/forge/audit-manifest.md`](.design/forge/audit-manifest.md) for the
`forge audit` manifest format.

**What each check re-derives, and the residual.** Checks [1]–[5] re-derive,
respectively, the universal faithfulness theorem (the auditor's Lean kernel), the
per-run TV agreement (the auditor's Z3, every corpus program), the falsification
teeth, the correspondence-non-drift, and the third-party proof re-check. What
remains **trusted** after a clean run (check [6]'s list): the Lean kernel + its
three standard axioms; Z3/Verus soundness (with the `z3-demotion.md` PoC already
covering the scalar core); the spec↔intent gap; the pinned inspection audit;
rustc/LLVM. Everything else is re-derived on the auditor's machine.

**Why this design.** Every claim in the README is "the kind of thing a liar could
also type" (README). The audit addresses this: a trust statement that *hides* its
assumptions is incomplete, so the audit *enumerates* the residual rather than
claiming zero trust, and refuses to claim success when a tool is missing.

**Limits / failure modes.** The audit re-checks the *committed* proof and corpus;
it cannot re-derive the residual trust items (that is what makes them residual).
A missing prover/kernel degrades to `INCONCLUSIVE` rather than a false pass.

**Direction.** As Z3 demotion lands, residual item 2 (Z3/Verus soundness) shrinks
and check [1]'s coverage grows.

---

## Directions (the active roadmap)

- **Lean as proof engine #2 — shipped, now widening**: the backend-neutral
  `Obligation` + `Engine` interface, the Lean exporter, `--engine verus|lean|auto`,
  per-obligation engine attribution, and the disagreement alarm are live
  (#204/#240/#247–#253; the section above). The open work is fragment width:
  the exporter stops at straight-line bodies today (`while` is next — the spine's
  rule is already proven).
  [`.design/verified/proof-backends.md`](.design/verified/proof-backends.md).
- **Full Z3 demotion**: close the upstream Lean-SMT QF_BV `sorry`, raise cvc5
  proof-rule coverage, and re-solve TV obligations through cvc5 so `h_tv` becomes
  kernel-checked rather than Z3-trusted.
  [`.design/verified/z3-demotion.md`](.design/verified/z3-demotion.md)
  (upstream-gated; the scalar core is already a proven PoC).
- **The extraction bridge**: a mechanized Lean→Rust extraction (or a Rust-side
  proof) that makes the Rust reference encoder equal the Lean model *by
  construction*, upgrading the Rust↔Lean correspondence residual from inspection
  to mechanization.
  [`.design/verified/rust-lean-correspondence.md`](.design/verified/rust-lean-correspondence.md)
  REQ-2.
- **The microkernel**: the kernel target's destination, a verified, OS-less
  microkernel linked from proven Thermite rlibs (§13).
  [`.design/build/kernel-target.md`](.design/build/kernel-target.md).

---

## What this document is

This is the **metaphor→mechanism resolution layer**. The README uses plain terms
("a cage," "the ladder," "promises," "kill the mutants") for communication. This
file makes each of those terms *precise*: each resolves to an established concept
with a name and a lineage, a concrete mechanism with file+symbol pointers into
this repo, the engineering tradeoff that chose it over its alternatives, the limit
where it stops protecting, and the tracked direction it is heading. Nothing here
is a new claim; it is the assembly, in standard PL/systems vocabulary, of what the
code and the `.design/` docs already establish, so that a skeptical reader can
confirm that every term the project uses names a real thing.
