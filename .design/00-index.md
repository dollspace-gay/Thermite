# Thermite design-doc index

Per-component design docs governing the Thermite toolchain. Each is the
contract that sits between `thermite-design.md` (the thesis) and the
implementation. The route table `tooling/spec-routes.toml` maps each
toolchain source file to the doc that governs it; the spec-discipline hook
blocks edits to a file whose design doc does not yet exist (dispatch
acto-doc-author to author it).

Authority chain: `thermite-design.md → .design/<area>/<doc>.md → conformance corpus / Verus golden files → impl`.

Docs are authored on demand as the ACToR loop reaches each component (they
do not all exist yet — a missing doc is a hook block, not an error). Status
key: **planned** (routed, doc not yet authored) · **draft** · **stable**.

## v0.1 Kernel (crosslink milestone #1)

### thermite-syntax — lexer, parser, AST, semantic addressing
- `syntax/lexer.md` — token grammar (planned)
- `syntax/parser.md` — recovering recursive-descent parser, per-item recovery (planned)
- `syntax/ast.md` — AST shape, mandatory `req`/`ens`/`fx` (planned)
- `syntax/semantic-addressing.md` — stable `loop#1.inv#2` addressing (planned)

### thermite-spec — SpecTherm combinator registry
- `spec/surface-grammar.md` — canonical surface grammar, source of the skill (planned)
- `spec/spectherm-combinators.md` — frozen combinators + triggers + L3/L1 forms (planned)

### thermite-lower — Thermite AST → Verus source
- `lower/verus-lowering.md` — req→requires, ens→ensures, inv/dec, spec fn (planned)
- `lower/l1-runtime-checks.md` — executable contracts, the L1 rung (planned)
- `lower/effect-subsumption.md` — compile-time `fx` row checking (planned)

### forge — CLI / verification driver
- `forge/cli.md` — `forge new`/`forge check` command surface (planned)
- `forge/check.md` — run the ladder, structured per-obligation JSON + counterexamples (planned)
- `forge/certificate-manifest.md` — certificate / build-manifest schema (planned)
- `forge/vacuity-triage.md` — structural §7.1 rejections (planned)
- `forge/slag.md` — `#[slag]` with mandatory reason/owner/review (planned)
- `forge/proof-cache.md` — per-item content-addressed cache (planned)

### thermite-skill — skill generator
- `skill/skill-generator.md` — generate `THERMITE.skill.md` + 6k-token CI budget (planned)

## References
- `../thermite-design.md` — the product thesis and v0.1–v0.5 roadmap.
- `../goal.md` — the binding contract and ACToR loop.
- Knowledge page `thermite-architecture-v0.1` (crosslink) — locked architecture decisions.
