# Plan: deep tone pass over all source comments

A handoff plan for a fresh session. The work is a deep, judgment-based tone pass
over **every source comment** in the repo (Rust + Lean), applying
`.design/tone-and-voice.md`. It is a register change only — no code, no
identifiers, no semantics move.

This is the gated base: other work pauses until this lands, then rebases onto
it. So edit freely across all crates on one branch; there is no concurrent-churn
constraint while this is the active slice.

## The standard

`.design/tone-and-voice.md` is authoritative. In one line: affirmative not
defensive, plain not emphatic, narrative only in intros/conclusions, substance
preserved. For comments specifically, the targets are:

- **Emphatic ALL-CAPS** used for emphasis (`ACTUALLY RUNS`, `THE TRUST
  BOUNDARY`, `NEVER`, `MUST`) → normal case. Keep ALL-CAPS only for real
  acronyms (SMT, JSON, EPR) and established status labels the schema depends on
  (e.g. `SHIPPED`/`NOT-STARTED` in REQ tables, enum variants).
- **Virtue adverbs** (`honestly`, `loudly`, `cleanly`) → cut; describe the
  behavior.
- **Antithesis pairs** (`not X — Y`, `X, not Y`, `not just X but Y`) → state the
  positive claim directly, unless the contrast is a genuine technical
  disambiguation.
- **Rhetorical bold** and **cute asides** → cut.
- **`exactly` / `precisely`** → claim-by-claim judgment, not blanket removal.
  Keep where it disambiguates (`passes exactly when no out-of-domain bit is
  set` is an iff). Strip where it is tonal (`computes exactly the relation`,
  `exactly the IO-membership projection`) — often the precise verb or noun was
  the real claim. This matters twice over in comments: residual emphasis is what
  a downstream agent anchors on and drifts toward.

## Hard constraints

- **Comments only.** No change to code, identifiers, string literals, test
  expectations, or behavior. Doc-comments (`///`, `//!`), line comments (`//`),
  block comments, and Lean `--`/`/- -/` are in scope.
- **No semantic change.** "The certificate honestly says PARTIAL" → "says
  PARTIAL" is fine; never alter what a comment asserts about the code.
- **Status labels and schema-bearing text stay.** REQ-status tables, enum
  names, the `## REQ status` doc-comment tables, and any text other tooling
  parses (doc-drift `audited-sha:` blocks, etc.) are untouched.
- **Verify per file/crate:** `git diff` shows only comment-line changes; the
  crate still builds and tests pass (`cargo test -p <crate>`); Lean edits leave
  `lake build` unaffected (comments do not affect proofs).

## Scope and rough sizing

Raw "tic" greps overstate, because precise `exactly`/`deliberately` uses are
legitimate. Treat these as upper bounds, not work units:

| crate / tree | ~tic-marked comment lines |
|---|---|
| `thermite-verified` | ~5 (mostly precise; little to do) |
| `thermite-spec` | ~28 |
| `thermite-syntax` | ~41 |
| `thermite-tv` | ~55 |
| `lean/` | ~83 |
| `thermite-lower` | ~116 |
| `forge` | ~212 |

The genuinely performative comments concentrate in `forge`, `thermite-lower`,
`lean/` (ALL-CAPS-heavy module banners, "ACTUALLY RUNS"-style emphasis).

## Recommended execution: a workflow

Merge-hell is gated away, so parallelism is safe. Suggested shape:

- One agent per crate (or per large module for `forge`), instructed to apply
  `.design/tone-and-voice.md` to comments with the `exactly` judgment above.
- Each paired with an **adversarial verifier** that checks the diff is
  comments-only and asserts no semantic/identifier change — the one real risk.
- All on one branch; PR when the tree is covered. Commit per crate so review and
  any revert are crate-scoped.

Inline crate-by-crate is the fallback (slower, serial). Either way: one branch,
gated, rebased onto by everything else after it lands.

## Order

Smallest/cleanest first to set the pattern, then the heavy crates:
`thermite-verified` → `thermite-spec` → `thermite-syntax` → `thermite-tv` →
`lean/` → `thermite-lower` → `forge`.

## Done when

Every crate's comments read as plain technical prose under the standard; the
full workspace builds and tests green; `lake build` green; the branch is one
coherent PR (or per-crate PRs) for review, ready to be the rebase base.

## Context already done (do not redo)

- Public docs (README, RATIONALE, thermite-design) — tone-passed and merged (#9).
- Root `.design/` thermite-2 docs + the standard — merged (#10).
- `docs/` external tree foundation — merged (#11).
- `.design/` **subdir** docs — left as historical artifacts; **out of scope**.
- The build workflow now carries **R-TONE-1** (`goal.md`) and tone references in
  the `acto-doc-author` / `acto-builder` agents, so new/rewritten prose follows
  the standard going forward.
