#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""
Canonical REQ registry validator and generated-view writer.

The short-term `req-status.py` gate scans repeated source-comment tables for
obvious contradictions. This tool is the next layer: a single machine-readable
registry with stable IDs, explicit owners, registry-declared status policy, and
typed evidence. Generated status views come from the registry; hand-written
comment tables are legacy input until they are migrated.

Usage:

    tooling/reqs check [--root <repo>]
    tooling/reqs render [--root <repo>]
    tooling/reqs query [--root <repo>] [--json]

    python3 tooling/req-registry.py [--root <repo>] [--check] [--write]
    python3 tooling/req-registry.py --inventory
"""

from __future__ import annotations

import argparse
import json
import re
import shlex
import shutil
import sys
import subprocess
from dataclasses import asdict, dataclass
from pathlib import Path

try:
    import tomllib  # Python 3.11+
except ImportError:  # pragma: no cover
    tomllib = None


REGISTRY_RELPATH = ".design/reqs/registry.toml"
SCHEMA_VERSION = 1

VALID_EVIDENCE_KINDS = {
    "file",
    "symbol",
    "test",
    "issue",
    "doc",
    "command",
}

PATH_EVIDENCE_KINDS = {"file", "test", "doc"}
SOURCE_SUFFIXES = {
    ".rs",
    ".lean",
    ".py",
    ".sh",
    ".md",
    ".toml",
    ".json",
    ".th",
    ".yml",
    ".yaml",
}
SKIP_DIRS = {".git", ".pytest_cache", "target", ".lake", "__pycache__"}
ID_RE = re.compile(r"^REQ-[A-Z0-9][A-Z0-9_.-]*$")
STATUS_RE = re.compile(r"^[a-z][a-z0-9_-]*$")
URI_RE = re.compile(r"^[a-z][a-z0-9+.-]*://\S+$", re.IGNORECASE)
TRACKER_REF_RE = re.compile(r"^[A-Za-z][A-Za-z0-9_.-]*:\S+$")
GITHUB_REF_RE = re.compile(r"^github:[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+#\d+$")


class EnvironmentError3(Exception):
    """The tool could not determine the answer; maps to exit 3."""


@dataclass(frozen=True)
class Evidence:
    kind: str
    target: str
    note: str = ""


@dataclass(frozen=True)
class StatusRule:
    name: str
    final: bool
    required_evidence_any: list[str]
    requires_blocker: bool
    requires_remaining_scope: bool


@dataclass(frozen=True)
class Requirement:
    id: str
    title: str
    owner: str
    status: str
    scope: str
    summary: str
    remaining_scope: str
    aliases: list[str]
    contributors: list[str]
    blockers: list[str]
    generated_to: list[str]
    evidence: list[Evidence]


@dataclass(frozen=True)
class LegacyMapping:
    path: str
    label: str
    id: str
    replacement_view: str
    note: str = ""


@dataclass(frozen=True)
class View:
    name: str
    path: str
    kind: str
    title: str
    mode: str
    region: str
    comment_prefix: str


@dataclass(frozen=True)
class Issue:
    kind: str
    item: str
    detail: str


@dataclass(frozen=True)
class RenderedView:
    path: str
    name: str
    mode: str
    title: str
    text: str
    comment_prefix: str


@dataclass(frozen=True)
class Registry:
    path: str
    schema_version: int | None
    statuses: list[StatusRule]
    views: list[View]
    requirements: list[Requirement]
    legacy_mappings: list[LegacyMapping]
    parse_issues: list[Issue]


def iter_source_files(root: Path):
    for p in sorted(root.rglob("*")):
        if not p.is_file() or p.suffix not in SOURCE_SUFFIXES:
            continue
        rel_parts = p.relative_to(root).parts
        if any(part in SKIP_DIRS for part in rel_parts):
            continue
        yield p


def searchable_text(root: Path) -> str:
    chunks: list[str] = []
    for p in iter_source_files(root):
        try:
            chunks.append(p.read_text(encoding="utf-8"))
        except UnicodeDecodeError:
            chunks.append(p.read_text(encoding="utf-8", errors="ignore"))
    return "\n".join(chunks)


def target_path_part(target: str) -> str:
    token = target.strip().rstrip(".,;:()[]{}")
    if "::" in token:
        token = token.split("::", 1)[0]
    if "#" in token:
        token = token.split("#", 1)[0]
    return token


def path_exists(root: Path, target: str) -> bool:
    token = target_path_part(target)
    return bool(token) and (root / token).exists()


def repo_or_abs_path_exists(root: Path, target: str) -> bool:
    path = Path(target)
    return path.exists() if path.is_absolute() else (root / path).exists()


def symbol_exists(haystack: str, target: str) -> bool:
    identifiers = re.findall(r"[A-Za-z_][A-Za-z0-9_]*", target)
    for ident in reversed(identifiers):
        if len(ident) < 3:
            continue
        if re.search(rf"(?<![A-Za-z0-9_]){re.escape(ident)}(?![A-Za-z0-9_])", haystack):
            return True
    return False


def command_path_candidate(token: str) -> str:
    token = target_path_part(token)
    if not token or token.startswith("-"):
        return ""
    if token in {".", ".."}:
        return token
    if "/" in token:
        return token
    suffix = Path(token).suffix
    if suffix in SOURCE_SUFFIXES or suffix in {".lock", ".txt"}:
        return token
    return ""


def command_detail(root: Path, target: str) -> str | None:
    if not target.strip():
        return "command evidence target must be non-empty"
    try:
        argv = shlex.split(target)
    except ValueError as exc:
        return f"command evidence target is not shell-parseable: {exc}"
    if not argv:
        return "command evidence target must include an executable"

    executable = argv[0]
    if "/" in executable or executable.startswith("."):
        if not repo_or_abs_path_exists(root, executable):
            return f"command executable path does not resolve: {executable}"
    elif shutil.which(executable) is None:
        return f"command executable does not resolve on PATH: {executable}"

    for token in argv[1:]:
        candidate = command_path_candidate(token)
        if candidate and not repo_or_abs_path_exists(root, candidate):
            return f"command path argument does not resolve: {candidate}"
    return None


def github_ref_parts(ref: str) -> tuple[str, str] | None:
    if not GITHUB_REF_RE.match(ref):
        return None
    repo, number = ref.removeprefix("github:").rsplit("#", 1)
    return repo, number


def live_github_issue_state(ref: str) -> str:
    parts = github_ref_parts(ref)
    if parts is None:
        raise EnvironmentError3(f"not a GitHub issue reference: {ref}")
    if shutil.which("gh") is None:
        raise EnvironmentError3(
            "live issue validation requested but `gh` is not available on PATH"
        )

    repo, number = parts
    result = subprocess.run(
        ["gh", "issue", "view", number, "--repo", repo, "--json", "state"],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        detail = result.stderr.strip() or result.stdout.strip() or "unknown gh failure"
        raise EnvironmentError3(f"live issue validation failed for {ref}: {detail}")
    try:
        payload = json.loads(result.stdout)
    except json.JSONDecodeError as exc:
        raise EnvironmentError3(
            f"live issue validation returned invalid JSON for {ref}: {exc}"
        ) from exc
    state = str(payload.get("state", "")).upper()
    if state not in {"OPEN", "CLOSED"}:
        raise EnvironmentError3(
            f"live issue validation returned unknown state for {ref}: {state or '<empty>'}"
        )
    return state


def reference_detail(ref: str, req_ids: set[str]) -> str | None:
    """Return None when a tracker/URI/REQ reference resolves structurally."""
    if not ref:
        return "reference is empty"
    if ref.startswith("req:"):
        req_id = ref.removeprefix("req:")
        if req_id in req_ids:
            return None
        return f"requirement reference does not resolve: {ref}"
    if GITHUB_REF_RE.match(ref):
        return None
    if URI_RE.match(ref):
        return None
    if TRACKER_REF_RE.match(ref):
        return None
    return (
        f"reference `{ref}` must be a URI, tracker:id, "
        "github:owner/repo#N, or req:REQ-ID"
    )


def _as_str(raw: dict, field: str, item: str, issues: list[Issue]) -> str:
    value = raw.get(field)
    if not isinstance(value, str) or not value.strip():
        issues.append(
            Issue(
                "BAD-FIELD",
                item,
                f"`{field}` must be a non-empty string",
            )
        )
        return ""
    return value.strip()


def _optional_str(raw: dict, field: str, item: str, issues: list[Issue]) -> str:
    value = raw.get(field, "")
    if value is None:
        return ""
    if not isinstance(value, str):
        issues.append(Issue("BAD-FIELD", item, f"`{field}` must be a string"))
        return ""
    return value.strip()


def _optional_raw_str(raw: dict, field: str, item: str, issues: list[Issue]) -> str:
    value = raw.get(field, "")
    if value is None:
        return ""
    if not isinstance(value, str):
        issues.append(Issue("BAD-FIELD", item, f"`{field}` must be a string"))
        return ""
    return value


def _list_of_str(raw: dict, field: str, item: str, issues: list[Issue]) -> list[str]:
    value = raw.get(field, [])
    if value is None:
        return []
    if not isinstance(value, list) or any(not isinstance(v, str) for v in value):
        issues.append(Issue("BAD-FIELD", item, f"`{field}` must be a list of strings"))
        return []
    return [v.strip() for v in value if v.strip()]


def _bool_field(
    raw: dict,
    field: str,
    item: str,
    issues: list[Issue],
    *,
    default: bool,
) -> bool:
    value = raw.get(field, default)
    if not isinstance(value, bool):
        issues.append(Issue("BAD-FIELD", item, f"`{field}` must be a boolean"))
        return default
    return value


def parse_evidence(raw_req: dict, item: str, issues: list[Issue]) -> list[Evidence]:
    raw_items = raw_req.get("evidence", [])
    if raw_items is None:
        return []
    if not isinstance(raw_items, list):
        issues.append(Issue("BAD-FIELD", item, "`evidence` must be a list of tables"))
        return []
    evidence: list[Evidence] = []
    for i, raw_ev in enumerate(raw_items):
        ev_item = f"{item}.evidence[{i}]"
        if not isinstance(raw_ev, dict):
            issues.append(Issue("BAD-FIELD", ev_item, "evidence entry must be a table"))
            continue
        evidence.append(
            Evidence(
                kind=_as_str(raw_ev, "kind", ev_item, issues),
                target=_as_str(raw_ev, "target", ev_item, issues),
                note=_optional_str(raw_ev, "note", ev_item, issues),
            )
        )
    return evidence


def parse_statuses(raw: dict, item: str, issues: list[Issue]) -> list[StatusRule]:
    raw_statuses = raw.get("status", [])
    if not isinstance(raw_statuses, list):
        issues.append(Issue("BAD-FIELD", item, "`status` must be a list of tables"))
        return []

    statuses: list[StatusRule] = []
    for i, raw_status in enumerate(raw_statuses):
        status_item = f"status[{i}]"
        if not isinstance(raw_status, dict):
            issues.append(Issue("BAD-FIELD", status_item, "status entry must be a table"))
            continue
        statuses.append(
            StatusRule(
                name=_as_str(raw_status, "name", status_item, issues),
                final=_bool_field(raw_status, "final", status_item, issues, default=False),
                required_evidence_any=_list_of_str(
                    raw_status, "required_evidence_any", status_item, issues
                ),
                requires_blocker=_bool_field(
                    raw_status, "requires_blocker", status_item, issues, default=False
                ),
                requires_remaining_scope=_bool_field(
                    raw_status,
                    "requires_remaining_scope",
                    status_item,
                    issues,
                    default=False,
                ),
            )
        )
    return statuses


def parse_legacy_mappings(raw: dict, item: str, issues: list[Issue]) -> list[LegacyMapping]:
    raw_mappings = raw.get("legacy_mapping", [])
    if not isinstance(raw_mappings, list):
        issues.append(Issue("BAD-FIELD", item, "`legacy_mapping` must be a list of tables"))
        return []

    mappings: list[LegacyMapping] = []
    for i, raw_mapping in enumerate(raw_mappings):
        mapping_item = f"legacy_mapping[{i}]"
        if not isinstance(raw_mapping, dict):
            issues.append(
                Issue("BAD-FIELD", mapping_item, "legacy_mapping entry must be a table")
            )
            continue
        mappings.append(
            LegacyMapping(
                path=_as_str(raw_mapping, "path", mapping_item, issues),
                label=_as_str(raw_mapping, "label", mapping_item, issues),
                id=_as_str(raw_mapping, "id", mapping_item, issues),
                replacement_view=_as_str(raw_mapping, "replacement_view", mapping_item, issues),
                note=_optional_str(raw_mapping, "note", mapping_item, issues),
            )
        )
    return mappings


def load_registry(root: Path, relpath: str = REGISTRY_RELPATH) -> Registry:
    if tomllib is None:
        raise EnvironmentError3("tomllib is unavailable (Python < 3.11)")

    path = root / relpath
    if not path.is_file():
        return Registry(
            relpath,
            None,
            [],
            [],
            [],
            [],
            [Issue("MISSING-REGISTRY", relpath, "registry file does not exist")],
        )

    try:
        raw = tomllib.loads(path.read_text(encoding="utf-8"))
    except tomllib.TOMLDecodeError as exc:
        return Registry(
            relpath,
            None,
            [],
            [],
            [],
            [],
            [Issue("INVALID-TOML", relpath, str(exc))],
        )
    except OSError as exc:
        raise EnvironmentError3(f"registry unreadable ({relpath}): {exc}") from exc

    issues: list[Issue] = []
    schema_version = raw.get("schema_version")
    if not isinstance(schema_version, int):
        issues.append(
            Issue("BAD-SCHEMA", relpath, "`schema_version` must be integer 1")
        )

    statuses = parse_statuses(raw, relpath, issues)
    legacy_mappings = parse_legacy_mappings(raw, relpath, issues)

    views: list[View] = []
    raw_views = raw.get("view", [])
    if not isinstance(raw_views, list):
        issues.append(Issue("BAD-FIELD", relpath, "`view` must be a list of tables"))
        raw_views = []
    for i, raw_view in enumerate(raw_views):
        item = f"view[{i}]"
        if not isinstance(raw_view, dict):
            issues.append(Issue("BAD-FIELD", item, "view entry must be a table"))
            continue
        views.append(
            View(
                name=_as_str(raw_view, "name", item, issues),
                path=_as_str(raw_view, "path", item, issues),
                kind=_as_str(raw_view, "kind", item, issues),
                title=_optional_str(raw_view, "title", item, issues),
                mode=_optional_str(raw_view, "mode", item, issues) or "file",
                region=_optional_str(raw_view, "region", item, issues),
                comment_prefix=_optional_raw_str(raw_view, "comment_prefix", item, issues),
            )
        )

    requirements: list[Requirement] = []
    raw_reqs = raw.get("requirement", [])
    if not isinstance(raw_reqs, list):
        issues.append(
            Issue("BAD-FIELD", relpath, "`requirement` must be a list of tables")
        )
        raw_reqs = []
    for i, raw_req in enumerate(raw_reqs):
        item = f"requirement[{i}]"
        if not isinstance(raw_req, dict):
            issues.append(Issue("BAD-FIELD", item, "requirement entry must be a table"))
            continue
        req_id = _as_str(raw_req, "id", item, issues)
        req_item = req_id or item
        requirements.append(
            Requirement(
                id=req_id,
                title=_as_str(raw_req, "title", req_item, issues),
                owner=_as_str(raw_req, "owner", req_item, issues),
                status=_as_str(raw_req, "status", req_item, issues),
                scope=_as_str(raw_req, "scope", req_item, issues),
                summary=_optional_str(raw_req, "summary", req_item, issues),
                remaining_scope=_optional_str(raw_req, "remaining_scope", req_item, issues),
                aliases=_list_of_str(raw_req, "aliases", req_item, issues),
                contributors=_list_of_str(raw_req, "contributors", req_item, issues),
                blockers=_list_of_str(raw_req, "blockers", req_item, issues),
                generated_to=_list_of_str(raw_req, "generated_to", req_item, issues),
                evidence=parse_evidence(raw_req, req_item, issues),
            )
        )

    return Registry(
        relpath,
        schema_version,
        statuses,
        views,
        requirements,
        legacy_mappings,
        issues,
    )


def validate_registry(root: Path, registry: Registry, *, live_issues: bool = False) -> list[Issue]:
    issues = list(registry.parse_issues)
    haystack = searchable_text(root)

    if registry.schema_version != SCHEMA_VERSION:
        issues.append(
            Issue(
                "BAD-SCHEMA",
                registry.path,
                f"schema_version must be {SCHEMA_VERSION}",
            )
        )

    status_rules: dict[str, StatusRule] = {}
    for status in registry.statuses:
        if not status.name:
            continue
        if not STATUS_RE.match(status.name):
            issues.append(
                Issue(
                    "BAD-STATUS-NAME",
                    status.name,
                    "status names must be lowercase tokens",
                )
            )
        if status.name in status_rules:
            issues.append(
                Issue("DUPLICATE-STATUS", status.name, "status names must be unique")
            )
        status_rules[status.name] = status
        for kind in status.required_evidence_any:
            if kind not in VALID_EVIDENCE_KINDS:
                issues.append(
                    Issue(
                        "BAD-STATUS-EVIDENCE-KIND",
                        status.name,
                        f"required evidence kind `{kind}` is not accepted",
                    )
                )
    if not status_rules:
        issues.append(
            Issue("MISSING-STATUS-DEFS", registry.path, "registry must declare statuses")
        )

    view_names: dict[str, View] = {}
    for view in registry.views:
        if not view.name:
            continue
        if view.name in view_names:
            issues.append(Issue("DUPLICATE-VIEW", view.name, "view names must be unique"))
        view_names[view.name] = view
        if view.kind not in {"full_inventory", "reference_list"}:
            issues.append(
                Issue(
                    "UNKNOWN-VIEW-KIND",
                    view.name,
                    "`kind` must be `full_inventory` or `reference_list` in schema v1",
                )
            )
        if view.mode not in {"file", "region"}:
            issues.append(
                Issue(
                    "BAD-VIEW-MODE",
                    view.name,
                    "`mode` must be `file` or `region`",
                )
            )
        if view.mode == "region" and not view.region:
            issues.append(
                Issue(
                    "MISSING-VIEW-REGION",
                    view.name,
                    "region-mode views must name a `region`",
                )
            )
        if view.mode == "file" and view.path and not view.path.startswith(".design/"):
            issues.append(
                Issue(
                    "BAD-VIEW-PATH",
                    view.name,
                    "whole-file generated views must live under `.design/`",
                )
            )
        if view.comment_prefix and view.mode != "region":
            issues.append(
                Issue(
                    "BAD-COMMENT-PREFIX",
                    view.name,
                    "`comment_prefix` is only valid for region-mode views",
                )
            )
        if "\n" in view.comment_prefix or "\r" in view.comment_prefix:
            issues.append(
                Issue(
                    "BAD-COMMENT-PREFIX",
                    view.name,
                    "`comment_prefix` must be a single-line prefix",
                )
            )
        if (
            view.mode == "region"
            and view.kind == "reference_list"
            and view.path
            and not view.path.endswith(".md")
            and not view.comment_prefix
        ):
            issues.append(
                Issue(
                    "MISSING-COMMENT-PREFIX",
                    view.name,
                    "source-file reference-list regions must declare `comment_prefix`",
                )
            )

    req_ids: dict[str, Requirement] = {}
    for req in registry.requirements:
        if req.id:
            if not ID_RE.match(req.id):
                issues.append(
                    Issue(
                        "BAD-REQ-ID",
                        req.id,
                        "requirement IDs must be stable `REQ-*` tokens",
                    )
                )
            if req.id in req_ids:
                issues.append(Issue("DUPLICATE-REQ-ID", req.id, "IDs must be unique"))
            req_ids[req.id] = req

    aliases: dict[str, str] = {}
    for req in registry.requirements:
        for alias in req.aliases:
            if alias in aliases:
                issues.append(
                    Issue(
                        "DUPLICATE-ALIAS",
                        req.id,
                        f"alias `{alias}` is already mapped to {aliases[alias]}",
                    )
                )
            aliases[alias] = req.id

    req_id_set = set(req_ids)

    seen_mappings: set[tuple[str, str]] = set()
    for mapping in registry.legacy_mappings:
        key = (mapping.path, mapping.label)
        if key in seen_mappings:
            issues.append(
                Issue(
                    "DUPLICATE-LEGACY-MAPPING",
                    mapping.id,
                    f"legacy mapping repeated for {mapping.path}: {mapping.label}",
                )
            )
        seen_mappings.add(key)
        if mapping.id not in req_ids:
            issues.append(
                Issue(
                    "UNKNOWN-LEGACY-ID",
                    mapping.id,
                    f"legacy mapping target does not exist for {mapping.path}: {mapping.label}",
                )
            )
        replacement = view_names.get(mapping.replacement_view)
        if replacement is None:
            issues.append(
                Issue(
                    "UNKNOWN-LEGACY-REPLACEMENT",
                    mapping.id,
                    f"legacy mapping replacement view `{mapping.replacement_view}` is unknown",
                )
            )
        elif replacement.path != mapping.path:
            issues.append(
                Issue(
                    "BAD-LEGACY-REPLACEMENT",
                    mapping.id,
                    "legacy mapping replacement view must target the same path",
                )
            )
        path = root / mapping.path
        try:
            source_text = path.read_text(encoding="utf-8")
        except FileNotFoundError:
            issues.append(
                Issue(
                    "UNRESOLVED-LEGACY-PATH",
                    mapping.id,
                    f"legacy mapping path does not exist: {mapping.path}",
                )
            )
            continue
        except UnicodeDecodeError:
            source_text = path.read_text(encoding="utf-8", errors="ignore")

        replacement_present = (
            replacement is not None and generated_start(replacement) in source_text
        )
        if mapping.label not in source_text and not replacement_present:
            issues.append(
                Issue(
                    "STALE-LEGACY-MAPPING",
                    mapping.id,
                    "legacy mapping is neither present as an old label nor replaced by its generated view",
                )
            )

    for req in registry.requirements:
        status_rule = status_rules.get(req.status)
        if status_rule is None:
            issues.append(
                Issue(
                    "BAD-STATUS",
                    req.id or "<missing>",
                    f"`status` must be declared in a top-level [[status]] table: {req.status}",
                )
            )

        if req.owner and ("/" in req.owner or req.owner.startswith(".")):
            if not path_exists(root, req.owner):
                issues.append(
                    Issue("UNRESOLVED-OWNER", req.id, f"owner path does not exist: {req.owner}")
                )

        if not req.generated_to:
            issues.append(
                Issue("MISSING-GENERATED-VIEW", req.id, "`generated_to` must name at least one view")
            )
        for view_name in req.generated_to:
            if view_name not in view_names:
                issues.append(
                    Issue("UNKNOWN-GENERATED-VIEW", req.id, f"unknown view `{view_name}`")
                )

        if status_rule is not None:
            if status_rule.required_evidence_any and not any(
                ev.kind in status_rule.required_evidence_any for ev in req.evidence
            ):
                issues.append(
                    Issue(
                        "WEAK-STATUS-EVIDENCE",
                        req.id,
                        f"`{req.status}` requires at least one evidence kind from: "
                        + ", ".join(status_rule.required_evidence_any),
                    )
                )
            if status_rule.requires_blocker and not req.blockers:
                issues.append(
                    Issue(
                        "MISSING-BLOCKER",
                        req.id,
                        f"`{req.status}` requires at least one blocker reference",
                    )
                )
            if status_rule.requires_remaining_scope and not req.remaining_scope:
                issues.append(
                    Issue(
                        "MISSING-REMAINING-SCOPE",
                        req.id,
                        f"`{req.status}` requires remaining_scope",
                    )
                )

        for blocker in req.blockers:
            detail = reference_detail(blocker, req_id_set)
            if detail is not None:
                issues.append(
                    Issue(
                        "BAD-BLOCKER",
                        req.id,
                        detail,
                    )
                )
            elif live_issues and github_ref_parts(blocker) is not None:
                state = live_github_issue_state(blocker)
                if state != "OPEN":
                    issues.append(
                        Issue(
                            "CLOSED-BLOCKER",
                            req.id,
                            f"blocker `{blocker}` resolved {state.lower()}",
                        )
                    )

        for ev in req.evidence:
            if ev.kind not in VALID_EVIDENCE_KINDS:
                issues.append(
                    Issue(
                        "BAD-EVIDENCE-KIND",
                        req.id,
                        f"evidence kind `{ev.kind}` is not accepted",
                    )
                )
                continue
            if ev.kind in PATH_EVIDENCE_KINDS and not path_exists(root, ev.target):
                issues.append(
                    Issue(
                        "UNRESOLVED-EVIDENCE",
                        req.id,
                        f"{ev.kind} evidence path does not exist: {ev.target}",
                    )
                )
            if ev.kind == "symbol" and not symbol_exists(haystack, ev.target):
                issues.append(
                    Issue(
                        "UNRESOLVED-EVIDENCE",
                        req.id,
                        f"symbol evidence does not resolve: {ev.target}",
                    )
                )
            if ev.kind == "issue":
                detail = reference_detail(ev.target, req_id_set)
                if detail is not None:
                    issues.append(
                        Issue(
                            "BAD-EVIDENCE-TARGET",
                            req.id,
                            detail,
                        )
                    )
                elif live_issues and github_ref_parts(ev.target) is not None:
                    live_github_issue_state(ev.target)
            if ev.kind == "command":
                detail = command_detail(root, ev.target)
                if detail is not None:
                    issues.append(
                        Issue(
                            "BAD-EVIDENCE-TARGET",
                            req.id,
                            detail,
                        )
                    )

    return sorted(issues, key=lambda issue: (issue.item, issue.kind, issue.detail))


def markdown_cell(text: str) -> str:
    cleaned = " ".join(text.split())
    return cleaned.replace("\\", "\\\\").replace("|", "\\|")


def render_evidence(req: Requirement) -> str:
    parts = []
    for ev in req.evidence:
        note = f" - {ev.note}" if ev.note else ""
        parts.append(f"{ev.kind}: `{ev.target}`{note}")
    return "<br>".join(parts) if parts else ""


def render_followup(req: Requirement) -> str:
    parts = []
    if req.remaining_scope:
        parts.append(req.remaining_scope)
    if req.blockers:
        parts.append("blockers: " + ", ".join(req.blockers))
    return "<br>".join(parts)


def _line_with_comment_prefix(prefix: str, line: str) -> str:
    if not prefix:
        return line
    if not line:
        return prefix.rstrip()
    return prefix + line


def generated_start(view: View | RenderedView) -> str:
    name = view.region if isinstance(view, View) and view.region else view.name
    return _line_with_comment_prefix(
        view.comment_prefix,
        f"<!-- generated:reqs view={name} -->",
    )


def generated_end(view: View | RenderedView) -> str:
    return _line_with_comment_prefix(view.comment_prefix, "<!-- /generated:reqs -->")


def render_full_inventory_body(registry: Registry, view: View) -> str:
    rows = [
        req
        for req in registry.requirements
        if view.name in req.generated_to
    ]
    rows.sort(key=lambda req: req.id)
    out = [
        f"Source: `{registry.path}`",
        "",
        "| ID | Status | Owner | Contributors | Scope | Title | Evidence | Follow-up |",
        "|---|---|---|---|---|---|---|---|",
    ]
    for req in rows:
        out.append(
            "| "
            + " | ".join(
                [
                    markdown_cell(req.id),
                    markdown_cell(req.status),
                    markdown_cell(f"`{req.owner}`"),
                    markdown_cell(", ".join(f"`{c}`" for c in req.contributors)),
                    markdown_cell(req.scope),
                    markdown_cell(req.title),
                    markdown_cell(render_evidence(req)),
                    markdown_cell(render_followup(req)),
                ]
            )
            + " |"
        )
    out.append("")
    return "\n".join(out)


def render_reference_list_body(registry: Registry, view: View) -> str:
    rows = [
        req
        for req in registry.requirements
        if view.name in req.generated_to
    ]
    rows.sort(key=lambda req: req.id)
    out = [
        f"Source: `{registry.path}`",
        "",
        "| ID | Status | Owner | Title | Follow-up |",
        "|---|---|---|---|---|",
    ]
    for req in rows:
        out.append(
            "| "
            + " | ".join(
                [
                    markdown_cell(req.id),
                    markdown_cell(req.status),
                    markdown_cell(f"`{req.owner}`"),
                    markdown_cell(req.title),
                    markdown_cell(render_followup(req)),
                ]
            )
            + " |"
        )
    out.append("")
    return "\n".join(out)


def render_full_file(registry: Registry, view: View, body: str) -> str:
    title = view.title or "Requirement Status Inventory"
    return "\n".join(
        [
            "<!-- generated by tooling/req-registry.py; do not edit by hand -->",
            f"# {title}",
            "",
            body,
        ]
    )


def region_block(view: View | RenderedView, body: str) -> str:
    if not view.comment_prefix:
        return f"{generated_start(view)}\n{body}{generated_end(view)}\n"

    prefixed_body = "\n".join(
        _line_with_comment_prefix(view.comment_prefix, line)
        for line in body.splitlines()
    )
    return f"{generated_start(view)}\n{prefixed_body}\n{generated_end(view)}\n"


def render_views(registry: Registry) -> list[RenderedView]:
    rendered: list[RenderedView] = []
    for view in registry.views:
        if view.kind == "full_inventory":
            body = render_full_inventory_body(registry, view)
        elif view.kind == "reference_list":
            body = render_reference_list_body(registry, view)
        else:
            continue
        text = body if view.mode == "region" else render_full_file(registry, view, body)
        rendered.append(
            RenderedView(
                path=view.path,
                name=view.region or view.name,
                mode=view.mode,
                title=view.title or "Requirement Status Inventory",
                text=text,
                comment_prefix=view.comment_prefix,
            )
        )
    return rendered


def existing_region_block(text: str, view: RenderedView) -> str | None:
    start = generated_start(view)
    end = generated_end(view)
    start_idx = text.find(start)
    if start_idx < 0:
        return None
    end_idx = text.find(end, start_idx)
    if end_idx < 0:
        return None
    end_idx += len(end)
    if end_idx < len(text) and text[end_idx] == "\n":
        end_idx += 1
    return text[start_idx:end_idx]


def default_region_document(view: RenderedView) -> str:
    if view.comment_prefix:
        return region_block(view, view.text)
    return f"# {view.title}\n\n{region_block(view, view.text)}"


def validate_generated(root: Path, rendered: list[RenderedView]) -> list[Issue]:
    issues: list[Issue] = []
    for view in sorted(rendered, key=lambda v: (v.path, v.name)):
        path = root / view.path
        try:
            actual = path.read_text(encoding="utf-8")
        except FileNotFoundError:
            issues.append(Issue("MISSING-GENERATED", view.path, "generated view is absent"))
            continue
        except OSError as exc:
            raise EnvironmentError3(f"generated view unreadable ({view.path}): {exc}") from exc

        if view.mode == "region":
            expected = region_block(view, view.text)
            existing = existing_region_block(actual, view)
            if existing is None:
                issues.append(
                    Issue(
                        "MISSING-GENERATED-REGION",
                        view.path,
                        f"generated region `{view.name}` is absent",
                    )
                )
            elif existing != expected:
                issues.append(
                    Issue(
                        "STALE-GENERATED",
                        view.path,
                        "generated region differs; run `python3 tooling/req-registry.py --write`",
                    )
                )
            continue

        if actual != view.text:
            issues.append(
                Issue(
                    "STALE-GENERATED",
                    view.path,
                    "generated view differs; run `python3 tooling/req-registry.py --write`",
                )
            )
    return issues


def write_generated(root: Path, rendered: list[RenderedView]) -> None:
    for view in sorted(rendered, key=lambda v: (v.path, v.name)):
        path = root / view.path
        path.parent.mkdir(parents=True, exist_ok=True)
        if view.mode != "region":
            path.write_text(view.text, encoding="utf-8")
            continue

        block = region_block(view, view.text)
        try:
            existing = path.read_text(encoding="utf-8")
        except FileNotFoundError:
            path.write_text(default_region_document(view), encoding="utf-8")
            continue
        start = generated_start(view)
        end = generated_end(view)
        start_idx = existing.find(start)
        end_idx = existing.find(end, start_idx if start_idx >= 0 else 0)
        if start_idx < 0 or end_idx < 0:
            suffix = "" if existing.endswith("\n") else "\n"
            path.write_text(existing + suffix + "\n" + block, encoding="utf-8")
            continue
        end_idx += len(end)
        if end_idx < len(existing) and existing[end_idx] == "\n":
            end_idx += 1
        path.write_text(existing[:start_idx] + block + existing[end_idx:], encoding="utf-8")


def render_inventory(registry: Registry) -> str:
    out = []
    for req in sorted(registry.requirements, key=lambda r: r.id):
        out.append(f"{req.status.upper()}  {req.id}  {req.owner}  {req.title}")
    return "\n".join(out)


def render_issues(issues: list[Issue]) -> str:
    out = []
    for issue in issues:
        out.append(f"{issue.kind}  {issue.item}\n  {issue.detail}")
    return "\n".join(out)


def registry_json(registry: Registry, issues: list[Issue]) -> str:
    return json.dumps(
        {
            "path": registry.path,
            "schema_version": registry.schema_version,
            "statuses": [asdict(status) for status in registry.statuses],
            "views": [asdict(view) for view in registry.views],
            "requirements": [asdict(req) for req in registry.requirements],
            "legacy_mappings": [asdict(mapping) for mapping in registry.legacy_mappings],
            "issues": [asdict(issue) for issue in issues],
        },
        indent=2,
        sort_keys=True,
    )


def normalize_argv(argv: list[str]) -> list[str]:
    if not argv:
        return argv
    command, rest = argv[0], argv[1:]
    if command == "check":
        return ["--check", *rest]
    if command == "render":
        return ["--write", *rest]
    if command == "query":
        if "--inventory" not in rest and "--json" not in rest:
            return ["--inventory", *rest]
        return rest
    return argv


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", default=".", help="repo root to scan")
    parser.add_argument("--registry", default=REGISTRY_RELPATH, help="registry path")
    parser.add_argument("--check", action="store_true", help="fail if generated views are stale")
    parser.add_argument("--write", action="store_true", help="rewrite generated views")
    parser.add_argument("--inventory", action="store_true", help="print normalized inventory")
    parser.add_argument("--json", action="store_true", help="emit JSON")
    parser.add_argument(
        "--live-issues",
        action="store_true",
        help="resolve GitHub issue refs with gh; closed blockers fail",
    )
    args = parser.parse_args(normalize_argv(argv))

    root = Path(args.root).resolve()
    try:
        registry = load_registry(root, args.registry)
        issues = validate_registry(root, registry, live_issues=args.live_issues)
        rendered = render_views(registry)

        if args.write and not issues:
            write_generated(root, rendered)
        elif args.write and issues:
            # Avoid rewriting generated docs from invalid registry data.
            pass

        if args.check and not args.write:
            issues.extend(validate_generated(root, rendered))
            issues = sorted(issues, key=lambda issue: (issue.item, issue.kind, issue.detail))
    except EnvironmentError3 as exc:
        print(f"REQ registry inconclusive: {exc}", file=sys.stderr)
        return 3

    if args.json:
        print(registry_json(registry, issues))
    elif args.inventory:
        inventory = render_inventory(registry)
        if inventory:
            print(inventory)
        if issues:
            print("\nREQ registry failed:\n" + render_issues(issues), file=sys.stderr)
    elif issues:
        print("REQ registry failed:\n" + render_issues(issues))
    elif args.write:
        print(f"REQ registry wrote {len(rendered)} generated view(s)")
    else:
        print(
            "REQ registry clean: "
            f"{len(registry.requirements)} requirement(s), {len(registry.views)} view(s)"
        )

    return 1 if issues else 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
