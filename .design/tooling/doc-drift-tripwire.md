# Doc-Drift Tripwire — pinned-SHA freshness for every routed design doc

<!--
tier: 3-component
status: draft
governs: tooling/doc-drift.py  (UNBUILT — blocker #258) + the `audited-sha:`
         header field this doc mandates for every routed .design doc + the
         `make doc-drift` Makefile target (and the SEQUENCED CI step — see
         REQ-10). Explicitly NOT `scripts/audit.sh`, which this component
         leaves byte-identical (decision 5).
audited-sha: 1523b7edd09d5fe614f2950b5d9ba16ef5639f14 (re-pinned at the #258 gauntlet HEAD; governed file last touched 1523b7ed)
thesis-refs:
  - thermite-design.md §1 (trust relocated: "a skeptical third party can audit in minutes")
  - thermite-design.md §8 (#[slag]: the unverified residue is LOUD, never silent)
issue: crosslink #258
prior-arc:
  - .design/verified/rust-lean-correspondence.md (the bespoke per-file pin table
    that `scripts/audit.sh` check [4] drift-checks — the precedent generalized here,
    and the reason check [4] BELONGS in the audit while this gate does not: that
    correspondence is a named residual-trust item; general doc freshness is not)
-->

## Summary

`.design/` docs are the per-component contracts (`goal.md` authority chain), and the
spec-discipline hook guarantees they EXIST and are READ before a routed edit — but
nothing checks their CONTENT is still true of the code. They drift silently:
`.design/forge/cli.md`'s Summary still says "This component is GREENFIELD … Every REQ
below is NOT-STARTED, blocked on issue #5" while `forge/src/cli.rs` is 2,778 lines with
a dozen verbs, and 21 commits have touched `cli.rs` since the doc's last-touch commit
`1004b7a1`. This component converts that staleness from a silent failure into a loud,
gated one — the same move `#[slag]` (§8) makes for unverified code: every routed design
doc pins an `audited-sha:` commit, and a new gate (`tooling/doc-drift.py`, run by
`make doc-drift` and — once the bootstrap backlog is worked off — a CI step) FAILS
whenever any file a doc governs has been committed since the doc's pin. The gate is
deliberately NOT part of `make audit`: doc freshness is a development-discipline
invariant, not a link in the proof-trust chain (decision 5). Clearing the gate is a
conscious act in a commit: re-pin (an explicit "doc still accurate" claim) or amend
the doc and pin the amendment.

## The drift this closes (grounded motivating example)

```
$ git log -1 --format=%h -- .design/forge/cli.md      # the doc's last claim
1004b7a1                                              # 2026-06-04, "#5"
$ git log --format=%H 1004b7a1..HEAD -- forge/src/cli.rs | wc -l
21                                                    # twenty-one unreviewed-by-the-doc commits
```

The doc is internally split (its `## REQ status` table was updated to SHIPPED at #5,
but its Summary prose still claims greenfield) AND externally stale (goal-repl #193,
kernel-target #197, proof-backends #247, currency-pass #257 all reshaped `cli.rs`
after the doc's last touch). Today nothing fires. After this component,
`make doc-drift` fires, naming the doc, the file, and those 21 commits.

## Design decisions (resolved here, grounded below)

1. **Pin granularity: ONE pin per doc** (`audited-sha:` in the doc's HTML-comment
   header), not per-route. The doc is the claim-bearing unit; the route table is
   many-to-many (107 routes, 48 distinct docs today — e.g.
   `.design/basis/09-option-result.md` governs 13 files, and `forge/src/check.rs`
   carries 6 governing docs), and the gate inverts routes to `doc → {files}` and
   checks every governed file against the doc's single pin. Per-route pins would put
   13 pins in one header and make "which pin?" ambiguous for shared docs. The
   finer-grained per-FILE pin table in `rust-lean-correspondence.md` stays bespoke to
   check [4] (it audits arm-by-arm per file; see OQ-1).
2. **Drift predicate: commit-set, never commit-date.** Doc D with pin P governing
   file set F(D) has drifted iff `git log --format=%H <P>..HEAD -- <f>` is non-empty
   for any `f ∈ F(D)`. Date comparison is wrong under rebases; the `<P>..HEAD`
   range is exactly "commits reachable from HEAD but not from the pin," which is the
   question being asked. A pin that does not resolve to a commit or is not an
   ancestor of HEAD is INVALID-PIN — a FAIL distinct from drift (it means the pin
   itself is broken, e.g. typo'd or orphaned by history surgery).
3. **No grandfathering.** A routed doc with no `audited-sha:` line FAILS the gate,
   naming the doc. No allowlist, no warning tier. The bootstrap (REQ-5) is the
   one-time pinning commit.
4. **Honest bootstrap: pin each doc at its OWN last-touch commit**
   (`git log -1 --format=%H -- <doc>`), NEVER blanket-pinned at HEAD. Blanket-HEAD
   would mechanically assert "all 48 docs are accurate now," which is known false —
   MEASURED false: at doc-last-touch pins, **35 of 48 routed docs are drifted**
   (derived with raw git against commit `6368550a`, pre-bootstrap — see the REQ-10
   backlog table for the heaviest entries). A blanket-HEAD bootstrap would
   rubber-stamp all 35, violating R-HONEST-3. Doc-last-touch pins each doc at the
   moment it last made its claims; the gate's first run then reports the TRUE drift
   backlog, each entry filed as a blocker and worked off by re-audit + re-pin or
   amendment. (Doc-last-touch is a proxy — see OQ-6. The 35/48 count is a
   snapshot and will shift slightly by the time the pin sweep lands: commits keep
   touching routed files — e.g. #257's `6368550a` itself touched
   `thermite-skill/src/generate.rs` and `forge/src/cli.rs` after the measurement.)
5. **Enforcement surface: `make doc-drift` + a SEQUENCED CI step — NOT
   `make audit`, NOT a per-edit hook.** Three exclusions, three reasons:
   - **Not `scripts/audit.sh`.** `make audit` re-derives the PROOF-TRUST chain:
     its six checks (README: "The six checks, precisely") are all links in the
     soundness story, and check [4] qualifies because the Rust↔Lean correspondence
     is a NAMED RESIDUAL-TRUST item in check [6]'s list. General design-doc
     freshness is a DEVELOPMENT-DISCIPLINE invariant: a stale
     `.design/forge/cli.md` does not weaken any shipped proof. Wiring this gate
     into the audit verdict would (a) muddy the trust statement the README sells —
     "the six checks, precisely" would no longer be precisely the soundness story —
     and (b) turn the audit INCONCLUSIVE/FAILED for non-soundness reasons,
     especially acute given the 35/48 bootstrap backlog (decision 4): a skeptic
     running `make audit` on day one would see FAILED over doc hygiene.
     `scripts/audit.sh` is byte-identical under this component (AC-7).
   - **Not a per-edit hook.** A PostToolUse gate would fire constantly mid-build:
     every builder commit touching a routed file drifts its doc until the closing
     re-pin, so freshness is a commit-time/CI-time invariant, not an edit-time one.
     Named as OQ-4.
   - **CI is sequenced, not day-one.** See REQ-10: the CI step lands only once the
     bootstrap backlog is cleared; until then the gate is runnable-but-advisory via
     `make doc-drift` — an honest, named, temporary state, not a silent one.

## Requirements

Substrate this component builds on (already shipped):

- REQ-1 (route table as the enumeration source): the set of checked docs is exactly
  the deduplicated `design` fields of `tooling/spec-routes.toml` `[[route]]` entries;
  the file set per doc is the union of that doc's routes' `crate_pattern`s. The route
  table is already "the single source of truth" and "the authoritative module map"
  (`goal.md` scope section). Source: `goal.md` authority chain; spec-routes.toml
  schema header.
- REQ-2 (parsing substrate): the gate reuses the spec-discipline parsing approach —
  stdlib `tomllib` (Python ≥3.11; this machine runs 3.13.13), and, should a route
  ever carry a glob `crate_pattern`, the `glob_to_regex`/`match_pattern` treatment —
  no third-party deps, consistent with the other two gates in `tooling/`. (Finding:
  ZERO of the 107 current routes use a glob; all `crate_pattern`s are literal paths.
  Glob handling is required only for forward-compat — see REQ-6.)
- REQ-3 (exit-3 honest-inconclusive PRECEDENT): `scripts/audit.sh`'s
  `pass`/`fail`/`skip` + `SKIPPED_GUARANTEES` discipline — "a skipped check is NOT
  a pass," `DEEP AUDIT INCONCLUSIVE` exits 3, distinct from FAILED's 1 — is the
  shape REQ-9's exit-code contract MIRRORS. That is this substrate's ONLY role
  here: a precedent the tool's own 0/1/3 contract copies. The tool's wiring does
  NOT call into `audit.sh` and `audit.sh` does not call the tool (decision 5);
  this REQ survives the REQ-10 redesign purely as the exit-code-3 precedent.
- REQ-4 (the precedent): check [4] (`scripts/audit.sh`, "CORRESPONDENCE DRIFT
  TRIPWIRE") already implements the bespoke single-doc version: extract pinned SHAs
  from `rust-lean-correspondence.md`'s table, compare each against
  `git log -1 --format=%h -- <file>`, FAIL on mismatch. It stays bespoke in v1
  (OQ-1); this component generalizes the IDEA, not that code path — and check [4]
  stays in the audit (where this gate does not go) because its subject is a named
  residual-trust item (decision 5).

New work (all UNBUILT — blocker #258):

- REQ-5 (the `audited-sha:` field + bootstrap): every doc referenced by a
  `[[route]].design` field carries, in its existing HTML-comment header block
  (the `tier:`/`status:`/`governs:` block every doc already has), one line

  ```
  audited-sha: <40-hex full commit SHA>[ <optional free-text annotation>]
  ```

  meaning: "this doc's claims were verified accurate against the tree as of this
  commit." Full 40-hex (not the 8-hex short form check [4]'s table uses) so the pin
  can never go ambiguous as the repo grows; extraction is the first line matching
  `^audited-sha:\s*([0-9a-f]{40})\b`. A one-time bootstrap commit adds the field to
  all 48 routed docs at each doc's own last-touch SHA (decision 4), and files one
  blocker per doc the first gate run reports as drifted.
- REQ-6 (the gate, `tooling/doc-drift.py`): a stdlib-only python3 tool that
  (a) loads `tooling/spec-routes.toml` via `tomllib`, (b) inverts routes to
  `doc → sorted({crate_pattern})`, (c) extracts each doc's pin per REQ-5,
  (d) validates the pin (`git rev-parse --verify <P>^{commit}` +
  `git merge-base --is-ancestor <P> HEAD`), and (e) applies the drift predicate
  (decision 2) per governed file — for a literal path, pathspec `<f>`; for a glob
  pattern, pathspec `:(glob)<f>`. A routed file with no commits in `<P>..HEAD`
  (including a file not yet committed at all — many v0.2–v0.5 routes point at
  unbuilt files, e.g. `forge/src/session.rs`) is CURRENT, not drift: there is
  nothing for the doc to be stale about.
- REQ-7 (loud, named failure): each drift is reported as the doc path, its pinned
  SHA, the governed file, and the intervening commits
  (`git log --oneline <P>..HEAD -- <f>`) — the same three-part naming check [4]'s
  FAIL lines use ("pinned X, current Y") extended with the commit list, so the
  re-auditor knows exactly which diffs to review before re-pinning. Output ordering
  is deterministic: sorted by doc path, then file path (R-CODE-5).
- REQ-8 (missing/invalid pin = FAIL): a routed doc with no `audited-sha:` line, or
  with a pin that fails REQ-6(d) validation, is a FAIL naming the doc and the
  defect class (`MISSING-PIN` / `INVALID-PIN`), distinct from `DRIFT`. No
  grandfathering (decision 3).
- REQ-9 (exit-code contract): `0` = every routed doc pinned and current; `1` = at
  least one DRIFT / MISSING-PIN / INVALID-PIN; `3` = the gate could not determine
  the answer (no git repo / git absent / `tomllib` absent / `spec-routes.toml`
  unreadable) — mirroring the audit's INCONCLUSIVE=3 precedent (REQ-3: "a skipped
  check is NOT a pass"). The tool never exits 0 without having checked all 48 docs.
- REQ-10 (Makefile + CI wiring, SEQUENCED — replaces the rejected audit.sh
  wiring): a `doc-drift` target in `Makefile` (added to the `.PHONY` line, which
  today reads `audit audit-fast check test fmt clippy gauntlet`) that invokes
  `python3 tooling/doc-drift.py`, printing its report. Exit-code caveat (builder
  finding, verified empirically): GNU make collapses ANY nonzero recipe exit to
  its own exit 2 — it never re-emits 1 or 3. So `make doc-drift` is 0 = clean /
  2 = needs-attention; the precise 1-vs-3 class is carried by the tool's printed
  report and by invoking `python3 tooling/doc-drift.py` directly, which remains
  the exit-code CONTRACT (REQ-9). `scripts/audit.sh` is NOT touched — the gate is
  development-discipline, not proof-trust (decision 5; AC-7 pins this as
  byte-identical). `.github/workflows/ci.yml` gains the step ONLY ONCE THE
  BOOTSTRAP BACKLOG IS CLEARED: flipping CI red on day one for 35 known-stale docs
  (decision 4's measured 35/48) would just train people to ignore the gate.

  **The enforcement-activation sequencing, explicitly:**
  1. tool (`tooling/doc-drift.py`) + bootstrap pins (REQ-5, doc-last-touch) land;
  2. the first gate run's backlog (~35 docs at measurement; the exact sweep-time
     count will differ slightly — see decision 4) is tracked as blocker issue(s);
  3. the backlog is worked off doc-by-doc: re-audit the intervening diffs, then
     re-pin or amend (the re-pin workflow below);
  4. the CI step lands IN THE COMMIT THAT CLEARS THE LAST BACKLOG ITEM, so CI's
     first doc-drift run is green by construction and every subsequent red is a
     genuinely new drift.

  Until step 4, the gate is RUNNABLE-BUT-ADVISORY (`make doc-drift`) — an honest,
  named, temporary state, not a silent one: the backlog blockers are the loud
  record that enforcement is pending. The heaviest measured backlog entries, for
  scale (raw git at `6368550a`, doc-last-touch pins): `.design/basis/09-option-result.md`
  (13 governed files drifted, e.g. `thermite-lower/src/lower.rs` +25 commits),
  `.design/lower/boundary-composition.md` (`lower.rs` +46),
  `.design/scaffold/workspace.md` (`forge/src/main.rs` +23),
  `.design/forge/cli.md` (`cli.rs` +20 at measurement, +21 at `6368550a` — the
  motivating witness).
- REQ-11 (self-governing route): `tooling/spec-routes.toml` gains

  ```toml
  [[route]]
  crate_pattern = "tooling/doc-drift.py"
  design = ".design/tooling/doc-drift-tripwire.md"
  reference = []
  conformance_ops = []
  ```

  so the gate is itself routed to this doc, and this doc's pin must be bumped in
  any commit that edits the gate (the gate fires on itself otherwise — the dogfood
  property). HONEST LIMITATION, verified against the hook source: NO `tooling/*.py`
  file is routed today, and even with this route the spec-discipline hook would not
  enforce it — `is_gated_path` in `tooling/spec-discipline.py` requires
  `TARGET_EXTENSION = ".rs"`, a crate dir matching `thermite-`/`forge`, and a `src/`
  component, all of which `tooling/doc-drift.py` fails. The route entry in v1 is
  therefore DECLARATIVE (it makes the gate's own doc-drift checkable by doc-drift.py
  itself, which IS enforced); extending `is_gated_path` to gate `tooling/*.py`
  edits is OQ-5, not assumed here.

## Acceptance criteria

- AC-1: with a routed file committed after its doc's pin, `python3
  tooling/doc-drift.py` exits 1 and its output names BOTH the doc path and the file
  path and at least one intervening commit SHA. (Bootstrap-time live witness: with
  `.design/forge/cli.md` pinned at `1004b7a1…`, the gate must report
  `forge/src/cli.rs` drifted with 21 intervening commits at `6368550a`, headed by
  `6368550a` itself.)
- AC-2: with every routed doc pinned at (or after) the last-touch of all its
  governed files, the tool exits 0 and prints one CURRENT summary line per doc
  (48 docs at bootstrap).
- AC-3: deleting the `audited-sha:` line from any one routed doc flips the exit to
  1 with a `MISSING-PIN` line naming that doc.
- AC-4: a pin that is not a 40-hex resolvable commit, or not an ancestor of HEAD,
  flips the exit to 1 with an `INVALID-PIN` line naming the doc — textually distinct
  from a `DRIFT` line.
- AC-5: run outside a git repo (or with `git` shadowed off `PATH`), the tool exits
  3, never 0 and never an unhandled traceback.
- AC-6: a route whose `crate_pattern` has never been committed (e.g.
  `forge/src/session.rs`) produces no drift for its doc (REQ-6 unbuilt-file rule).
- AC-7: `make doc-drift` exits 0 when the tool exits 0 and nonzero otherwise
  (GNU make collapses failing recipes to exit 2 — REQ-10 caveat; the 0/1/3
  contract is the DIRECT invocation `python3 tooling/doc-drift.py`) and prints
  the tool's report; `scripts/audit.sh` is UNCHANGED by this component
  (byte-identical — the gate is outside the proof-trust chain, decision 5).
- AC-8: two consecutive runs on an unchanged tree produce byte-identical output
  (deterministic ordering, R-CODE-5).

## Architecture

`tooling/doc-drift.py` is the third gate in `tooling/`, shaped like its siblings
(`spec-discipline.py`, `anti-pattern-gate.py`): stdlib-only python3, a
PROJECT-CUSTOMIZATION constants block, top-of-file docstring stating the rule it
enforces and citing this doc. Unlike the siblings it is NOT a Claude-Code hook in
v1 (decision 5): it is invoked via `make doc-drift` (and, post-backlog, the
sequenced CI step — REQ-10) and runnable standalone.

Pipeline (one pass, no state file):

1. **Enumerate** — `tomllib.load(open("tooling/spec-routes.toml","rb"))["route"]`,
   exactly the `load_routes` approach in `spec-discipline.py` (which guards
   `try: import tomllib / except ImportError: tomllib = None` for pre-3.11; the
   gate instead treats absent `tomllib` as exit-3 environment failure, because a
   CI gate that fails open is a silent pass — R-HONEST-3). Invert to
   `doc → sorted(set(crate_patterns))`.
2. **Extract** — first `^audited-sha:\s*([0-9a-f]{40})\b` match per doc (REQ-5).
   The field lives in the same HTML-comment header every doc already carries
   (`tier:` / `status:` / `governs:` / `thesis-refs:` — see any routed doc;
   grounded at the original audit: `grep -rn "audited-sha" .design/` returned
   ZERO hits outside this doc, so the field name is unclaimed and the bootstrap
   is a clean additive sweep).
3. **Validate + compare** — per doc: pin validation (REQ-6(d)), then per governed
   file the commit-set predicate `git log --format=%H <P>..HEAD -- <pathspec>`.
   Subprocess exit statuses are always inspected; a git invocation failing for
   environmental reasons is exit 3, never treated as "no drift" (the R-CODE-4
   discipline applied to git instead of a solver).
4. **Report + exit** — REQ-7 lines, REQ-9 codes.

**The trust boundary (why this gate lives OUTSIDE `scripts/audit.sh`):**
`make audit` re-derives the proof-trust chain — its six checks (README: "The six
checks, precisely") are each links in the soundness story, ending in check [6]'s
honest residual-trust statement. Check [4] earns its slot because the Rust↔Lean
correspondence it drift-checks is one of [6]'s NAMED residual-trust items: if that
doc is stale, the inspection-tier residual is stale and the audit verdict honestly
degrades. No `.design/` contract has that property — a stale `.design/forge/cli.md`
weakens no shipped proof. Putting this gate in the audit would dilute the verdict's
meaning and, given the measured 35/48 bootstrap backlog, make the audit FAIL for
reasons a skeptic does not care about. So: check [4] stays in the audit, this gate
stays out, and the two mechanisms share only the IDEA of pinned-SHA drift-checking.

The relationship to check [4] (`scripts/audit.sh`), mechanically: check [4] reads a
PER-FILE pin table inside one unrouted doc (`rust-lean-correspondence.md`, whose
"Audited commits" table pins five artifacts in backticked 8-hex, extracted by the
`pin_sha_for` awk helper) and compares each against
`git log -1 --format=%h -- <file>`. That doc is not a `[[route]].design` target
(verified against the route table — no route names it), so the general gate does
not see it and the two mechanisms do not overlap in v1. The general gate also
deliberately uses the commit-SET predicate rather than check [4]'s
last-touch-equality compare: equality of `git log -1` output is correct for
check [4]'s purpose but conflates "drifted" with "pin newer than last touch"
(which the re-pin workflow legitimately produces when a doc is re-audited without
the code changing). Unification is OQ-1.

**The re-pin workflow** (how the gate is cleared, per the §8 loudness model):

- *Code changed, doc still accurate*: review `git log --oneline <P>..HEAD -- <f>`
  (the gate prints it), confirm doc claims hold, bump `audited-sha:` to the new
  last-touch (or HEAD) in a commit whose message states the verification — the
  exact ceremony the two re-pin amendments in `rust-lean-correspondence.md`
  ("VERIFIED additive-only, NOT rubber-stamped") already model.
- *Code changed, doc now wrong*: dispatch acto-doc-author to amend the doc
  (R-DOC-1: the doc adapts to the code), pinning the amendment commit's tree state.
- The gate cannot distinguish the two (both are "pin bumped in a commit") — OQ-2.

## Verification

- **Fixture tests** (`tooling/tests/test_doc_drift.py` or a shell harness — note:
  `tooling/` has NO existing test convention; the two shipped gates are untested,
  so this gate introduces the first one): build a throwaway git repo in `tmpdir`
  with a mini route table + two docs + governed files, then assert AC-1 (commit
  after pin → exit 1 naming both), AC-2 (exit 0), AC-3 (`MISSING-PIN`), AC-4
  (`INVALID-PIN` on a bogus 40-hex and on a non-ancestor), AC-6 (route to an
  uncommitted path), AC-8 (byte-identical reruns). Expected values are hand-built
  fixture facts, never the tool's own output (R-CHAR-3).
- **Live-tree smoke**: `python3 tooling/doc-drift.py` on the real repo at the
  bootstrap commit — exit/report must match the independently-derived backlog
  (the 35/48 measurement and the cli.md/cli.rs 21-commit witness in AC-1, derived
  by raw git, not by the tool; exact sweep-time counts re-derived the same way).
- **Makefile wiring**: `make doc-drift` is exit 0 iff the tool exits 0, and
  nonzero (make's collapsed 2) for both the 1- and 3-cases, with the class
  visible in the printed report (AC-7 as amended; the 3-case exercised by
  shadowing `python3` or `git`).
- **Audit untouched**: `git diff <pre-component-commit> -- scripts/audit.sh` is
  empty in the component's commits (AC-7's second half); `bash scripts/audit.sh`
  output names the same six checks before and after.

## Route-table addition needed (NOT made by this doc — R-DOC-1, builder's commit)

The REQ-11 `[[route]]` block above, appended to `tooling/spec-routes.toml` in the
same commit that creates `tooling/doc-drift.py`. Finding for the orchestrator: no
`tooling/*` path is routed today, and the spec-discipline hook structurally cannot
gate `.py` files (REQ-11 evidence), so this route is enforceable only by
doc-drift.py itself until OQ-5 is resolved.

(Separately: `.design/00-index.md` nominally indexes the docs, but it has not been
updated since commit `1e008994` — it still lists every doc as "planned" and knows
nothing of `.design/basis/`, `.design/verified/`, `.design/build/`, or this
`.design/tooling/` area. Index maintenance is NOT a live convention; this doc adds
no index entry and flags the index itself as a doc-drift instance the route table
cannot catch, since the index is unrouted. Named in OQ-7.)

## REQ status

| REQ | Status | Evidence |
|---|---|---|
| REQ-1 (route table as enumeration source) | SHIPPED | `tooling/spec-routes.toml` header: "spec-routes.toml — the Thermite route table (single source of truth)… Each route maps a toolchain source file to the design doc that governs it". Non-test consumer: `def load_routes in tooling/spec-discipline.py` → `def find_routes`, wired as the PreToolUse/PostToolUse hook in `.claude/settings.json` (`python3 "$HOOK"` on `tooling/spec-discipline.py`). Verification: `python3 -c "import tomllib; …"` → 107 routes, 48 distinct `design` docs, all 48 exist on disk, 0 glob patterns. |
| REQ-2 (tomllib/glob parsing substrate) | SHIPPED | `try: import tomllib  # Python 3.11+` + `def load_routes` + `def glob_to_regex` + `def match_pattern in tooling/spec-discipline.py`. Non-test consumer: the spec-discipline hook itself (`.claude/settings.json` PreToolUse on Write\|Edit). Verification: `python3 --version` → `Python 3.13.13` (tomllib available); the hook blocks routed edits live in this harness. |
| REQ-3 (exit-3 honest-inconclusive precedent) | SHIPPED | `scripts/audit.sh`: `pass()`/`fail()`/`skip()` helpers; `SKIPPED_GUARANTEES=()`; verdict block "INCONCLUSIVE is NOT a pass… Exit NONZERO (3, distinct from FAILED's 1) so automation cannot read a skipped-guarantee run as green (R-HONEST-3)". Non-test consumer: `make audit` (`Makefile`: `audit: @bash scripts/audit.sh`). Role here is PRECEDENT-ONLY: REQ-9 mirrors the 0/1/3 shape; nothing in this component calls into or out of `audit.sh` (decision 5). |
| REQ-4 (check [4] precedent, stays bespoke AND stays in the audit) | SHIPPED | `scripts/audit.sh` "[4/5] CORRESPONDENCE DRIFT TRIPWIRE": `pin_sha_for()` extracts backticked hex from `.design/verified/rust-lean-correspondence.md`'s "Audited commits (PINNED…)" table; compare `cur="$(git log -1 --format=%h -- "$pf")"`; on mismatch `fail "$pf — DRIFTED: pinned $pinned, current $cur"` + `RC=1`. Non-test consumer: `make audit`. Verification: the doc's two amendment blocks record the tripwire firing and being cleared by verified re-pins (#200, #255). Its subject is a check-[6] residual-trust item — the property this gate's subjects lack (decision 5). |
| REQ-5 (`audited-sha:` field + 48-doc bootstrap) | NOT-STARTED | open blocker #258. `grep -rn "audited-sha" .design/` → zero hits outside this doc; no routed doc carries a pin; the bootstrap pinning commit does not exist. Known sweep-time backlog: 35/48 drifted at doc-last-touch pins (measured `6368550a`, decision 4). |
| REQ-6 (the gate `tooling/doc-drift.py`) | NOT-STARTED | open blocker #258. `ls tooling/` → `anti-pattern-gate.py  spec-discipline.py  spec-routes.toml` only; no `doc-drift.py` exists. |
| REQ-7 (loud doc+file+commits failure report) | NOT-STARTED | open blocker #258 (no tool to carry it; report shape specified here). |
| REQ-8 (missing/invalid pin = FAIL, no grandfathering) | NOT-STARTED | open blocker #258 (the MISSING-PIN/INVALID-PIN classes exist only in this spec). |
| REQ-9 (exit-code contract 0/1/3) | NOT-STARTED | open blocker #258. |
| REQ-10 (Makefile + sequenced CI wiring) | NOT-STARTED | open blocker #258. `Makefile` `.PHONY` line is `audit audit-fast check test fmt clippy gauntlet` — no `doc-drift` target; `.github/workflows/ci.yml` has no doc-drift step (steps today: checkout, toolchain, Verus install, build/test/clippy/fmt, skill budget gate). `scripts/audit.sh` is explicitly OUT OF SCOPE for this REQ (decision 5) — its current shape is not a gap. CI step is additionally gated on backlog clearance (the sequencing plan in REQ-10), so its absence at tool-landing time will be the PLANNED advisory state, not drift from this doc. |
| REQ-11 (self-governing route entry) | NOT-STARTED | open blocker #258. `tooling/spec-routes.toml` has no `tooling/*` route (verified: all 107 `crate_pattern`s are `thermite-*`/`forge` `.rs` paths); the hook's `is_gated_path` cannot gate `.py` regardless (see REQ-11 body). |

## Open questions

- **OQ-1 (check [4] unification):** keep check [4] bespoke in v1 (RECOMMENDED,
  assumed above — and reinforced by decision 5: check [4] is audit-side, this gate
  is not, so unifying the code would blur the trust boundary). Its pin table is
  per-FILE inside an UNROUTED doc, its compare is last-touch equality, and its
  semantics ("the arm-by-arm inspection predates this encoder") are finer-grained
  than per-doc freshness. A v2 could let doc-drift.py read an in-doc per-file pin
  table as an override of the header pin and retire the awk extractor; not
  designed here.
- **OQ-2 (re-pin vs amendment provenance):** the gate sees only "pin bumped in a
  commit"; it cannot distinguish a verified re-pin from a rubber-stamp, nor a
  content amendment from a pin-only bump. Out of scope v1 — the commit-message
  ceremony (the rust-lean-correspondence amendment precedent) carries the claim,
  and the acto-critic adversarially audits rubber-stamps. A v2 could require the
  re-pin commit to also touch the doc body, or to cite the reviewed range.
- **OQ-3 (QUOTED-SYMBOL lint):** extracting backticked symbol anchors from docs
  (`pub fn lower_fn in lower.rs`) and grepping them against the tree would catch
  CONTENT drift, not just commit drift. DEFERRED — named, not in v1; it needs an
  anchor grammar and a false-positive budget the SHA tripwire doesn't.
- **OQ-4 (per-edit hook):** running the gate as a PostToolUse/PreToolUse hook would
  fire on every mid-flight builder edit (any routed-file edit drifts its doc until
  the closing re-pin), making the loop unworkable. v1 is `make doc-drift` + the
  sequenced CI step only; a commit-time (pre-commit) variant is the plausible
  middle ground if silent drift re-accumulates between runs.
- **OQ-5 (gating `tooling/*.py` edits):** extending `is_gated_path` in
  `spec-discipline.py` (`.py` extension + `tooling/` dir) would make REQ-11's route
  hook-enforced, and would for the first time route the gates themselves. Touches
  the hook's PROJECT CUSTOMIZATION constants; orchestrator's call, not assumed
  here.
- **OQ-6 (bootstrap pin proxy):** doc-last-touch is a proxy for "when the doc's
  claims were last verified" — too generous when a doc's last touch was a trivial
  cite fix, too strict never. The alternative (manually re-auditing all 48 docs at
  bootstrap) is the honest maximum but is itself the backlog the gate exists to
  schedule (measured: 35/48 drifted — decision 4). v1 accepts the proxy and lets
  the per-doc re-pin work restore full honesty incrementally.
- **OQ-7 (unrouted docs):** the gate covers exactly the routed docs. Unrouted
  `.design/` files — `00-index.md` (stale since `1e008994`, still calls every doc
  "planned"), `.design/research/**`, `rust-lean-correspondence.md` (check [4]'s
  domain) — are invisible to it. Whether the index gets a pin (or gets deleted as
  dead convention) is an orchestrator decision; the gate should not silently imply
  it covers them.
