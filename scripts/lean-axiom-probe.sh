#!/usr/bin/env bash
# Build the Lean spine and require the listed theorem axioms to stay within
# {propext, Classical.choice, Quot.sound}.
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LEAN_DIR="$ROOT/lean"

if [ -t 1 ]; then G=$'\033[32m'; R=$'\033[31m'; Z=$'\033[0m'; else G=; R=; Z=; fi
pass() { printf '  %sPASS%s %s\n' "$G" "$Z" "$1"; }
fail() { printf '  %sFAIL%s %s\n' "$R" "$Z" "$1"; }

# Build targets include the spine, its negative pins, and the Stage 3
# reconstruction probe.
IMPORTS=(
  "Thermite.Faithfulness"
  "Thermite.Soundness"
  "Thermite.Exec"
  "Thermite.Exec.Stmt"
  "Thermite.Exec.Loop"
  "Thermite.Relax"
  "Thermite.Strat.Denote"
  "Thermite.PinFiniteEscape"
  "Thermite.Strat.SubstKit"
  "Thermite.PinBrokenLift"
  "Thermite.Strat.Fragment"
  "Thermite.Strat.Cls.Wire"
  "Thermite.Strat.Soundness"
  "Thermite.PinStratFlip"
  "Thermite.PinStratCapture"
  "Thermite.Strat.CombDeriv"
  "Thermite.PinCombDeriv"
  "Thermite.Strat.Restratify"
  "Thermite.PinRestratDropSide"
  "Thermite.Strat.Faithfulness"
  "Thermite.PinStratSelfLoop"
  "Thermite.PinNNFPolarity"
  "Thermite.PinRelaxRefute"
  "Thermite.BvModel"
  "Thermite.PinReconstruction"
  "Thermite.Strat.EprReplay"
  "Thermite.Strat.StructuralInstantiation"
  "Thermite.PinSubstitutionCapture"
  "Thermite.PinSkolemDependencies"
  "Thermite.PinGroundingCompleteness"
  "Thermite.PinInstantiationOmission"
  "Thermite.PinStructuralSkolemScopes"
  "Thermite.PinEprReplay"
)

# Universal soundness theorems and the permanent reconstruction probe.
THEOREMS=(
  "Thermite.lowering_faithful"
  "Thermite.ref_sound"
  "Thermite.Exec.exec_ref_sound"
  "Thermite.Exec.body_ref_sound"
  "Thermite.Exec.while_rule"
  "Thermite.Relax.r_relax_sound"
  "Thermite.Relax.rencode_sound"
  "Thermite.Strat.Cls.classifier_correct"
  "Thermite.Strat.strat_ref_sound"
  "Thermite.Strat.Cls.restrat_conservative"
  "Thermite.Strat.strat_lowering_faithful"
  "Thermite.bv_reconstruction_lrat_probe"
  "Thermite.Strat.Cls.source_false_of_verifiedInstantiation"
  "Thermite.Strat.Cls.source_false_of_verifiedStructuralInstantiation"
  "Thermite.Strat.Cls.source_false_of_verifyEprReplay"
  "Thermite.Strat.Cls.checked_implication_of_problem_unsat"
  "Thermite.Strat.Cls.checked_structural_implication_of_problem_unsat"
  "Thermite.checked_source_is_false"
)
ALLOWED="propext Classical.choice Quot.sound"

if ! command -v lake >/dev/null 2>&1; then
  echo "lean-axiom-probe: lake not found on PATH" >&2
  exit 2
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

echo "  building the Lean spine (lake build) ..."
if ! (cd "$LEAN_DIR" && lake build "${IMPORTS[@]}") >"$TMP/build.log" 2>&1; then
  fail "lake build failed"
  tail -15 "$TMP/build.log" | sed 's/^/      /'
  exit 2
fi
pass "lake build succeeded"

PROBE="$TMP/axprobe.lean"
{
  for m in "${IMPORTS[@]}"; do echo "import $m"; done
  for t in "${THEOREMS[@]}"; do echo "#print axioms $t"; done
} >"$PROBE"

AX_OUT="$( (cd "$LEAN_DIR" && lake env lean "$PROBE") 2>&1 )"
AX_RC=$?
if [ "$AX_RC" -ne 0 ]; then
  fail "the axiom probe failed to elaborate (lake env lean exited $AX_RC)"
  echo "$AX_OUT" | tail -10 | sed 's/^/      /'
  exit 2
fi

THM_FAIL=0
for t in "${THEOREMS[@]}"; do
  # Lean may wrap the report, so capture through its closing bracket.
  report="$(
    printf '%s\n' "$AX_OUT" | awk -v target="'$t'" '
      index($0, target) { found = 1 }
      found {
        print
        if (index($0, "does not depend on any axioms") || index($0, "]")) exit
      }
    '
  )"
  if [ -z "$report" ]; then
    fail "$t — no axiom report found"
    THM_FAIL=1
    continue
  fi
  axlist="$(
    printf '%s\n' "$report" |
      tr '\n' ' ' |
      sed -n 's/.*\[\(.*\)\].*/\1/p' |
      tr ',' '\n' |
      sed 's/[[:space:]]//g'
  )"
  bad=""
  while IFS= read -r ax; do
    [ -z "$ax" ] && continue
    case " $ALLOWED " in
      *" $ax "*) : ;;
      *) bad="$bad $ax" ;;
    esac
  done <<<"$axlist"
  if [ -n "$bad" ]; then
    fail "$t — disallowed axiom(s):$bad"
    THM_FAIL=1
  else
    pass "$t — axioms ⊆ {propext, Classical.choice, Quot.sound}"
  fi
done

[ "$THM_FAIL" -eq 0 ] || exit 1
exit 0
