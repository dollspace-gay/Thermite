# Canonical REQ Status Inventory

<!-- generated:reqs view=status -->
Source: `.design/reqs/registry.toml`

| ID | Status | Owner | Contributors | Scope | Title | Evidence | Follow-up |
|---|---|---|---|---|---|---|---|
| REQ-REG-1 | shipped | `.design/tooling/req-registry.md` |  | tooling | Stable requirement identity and ownership | file: `.design/reqs/registry.toml` - schema v1 registry source<br>doc: `.design/tooling/req-registry.md` - governing design contract |  |
| REQ-REG-2 | shipped | `tooling/req-registry.py` |  | tooling | Registry-declared status policy | symbol: `StatusRule` - status policy model<br>test: `tooling/tests/test_req_registry.py::ReqRegistryOracleTest.test_rejects_unknown_status` - requirements may only use declared statuses |  |
| REQ-REG-3 | shipped | `tooling/req-registry.py` |  | tooling | Typed evidence validation | symbol: `VALID_EVIDENCE_KINDS` - accepted evidence kind set<br>test: `tooling/tests/test_req_registry.py::ReqRegistryOracleTest.test_rejects_unresolved_file_evidence` - path evidence must resolve<br>test: `tooling/tests/test_req_registry.py::ReqRegistryOracleTest.test_blocked_requires_issue_blocker` - future status blockers are typed |  |
| REQ-REG-4 | shipped | `tooling/req-registry.py` |  | tooling | Generated status views | symbol: `render_full_inventory` - status view renderer<br>test: `tooling/tests/test_req_registry.py::ReqRegistryOracleTest.test_check_detects_stale_generated_view` - stale generated output is a failing condition<br>command: `python3 tooling/req-registry.py --check` - CI-facing generated-view check |  |
| REQ-REG-5 | partial | `tooling/req-status.py` | `tooling/req-registry.py` | tooling | Legacy source-comment bridge | file: `tooling/req-status.py` - short-term contradiction tripwire<br>command: `python3 tooling/req-status.py` - legacy row lint stays green during migration | Migrate the 429 legacy source-comment rows into stable registry IDs, then replace hand-maintained tables with generated regions or links.<br>blockers: github:dollspace-gay/Thermite#17 |
| REQ-REG-6 | deferred | `.design/tooling/req-registry.md` |  | tooling | Generated-region migration | issue: `github:dollspace-gay/Thermite#17` - RFC tracking the full migration plan | Define generated-region markers and migrate source comments doc-by-doc after stable ID mapping is reviewed.<br>blockers: github:dollspace-gay/Thermite#17 |
<!-- /generated:reqs -->
