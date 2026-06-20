#!/usr/bin/env bash
# Thermite Lean axiom probe — the SINGLE SOURCE OF TRUTH for check [1] of the trust
# chain: `lake build` the Lean proof spine, then `#print axioms` the load-bearing
# theorems PLUS the relax-route spine lemmas and PARSE each axiom list. PASS iff every
# list is a subset of {propext, Classical.choice, Quot.sound} — no sorryAx, no custom
# axiom (which would mean the proof is not kernel-clean).
#
# Used by BOTH `scripts/audit.sh` (check [1] of the local deep audit) and the `lean` CI
# job (`.github/workflows/ci.yml`), so the probed-theorem set cannot drift between the
# local audit and CI (trust-audit finding F4: the Lean spine + axiom probe must run in
# CI, not only in a local `make audit`).
#
# It builds exactly the modules the probe imports (NOT the `Smt`-importing `SmtDemo`
# Z3-demotion PoC, which is not part of the proof spine), so it stays runnable in CI
# without the vendored-cvc5 FFI build. Assumes `lake` is on PATH (callers locate elan).
#
# Exit: 0 = every probed theorem axiom-clean / 1 = a disallowed axiom or a missing
# theorem / 2 = build or elaboration failure (an environment problem, not a proof defect).
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LEAN_DIR="$ROOT/lean"

if [ -t 1 ]; then G=$'\033[32m'; R=$'\033[31m'; Z=$'\033[0m'; else G=; R=; Z=; fi
pass() { printf '  %sPASS%s %s\n' "$G" "$Z" "$1"; }
fail() { printf '  %sFAIL%s %s\n' "$R" "$Z" "$1"; }

# Modules imported by the probe so every theorem below resolves. These are also the
# build targets — building them compiles the whole core spine (Faithfulness transitively
# pulls Ast/Denote/RefEncode/Soundness/Exec) plus the Mathlib-island Relax module, while
# leaving the Smt-importing SmtDemo experiment out of the build.
IMPORTS=(
  "Thermite.Faithfulness"
  "Thermite.Soundness"
  "Thermite.Exec"
  "Thermite.Exec.Stmt"
  "Thermite.Exec.Loop"
  "Thermite.Relax"
  # Stage-2 REQ-1 (#323): the Strat spine foundation, added as BUILD targets so a
  # `sorry` or a broken proof under `lean/Thermite/Strat/` (or `PinFiniteEscape`)
  # fails the CI Lean job (AC-1: "lake build green with Strat/Syntax,Carrier,Denote
  # + PinFiniteEscape"). `Strat.Denote` transitively pulls `Strat.Syntax`/
  # `Strat.Carrier`; both are Mathlib-free. These are BUILD targets only — the
  # axiom-gated THEOREMS list below is extended to the stratified soundness
  # theorems by REQ-9 ([1′]), not here (there is no Strat soundness theorem yet).
  "Thermite.Strat.Denote"
  "Thermite.PinFiniteEscape"
  # Stage-2 REQ-3 (#325): the classifier kernel half. `Strat.Fragment` transitively
  # pulls `Strat.Nnf`/`Strat.Graph` (the NNF/prenex normaliser + the sort graph), all
  # Mathlib-free, so a `sorry` or broken proof under them fails the CI Lean job
  # (AC-3: "lake build green; zero sorry under lean/Thermite/Strat/"). Unlike the
  # REQ-1 spine, this increment DOES carry a stratified soundness theorem
  # (`classifier_correct`, T3-C), so its axiom list is gated below (AC-3 "axiom-clean").
  "Thermite.Strat.Fragment"
)
# The five load-bearing universal-theorem pillars + the two relax-route spine lemmas
# (REQ-8a) + the stage-2 classifier coincidence theorem T3-C (REQ-3, AC-3). Keep this
# list in lock-step with the prose in `scripts/audit.sh` check [1].
THEOREMS=(
  "Thermite.lowering_faithful"
  "Thermite.ref_sound"
  "Thermite.Exec.exec_ref_sound"
  "Thermite.Exec.body_ref_sound"
  "Thermite.Exec.while_rule"
  "Thermite.Relax.r_relax_sound"
  "Thermite.Relax.rencode_sound"
  "Thermite.Strat.classifier_correct"
)
ALLOWED="propext Classical.choice Quot.sound"

if ! command -v lake >/dev/null 2>&1; then
  echo "lean-axiom-probe: lake not found on PATH" >&2
  exit 2
fi

TMP="$(mktemp -d)"; trap 'rm -rf "$TMP"' EXIT

echo "  building the Lean spine (lake build) ..."
if ! ( cd "$LEAN_DIR" && lake build "${IMPORTS[@]}" ) >"$TMP/build.log" 2>&1; then
  fail "lake build FAILED — the Lean spine did not compile on this toolchain"
  tail -15 "$TMP/build.log" | sed 's/^/      /'
  exit 2
fi
pass "lake build succeeded (the Lean spine compiled from source on your toolchain)"

PROBE="$TMP/axprobe.lean"
{
  for m in "${IMPORTS[@]}"; do echo "import $m"; done
  for t in "${THEOREMS[@]}"; do echo "#print axioms $t"; done
} > "$PROBE"

AX_OUT="$( ( cd "$LEAN_DIR" && lake env lean "$PROBE" ) 2>&1 )"; AX_RC=$?
if [ "$AX_RC" -ne 0 ]; then
  fail "the axiom probe failed to elaborate (lake env lean exited $AX_RC)"
  echo "$AX_OUT" | tail -10 | sed 's/^/      /'
  exit 2
fi

THM_FAIL=0
for t in "${THEOREMS[@]}"; do
  # `#print axioms` prints either "'t' depends on axioms: [..]" or "'t' does not depend
  # on any axioms" (no brackets ⇒ empty list ⇒ trivially clean).
  line="$(echo "$AX_OUT" | grep -F "'$t'")"
  if [ -z "$line" ]; then
    fail "$t — no axiom line emitted (theorem missing or renamed?)"; THM_FAIL=1; continue
  fi
  axlist="$(printf '%s' "$line" | sed -n 's/.*\[\(.*\)\].*/\1/p' | tr ',' '\n' | sed 's/[[:space:]]//g')"
  bad=""
  while IFS= read -r ax; do
    [ -z "$ax" ] && continue
    case " $ALLOWED " in
      *" $ax "*) : ;;
      *) bad="$bad $ax" ;;
    esac
  done <<< "$axlist"
  if [ -n "$bad" ]; then
    fail "$t — DISALLOWED axiom(s):$bad  (sorryAx or a custom axiom = the proof is NOT kernel-clean)"
    THM_FAIL=1
  else
    pass "$t — axioms ⊆ {propext, Classical.choice, Quot.sound}"
  fi
done

[ "$THM_FAIL" -eq 0 ] || exit 1
exit 0
