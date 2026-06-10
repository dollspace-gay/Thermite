# Thermite — convenience targets. The build/test system is Cargo; these are
# thin entry points. `make audit` is the headline: a FULL TRUST-CHAIN
# re-derivation a skeptic runs on their own machine (see scripts/audit.sh).
.PHONY: audit audit-fast check test fmt clippy gauntlet

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
