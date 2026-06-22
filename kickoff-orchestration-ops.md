---
title: "Kickoff orchestration & doc-gate operations"
tags: ["process", "operations", "doc-drift", "kickoff", "design-doc"]
sources: []
contributors: ["rApq"]
created: 2026-06-18
updated: 2026-06-22
---



## Design Specification

### the orchestrator/agent split

- Kickoff agents work in `.worktrees/<agent-id>/` tmux sessions and **cannot
  `git push`** (blocked by policy). They author + commit; the **orchestrator
  pushes, watches CI, and merges** each branch.
- Agents self-skip live, env-gated tests (`lake_present()`/`verus_present()`
  guards) so their local gauntlet is honest without the full toolchain.
- Standing agent rules: inline the verus/lake paths to avoid permission-prompt
  stalls; self-re-pin doc-drift; never force a merge.

### merge-on-green pattern

### merge-on-green pattern

After pushing a branch, arm a Monitor (or a background poll-until-done loop) that
polls `gh pr checks <N>` and emits one line per check, exiting when the run
completes. CI jobs (post-#76 split): `checks` (fmt + clippy + doctests + doc-drift
+ reqs gates), `test (1-4)` shards, `lean-probe` (spine build + axiom probe), and
`lean-spine-forge (1-4)` — the forge suite WITH the spine, sharded via
`cargo nextest --partition count:N/4`, the only place lake-gated live tests
actually run (see audit F4 / #309). The split cut the lean wall-clock ~16min → ~8min
with zero coverage loss; shard 1 of `lean-spine-forge` also runs the G2 audit-gate
step (`forge strat-tv` / `strat-faithful-tv`).

### doc-drift: two pin kinds — re-pin both after merging main

A governed design doc's header can carry `audited-sha:` (legacy 40-hex commit
pin) AND/OR `audited-content-sha256:` (64-hex digest over the doc's *governed
files*). When both are present, **`doc-drift.py` uses the content-sha256 and
ignores the commit pin.** A re-pin script that rewrites only `audited-sha:` lines
silently misses content-pinned docs (e.g. `.design/tooling/req-registry.md`) →
DRIFT persists. Fix: set `audited-content-sha256:` to the `current` hash that
`doc-drift.py` prints. There is **no `--repin` flag** — edit by hand.

**Merge-ref gotcha:** `doc-drift.py` computes drift with a simplified `git log
<pin>..HEAD` (no `--full-history`). After merging main into a feature branch, a
governed file touched on both sides can read 0-drift locally but DRIFT on CI
(GitHub tests the PR-merge ref, a different parent order). So after merging main,
**re-pin every doc CI flags to the branch tip** (so the next commit touches only
`.design/`), and trust CI's `DRIFT` list over a local run.

### parallel kickoffs → registry.toml union conflict

When two kickoffs each add a `[[requirement]]` entry, merging main conflicts on
`.design/reqs/registry.toml`. It is a **union** (keep BOTH entries), never a
take-ours. After resolving: regenerate the views with `python3
tooling/req-registry.py --write`, then validate with `tooling/reqs check` (must
report "clean"). `status.md` auto-merges but must be regenerated from the unioned
registry.

### stale signing key after removing a kickoff worktree

`crosslink kickoff` sets a worktree-scoped `user.signingkey` in
`.crosslink/.hub-cache` pointing at a per-agent key inside the worktree. After
`git worktree remove`, the key file is gone but the config still references it →
the next `crosslink quick`/issue-creation fails with `Couldn't load public key …`
→ `failed to write commit object`. Fix:
`git -C .crosslink/.hub-cache config --worktree --unset user.signingkey`
(falls back to the repo signing key). Diagnose with `… config --show-origin
--get user.signingkey`.

### running the full forge gauntlet locally

The forge L3/L4 tests need Verus/Z3 (not on PATH) and the built Lean spine:
```
export VERUS_BIN="$HOME/verus-dist/verus-arm64-macos/verus"   # match the CI pin
export PATH="$(dirname "$VERUS_BIN"):$PATH"
( cd lean && lake build Thermite.Stabilize Thermite.Exec.Stmt Thermite.Exec.WhileBody )  # ~13s
cargo test -p forge            # 0 ignored ⇒ no live test self-skipped
```
`0 ignored` is the strong signal: every lake-gated discharge actually ran.

### gate discipline (r-gate-1)

The public README/RATIONALE headline changes **at gate time, not merge time** —
build the feature behind the gate, flip the docs only when the gate's checklist
is verified green in one run. Tick umbrella ACs only against directly-verified
evidence; leave stage-N+1 and process-historical ACs honestly unchecked.

