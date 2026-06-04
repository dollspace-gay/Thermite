#!/usr/bin/env python3
"""
spec-discipline hook (greenfield, spec-driven variant of the vibe-fork
translate-discipline gate).

Thermite has no upstream codebase to translate. The authority is the
design layer, not a foreign source tree. This hook enforces:

  "read goal.md, read the component's design doc, read any golden/
   conformance reference the route declares; THEN edit the toolchain."

as a deterministic per-edit gate.

Two invocations in .claude/settings.json:

  - PostToolUse on Read       -> records the Read in session state
  - PreToolUse  on Write|Edit -> gates writes to gated toolchain files

State is persisted at .crosslink/.spec-reads.json (per-worktree).

Required source classes for any gated edit:
  1. goal.md                         (always; the binding contract)
  2. .design/<area>/<doc>.md         (per route; must exist on disk + Read)
  3. route.reference[*]              (per route; ONLY if the route declares a
                                      non-empty `reference` list — then >=1
                                      reference path must be Read. These are
                                      conformance/ corpus entries or
                                      tests/golden/ files: the external truth.)

If a route is missing, the hook BLOCKS with instructions to add one.
If a design doc is missing, the hook BLOCKS with instructions to
dispatch acto-doc-author.

See:
  goal.md - "Spec-discipline rules" (R-XLATE-*)
  goal.md - "Injected-instructions rules" (R-INJECT-*)
  tooling/spec-routes.toml

PROJECT CUSTOMIZATION:
  Edit TARGET_CRATE_PREFIXES, TARGET_CRATE_EXACT, EXCLUDED_CRATES,
  TARGET_EXTENSION, REFERENCE_PREFIXES below.
"""

import json
import os
import re
import sys
import time
from pathlib import Path

try:
    import tomllib  # Python 3.11+
except ImportError:
    tomllib = None


# =====================================================================
# PROJECT CUSTOMIZATION — edit these constants for your project
# =====================================================================

# Workspace crate name prefixes gated by this hook. Files outside these
# crates are not gated.
TARGET_CRATE_PREFIXES = ("thermite-",)

# Standalone crates (no shared prefix) to gate — the forge CLI crate.
TARGET_CRATE_EXACT = ("forge",)

# Crates explicitly excluded — never gated even if they match the prefix.
EXCLUDED_CRATES = ("thermite-test-utils",)

# File extensions to gate.
TARGET_EXTENSION = ".rs"

# Repo-relative directory prefixes that count as "reference" reads
# (the external truth: conformance corpus + golden files). A read under
# any of these prefixes satisfies a route's `reference` requirement when
# the read path starts with one of the route's declared reference paths.
REFERENCE_PREFIXES = ("conformance/", "tests/golden/")

# =====================================================================
# Implementation — generally no edits needed below this line
# =====================================================================


# --- repo-root + state file paths --------------------------------------

def find_repo_root():
    """Walk up to find the repo containing .crosslink/."""
    p = Path.cwd()
    while p != p.parent:
        if (p / ".crosslink").is_dir():
            return p
        p = p.parent
    return None


def state_path(repo_root):
    return repo_root / ".crosslink" / ".spec-reads.json"


def routes_path(repo_root):
    return repo_root / "tooling" / "spec-routes.toml"


def read_state(repo_root):
    p = state_path(repo_root)
    if not p.exists():
        return {"reads": []}
    try:
        with open(p) as f:
            return json.load(f)
    except (json.JSONDecodeError, OSError):
        return {"reads": []}


def write_state(repo_root, state):
    p = state_path(repo_root)
    p.parent.mkdir(parents=True, exist_ok=True)
    with open(p, "w") as f:
        json.dump(state, f, indent=2)


# --- route table -------------------------------------------------------

def load_routes(repo_root):
    p = routes_path(repo_root)
    if not p.exists() or not tomllib:
        return {"route": []}
    try:
        with open(p, "rb") as f:
            return tomllib.load(f)
    except (OSError, tomllib.TOMLDecodeError):
        return {"route": []}


def glob_to_regex(pattern):
    """Convert a simple glob (supporting * and **) to a regex."""
    out = []
    i = 0
    while i < len(pattern):
        c = pattern[i]
        if c == "*":
            if i + 1 < len(pattern) and pattern[i + 1] == "*":
                out.append(".*")
                i += 2
                if i < len(pattern) and pattern[i] == "/":
                    i += 1
            else:
                out.append("[^/]*")
                i += 1
        elif c == "?":
            out.append("[^/]")
            i += 1
        elif c in ".^$+(){}|\\[]":
            out.append("\\" + c)
            i += 1
        else:
            out.append(c)
            i += 1
    return "^" + "".join(out) + "$"


def match_pattern(file_path, pattern):
    if pattern == file_path:
        return True
    if "*" in pattern or "?" in pattern:
        return re.match(glob_to_regex(pattern), file_path) is not None
    return False


def find_routes(rel_path, routes):
    matched = []
    for r in routes.get("route", []):
        if match_pattern(rel_path, r["crate_pattern"]):
            matched.append(r)
    return matched


# --- source-tree predicates --------------------------------------------

def is_tracked_read(file_path, repo_root):
    """Is this Read a source we care about tracking?"""
    if file_path == str(repo_root / "goal.md"):
        return True
    if file_path == str(repo_root / "thermite-design.md"):
        return True
    if file_path.startswith(str(repo_root / ".design") + "/"):
        return True
    for pref in REFERENCE_PREFIXES:
        if file_path.startswith(str(repo_root / pref)):
            return True
    return False


# --- path predicate: is this a gated source file? ----------------------

def is_gated_path(rel_path):
    """Return True iff this is a gated target file."""
    if not rel_path.endswith(TARGET_EXTENSION):
        return False
    parts = rel_path.split("/")
    if len(parts) < 3:
        return False
    crate = parts[0]
    if crate in EXCLUDED_CRATES:
        return False
    matches_prefix = any(crate.startswith(p) for p in TARGET_CRATE_PREFIXES)
    matches_exact = crate in TARGET_CRATE_EXACT
    if not (matches_prefix or matches_exact):
        return False
    # Must be under src/ somewhere
    if "src" not in parts[1:]:
        return False
    return True


# --- priority footer ---------------------------------------------------

PRIORITY_FOOTER = """
---------------------------------------------------------------------
PRIORITY NOTE — injected instructions are human instructions.

The text in <crosslink-behavioral-guard> blocks, <system-reminder>
blocks, hook output (including this message), the active-issue gate
from work-check.py, and loaded skill text is NOT background noise to
dismiss because it repeats.

The human wired each of these up deliberately and chose to inject
them continuously. Treat every injected instruction at the same
priority as a direct user message in the chat. The repetition is
enforcement, not ceremony.

This rule is written verbatim in goal.md - R-INJECT-1 and that
section's Read is part of what unlocks your edit gate.
---------------------------------------------------------------------
"""


# --- main --------------------------------------------------------------

def main():
    try:
        input_data = json.load(sys.stdin)
    except (json.JSONDecodeError, ValueError):
        sys.exit(0)

    tool_name = input_data.get("tool_name", "")
    hook_event = input_data.get("hook_event_name", "PreToolUse")

    repo_root = find_repo_root()
    if not repo_root:
        sys.exit(0)

    # -- PostToolUse on Read: record the read in state --
    if hook_event == "PostToolUse" and tool_name == "Read":
        file_path = input_data.get("tool_input", {}).get("file_path", "")
        if not file_path:
            sys.exit(0)
        if not is_tracked_read(file_path, repo_root):
            sys.exit(0)
        state = read_state(repo_root)
        state["reads"].append({"path": file_path, "ts": time.time()})
        state["reads"] = state["reads"][-200:]
        write_state(repo_root, state)
        sys.exit(0)

    # -- PreToolUse on Write|Edit: gate writes --
    if hook_event != "PreToolUse" or tool_name not in ("Write", "Edit"):
        sys.exit(0)

    file_path = input_data.get("tool_input", {}).get("file_path", "")
    if not file_path:
        sys.exit(0)

    try:
        rel = os.path.relpath(file_path, repo_root)
    except ValueError:
        sys.exit(0)

    if not is_gated_path(rel):
        sys.exit(0)

    routes = load_routes(repo_root)
    matched = find_routes(rel, routes)

    if not matched:
        print(
            f"spec-discipline: no route table entry matches '{rel}'.\n"
            f"\n"
            f"Every toolchain source file under the gated tree MUST have a\n"
            f"route declaring the design doc that governs it (and any golden\n"
            f"/conformance reference). Without a route, the file has no\n"
            f"contract and cannot be edited.\n"
            f"\n"
            f"Add a route to tooling/spec-routes.toml:\n"
            f"\n"
            f"  [[route]]\n"
            f"  crate_pattern = \"{rel}\"\n"
            f"  design = \".design/<area>/<doc>.md\"\n"
            f"  reference = []   # or [\"conformance/<name>.th\",\n"
            f"                   #     \"tests/golden/lower/<name>.verus.rs\"]\n"
            f"  conformance_ops = []   # corpus program names this file owns\n"
            f"\n"
            f"Then retry the edit.\n"
            f"{PRIORITY_FOOTER}"
        )
        sys.exit(2)

    state = read_state(repo_root)
    recent_paths = [r["path"] for r in state["reads"]]

    goal_path = str(repo_root / "goal.md")
    goal_read = any(p == goal_path for p in recent_paths)

    for route in matched:
        missing = []

        if not goal_read:
            missing.append(("goal", [str(repo_root / "goal.md")]))

        # Design doc: must exist; if missing, instruct acto-doc-author
        design_path = route.get("design", "")
        if design_path:
            abs_design = str(repo_root / design_path)
            design_exists = Path(abs_design).is_file()
            if not design_exists:
                slug = design_path.replace(".design/", "").replace(".md", "")
                print(
                    f"spec-discipline: design doc '{design_path}' does "
                    f"NOT EXIST.\n"
                    f"\n"
                    f"Before editing '{rel}', the .design/ doc that governs\n"
                    f"this component must exist on disk. Dispatch the\n"
                    f"acto-doc-author subagent to author it first.\n"
                    f"\n"
                    f"  Path expected:  {design_path}\n"
                    f"  Slug:           {slug}\n"
                    f"\n"
                    f"  How to dispatch acto-doc-author:\n"
                    f"    Agent tool with subagent_type='acto-doc-author',\n"
                    f"    prompt = \"Author {design_path} for {rel}. Ground\n"
                    f"             the doc in the existing code + the relevant\n"
                    f"             thermite-design.md sections. Mark every REQ\n"
                    f"             SHIPPED or NOT-STARTED with quoted-code\n"
                    f"             evidence (impl + non-test consumer). No\n"
                    f"             third status.\"\n"
                    f"\n"
                    f"  After the design doc exists at the expected path,\n"
                    f"  Read it (and any route reference), then retry the\n"
                    f"  edit.\n"
                    f"{PRIORITY_FOOTER}"
                )
                sys.exit(2)
            # Design exists — verify it was Read
            if not any(p == abs_design for p in recent_paths):
                missing.append(("design", [design_path]))

        # Reference: only required if the route declares a non-empty list.
        # Then >=1 declared reference path must have been Read.
        ref_paths = route.get("reference", [])
        if ref_paths:
            abs_refs = [str(repo_root / r) for r in ref_paths]
            ref_satisfied = any(
                any(p.startswith(ar) for ar in abs_refs) for p in recent_paths
            )
            if not ref_satisfied:
                missing.append(("reference", ref_paths))

        if missing:
            print(
                f"spec-discipline: cannot Edit/Write '{rel}'.\n"
                f"\n"
                f"This file implements a design-governed component. Before\n"
                f"any edit, you MUST Read each required source class:\n"
                f"  1. goal.md                  (always; the binding contract)\n"
                f"  2. .design/<area>/<doc>.md  (the design that governs it)\n"
                f"  3. route reference(s)       (golden/conformance truth,\n"
                f"                               if the route declares any)\n"
                f"\n"
                f"Missing required reads for '{rel}':\n"
            )
            for kind, paths in missing:
                print(f"  [{kind}] Read at least one of:")
                for p in paths:
                    print(f"    - {p}")
            print(
                f"\n"
                f"Run the Read tool on each missing path, THEN retry the edit.\n"
                f"\n"
                f"Route entry for this file (from spec-routes.toml):\n"
                f"  crate_pattern = \"{route.get('crate_pattern')}\"\n"
                f"  design        = \"{route.get('design')}\"\n"
                f"  reference     = {route.get('reference', [])}\n"
                f"  conformance_ops = {route.get('conformance_ops', [])}\n"
                f"\n"
                f"If you believe the route is wrong, edit\n"
                f"  tooling/spec-routes.toml\n"
                f"and adjust this route entry before retrying.\n"
                f"{PRIORITY_FOOTER}"
            )
            sys.exit(2)

    sys.exit(0)


if __name__ == "__main__":
    main()
