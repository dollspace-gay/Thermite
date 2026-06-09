# Thermite — convenience targets. The build/test system is Cargo; these are
# thin entry points. `make audit` is the headline: an independent, reproducible
# check that the "L3" verification claim is real (see scripts/audit.sh).
.PHONY: audit check test fmt clippy gauntlet

# Independently audit the L3 claim: faithful program certifies L3, the SAME
# program with an injected bug is REFUSED, and the emitted proof re-verifies
# under third-party Verus with forge excluded. Requires the Verus/Z3 prover
# (set VERUS_BIN, or put `verus` on PATH, or install to ~/.local/bin/verus).
audit:
	@bash scripts/audit.sh

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
