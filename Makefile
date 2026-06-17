# Thermite — convenience targets. The build/test system is Cargo; these are
# thin entry points. `make audit` is the headline: a FULL TRUST-CHAIN
# re-derivation a skeptic runs on their own machine (see scripts/audit.sh).
.PHONY: audit audit-fast check test fmt clippy gauntlet doc-drift doc-drift-ci doc-drift-worktree doc-drift-test req-status req-status-test req-registry req-registry-test

DOC_DRIFT_CI_BASE ?= origin/main
DOC_DRIFT_CI_HEAD ?= HEAD

# Re-derive the WHOLE trust chain on the skeptic's machine (SLOW — minutes):
#   1  the universal faithfulness theorem re-verified by the local Lean kernel
#      (`lake build` from source + `#print axioms` parsed for sorryAx/custom axioms);
#   2  full-corpus translation-validation (every admitted .th — zero Divergent);
#   3  the multi-class falsification battery (the teeth suites Z3 must CATCH) + a
#      visible end-to-end mutant;
#   4  the Rust<->Lean correspondence drift tripwire (pinned SHAs vs current);
#   5  the emitted proof re-verified under third-party Verus (forge excluded);
#   6  the verdict + the honest residual-trust statement.
# Each guarantee-bearing check SKIPs loudly (stating the consequence) when its tool
# is absent, and a SKIP degrades the verdict. Requires elan/lake (check 1) and the
# Verus/Z3 prover (checks 2/3/5: set VERUS_BIN, put `verus` on PATH, or ~/.local/bin/verus).
audit:
	@bash scripts/audit.sh

# The fast existence demo (the legacy A/B/D shape on one program): faithful program
# certifies L3, the SAME program with an injected bug is REFUSED, and the emitted
# proof re-verifies under third-party Verus with forge excluded. Requires Verus/Z3.
audit-fast:
	@bash scripts/audit.sh --fast

# The full local gauntlet (mirrors CI).
gauntlet:
	cargo build --workspace
	cargo test --workspace
	cargo clippy --workspace --all-targets -- -D warnings
	cargo fmt --all --check
	python3 tooling/req-status.py
	tooling/reqs check

check:
	cargo build --workspace

test:
	cargo test --workspace

fmt:
	cargo fmt --all

clippy:
	cargo clippy --workspace --all-targets -- -D warnings

# Doc-drift tripwire (crosslink #258, .design/tooling/doc-drift-tripwire.md):
# FAIL if any routed design doc's governed file contents differ from the doc's
# `audited-content-sha256:` pin; legacy `audited-sha:` pins fall back to a
# full-history commit-set check. The Python tool's own exit code is the contract
# (0 current / 1 drift-or-bad-pin / 3 environment-inconclusive — REQ-9); run it
# directly or via `make doc-drift-worktree` when a script must branch on 1-vs-3,
# because GNU make collapses any nonzero recipe exit to its own code 2. `make
# doc-drift` mirrors pull-request CI by evaluating a synthetic base-first merge
# commit (`DOC_DRIFT_CI_BASE`, default origin/main, merged with
# `DOC_DRIFT_CI_HEAD`, default HEAD) in a temporary worktree. Deliberately NOT
# part of `make audit` — doc freshness is a development-discipline invariant,
# not a link in the proof-trust chain (decision 5); scripts/audit.sh stays
# byte-identical.
doc-drift: doc-drift-ci

doc-drift-ci:
	@set -eu; \
	base_ref="$(DOC_DRIFT_CI_BASE)"; \
	head_ref="$(DOC_DRIFT_CI_HEAD)"; \
	base_sha="$$(git rev-parse --verify "$$base_ref^{commit}")"; \
	head_sha="$$(git rev-parse --verify "$$head_ref^{commit}")"; \
	printf 'doc-drift: evaluating CI-style merge base=%s head=%s\n' "$$base_sha" "$$head_sha" >&2; \
	if [ "$$base_sha" = "$$head_sha" ]; then \
		merge_sha="$$head_sha"; \
	else \
		if ! tree_sha="$$(git merge-tree --write-tree --no-messages "$$base_sha" "$$head_sha")"; then \
			printf 'doc-drift: could not synthesize CI merge tree for %s and %s\n' "$$base_ref" "$$head_ref" >&2; \
			exit 3; \
		fi; \
		merge_sha="$$(git commit-tree "$$tree_sha" -p "$$base_sha" -p "$$head_sha" -m "doc-drift synthetic CI merge")"; \
	fi; \
	tmp_dir="$$(mktemp -d)"; \
	cleanup() { git worktree remove -f "$$tmp_dir" >/dev/null 2>&1 || rm -rf "$$tmp_dir"; }; \
	trap cleanup EXIT HUP INT TERM; \
	git worktree add --detach --quiet "$$tmp_dir" "$$merge_sha"; \
	python3 "$$tmp_dir/tooling/doc-drift.py" --root "$$tmp_dir"

doc-drift-worktree:
	@python3 tooling/doc-drift.py

# The gate's own oracle fixture suite (hand-authored expected values, R-CHAR-3).
doc-drift-test:
	@python3 -m unittest discover -s tooling/tests -v

# Source-comment REQ-status inventory/contradiction lint. Complements
# doc-drift's audited-sha freshness check by catching semantic contradictions in
# `//! | REQ | SHIPPED/NOT-STARTED | evidence |` rows.
req-status:
	@python3 tooling/req-status.py

req-status-test:
	@python3 -m unittest discover -s tooling/tests -v

# Canonical REQ registry + generated status views. `--check` validates the
# machine-readable registry and fails if checked-in generated views are stale.
req-registry:
	@tooling/reqs check

req-registry-test:
	@python3 -m unittest discover -s tooling/tests -v
