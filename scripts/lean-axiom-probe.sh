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
  # Stage-2 REQ-2 (#324): the SubstKit binder kit + its broken-lift micro-pin, added
  # as BUILD targets so a `sorry` or a broken proof in `Strat/SubstKit.lean` (or the
  # pin) fails the CI Lean job (AC-2). `Strat.SubstKit` transitively pulls
  # `Strat.Denote`; `PinBrokenLift` pulls `Strat.SubstKit`. Both Mathlib-free. The
  # SubstKit's two load-bearing lemmas are axiom-probed IN-FILE (`#print axioms`,
  # per the SPIKE-1 conventions note §4) — NOT added to the THEOREM list below, which
  # is the fixed universal-pillar set and must not be perturbed by stage-2 targets.
  "Thermite.Strat.SubstKit"
  "Thermite.PinBrokenLift"
  # Stage-2 REQ-3 (#325): the classifier kernel half. `Strat.Fragment` transitively
  # pulls `Strat.Nnf`/`Strat.Graph` (the NNF/prenex normaliser + the sort graph), all
  # Mathlib-free, so a `sorry` or broken proof under them fails the CI Lean job
  # (AC-3: "lake build green; zero sorry under lean/Thermite/Strat/"). Unlike the
  # REQ-1 spine, this increment DOES carry a stratified soundness theorem
  # (`classifier_correct`, T3-C, in namespace `Thermite.Strat.Cls`), gated below.
  "Thermite.Strat.Fragment"
  # Stage-2 REQ-4 (#326): the classifier differential battery's Lean entry point
  # (`Thermite.Strat.Cls.Wire` — the wire parser + `main` `lake env lean --run` drives).
  # A BUILD target so (a) a compile break in the wire parser fails the CI Lean job, and
  # (b) its dependency `Thermite.Strat.Fragment` is built, so `forge strat-tv` /
  # `forge/tests/strat_differential.rs` can `lake env lean --run` it in the same job. It
  # carries no theorem (it is an IO tool with a `partial` parser), so it is NOT added to
  # the axiom-gated THEOREM list below.
  "Thermite.Strat.Cls.Wire"
  # Stage-2 REQ-5 (#327): the encoder + T1-S. `Thermite.Strat.Soundness` transitively
  # pulls `Strat.RefEncode`/`Strat.TokDenote` (the `sencode` trigger-free MBQI token
  # surface + `tokDenote`), all Mathlib-free, so a `sorry` or broken proof under them
  # fails the CI Lean job (AC-5). `strat_ref_sound` (T1-S) is added to the axiom-gated
  # THEOREMS list below (the REQ-9 [1′] extension brought forward, since AC-5 requires
  # the stratified encoder soundness be axiom-clean). The two broken-encoder pins are
  # BUILD targets (decide-checked, no theorem in the gated list).
  "Thermite.Strat.Soundness"
  "Thermite.PinStratFlip"
  "Thermite.PinStratCapture"
  # Stage-2 REQ-6 (#328): combinator demotion. `Thermite.Strat.CombDeriv` carries
  # the eight `comb_deriv_*` lemmas (the six bounded combinators' raw-quantifier
  # expansions over the structural `fdenote`, plus the two SPIKE-2 census
  # combinators' definitional aggregate forms), all Mathlib-free, so a `sorry` or
  # broken proof fails the CI Lean job (AC-6). The eight lemmas are axiom-probed
  # IN-FILE (`#print axioms`, the same convention REQ-2's SubstKit uses) — NOT
  # added to the fixed universal-pillar THEOREMS list below. `PinCombDeriv` is the
  # off-by-one neighbour pin (decide-checked, no theorem in the gated list).
  "Thermite.Strat.CombDeriv"
  "Thermite.PinCombDeriv"
  # Stage-2 REQ-7 (#329): restratify. `Thermite.Strat.Restratify` carries the rewrite
  # `restrat` + the side obligation `Side` + the four T4-R theorems
  # (`restrat_conservative`/`restrat_admits`/`restrat_complete`/`side_admitted`), all
  # Mathlib-free (it imports only `Strat.Fragment`), so a `sorry` or broken proof fails
  # the CI Lean job (AC-7). `restrat_conservative` (T4-R, the R-SIDE-1 certificate
  # bridge) is added to the axiom-gated THEOREMS list below (the REQ-9 [1′] extension
  # brought forward for AC-7, as REQ-5 did for `strat_ref_sound`); the three other T4-R
  # theorems (`decide`-checked admissibility / witness-oracle completeness) are BUILD
  # targets, axiom-probed IN-FILE. `PinRestratDropSide` is the drop-Side mis-certification
  # pin (decide-checked, no theorem in the gated list).
  "Thermite.Strat.Restratify"
  "Thermite.PinRestratDropSide"
  # Stage-2 REQ-8 (#330): faithfulness + the atom-grounding. `Thermite.Strat.Faithfulness`
  # carries T2-S (`strat_lowering_faithful`) + the `SFnTvWitness` grounding (it imports
  # `Strat.Soundness` + the v1 `Thermite.Denote` seam; deliberately NOT the spine
  # `Strat.Denote`, which would re-introduce the #68 two-syntax probe collision the `.Cls`
  # split fixed). A `sorry` or broken proof fails the CI Lean job (AC-8). `strat_lowering_faithful`
  # (T2-S, the source-meaning grounding that lifts cage L4 above structural-only) is added to
  # the axiom-gated THEOREMS list below (the REQ-9 [1′] extension brought forward for AC-8, as
  # REQ-5 did for `strat_ref_sound`); the `qfree_iff` corollary is axiom-probed IN-FILE.
  "Thermite.Strat.Faithfulness"
  # Stage-2 REQ-10 (#332): the pin battery, complete (AC-10, gate G2). The three pins
  # that did not land in a prior increment, added as BUILD targets so a `sorry` or a
  # broken `decide`/proof in any of them fails the CI Lean job. `PinStratSelfLoop` and
  # `PinNNFPolarity` pull `Strat.Fragment` (the classifier kernel — they refute graph
  # neighbours of `admitted`/`classifier_correct`); `PinRelaxRefute` pulls `Relax` (the
  # Mathlib island — it refutes the converse of `r_relax_sound`). All decide/kernel-checked
  # with no theorem added to the gated THEOREMS list below (they are negative pins, like the
  # five pins already present: PinFiniteEscape, PinStratFlip/Capture, PinCombDeriv,
  # PinRestratDropSide). The eight-pin battery is cited theorem-by-theorem in
  # `.design/verified/strat-rust-lean-correspondence.md` ("The stage-2 pin battery").
  "Thermite.PinStratSelfLoop"
  "Thermite.PinNNFPolarity"
  "Thermite.PinRelaxRefute"
)
# The five load-bearing universal-theorem pillars + the two relax-route spine lemmas
# (REQ-8a) + the stage-2 classifier coincidence theorem T3-C (REQ-3, AC-3) + the
# stage-2 stratified encoder soundness T1-S (REQ-5, AC-5) + the stage-2 restratification
# conservativity T4-R (REQ-7, AC-7) + the stage-2 lowering faithfulness / atom-grounding
# T2-S (REQ-8, AC-8). Keep this list in lock-step with the prose in
# `scripts/audit.sh` check [1].
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
