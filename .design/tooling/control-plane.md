# Control Plane — the gate that guards the gates

<!--
tier: 3-component
status: draft
governs: tooling/control-plane-check.py + the control-plane files it and
         doc-drift.py now pin (.claude/settings.json, .claude/agents/*.md) +
         the `make control-plane` Makefile target and its CI step. Explicitly
         NOT `scripts/audit.sh`, which this component leaves byte-identical
         (the doc-drift decision-5 precedent).
audited-content-sha256: cdce9510c89d0bd00fb08a9a441e07a8299ad4eb71e43e20d4c29e928797b59e
thesis-refs:
  - thermite-design.md §1 (trust relocated: "a skeptical third party can audit in minutes")
  - thermite-design.md §8 (#[slag]: the unverified residue is LOUD, never silent)
issue: crosslink #93
prior-arc:
  - .design/tooling/doc-drift-tripwire.md (the sibling gate: pinned freshness for
    routed design docs. This component is its missing complement — doc-drift pins
    the CONTENT of what the routes govern; nothing pinned whether the routes are
    WIRED. Same 0/1/3 exit contract, same fixture-oracle test convention.)
-->

## Summary

`tooling/spec-discipline.py` and `tooling/anti-pattern-gate.py` are the two
agent-facing enforcement gates. They are real, tested, and tracked — and they
only ever fire because `.claude/settings.json` wires them into Claude Code's
`PreToolUse`/`PostToolUse` events. That wiring is the entire control plane, and
until this component nothing checked it.

On 2026-06-21 commit `5581b65f` ("Stage-3 REQ-1: the @bv clause tag,
parse-gated") removed three hook entries from `.claude/settings.json`: the
`PostToolUse`/`Read` recorder and the `PreToolUse`/`Write|Edit` pair invoking
both gates. The commit message is entirely about the `@bv` clause tag and never
mentions the settings change; the crosslink-generic entries survived while the
project-specific ones vanished. The signature is a `crosslink init` regenerating
`settings.json` from its generic template — the file is tracked but
machine-authored, so a routine (or `--force`) re-init clobbers it.

Both gates were therefore **dormant for the entire Stage-3 arc**, while:

- `README.md:172` — "`.claude/settings.json` (also tracked) wires these into
  Claude Code's `PreToolUse`/`PostToolUse` events, so they enforce automatically
  — no setup."
- `goal.md:183` — "### Spec-discipline (enforced by `tooling/spec-discipline.py`)"
- all four `.claude/agents/acto-*.md` — "The spec-discipline hook enforces these
  reads before it lets you edit."

This is precisely the **"asserted enforcement that isn't"** failure mode the
methodology exists to prevent, and it survived weeks of ACToR loops because
nothing in the repo could see it.

## The blind spot this closes

`tooling/doc-drift.py` pins design↔source freshness for every file reachable
from `tooling/spec-routes.toml`. It is thorough, CI-enforced, and it even routes
its own gate (`tooling/doc-drift.py` → `.design/tooling/doc-drift-tripwire.md`,
REQ-11's dogfood). But:

```
$ grep -nE "\.claude|settings\.json|agents/" tooling/spec-routes.toml
(no output)
```

No route covered the control plane. The design layer governed every source file
in the workspace **except the file that decides whether the governance runs** —
so a `settings.json` regression was structurally invisible, and an
`.claude/agents/*.md` regression with it. (That second gap is not hypothetical:
`acto-critic.md` and `acto-doc-author.md` carry `model: fable` in frontmatter
while their bodies say "Opus — always. … Never substitute." Nothing catches the
contradiction because agent defs are not routed. Reconciling that is out of
scope here — see Open questions OQ-1 — but routing them is not.)

The custom anti-pattern rules also have no CI backstop: there is no
`clippy.toml`, and `ci.yml:95` runs stock `cargo clippy --workspace
--all-targets -- -D warnings` with no `disallowed_*` / `unwrap_used` lints. So
the anti-pattern gate being dead was not compensated elsewhere (OQ-2).

## Design decisions (resolved here, grounded below)

1. **Assert the wiring, don't regenerate it.** A merge step that re-injects the
   entries after `crosslink init` would be silent repair — and silent repair of
   a control plane is how you stop noticing that something keeps breaking it.
   The gate FAILS LOUDLY and prints the exact JSON to paste back. The human sees
   every occurrence.

2. **Matcher coverage, not matcher equality.** A required wiring is satisfied
   when the entry's matcher fires for every required tool. `Write|Edit|Bash`
   covers a `Write|Edit` requirement; `Write` alone does not. Equality would
   false-fail on a harmless reorder; substring matching would pass a matcher
   that silently dropped `Edit`. Alternative-set containment is the predicate
   that means what the requirement means.

3. **A malformed `settings.json` is a FINDING (exit 1), not INCONCLUSIVE.**
   Claude Code loads no hooks at all from a settings file it cannot parse, so
   unparseable is maximally gate-dead. Exit 3 is reserved for failures of the
   gate's own environment (git absent / not a repo) — the `doc-drift.py` REQ-9
   and `scripts/audit.sh` precedent: a gate that fails open is a silent pass
   (R-HONEST-3).

4. **Wired-but-absent is its own defect class.** Every hook command is
   `if [ -f "$HOOK" ]; then … else exit 0; fi`-guarded, so a wiring naming a
   script that does not exist degrades to a silent no-op — indistinguishable
   from success at runtime. `MISSING-SCRIPT` names it separately from
   `MISSING-WIRING` because the fix is different (restore the file, not the
   JSON).

5. **Not part of `make audit`.** Hook wiring is a development-discipline
   invariant, not a link in the proof-trust chain. This mirrors doc-drift's
   decision 5 exactly, and `scripts/audit.sh` is left byte-identical.

6. **The control-plane routes are DECLARATIVE for spec-discipline, ENFORCED for
   doc-drift.** `is_gated_path` in `tooling/spec-discipline.py` requires a
   `.rs` extension, a `thermite-`/`forge` crate dir, and a `src/` component, so
   it structurally cannot gate `.claude/settings.json` or a `.md` agent def —
   the same honest limitation `doc-drift-tripwire.md` REQ-11 records for
   `tooling/*.py` (its OQ-5). The routes added here therefore do not make
   spec-discipline block control-plane edits; they make the control plane
   **content-pinned under `doc-drift.py`**, which IS CI-enforced. Changing
   `settings.json` or an agent def without re-pinning this doc fires the
   tripwire.

## Requirements

- **REQ-1 (settings.json is the subject).** The gate loads the tracked
  `.claude/settings.json` from the repo root. Absent or unparseable is a
  finding (`UNPARSEABLE`, exit 1), never a traceback and never exit 3.

- **REQ-2 (required-wiring predicate).** For each entry in `REQUIRED_HOOKS`
  — a `(event, tools, script, claim)` tuple — the gate asserts some hook entry
  under `event` has a matcher COVERING every tool in `tools` (per decision 2)
  and names `script` in one of its commands. A failure emits `MISSING-WIRING`,
  the doc line that would go false, and the JSON entry to restore.
  The three required wirings are exactly the three `5581b65f` removed:
  `PostToolUse`/`Read` → `spec-discipline.py`;
  `PreToolUse`/`Write|Edit` → `spec-discipline.py`;
  `PreToolUse`/`Write|Edit` → `anti-pattern-gate.py`.

- **REQ-3 (wired implies present).** Each required `script` must exist at its
  repo-relative path. A wiring whose script is absent emits `MISSING-SCRIPT`
  (decision 4).

- **REQ-4 (deterministic report).** Findings print in `REQUIRED_HOOKS` order,
  one block per requirement, with the literal tokens `WIRED` /
  `MISSING-WIRING` / `MISSING-SCRIPT` / `UNPARSEABLE` (R-CODE-5). Two runs over
  an unchanged tree produce byte-identical stdout.

- **REQ-5 (exit contract).** `0` = every required hook wired and present;
  `1` = at least one finding; `3` = the gate could not determine the answer
  (git absent / not a repo). Mirrors `doc-drift.py` REQ-9.

- **REQ-6 (control plane routed).** `tooling/spec-routes.toml` carries routes
  for `.claude/settings.json`, `.claude/agents/*.md`, and
  `tooling/control-plane-check.py`, all governed by this doc, so `doc-drift.py`
  content-pins them (decision 6).

- **REQ-7 (CI enforcement).** The gate runs in the `checks` job of
  `.github/workflows/ci.yml` and via `make control-plane`, so the assertion is
  a red build and not a thing someone remembers to run.

## Acceptance criteria

- **AC-1**: with the three entries removed from `.claude/settings.json` (the
  verbatim post-`5581b65f` file), `python3 tooling/control-plane-check.py`
  exits 1 and names all three missing wirings and both script paths.
- **AC-2**: with the entries restored, the gate exits 0 and prints one `WIRED`
  line per requirement.
- **AC-3**: a wiring whose script is absent from disk exits 1 with
  `MISSING-SCRIPT`, distinct from `MISSING-WIRING`.
- **AC-4**: an unparseable `settings.json` exits 1 with `UNPARSEABLE` and no
  `Traceback` on stderr.
- **AC-5**: invoked with no `--root` outside a git repository, the gate exits 3
  and never 0.
- **AC-6**: a matcher of `Write|Edit|Bash` satisfies a `Write|Edit`
  requirement; a matcher of `Write` alone does not.
- **AC-7**: `scripts/audit.sh` is byte-identical to its pre-component state.

## Verification

`tooling/tests/test_control_plane.py` — nine hand-authored oracle fixtures
(O-1..O-9), the same convention as `test_doc_drift.py`: build a throwaway
control plane in a tmpdir, run the gate by subprocess with `--root`, assert
against expected values the spec fixes, never against the tool's own output
(R-CHAR-3).

O-2 is load-bearing: its fixture is the **verbatim** de-wired `settings.json`
that `5581b65f` left on `main`, so if the gate ever stops catching the exact
regression it was built for, the suite goes red.

Run: `make control-plane-test`, or `python3 -m unittest discover -s tooling/tests`.

## REQ status

| REQ | Status | Evidence |
| --- | --- | --- |
| REQ-1 (settings.json is the subject) | SHIPPED | `SETTINGS_RELPATH = ".claude/settings.json"` + `def evaluate` in `tooling/control-plane-check.py`; the absent/unparseable branches return `(EXIT_FAIL, [UNPARSEABLE …])`. Non-test consumer: the `control-plane gate (hook wiring)` step in `.github/workflows/ci.yml` and `make control-plane`. Verification: O-4/O-5 in `tooling/tests/test_control_plane.py`. |
| REQ-2 (required-wiring predicate) | SHIPPED | `REQUIRED_HOOKS` + `def _matcher_covers` + `def _entry_commands` + `def _restore_snippet` in `tooling/control-plane-check.py`. Non-test consumer: as REQ-1. Verification: O-1 (all wired → exit 0), O-2 (the verbatim `5581b65f` fixture → exit 1, three findings), O-6/O-7 (matcher coverage) in `tooling/tests/test_control_plane.py`. |
| REQ-3 (wired implies present) | SHIPPED | the `if not (root / script).is_file():` branch emitting `MISSING_SCRIPT` in `def evaluate`. Non-test consumer: as REQ-1. Verification: O-3. |
| REQ-4 (deterministic report) | SHIPPED | `def evaluate` iterates `REQUIRED_HOOKS` in declaration order; no set/dict iteration reaches the output. Non-test consumer: as REQ-1. Verification: O-8 (two runs byte-identical). |
| REQ-5 (exit contract) | SHIPPED | `EXIT_OK`/`EXIT_FAIL`/`EXIT_INCONCLUSIVE` + `class EnvironmentError3` + the `except EnvironmentError3` arm in `def main`. Non-test consumer: CI reads the exit status. Verification: O-9 (non-git cwd → exit 3, never 0). |
| REQ-6 (control plane routed) | SHIPPED | the `# tooling — the control plane gating itself` block in `tooling/spec-routes.toml`: three `[[route]]` entries (`.claude/settings.json`, `.claude/agents/*.md`, `tooling/control-plane-check.py`) all `design = ".design/tooling/control-plane.md"`. Non-test consumer: `def load_doc_files in tooling/doc-drift.py` inverts the table and content-pins this doc's governed set. Verification: `python3 tooling/doc-drift.py` reports this doc CURRENT at the pinned aggregate. |
| REQ-7 (CI enforcement) | SHIPPED | `.github/workflows/ci.yml` `checks` job step `control-plane gate (hook wiring)` → `python3 tooling/control-plane-check.py`; `Makefile` targets `control-plane` / `control-plane-test`. Verification: the step is sequenced with the sibling `doc-drift tripwire` step in the same job. |

## Open questions

- **OQ-1 (agent-def self-contradiction).** `.claude/agents/acto-critic.md` and
  `acto-doc-author.md` declare `model: fable` in frontmatter while their bodies
  read "Opus — always. … Never substitute." Routing them (REQ-6) makes any
  future edit re-pin this doc, but the gate does not yet assert
  frontmatter↔body consistency, and resolving which side is correct is a
  harness-behavior decision for the maintainer, not a mechanical fix. A
  follow-up could add a `REQUIRED_AGENTS` block asserting declared `model:` and
  `tools:` against a pinned expectation.

- **OQ-2 (anti-pattern rules have no CI backstop).** There is no `clippy.toml`;
  `ci.yml:95` is stock `cargo clippy --workspace --all-targets -- -D warnings`
  with no `disallowed_*` / `unwrap_used` lints. This component restores and pins
  the hook, but the hook is still the only enforcement of those rules — a
  clone that never runs Claude Code gets none of them. A `clippy.toml`
  encoding the same rules would make the property hold in CI independent of the
  harness.

- **OQ-3 (read-only roles are conventional, not capability-enforced).**
  `acto-critic` / `acto-doc-author` withhold `Edit` but retain `Write` and
  unrestricted `Bash`, so "the critic cannot modify production code" is patched
  with prose (`acto-critic.md:37`) on the `Write`/`Bash` vectors. The critic
  genuinely needs `Write` to author failing tests, so the fix is not "drop
  `Write`" — it is a read-only mount or a path-scoped write allowlist. Out of
  scope here; recorded so the gap is not lost.
