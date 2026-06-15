#!/usr/bin/env python3
"""
Canonical REQ registry validator and generated-view writer.

The short-term `req-status.py` gate scans repeated source-comment tables for
obvious contradictions. This tool is the next layer: a single machine-readable
registry with stable IDs, explicit owners, accepted statuses, and typed
evidence. Generated status views come from the registry; hand-written comment
tables are legacy input until they are migrated.

Usage:

    python3 tooling/req-registry.py [--root <repo>] [--check] [--write]
    python3 tooling/req-registry.py --inventory
    python3 tooling/req-registry.py --json
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import asdict, dataclass
from pathlib import Path

try:
    import tomllib  # Python 3.11+
except ImportError:  # pragma: no cover
    tomllib = None


REGISTRY_RELPATH = ".design/reqs/registry.toml"
SCHEMA_VERSION = 1

VALID_STATUSES = {
    "shipped",
    "not_started",
    "partial",
    "blocked",
    "deferred",
}

VALID_EVIDENCE_KINDS = {
    "file",
    "symbol",
    "test",
    "issue",
    "doc",
    "command",
}

PATH_EVIDENCE_KINDS = {"file", "test", "doc"}
SHIPPED_PROOF_KINDS = {"file", "symbol", "test"}
FUTURE_STATUSES = {"not_started", "blocked", "deferred"}
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
ISSUE_RE = re.compile(r"^(#\d+|https://github\.com/[^/]+/[^/]+/issues/\d+)$")


class EnvironmentError3(Exception):
    """The tool could not determine the answer; maps to exit 3."""


@dataclass(frozen=True)
class Evidence:
    kind: str
    target: str
    note: str = ""


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
    blockers: list[str]
    generated_to: list[str]
    evidence: list[Evidence]


@dataclass(frozen=True)
class View:
    name: str
    path: str
    kind: str
    title: str


@dataclass(frozen=True)
class Issue:
    kind: str
    item: str
    detail: str


@dataclass(frozen=True)
class Registry:
    path: str
    schema_version: int | None
    views: list[View]
    requirements: list[Requirement]
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


def symbol_exists(haystack: str, target: str) -> bool:
    identifiers = re.findall(r"[A-Za-z_][A-Za-z0-9_]*", target)
    for ident in reversed(identifiers):
        if len(ident) < 3:
            continue
        if re.search(rf"(?<![A-Za-z0-9_]){re.escape(ident)}(?![A-Za-z0-9_])", haystack):
            return True
    return False


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


def _list_of_str(raw: dict, field: str, item: str, issues: list[Issue]) -> list[str]:
    value = raw.get(field, [])
    if value is None:
        return []
    if not isinstance(value, list) or any(not isinstance(v, str) for v in value):
        issues.append(Issue("BAD-FIELD", item, f"`{field}` must be a list of strings"))
        return []
    return [v.strip() for v in value if v.strip()]


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
                blockers=_list_of_str(raw_req, "blockers", req_item, issues),
                generated_to=_list_of_str(raw_req, "generated_to", req_item, issues),
                evidence=parse_evidence(raw_req, req_item, issues),
            )
        )

    return Registry(relpath, schema_version, views, requirements, issues)


def validate_registry(root: Path, registry: Registry) -> list[Issue]:
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

    view_names: dict[str, View] = {}
    for view in registry.views:
        if not view.name:
            continue
        if view.name in view_names:
            issues.append(Issue("DUPLICATE-VIEW", view.name, "view names must be unique"))
        view_names[view.name] = view
        if view.kind != "full_inventory":
            issues.append(
                Issue(
                    "UNKNOWN-VIEW-KIND",
                    view.name,
                    "`kind` must be `full_inventory` in schema v1",
                )
            )
        if view.path and not view.path.startswith(".design/"):
            issues.append(
                Issue(
                    "BAD-VIEW-PATH",
                    view.name,
                    "generated views must live under `.design/`",
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

        if req.status not in VALID_STATUSES:
            issues.append(
                Issue(
                    "BAD-STATUS",
                    req.id or "<missing>",
                    f"`status` must be one of {', '.join(sorted(VALID_STATUSES))}",
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

        if req.status == "shipped":
            if not any(ev.kind in SHIPPED_PROOF_KINDS for ev in req.evidence):
                issues.append(
                    Issue(
                        "WEAK-SHIPPED-EVIDENCE",
                        req.id,
                        "shipped requirements need file, symbol, or test evidence",
                    )
                )

        if req.status in FUTURE_STATUSES and not (req.blockers or req.remaining_scope):
            issues.append(
                Issue(
                    "MISSING-FUTURE-SCOPE",
                    req.id,
                    f"`{req.status}` requirements need blockers or remaining_scope",
                )
            )

        if req.status == "blocked" and not req.blockers:
            issues.append(
                Issue("MISSING-BLOCKER", req.id, "blocked requirements need blockers")
            )

        if req.status == "partial":
            if not req.remaining_scope:
                issues.append(
                    Issue(
                        "MISSING-REMAINING-SCOPE",
                        req.id,
                        "partial requirements need remaining_scope",
                    )
                )
            if not req.evidence:
                issues.append(
                    Issue("MISSING-PARTIAL-EVIDENCE", req.id, "partial requirements need evidence")
                )

        for blocker in req.blockers:
            if not ISSUE_RE.match(blocker):
                issues.append(
                    Issue(
                        "BAD-BLOCKER",
                        req.id,
                        f"blocker `{blocker}` must be #N or a GitHub issue URL",
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
            if ev.kind == "issue" and not ISSUE_RE.match(ev.target):
                issues.append(
                    Issue(
                        "BAD-EVIDENCE-TARGET",
                        req.id,
                        f"issue evidence `{ev.target}` must be #N or a GitHub issue URL",
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


def render_full_inventory(registry: Registry, view: View) -> str:
    rows = [
        req
        for req in registry.requirements
        if view.name in req.generated_to
    ]
    rows.sort(key=lambda req: req.id)
    title = view.title or "Requirement Status Inventory"
    out = [
        "<!-- generated by tooling/req-registry.py; do not edit by hand -->",
        f"# {title}",
        "",
        f"Source: `{registry.path}`",
        "",
        "| ID | Status | Owner | Scope | Title | Evidence | Follow-up |",
        "|---|---|---|---|---|---|---|",
    ]
    for req in rows:
        out.append(
            "| "
            + " | ".join(
                [
                    markdown_cell(req.id),
                    markdown_cell(req.status),
                    markdown_cell(f"`{req.owner}`"),
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


def render_views(registry: Registry) -> dict[str, str]:
    rendered: dict[str, str] = {}
    for view in registry.views:
        if view.kind == "full_inventory":
            rendered[view.path] = render_full_inventory(registry, view)
    return rendered


def validate_generated(root: Path, rendered: dict[str, str]) -> list[Issue]:
    issues: list[Issue] = []
    for relpath, expected in sorted(rendered.items()):
        path = root / relpath
        try:
            actual = path.read_text(encoding="utf-8")
        except FileNotFoundError:
            issues.append(Issue("MISSING-GENERATED", relpath, "generated view is absent"))
            continue
        except OSError as exc:
            raise EnvironmentError3(f"generated view unreadable ({relpath}): {exc}") from exc
        if actual != expected:
            issues.append(
                Issue(
                    "STALE-GENERATED",
                    relpath,
                    "generated view differs; run `python3 tooling/req-registry.py --write`",
                )
            )
    return issues


def write_generated(root: Path, rendered: dict[str, str]) -> None:
    for relpath, text in sorted(rendered.items()):
        path = root / relpath
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")


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
            "views": [asdict(view) for view in registry.views],
            "requirements": [asdict(req) for req in registry.requirements],
            "issues": [asdict(issue) for issue in issues],
        },
        indent=2,
        sort_keys=True,
    )


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", default=".", help="repo root to scan")
    parser.add_argument("--registry", default=REGISTRY_RELPATH, help="registry path")
    parser.add_argument("--check", action="store_true", help="fail if generated views are stale")
    parser.add_argument("--write", action="store_true", help="rewrite generated views")
    parser.add_argument("--inventory", action="store_true", help="print normalized inventory")
    parser.add_argument("--json", action="store_true", help="emit JSON")
    args = parser.parse_args(argv)

    root = Path(args.root).resolve()
    try:
        registry = load_registry(root, args.registry)
        issues = validate_registry(root, registry)
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
