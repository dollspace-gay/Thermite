#!/usr/bin/env bash
# Gate G3: parse both release configurations, exercise the real BV route, and
# replay the resulting validity theorems in Lean. Missing tools are failures.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

for tool in cargo lake verus z3; do
  command -v "$tool" >/dev/null 2>&1 || {
    echo "g3-gate: required tool not found: $tool" >&2
    exit 2
  }
done

echo "[G3 1/5] release parser without bv plumbing"
cargo test -p thermite-syntax --release --test bv_tag_parse

echo "[G3 2/5] release parser with bv plumbing"
cargo test -p thermite-syntax --release --features bv --test bv_tag_parse

echo "[G3 3/5] fixed-width lowering and invariant preservation"
cargo test -p thermite-lower tagged_ -- --nocapture
cargo test -p forge --features bv --test bv_invariants -- --nocapture

echo "[G3 4/5] Lean spine and reconstruction axiom probe"
bash scripts/lean-axiom-probe.sh

echo "[G3 5/5] checked validity replay and the live BV route"
cargo test -p forge --features bv --bin forge lean_smt_export::tests -- --nocapture
cargo test -p forge --features bv --bin forge \
  check::tests::req8_arithmetic_and_bitwise_clauses_migrate_to_kernel_checked \
  -- --nocapture
cargo test -p forge --features bv --test bv_lowering -- --nocapture

echo "G3 gate passed"
