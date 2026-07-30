#!/usr/bin/env bash
# Gate G4: canonical S₂.0 bridge, finite EPR replay, and production defaults.
# Missing proof or solver tooling is a hard failure.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

for tool in cargo lake python3; do
  command -v "$tool" >/dev/null 2>&1 || {
    echo "g4-gate: required tool not found: $tool" >&2
    exit 2
  }
done

# The SAT solver and LRAT converter are built from the exact revisions in
# scripts/g4-toolchain.env. The gate never falls back to a system package.
# shellcheck source=g4-toolchain.env
source "$ROOT/scripts/g4-toolchain.env"
export THERMITE_EPR_CADICAL="$ROOT/target/g4-tools/bin/cadical"
export THERMITE_EPR_DRAT_TRIM="$ROOT/target/g4-tools/bin/drat-trim"

[[ -x "$THERMITE_EPR_CADICAL" ]] || {
  echo "g4-gate: pinned CaDiCaL is missing; run scripts/install-g4-tools.sh" >&2
  exit 2
}
[[ -x "$THERMITE_EPR_DRAT_TRIM" ]] || {
  echo "g4-gate: pinned drat-trim is missing; run scripts/install-g4-tools.sh" >&2
  exit 2
}
[[ "$("$THERMITE_EPR_CADICAL" --version)" == "$CADICAL_VERSION" ]] || {
  echo "g4-gate: CaDiCaL version does not match the Stage 4 pin" >&2
  exit 2
}
[[ "$("$THERMITE_EPR_DRAT_TRIM" --thermite-version)" == \
   "drat-trim $DRAT_TRIM_REV" ]] || {
  echo "g4-gate: drat-trim revision does not match the Stage 4 pin" >&2
  exit 2
}

echo "[G4 1/5] canonical bridge and classifier differential"
cargo test -p thermite-spec -p thermite-tv --no-fail-fast

echo "[G4 2/5] Lean normalization, Skolemization, grounding, and replay pins"
(
  cd lean
  lake build \
    Thermite.PinSubstitutionCapture \
    Thermite.PinSkolemDependencies \
    Thermite.PinGroundingCompleteness \
    Thermite.PinInstantiationOmission \
    Thermite.PinEprReplay
)

echo "[G4 3/5] axiom footprint"
bash scripts/lean-axiom-probe.sh

echo "[G4 4/5] release defaults and automatic BV routing"
cargo build -p forge --release
cargo test -p forge --bin forge check::tests -- --nocapture

echo "[G4 5/5] no proof placeholders or custom axioms"
python3 - lean/Thermite <<'PY'
from pathlib import Path
import re
import sys


def code_without_comments_or_strings(source: str) -> str:
    out = []
    i = 0
    block_depth = 0
    in_line = False
    in_string = False
    while i < len(source):
        pair = source[i : i + 2]
        char = source[i]
        if in_line:
            if char == "\n":
                in_line = False
                out.append(char)
            else:
                out.append(" ")
            i += 1
            continue
        if block_depth:
            if pair == "/-":
                block_depth += 1
                out.extend("  ")
                i += 2
            elif pair == "-/":
                block_depth -= 1
                out.extend("  ")
                i += 2
            else:
                out.append("\n" if char == "\n" else " ")
                i += 1
            continue
        if in_string:
            if char == "\\" and i + 1 < len(source):
                out.extend("  ")
                i += 2
            else:
                if char == '"':
                    in_string = False
                out.append("\n" if char == "\n" else " ")
                i += 1
            continue
        if pair == "--":
            in_line = True
            out.extend("  ")
            i += 2
        elif pair == "/-":
            block_depth = 1
            out.extend("  ")
            i += 2
        elif char == '"':
            in_string = True
            out.append(" ")
            i += 1
        else:
            out.append(char)
            i += 1
    return "".join(out)


forbidden = re.compile(r"\b(?:sorry|admit|native_decide)\b|^\s*axiom\b", re.MULTILINE)
failures = []
for path in sorted(Path(sys.argv[1]).rglob("*.lean")):
    code = code_without_comments_or_strings(path.read_text())
    for match in forbidden.finditer(code):
        line = code.count("\n", 0, match.start()) + 1
        token = match.group(0).strip()
        failures.append(f"{path}:{line}: forbidden `{token}`")

if failures:
    print("\n".join(failures), file=sys.stderr)
    raise SystemExit(1)
PY

echo "G4 gate passed"
