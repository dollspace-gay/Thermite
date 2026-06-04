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

**Build order & dependencies (v0.1, leaf-first per R-DEFER-7).**
`thermite-syntax` (#3) and `thermite-spec` (#2) are **independent leaves** —
the parser parses combinator calls as generic call expressions and does NOT
depend on the registry. `thermite-lower` (#4) depends on **both** (#2 for
combinator L3/L1 forms, #3 for the AST). `forge` (#5/#6/#8) depends on the
libs; `thermite-skill` (#7) depends on the grammar + registry. Practical
order: scaffold (#1, done) → #3 / #2 (either first; #3 is the current loop) →
#4 → #5/#6 → #7/#8. #2 is best built just before #4 so its registry ships
with a consumer (R-DEFER-1).

## v0.1 Kernel (crosslink milestone #1)

### thermite-syntax — surface grammar, lexer, parser, AST, semantic addressing (issue #3)
- `syntax/surface-grammar.md` — the canonical surface grammar (fn/req/ens/fx/loop/inv/dec/spec fn/#[slag]); the parser is its executable form (planned)
- `syntax/lexer.md` — token grammar (planned)
- `syntax/parser.md` — recovering recursive-descent parser, per-item recovery; parses combinator calls as generic call expressions (registry-free) (planned)
- `syntax/ast.md` — AST shape, mandatory `req`/`ens`/`fx` (planned)
- `syntax/semantic-addressing.md` — stable `loop#1.inv#2` addressing (planned)

### thermite-spec — SpecTherm combinator registry (issue #2)
- `spec/spectherm-combinators.md` — frozen combinators + triggers + L3/L1 forms; enforces the "fixed combinator set" semantic rule (§4.2). Built just before thermite-lower (#4), its first consumer (planned)

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
