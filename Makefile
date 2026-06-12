# Thermite — convenience targets. The build/test system is Cargo; these are
# thin entry points. `make audit` is the headline: a FULL TRUST-CHAIN
# re-derivation a skeptic runs on their own machine (see scripts/audit.sh).
.PHONY: audit audit-fast check test fmt clippy gauntlet doc-drift doc-drift-test

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

check:
	cargo build --workspace

test:
	cargo test --workspace

fmt:
	cargo fmt --all

clippy:
	cargo clippy --workspace --all-targets -- -D warnings

# Doc-drift tripwire (crosslink #258, .design/tooling/doc-drift-tripwire.md):
# FAIL if any routed design doc's governed files were committed after the doc's
# `audited-sha:` pin. The tool's own exit code is the contract (0 current /
# 1 drift-or-bad-pin / 3 environment-inconclusive — REQ-9); run the tool
# directly when a script must branch on 1-vs-3, because GNU make collapses any
# nonzero recipe exit to its own code 2 (it never re-emits 1 or 3). `make
# doc-drift` thus signals 0 = clean / nonzero = needs attention, with the
# precise class in the tool's printed report. Deliberately NOT part of
# `make audit` — doc freshness is a development-discipline invariant, not a link
# in the proof-trust chain (decision 5); scripts/audit.sh stays byte-identical.
doc-drift:
	@python3 tooling/doc-drift.py

# The gate's own oracle fixture suite (hand-authored expected values, R-CHAR-3).
doc-drift-test:
	@python3 -m unittest discover -s tooling/tests -v
