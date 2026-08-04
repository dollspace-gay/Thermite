# THERMITE.skill.md Generator + 6k-Token Budget Gate

<!--
tier: 3-component
status: draft
audited-sha: 92396428567edc6940a9e2845217f5ff4c2ea3c6 (re-pinned 2026-06-16, user-authorized: the only change to this doc's governed files since the prior pin is the additive stage-1 forge-tier increment 2a — the new Item::Forge surface + inert Item::Forge match arms, verified net-additive with no substantive removal of existing v1 logic (git log <main>..HEAD = the 8 forge commits); the v1 behavior this doc governs is unchanged, and the new forge-tier surface is specified in .design/stage1-forge-tier.md / REQ-S1-3)
audited-content-sha256: 96997867d6be97100d897f1b7c9fc52c328f6fbb88d3881a63552294bf439d35 (re-pinned 2026-08-04 after documenting distinct-bit and epoch-acknowledgement authoring)
governs: thermite-skill/src/generate.rs
thesis-refs:
  - thermite-design.md §2.2
  - thermite-design.md §10
  - thermite-design.md §4
  - thermite-design.md §4.2
  - thermite-design.md §4.4
  - thermite-design.md §6
  - thermite-design.md §8
  - thermite-design.md Appendix B
-->

## Summary

`thermite-skill` assembles the canonical `THERMITE.skill.md` — the complete
Thermite surface grammar, the SpecTherm combinator library (one example each),
the recursion-scheme library, the Forge command set, the ladder semantics, and
the slag rules — as a single deterministic `String`, and enforces the **≤ 6,000-
token hard budget** that `thermite-design.md` §2.2 makes a pillar and §10 makes a
CI gate.

The skill has two content kinds, and **§10's "the skill IS the spec, no version
skew" pillar is realized mechanically for the parts that drift**:

1. The **SURFACE INVENTORY** (the drifty part — the language's constructs) is
   DYNAMIC, by one of two compiler-backed mechanisms (REQ-8):
   - **Registry-driven** (auto-appears): the SpecTherm combinator library
     iterates `thermite_spec::all()` (already, REQ-2) and the recursion-scheme
     library iterates `thermite_spec::schemes::all()` (REQ-9). Adding a registry
     entry auto-renders into the skill; removing one auto-drops it.
   - **Exhaustive-match-driven** (compile-forced): the type grammar, the
     expression/item/pattern grammar, and the effect atoms are rendered by an
     EXHAUSTIVE `match` (no `_` wildcard) over the definitional enums
     `thermite_syntax::Type` / `Expr` / `Item` / `Pattern` / `Effect` (REQ-10).
     Because the match is exhaustive, **adding a new variant FAILS TO COMPILE
     until its skill arm is added** — the compiler is the freshness enforcer; the
     skill cannot go stale silently (REQ-8, the key property).

2. The **EXPLANATORY PROSE** (the stable part — narrative that cannot be
   mechanically derived: the thesis framing, the §6 ladder semantics, the §8 slag
   rules, the §5.1 Forge framing, the §4.2 flat-closure rule) stays CURATED, but
   guarded by the committed-`==-generate()` freshness test (REQ-5) and the 6k-
   token budget gate (REQ-4) (REQ-11).

The Forge method inventory is no longer prose. `ForgeMethod` and its metadata
registry live in `thermite-skill`; Forge consumes the same registry to recognize
top-level methods and build its usage text, while `render_forge` consumes it to
write the skill. The dependency already points from Forge to `thermite-skill`,
so this removes the old cycle concern without exposing Forge's private parsed
`Command` values. The build record now advertises the paired
`--compose-export`/`--compose-shell` rich-state L3 surface and describes its
output as an exact-source link/composition bundle (#104). It also advertises the
optional `--primitive-registry <registry.json>` refinement input inside that
composition surface; regenerating the skill keeps the synopsis identical in the
CLI and agent reference.

The crate exposes `generate() -> String` (the library API) and a `thermite-skill`
binary (`--emit`, `--check-budget`) that the CI gauntlet runs. The committed
repo-root `THERMITE.skill.md` is the generator's output, kept fresh by an
up-to-date `cargo test`.

**Status as of this amendment.** REQ-1..REQ-7 are SHIPPED (issue #7, commit
`365734e`): `generate()`, the five sections, the machine-rendered combinator
section, the token-count heuristic + budget gate, the committed skill + freshness
test, the bin, and the CI step all exist (`thermite-skill/src/generate.rs`,
`main.rs`). BUT the surface-inventory sections OTHER than the combinator library
(`render_grammar` / `render_forge` / `render_ladder` / `render_slag`) are CURATED
STATIC STRINGS and have **drifted from the shipped language**: the committed
`THERMITE.skill.md` asserts "no `struct`/`enum`" while `thermite-syntax` ships
`Item::Struct` / `Item::Enum` / `Type::Vec` / `Type::String` / `Expr::StrLit` /
`Expr::Is` / `Expr::StructLit` / `Expr::Deref`, and the recursion schemes
(`thermite_spec::schemes::all()` — `fold`/`map`/`for_all`/`exists`/`traverse`) do
not appear at all. REQ-8..REQ-11 (this amendment) make the surface inventory
DYNAMIC so this class of drift becomes a compile error (or auto-tracks the
registry) instead of a silent lie. They are **SHIPPED (issue #84)** — the
exhaustive-match renderers (`render_{type,expr,item,pattern,effect,binop,prim}_arm`)
+ the registry-driven `render_schemes` are in `generate.rs`, the committed
`THERMITE.skill.md` now carries the full Stages-1–8 surface, and the
"no struct/enum" lie is gone (see the REQ status table).

## Scope boundary (what this component owns vs. what it does NOT)

- **Owns** `thermite-skill/src/generate.rs` (the generator), `src/main.rs` (the
  `--emit` / `--check-budget` CLI), the committed `THERMITE.skill.md` at the repo
  root, the up-to-date `cargo test`, and the `--check-budget` CI step.
- **Owns** the public Forge method registry and the two generated skill formats:
  canonical Markdown and Claude-compatible Markdown with frontmatter.
- **Does NOT own** Forge's parsed arguments, dispatch, filesystem writes, or
  exit-code behavior. Those remain in `forge/src/cli.rs`; Forge reads this
  crate's registry and generated strings through its existing dependency.
- **Does NOT mutate any toolchain enum.** The exhaustive-match mechanism READS
  the definitional enums (`thermite_syntax::Type`/`Expr`/`Item`/`Pattern`/
  `Effect`); it adds no variant and changes no AST. The doc adapts to the AST,
  never the reverse (R-DOC-1).

## The dynamic-generation design (the heart of this amendment)

§10 promises "the skill is regenerated from the grammar and combinator registry
… the agent's mental model and the checker are never out of sync." The original
#7 realized this for the combinator registry ONLY (REQ-2) and left the rest as
curated strings — which then drifted through Basis Stages 1–8 (the ADTs, the
recursion schemes, `Vec`, `String`, the effect atoms). This amendment closes the
gap by pinning, per surface-inventory section, **WHICH mechanism keeps it fresh**:

| Skill content | Source of truth | Mechanism | Freshness enforcer |
|---|---|---|---|
| SpecTherm combinators | `thermite_spec::all()` | registry-driven (REQ-2, shipped) | iterate-all coverage test (AC-2) |
| Recursion schemes | `thermite_spec::schemes::all()` | registry-driven (REQ-9) | iterate-all coverage test (AC-9) |
| Type grammar | `thermite_syntax::Type` | exhaustive `match` (REQ-10) | **compiler** (no `_` arm) + AC-10 |
| Expression grammar | `thermite_syntax::Expr` | exhaustive `match` (REQ-10) | **compiler** + AC-10 |
| Item grammar | `thermite_syntax::Item` | exhaustive `match` (REQ-10) | **compiler** + AC-10 |
| Pattern grammar | `thermite_syntax::Pattern` | exhaustive `match` (REQ-10) | **compiler** + AC-10 |
| Effect atoms | `thermite_syntax::Effect` | exhaustive `match` (REQ-10) | **compiler** + AC-10 |
| Forge command list | `thermite_skill::ForgeMethod::ALL` | shared registry | Forge's exhaustive method dispatch + iterate-all coverage |
| Thesis / ladder / slag / §5.1 framing prose | the design narrative | curated (REQ-11) | committed-==-generate (REQ-5) |

**The exhaustive-`match` mechanism (REQ-10), and why it gives the compile-error-
on-new-variant property.** For each definitional enum, the generator holds a
function `fn render_<enum>_arm(&Variant) -> SkillFragment` whose body is a `match`
over EVERY variant with NO `_` wildcard arm. Rust's exhaustiveness checking makes
a non-exhaustive `match` (over a non-`#[non_exhaustive]` enum in the same
workspace) a HARD compile error (`E0004`). Therefore: when a contributor adds
(say) `Type::Map { .. }` or `Expr::Lambda { .. }` to `thermite-syntax`, the
`thermite-skill` crate FAILS TO COMPILE — `cargo build`/`cargo test` go red —
until the contributor adds the corresponding `Type::Map => …` arm emitting that
construct's `{ grammar fragment, one-line description, tiny example }`. The skill
literally cannot ship behind the language. The compiler, not a human's diligence
and not a separately-curated string, is the freshness enforcer. (This is the
exact property the curated strings lacked: each of Stages 1–8 needed a manual
edit, and several were missed — most recently the whole ADT/scheme/`Vec`/`String`
basis.)

**Where the per-variant text lives (the DECISION — exhaustive `match`, not a
`trait SkillEntry`/`SKILL: &[…]` table).** The cleanest realization that gives
the compile-error-on-new-variant property is a per-variant **`match` arm in
`generate.rs`** that returns the fragment for that variant, NOT a `trait
SkillEntry` impl or a `static SKILL: &[…]` table. Rationale:
- A `match` arm IS the exhaustiveness guarantee — adding a variant to the enum
  immediately makes the match non-exhaustive, which is the compile error we want.
  No extra machinery is needed.
- A `trait SkillEntry` on the enum would have to dispatch per-variant
  internally — but a trait method body that itself `match`es the variants still
  needs the exhaustive `match` to get the guarantee, so the trait adds an
  indirection without adding safety. A `static SKILL: &[(Variant, &str)]` table
  is WORSE: a table is NOT checked for completeness against the enum's variants
  by the compiler at all — a new variant compiles fine with a stale table, which
  is precisely the silent-drift failure we are eliminating. Only the exhaustive
  `match` (or a `match` that constructs every arm, e.g. driving an iteration over
  a value of the type) gets the guarantee. So: **exhaustive `match` in the
  generator**. (This is the doc's least-confident decision — see OQ-6 — but the
  table alternative is rejected on the no-completeness-check ground above.)

  A subtlety the builder must respect: some of these enums carry payload (e.g.
  `Type::Ref { mutable, inner }`, `Expr::Binary { op, .. }`). The arm matches the
  variant shape with `{ .. }` / `(_)` field-elisions for payload it does not
  render — the elision does NOT defeat exhaustiveness (it is the VARIANT set, not
  the field set, the compiler checks for completeness). The arm emits a fixed
  representative fragment for that variant kind (e.g. `Type::Ref` → "&T / &mut T
  — a shared/exclusive reference"), so the rendered text is a deterministic
  function of the variant set, not of any particular value's payload (R-CODE-5,
  AC-6). `BinOp` and `PrimType` (the leaf operator/primitive enums) are likewise
  rendered by exhaustive `match` so a new operator/primitive also compile-forces
  a skill entry.

**The Forge command list.** `thermite-skill` owns a small public
`ForgeMethod` enum plus one metadata record per variant (verb, synopsis, and
plain-language purpose). A single macro invocation declares the enum and
registry together, so there is no separately maintained list. Forge parses the
first argument through this registry and exhaustively matches `ForgeMethod` to
produce its private `Command`; the skill and usage banner iterate the same
records. This preserves the dependency direction (`forge` →
`thermite-skill`) and makes the public method surface mechanically current
without exposing Forge internals.

## Requirements

- **REQ-1 (`generate()` API + the canonical sections):** `thermite-skill`
  exposes `pub fn generate() -> String` returning the complete
  `THERMITE.skill.md` as one deterministic string, assembled from the §10
  sections in order: **(1)** the surface grammar, **(2)** the SpecTherm
  combinator library (one example each), **(2b)** the recursion-scheme library
  (REQ-9, this amendment), **(3)** the Forge command set, **(4)** the ladder
  semantics, **(5)** the slag rules. The function takes no arguments, reads no
  environment, no wall-clock, no filesystem — a pure function of the compiled-in
  curated strings, `thermite_spec::all()`, `thermite_spec::schemes::all()`, and
  the exhaustive matches over the `thermite_syntax` enums (R-CODE-5). Source:
  `thermite-design.md` §10. **SHIPPED** (the scheme section 2b lands with REQ-9).

- **REQ-2 (combinator section is MACHINE-RENDERED from the frozen registry):**
  Section (2) iterates `thermite_spec::all() -> &[CombinatorSig]` (per
  `pub fn all in combinators.rs`) and renders, for each entry, the SURFACE
  signature derived from `name`/`arity`/`arg_kinds`/`result` plus one usage
  example, plus the §4.2 flat-closure rule. Every entry and only those entries
  render — adding a combinator auto-appears, removing one auto-drops (§10 anti-
  drift). The verbose `verus_l3`/`l1` bodies are NOT rendered. Source: §10, §4.2.
  **SHIPPED** (`render_combinators` in `generate.rs`).

- **REQ-3 (curated PROSE sections sourced from the design):** The narrative
  content that cannot be mechanically derived stays as templated strings sourced
  from the design: the §5.1 Forge framing, the §6 ladder semantics (incl. the
  L0/slag clarification), the §8 slag rules, and the §4.2 flat-closure prose.
  These are CURATED and versioned-with-the-toolchain. NB: this amendment NARROWS
  REQ-3 — the *structural* surface inventory it formerly covered (the type /
  expression / item / pattern / effect grammar, the forge verb LIST) moves to the
  dynamic mechanisms (REQ-9/REQ-10) or the freshness-tested curated table
  (REQ-11); REQ-3 now covers only the irreducible PROSE. Source: §5.1/§6/§8/§4.2.
  **SHIPPED** (the prose renderers exist; the narrowing is a re-scoping, not new
  code).

- **REQ-4 (deterministic token count + ≤ 6,000-token budget gate):**
  `thermite-skill` defines a documented, deterministic token-counting method
  (`pub fn token_count(s: &str) -> usize` = `(chars*2).div_ceil(7)`, i.e.
  `ceil(chars/3.5)`, integer arithmetic) and a named ceiling
  `SKILL_TOKEN_BUDGET = 6000`. The gate `token_count(generate()) <= 6000` is
  enforced by (a) the bin's `--check-budget` (non-zero exit on overflow) and
  (b) a `cargo test`. A HARD fail (R-DEFER-6); §10 "if it exceeds budget, the
  feature that pushed it over is reverted." This gate is RETAINED unchanged by
  the dynamic refactor: the dynamic surface inventory MUST render concisely (a
  line + a tiny example per construct, not verbose) so the gate keeps holding —
  see "Budget after the refactor" below. Source: §2.2, §10. **SHIPPED**
  (`token_count`/`SKILL_TOKEN_BUDGET` in `generate.rs`).

- **REQ-5 (committed `THERMITE.skill.md` + the up-to-date freshness check):** The
  generator's output is committed at the repo root as `THERMITE.skill.md`,
  regenerated by `forge skill --write THERMITE.skill.md` (the lower-level
  `cargo run -p thermite-skill -- --emit` stdout interface remains available). A
  `cargo test` asserts the committed bytes equal `generate()` exactly, so the
  committed skill can never go stale relative to the generator (§10 "no version
  skew"). `forge skill --check THERMITE.skill.md` exposes the same comparison to
  contributors and CI. Source: §10; R-CHAR-3 (committed artifact == its generator).
  **SHIPPED** (`committed_skill_is_fresh`).

- **REQ-6 (the `thermite-skill` bin — `--emit` / `--check-budget`):** a
  `[[bin]]` with exactly two hand-matched modes — `--emit` prints `generate()`
  and exits 0; `--check-budget` prints the count and exits non-zero iff
  `> SKILL_TOKEN_BUDGET`. No `clap`. No panics in production. Source: `goal.md`
  Gauntlet; Appendix B; R-CODE-2. **SHIPPED** (`main::run`).

- **REQ-7 (CI wiring — the `--check-budget` gauntlet step):**
  `.github/workflows/ci.yml` runs `cargo run -p thermite-skill -- --check-budget`
  as a must-pass step. Source: §2.2/§10; `.design/scaffold/workspace.md` REQ-7.
  **SHIPPED** (the step is in `ci.yml`).

### Dynamic surface requirements

- **REQ-8 (the compiler-enforced no-staleness GUARANTEE — the key property):**
  The skill's SURFACE INVENTORY (the set of language constructs an agent must
  know: the types, expressions, items, patterns, effect atoms, combinators, and
  recursion schemes) is rendered ONLY by mechanisms that mechanically track the
  language — never by a hand-curated list of constructs. Two mechanisms qualify:
  (i) **registry iteration** over a frozen `pub fn all()`-style registry (REQ-2,
  REQ-9), whose new entries auto-render; and (ii) **exhaustive `match`** (no `_`
  wildcard) over a definitional enum (REQ-10), whose new variants make
  `thermite-skill` FAIL TO COMPILE until a skill arm is added. The GUARANTEE
  REQ-8 pins is: **adding a language construct either auto-appears in the skill
  (registry case) or produces a compile error in `thermite-skill` (exhaustive-
  match case) — it can never silently leave the skill stale.** This is the
  mechanical realization of the §10 "the skill IS the spec, no version skew"
  pillar (the curated-string version of which repeatedly drifted: it missed
  `forge build`, the sandbox, then the whole ADT/scheme/`Vec`/`String`/effects
  basis). Source: `thermite-design.md` §10; §2.2. **SHIPPED**.

- **REQ-9 (recursion-scheme section is REGISTRY-DRIVEN from `schemes::all()`):**
  A new skill section (2b, after the combinator library) iterates
  `thermite_spec::schemes::all() -> &[SchemeSig]` (per `pub fn all in schemes.rs`)
  and renders, for each scheme, its name, its call shape derived from
  `scrutinee_args` / `step_shape` / `total_arity` (e.g. `fold(l, init, |x, acc|
  …)`, `for_all(l, |x| …)`), its result kind (`SchemeResult::{Accumulator →
  nat, Bool → bool, SameAdt → the ADT}`), and one tiny example. Every entry and
  only those entries render (the `render_combinators` precedent, REQ-2) — adding
  a scheme to the frozen registry auto-appears, removing one auto-drops. This is
  the registry-driven half of REQ-8 for the schemes. `schemes.rs`'s own doc-
  comment names `thermite-skill #7` as the intended consumer of `all()` ("so a
  later consumer (`thermite-skill` #7) can regenerate the skill's scheme section
  from the single source of truth"), so REQ-9 makes `schemes::all()` a non-test
  production consumer (R-DEFER-1 — `schemes::all()` is an existing pub API; REQ-9
  is its skill consumer). Source: `thermite-design.md` §4.4 (closed built-in
  scheme set), §10 (anti-drift); `.design/basis/02-recursion-schemes.md` REQ-1.
  **SHIPPED**.

- **REQ-10 (type/expr/item/pattern/effect grammar is EXHAUSTIVE-MATCH-DRIVEN):**
  The surface-grammar section's CONSTRUCT INVENTORY — the type forms, the
  expression forms, the item forms, the pattern forms, and the effect atoms — is
  rendered by an EXHAUSTIVE `match` (no `_` wildcard arm) over each of
  `thermite_syntax::Type`, `Expr`, `Item`, `Pattern`, and `Effect` (and the leaf
  operator/primitive enums `BinOp` / `PrimType`), one arm per variant, each arm
  emitting `{ a grammar fragment, a one-line description, a tiny example }` for
  that construct. Because the matches are exhaustive over workspace-local,
  non-`#[non_exhaustive]` enums, a new variant is a HARD compile error (`E0004`)
  in `thermite-skill` until its arm is added (REQ-8). The narrative SCAFFOLDING
  around the construct list (the contract-first framing, the "removed from Rust"
  motivation, the one-call-syntax rule) stays curated prose (REQ-11) — REQ-10
  governs the per-construct INVENTORY, the part that grows with the language and
  drifted. The per-variant arm text is generator-side (the same posture as the
  combinator example table, OQ-2): the enums carry no skill-text field, and skill
  text is a skill concern, not an AST concern (R-DOC-1 — REQ-10 reads the enums,
  never mutates them). Source: `thermite-design.md` §4/§4.2/§4.4/§10;
  `thermite-syntax/src/ast.rs` (`enum Type`/`Expr`/`Item`/`Pattern`/`Effect`).
  **SHIPPED**.

- **REQ-11 (the explanatory PROSE stays curated + freshness-tested):** The
  narrative that cannot be
  mechanically derived from a registry or an enum — the thesis framing, the §6
  ladder semantics, the §8 slag rules, the §5.1 Forge framing, the §4.2 flat-
  closure rule, and the "removed from Rust" motivation — stays CURATED (REQ-3),
  guarded by the committed-`==-generate()` freshness test (REQ-5) and the 6k
  budget gate (REQ-4). The Forge command list is excluded from this requirement:
  it is generated from the shared method registry. This REQ pins the
  prose/derived boundary. Source: §10. **SHIPPED**.

## Acceptance criteria

- **AC-1 (budget gate — generate() is under 6,000 tokens):**
  `token_count(generate()) <= 6000` (the §2.2 symbolic constant, not a value read
  back — R-CHAR-3), asserted by `cargo test` AND by `--check-budget` exiting 0.
  The dynamic surface inventory MUST render concisely enough that this still
  holds after the refactor (see "Budget after the refactor"). (REQ-4, REQ-8)

- **AC-2 (combinator coverage — every entry in `all()`, with an example):** for
  every `CombinatorSig` in `thermite_spec::all()`, `generate()` contains the
  `name` and one example marker — the registry-driven anti-drift coverage test.
  (REQ-2)

- **AC-3 (ladder coverage — L0–L3 all present + the L0/slag clarification):**
  substring assertions, expected strings from §6. (REQ-3, REQ-11)

- **AC-4 (Forge / slag / grammar-keyword coverage):** for every entry in
  `ForgeMethod::ALL`, `generate()` contains its verb and synopsis. The test also
  checks the three mandatory slag fields and grammar clause keywords
  (`req`/`ens`/`fx`/`inv`/`dec`/`spec fn`/`#[slag]`). Adding a Forge method
  expands this check automatically.

- **AC-9 (recursion-scheme coverage — every entry in `schemes::all()`, with an
  example):** for every `SchemeSig` in `thermite_spec::schemes::all()`,
  `generate()` contains the scheme `name` (`fold`/`map`/`for_all`/`exists`/
  `traverse`) and one example marker. Mechanically: a test iterates
  `thermite_spec::schemes::all()` and asserts `contains(name)` + an example
  marker per entry — the registry-driven anti-drift coverage test for schemes
  (the AC-2 analogue). Adding/removing a scheme changes this test's coverage
  automatically. (REQ-9)

- **AC-10 (the compile-forced no-staleness mechanism — STRUCTURAL + coverage):**
  the no-staleness guarantee (REQ-8) is verified two ways.
  (i) **Structural / demonstrative (the mechanism):** the renderer functions for
  `Type`/`Expr`/`Item`/`Pattern`/`Effect` (and `BinOp`/`PrimType`) contain a
  `match` with NO `_` wildcard arm — so a NEW variant added to any of these enums
  makes `thermite-skill` FAIL TO COMPILE (Rust `E0004`) until its arm is added.
  This is demonstrated, not merely asserted: a `// COMPILE-FAIL DEMO` doc block /
  a `trybuild`-style compile-fail fixture (or, at minimum, a doc-comment + an
  inline structural comment naming the no-`_` invariant) shows that a synthetic
  added variant fails to compile against the renderer. (ii) **Coverage (the
  output):** for each of the current shipped variants — at minimum the Stage-1-8
  surface (`Type::Vec`, `Type::String`, `Type::Box`, `Type::Named`,
  `Item::Struct`, `Item::Enum`, `Expr::StructLit`, `Expr::Is`, `Expr::StrLit`,
  `Expr::Deref`, `Expr::Match`, each `Effect` atom) — a test asserts a
  representative substring for that construct appears in `generate()`. Together:
  (i) proves a future variant cannot silently drift, (ii) proves the current
  surface is covered. Expected substrings are derived from the construct's name /
  §4.4, never copied back from the generator (R-CHAR-3). (REQ-8, REQ-10)

- **AC-5 (committed `THERMITE.skill.md` == generate()):** the repo-root file's
  bytes equal `generate()` exactly (regeneration command in the failure message)
  — the generated-file freshness check. (REQ-5)

- **AC-6 (determinism — generate() is pure):** two `generate()` calls and two
  `--emit` runs are byte-identical; no timestamp/path/env/RNG/wall-clock content.
  The exhaustive-match arms emit per-variant text that is a deterministic
  function of the variant set, not of any value's payload (REQ-10). (REQ-1, REQ-4)

- **AC-7 (no panics; Result discipline; clippy clean):**
  `cargo clippy -p thermite-skill --all-targets -- -D warnings` clean; the anti-
  pattern gate passes; no `unwrap`/`expect`/`panic!`/`todo!`/`unimplemented!` in
  non-test code; the bin's fallible paths return `Result`/`ExitCode`. (REQ-1,
  REQ-6)

## Architecture

The component is a single generator module plus a thin binary, in
`thermite-skill/src/generate.rs` and `thermite-skill/src/main.rs`. It depends on
`thermite-spec` (the frozen registries — `pub fn all in combinators.rs`,
`pub fn all in schemes.rs`) and `thermite-syntax` (the definitional enums —
`enum Type`/`Expr`/`Item`/`Pattern`/`Effect` in `ast.rs`). Both deps are already
declared in `thermite-skill/Cargo.toml`; the `thermite-syntax` dep — present-but-
light in the original #7 (the grammar was curated text) — becomes LOAD-BEARING
under REQ-10 (the exhaustive matches walk its enums). Symbol anchors, never line
numbers (R-CITE-2b).

### `generate()` — the assembly (REQ-1)

`pub fn generate() -> String` concatenates the section renderers in §10 order:
`render_grammar` (curated framing + the REQ-10 exhaustive-match construct
inventory), `render_combinators` (REQ-2, registry-driven), `render_schemes`
(REQ-9, registry-driven), `render_forge` (curated framing + the shared
`ForgeMethod` registry), `render_ladder` (curated prose), `render_slag` (curated
prose). Pure: no I/O, env, clock, or RNG (R-CODE-5, AC-6).

### `render_combinators` — the shipped registry renderer (REQ-2)

`render_combinators` iterates `thermite_spec::all()` and renders each entry's
surface signature (`name`/`arg_kinds`/`result` via `render_arg_kind` /
`render_result_kind`) + one example from `example_for`. Unchanged by this
amendment.

### `render_schemes` — the NEW registry renderer (REQ-9)

`render_schemes` iterates `thermite_spec::schemes::all()` and, per `SchemeSig`,
renders the call shape (`name` + the `scrutinee_args` positional args + the
trailing `step_shape` closure — `StepShape::ElementAcc` → `|x, acc|`,
`StepShape::Element` → `|x|`), the result (`SchemeResult::Accumulator` → `nat`,
`Bool` → `bool`, `SameAdt` → the same ADT), and one tiny example, mirroring
`render_combinators` (the `render_one_combinator` precedent). An example table
keyed by scheme name lives generator-side (the `example_for` precedent, OQ-2).
The flat-step-closure rule (the scheme analogue of the §4.2 combinator flat-
closure rule — no nested scheme in a step) is rendered once as prose at the head
of section (2b). The generated-fn lowering names (`fold_<e>` etc.) are NOT
rendered — the skill teaches the surface call, not the lowering (the REQ-2
posture).

### `render_grammar`'s construct inventory — the EXHAUSTIVE matches (REQ-10)

The surface-grammar section keeps its curated narrative scaffolding (the
contract-first framing, the clause-order rules, the loop `inv`/`dec` rule, the
"removed from Rust" motivation, the one-call-syntax rule) but its CONSTRUCT
INVENTORY is driven by exhaustive `match`es. Five (plus two leaf) renderer
functions — `render_type_arm(&Type)`, `render_expr_arm(&Expr)`,
`render_item_arm(&Item)`, `render_pattern_arm(&Pattern)`,
`render_effect_arm(&Effect)`, `render_binop_arm(BinOp)`,
`render_prim_arm(PrimType)` — each a `match` with NO `_` arm, one arm per
variant emitting that construct's fragment + description + tiny example. The
section drives these by enumerating each variant once (e.g. a small fixed list
of representative variant VALUES the renderer maps over, or a `match` invoked per
variant), so that the OUTPUT covers every construct AND the COMPILER guarantees
no variant can be added without an arm. Payload fields the arm does not render
are elided (`{ .. }` / `(_)`); the elision does not weaken exhaustiveness (the
compiler checks the VARIANT set, REQ-10). Per-arm text is generator-side
(R-DOC-1: read the enums, never mutate them).

### The no-staleness guarantee — how a new variant forces a skill entry (REQ-8)

Walk the mechanism concretely. Suppose a future Basis stage adds
`Type::Map { key: Box<Type>, value: Box<Type> }` to `thermite_syntax::ast`
(exactly the `Map<K,V>` deferred in `ast.rs` REQ-2 / epic #62). Then:
`render_type_arm`'s `match self { Type::Prim(..) => …, Type::Vec(..) => …, … }`
is no longer exhaustive — `Type::Map` is unhandled and there is no `_` arm — so
`rustc` emits `E0004: non-exhaustive patterns: Type::Map { .. } not covered` and
`cargo build -p thermite-skill` / `cargo test -p thermite-skill` / the CI gauntlet
all go RED. The build stays broken until a contributor adds
`Type::Map { .. } => SkillFragment { … }` describing the new type in the skill.
The skill cannot ship behind the language because the toolchain will not compile.
Contrast the registry case (REQ-9): adding a `SchemeSig` to `schemes::REGISTRY`
needs no `thermite-skill` edit at all — `render_schemes`'s iteration auto-renders
it (and AC-9's coverage test auto-tracks it). Both mechanisms eliminate silent
drift; the exhaustive-match case is the stronger (compile-error) form for the
open-coded enums, the registry case is the zero-touch form for the closed
frozen registries.

### Token counting + the budget gate (REQ-4) — unchanged

`pub fn token_count(s: &str) -> usize` = `(s.chars().count() * 2).div_ceil(7)`
(`ceil(chars/3.5)`, integer, deterministic, no dep, no model). The 6,000 ceiling
and the divisor are named constants. Gate: `token_count(generate()) <=
SKILL_TOKEN_BUDGET`, asserted by a test and the bin. Unchanged by the dynamic
refactor.

### Budget after the refactor — does 6k still hold?

At the pre-#84 baseline the committed skill measured **2,560 tokens** (issue #7
result comment; ~3.4k headroom under 6,000); after the #84 refactor + the
#199/#257 currency passes it measured **5,988** at the #262 re-audit. After the
packed-`u64` bit-method and packed-bitmap authoring guidance, it measures
**5,990** (`--check-budget`, 2026-08-04) — under the ceiling, the estimate below having been roughly
2x optimistic but directionally right. The dynamic refactor ADDS: the recursion-scheme
section (5 schemes × ~1 line + 1 example ≈ ~15 lines), and a per-variant line for
the previously-omitted constructs (the ADT items, `Vec`/`String`/`Box`/`Named`
types, the `StructLit`/`Is`/`StrLit`/`Deref`/`Match` exprs, the struct pattern,
the 8 effect atoms — ~30–40 lines total) — minus some now-redundant curated grammar
prose the exhaustive inventory replaces. The CONCISE-RENDERING requirement (REQ-4
/ AC-1: a line + a tiny example per construct, NOT verbose — never the lowering
bodies) keeps each construct cheap. Estimate: ~50–60 new lines × ~10 tokens/line
≈ ~500–700 tokens added → ~3,100–3,300 tokens, still ~1.8x under the 6,000 ceiling.
**RECOMMENDATION: the 6,000 budget HOLDS; no adjustment is needed** — render
concisely first (the design's whole point of keeping the language small, §2.3).
The builder MUST verify `--check-budget` exits 0 after the refactor; IF the
concise rendering genuinely exceeds 6,000 (it should not, per this estimate), the
builder escalates a budget-adjustment recommendation WITH the measured count and
rationale (a `thermite-design.md` §2.2 amendment), and does NOT silently overflow
(R-SPEC-4). The conservative `/3.5` divisor over-counts vs. a real tokenizer, so
the heuristic failing early is the safe direction.

### The bin, the committed artifact, and Forge

`--emit`/`--check-budget` (REQ-6), the committed `THERMITE.skill.md` + freshness
test (REQ-5), and the 6,000-token gate remain. Forge adds the user-facing
`forge skill` wrapper: raw or Claude-compatible output can go to stdout, be
written to a path, or be compared with an existing path.

## Verification

`cargo test -p thermite-skill` discharges the ACs against the design's symbolic
constants and the live registries/enums (R-CHAR-3):

- **AC-1:** `assert!(token_count(generate()) <= 6000)` + `--check-budget` exits 0.
- **AC-2:** iterate `thermite_spec::all()`; assert name + example per entry.
- **AC-9:** iterate `thermite_spec::schemes::all()`; assert name + example per
  entry (the scheme anti-drift coverage test).
- **AC-10 (i):** the renderer `match`es carry no `_` arm (structural — an inline
  invariant comment + a compile-fail fixture / doc demo showing a synthetic added
  variant fails to compile). **AC-10 (ii):** assert a representative substring per
  current Stage-1–8 construct (`struct`, `enum`, `String`, `Vec`, `Box`, `is`,
  `match`, `StructLit`, deref, each effect atom) appears in `generate()`.
- **AC-3/AC-4:** ladder labels + L0/slag clarification; every
  `ForgeMethod::ALL` entry, slag fields, and grammar keywords.
- **AC-5:** `committed_skill_is_fresh` reads repo-root `THERMITE.skill.md` and
  asserts `== generate()`.
- **AC-6:** `assert_eq!(generate(), generate())` + a no-timestamp grep.
- **AC-7:** clippy `-D warnings`, fmt `--check`, anti-pattern gate.

Gauntlet (R-DEFER-6): `cargo test -p thermite-skill`,
`cargo clippy -p thermite-skill --all-targets -- -D warnings`,
`cargo fmt --check`, **and** `cargo run -p thermite-skill -- --check-budget`.

There is no conformance-corpus or golden-file check for this component — the skill
is a generated document, verified by the coverage + compile-forced + freshness +
determinism tests above, not by the cert oracle (which is forge / thermite-lower's).

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (`generate()` API + canonical sections) | SHIPPED | `pub fn generate in generate.rs` concatenates `render_grammar`/`render_combinators`/`render_forge`/`render_ladder`/`render_slag` in §10 order; consumed by `main::run` (the `--emit`/`--check-budget` bin) + the freshness/coverage tests. (Section 2b `render_schemes` lands with REQ-9.) Verification: issue #7 commit `365734e`, `thermite-skill` 18 tests green. |
| REQ-2 (combinator section machine-rendered from `all()`) | SHIPPED | `render_combinators in generate.rs` iterates `thermite_spec::all()`, renders each entry's surface signature + `example_for`; consumed by `generate`. Verification: `combinator_coverage` asserts every `all()` name + one example marker. |
| REQ-3 (curated PROSE sections — narrowed) | SHIPPED | `render_ladder`/`render_slag` + the curated framing in `render_grammar`/`render_forge` return compiled-in strings from §5.1/§6/§8/§4.2; consumed by `generate`. Verification: `ladder_coverage`, `grammar_forge_slag_coverage`. (This amendment NARROWS the scope to PROSE; the structural-inventory part moves to REQ-9/REQ-10/REQ-11.) |
| REQ-4 (deterministic token count + ≤ 6,000 gate) | SHIPPED | `pub fn token_count in generate.rs` = `(chars*2).div_ceil(7)`; `SKILL_TOKEN_BUDGET = 6000`; consumed by `main::run` (`--check-budget`) + `budget_gate`. Measured count 5,988 at the #262 re-audit (`--check-budget`, 2026-06-12; 2,560 at the original #7). |
| REQ-5 (committed `THERMITE.skill.md` + freshness check) | SHIPPED | repo-root `THERMITE.skill.md` (21,209 bytes at the #262 re-audit; 9,397 at the original #7) is `generate()`'s output; `committed_skill_is_fresh` asserts committed bytes `== generate()`. |
| REQ-6 (`thermite-skill` bin — `--emit`/`--check-budget`) | SHIPPED | `main::run in main.rs` dispatches `--emit`→`generate()` and `--check-budget`→`token_count(generate())`; consumes both `generate` and `token_count`. |
| REQ-7 (CI `--check-budget` step) | SHIPPED | the `cargo run -p thermite-skill -- --check-budget` step in `.github/workflows/ci.yml` runs the gate in CI (#7). |
| REQ-8 (compiler-enforced no-staleness GUARANTEE) | SHIPPED | #84. The surface inventory is rendered ONLY by registry iteration (`render_combinators`/`render_schemes`) or by exhaustive `match` with NO `_` arm (`render_{type,expr,item,pattern,effect,binop,prim}_arm` in `generate.rs`); consumed by `render_grammar`/`render_schemes` in `generate`. A new variant FAILS TO COMPILE (`E0004`); a new registry entry auto-renders. Verified: `surface_construct_coverage` (output, in `tests/skill.rs`) + `renderers_are_exhaustive_no_wildcard` (structural, the no-`_` invariant; the green build is the compile-forced proof). The committed skill's "no struct/enum" lie is gone (the test asserts `!contains("no \`struct\`")`). |
| REQ-9 (recursion-scheme section registry-driven from `schemes::all()`) | SHIPPED | #84. `render_schemes in generate.rs` iterates `thermite_spec::schemes::all()`, renders each `SchemeSig`'s call shape (`scrutinee_args` + `step_shape`) + `SchemeResult` + one `scheme_example_for` example; consumed by `generate`. This is `thermite-skill`'s non-test consumer of `schemes::all()` (R-DEFER-1). Verified: `every_scheme_appears_with_an_example` (AC-9) iterates `schemes::all()` and asserts name + call shape per entry. The 5 schemes (`fold`/`map`/`for_all`/`exists`/`traverse`) now appear in the committed skill. |
| REQ-10 (type/expr/item/pattern/effect grammar exhaustive-match-driven) | SHIPPED | #84. `render_{type,expr,item,pattern,effect,binop,prim}_arm in generate.rs` are exhaustive `match`es (NO `_` arm) over `thermite_syntax::{Type,Expr,Item,Pattern,Effect,BinOp,PrimType}`, each arm a `SkillFragment { fragment, description, example }`; driven over per-variant inventories (`type_inventory`/`item_inventory`/`expr_inventory`/`pattern_inventory`/`effect_inventory`/`prim_inventory`/`binop_inventory`) by `render_grammar`. Verified: `surface_construct_coverage` asserts the Stage-1–8 surface (`struct`/`enum`, `Box`/`Vec`/`String`, `is`/`*`/`StructLit`/`match`, each effect atom). |
| REQ-11 (prose curated + freshness-tested) | SHIPPED | The irreducible prose stays curated in `render_grammar`'s framing and `render_ladder`/`render_slag`/the Forge introduction. The Forge method list is generated from `ForgeMethod::ALL`, consumed by Forge parsing/help and checked by iterate-all tests. `committed_skill_is_fresh` and the token budget guard the resulting document. |

## Open questions (for the orchestrator before the builder runs)

- **OQ-1 (token-counting method — heuristic vs. exact tokenizer):** RESOLVED at
  #7 — deterministic `ceil(chars/3.5)` heuristic shipped; an exact tokenizer is a
  one-function swap behind `token_count`. Unchanged by this amendment.

- **OQ-2 (per-construct example source):** the combinator example table
  (`example_for`) lives generator-side (resolved at #7 — keeps the registry
  pure). REQ-9 / REQ-10 extend the same posture: scheme examples and per-variant
  grammar fragments are generator-side, NOT fields on `SchemeSig` / the AST enums.
  RECOMMENDATION: keep examples/fragments generator-side. Not a blocker.

- **OQ-3 (`forge skill` ownership):** RESOLVED — `thermite-skill` owns the
  generated content and formats; Forge owns the user-facing print/write/check
  command.

- **OQ-5 (the Forge command list — can it be compile-forced?):** RESOLVED. The
  public `ForgeMethod` registry lives in the dependency both consumers can
  already see. Forge's private parser exhaustively matches its variants; the
  skill and usage text iterate the same registry. No dependency cycle or public
  parsed-command type is required.

- **OQ-6 (exhaustive `match` vs. `trait SkillEntry`/`SKILL: &[…]` table — the
  DECISION):** this doc DECIDES the exhaustive `match` in `generate.rs` (REQ-10
  Architecture), because (i) the `match` arm IS the compiler-checked
  exhaustiveness guarantee with no extra machinery, and (ii) a `static SKILL:
  &[…]` table is NOT checked for completeness against the enum's variants — a new
  variant compiles fine against a stale table, which is the exact silent-drift
  failure being eliminated. A `trait SkillEntry` adds indirection without adding
  safety (its body must still exhaustively match). This is the doc's LEAST-
  CONFIDENT decision — a contributor might prefer a `trait` for ergonomics — but
  the table form is rejected on the no-completeness-check ground, and the trait
  form reduces to the match form for the guarantee. RECOMMENDATION: exhaustive
  `match`. Not a blocker (the builder may surface a cleaner equivalent THAT KEEPS
  the compile-error-on-new-variant property).
