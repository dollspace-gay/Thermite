#!/usr/bin/env bash
# Thermite L3 audit — an independent, reproducible check that the "L3" claim is
# REAL, runnable by a skeptic who trusts neither the agent nor the "L3" label.
#
# It runs three checks, building the toolchain from source first:
#   (A) a faithful program certifies L3;
#   (B) the SAME program with an injected bug is REFUSED (the prover has teeth —
#       a rubber stamp would still say L3 here);
#   (D) the emitted proof re-verifies under THIRD-PARTY Verus/Z3, with `forge`
#       removed from the loop entirely.
#
# Exit 0  => the L3 machinery certifies the faithful program, refuses the buggy
#            one, and the proof reproduces independently. Trust then reduces to
#            the named set {Z3/Verus soundness, the Thermite->Verus lowering}.
# Exit !=0 => a check failed (which is itself the finding — see the FAIL lines).
#
# Usage:  make audit            (default: binary_search)
#         bash scripts/audit.sh [PROGRAM.th] [EMITTED_PROOF.verus.rs]
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

PROG="${1:-conformance/binary_search.th}"
GOLDEN="${2:-tests/golden/lower/binary_search.verus.rs}"
ITEM="$(basename "$PROG" .th)"

if [ -t 1 ]; then B=$'\033[1m'; G=$'\033[32m'; R=$'\033[31m'; Z=$'\033[0m'; else B=; G=; R=; Z=; fi
bold() { printf '%s%s%s\n' "$B" "$1" "$Z"; }
pass() { printf '  %sPASS%s %s\n' "$G" "$Z" "$1"; }
fail() { printf '  %sFAIL%s %s\n' "$R" "$Z" "$1"; }

# --- locate verus (REQUIRED: both L3 and the re-check need the prover) ---
find_verus() {
  if [ -n "${VERUS_BIN:-}" ] && [ -x "${VERUS_BIN}" ]; then printf '%s' "$VERUS_BIN"; return 0; fi
  if command -v verus >/dev/null 2>&1; then command -v verus; return 0; fi
  if [ -x "$HOME/.local/bin/verus" ]; then printf '%s' "$HOME/.local/bin/verus"; return 0; fi
  return 1
}
VERUS="$(find_verus || true)"
if [ -z "${VERUS:-}" ]; then
  bold "Thermite L3 audit"
  fail "verus not found — set VERUS_BIN, put 'verus' on PATH, or install to ~/.local/bin/verus."
  echo  "       The L3 proof AND the independent re-check both require the Verus/Z3 prover."
  exit 2
fi

bold "Thermite L3 audit — independent + reproducible"
echo "  program : $PROG"
echo "  prover  : $VERUS ($("$VERUS" --version 2>/dev/null | head -1))"
echo

echo "  building forge from source (audit the tool you can read) ..."
if ! cargo build -q -p forge; then fail "forge build failed"; exit 2; fi
FORGE="$ROOT/target/debug/forge"
echo

RC=0
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# ===== (A) faithful program certifies L3 =====
bold "(A) the faithful $ITEM should certify L3"
A_OUT="$("$FORGE" check "$PROG" 2>&1)"
echo "$A_OUT" | grep -iE "item:|level:|assurance" | sed 's/^/      /'
if echo "$A_OUT" | grep -qiE "level:[[:space:]]*L3"; then
  pass "$ITEM certified L3 (proven for all inputs)"
else
  fail "$ITEM did NOT certify L3 (expected L3) — see the output above"; RC=1
fi
echo

# ===== (B) the same program, with a WRONG return value, must be REFUSED =====
bold "(B) the SAME program with an injected bug must be REFUSED"
BUG="$TMP/${ITEM}_bug.th"
sed 's/return Some(mid);/return Some(mid + 1);/' "$PROG" > "$BUG"
if diff -q "$PROG" "$BUG" >/dev/null; then
  fail "could not inject a bug (the 'return Some(mid);' pattern is not in $PROG);"
  echo  "       run the audit on the default binary_search, or adapt the mutation for your program."
  RC=1
else
  echo "      injected: $(grep -n 'Some(mid + 1)' "$BUG" | head -1 | sed 's/^[0-9]*:[[:space:]]*//')"
  B_OUT="$("$FORGE" check "$BUG" 2>&1)"
  echo "$B_OUT" | grep -iE "item:|level:|FAIL|postcondition|assurance" | sed 's/^/      /'
  if echo "$B_OUT" | grep -qiE "level:[[:space:]]*L3"; then
    fail "the BUGGY program STILL certified L3 — the prover did not catch the bug!"; RC=1
  else
    pass "the buggy program was REFUSED (not L3) — the prover has teeth"
  fi
fi
echo

# ===== (D) third-party Verus re-check, forge excluded =====
bold "(D) the emitted proof must re-verify under THIRD-PARTY Verus (forge NOT involved)"
if [ ! -f "$GOLDEN" ]; then
  fail "emitted proof file not found: $GOLDEN"; RC=1
else
  # verus rejects a '.' in the crate name derived from the filename -> dot-free copy
  COPY="$TMP/${ITEM}_golden.rs"
  cp "$GOLDEN" "$COPY"
  echo "      proof file : $GOLDEN  (Thermite's emitted Verus, committed)"
  D_OUT="$("$VERUS" "$COPY" 2>&1)"; D_RC=$?
  echo "$D_OUT" | grep -iE "verification results|verified|errors" | sed 's/^/      /'
  if [ "$D_RC" -eq 0 ] && echo "$D_OUT" | grep -qiE "0 errors"; then
    pass "third-party Verus re-verified the proof (0 errors) — forge excluded"
  else
    fail "third-party Verus did NOT verify the emitted proof"; RC=1
  fi
fi
echo

bold "VERDICT"
if [ "$RC" -eq 0 ]; then
  printf '  %sAUDIT PASSED%s — L3 certifies the faithful program, REFUSES the buggy one,\n' "$G" "$Z"
  echo  "  and the proof reproduces under independent Verus. The only remaining trust is the"
  echo  "  named set { Z3/Verus soundness, the Thermite->Verus lowering } — nothing else, and"
  echo  "  not the agent or the label."
else
  printf '  %sAUDIT FAILED%s — one or more checks did not hold (see the FAIL lines above).\n' "$R" "$Z"
fi
exit "$RC"
