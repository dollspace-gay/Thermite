#!/usr/bin/env bash
# Thermite audit — the skeptic re-derives the ENTIRE trust chain on their own
# machine. The shallow "L3" demo proves an existence claim (one program certifies,
# one mutant is refused). The DEEP audit (default) re-derives each LINK of the
# ACTUAL guarantee and prints, honestly, what it could NOT discharge:
#
#   1  THE UNIVERSAL THEOREM, re-checked locally  — `lake build` the Lean spine
#      from source, then `#print axioms` the twelve gated theorems and PARSE the
#      axiom lists: PASS iff every list ⊆ {propext, Classical.choice, Quot.sound}
#      (no sorryAx, no custom axiom). This is the ∀-programs faithfulness theorem
#      re-verified by YOUR Lean kernel — trust does NOT include our claim of having
#      proven it. (Requires elan/lake; SKIPs-with-consequence if absent.)
#   2  FULL-CORPUS cross-validation                — `forge tv`/`exec-tv`/`body-tv`
#      over EVERY admitted `.th` in conformance/. PASS iff ZERO Divergent across the
#      corpus (Skipped/Unverifiable counted + printed, not failing).
#   3  THE FALSIFICATION BATTERY (multi-class)     — the live teeth suites that
#      inject production-side infidelities and assert Z3 CATCHES them (thermite-tv
#      teeth/body_teeth/exec_teeth/loop_teeth), PLUS one visible end-to-end sed
#      mutant (the legible illustration; the battery is the evidence).
#   4  CORRESPONDENCE DRIFT TRIPWIRE               — the pinned encoder/Lean SHAs in
#      .design/verified/rust-lean-correspondence.md vs each file's CURRENT last-touch.
#      MISMATCH => the inspection-tier audit predates the current code => FAIL.
#   5  THIRD-PARTY PROVER RE-CHECK                 — the emitted golden proof
#      re-verifies under Verus/Z3 with `forge` excluded (the legacy (D) check).
#   G2 STAGE-2 STRATIFIED CAGE — THE G2 GATE             — the four stage-2 checks
#      [1′] (axiom probe extended to the four stratified soundness theorems) / [4′]
#      (doc-drift over the three mirrored Rust files) / [8] (the classifier differential
#      battery) / [9] (the two-phase TV sweep), combined by `forge g2-gate`: the tested
#      code path that mechanically WITHHOLDS the stratified certificate trust flip unless
#      all four are green in THIS run (REQ-9 / AC-9).
#   6  THE VERDICT + THE RESIDUAL-TRUST STATEMENT  — what you are STILL trusting,
#      everything else having been re-derived here just now.
#
# A guarantee-bearing check that SKIPs (its tool is absent) makes the verdict say so.
#
# `make audit`       runs the deep audit (SLOW — minutes: Lean build + corpus TV + teeth).
# `make audit-fast`  runs the legacy A/B/D existence demo (the old shape) on one program.
#
# Usage:  bash scripts/audit.sh [--fast] [PROGRAM.th] [EMITTED_PROOF.verus.rs]
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

FAST=0
ARGS=()
for a in "$@"; do
  case "$a" in
    --fast) FAST=1 ;;
    *) ARGS+=("$a") ;;
  esac
done

PROG="${ARGS[0]:-conformance/binary_search.th}"
GOLDEN="${ARGS[1]:-tests/golden/lower/binary_search.verus.rs}"
ITEM="$(basename "$PROG" .th)"

if [ -t 1 ]; then B=$'\033[1m'; G=$'\033[32m'; R=$'\033[31m'; Y=$'\033[33m'; Z=$'\033[0m'; else B=; G=; R=; Y=; Z=; fi
bold() { printf '%s%s%s\n' "$B" "$1" "$Z"; }
pass() { printf '  %sPASS%s %s\n' "$G" "$Z" "$1"; }
fail() { printf '  %sFAIL%s %s\n' "$R" "$Z" "$1"; }
skip() { printf '  %sSKIP%s %s\n' "$Y" "$Z" "$1"; }
note() { printf '       %s\n' "$1"; }

# --- locate verus (REQUIRED for the prover-bearing checks) ---
find_verus() {
  if [ -n "${VERUS_BIN:-}" ] && [ -x "${VERUS_BIN}" ]; then printf '%s' "$VERUS_BIN"; return 0; fi
  if command -v verus >/dev/null 2>&1; then command -v verus; return 0; fi
  if [ -x "$HOME/.local/bin/verus" ]; then printf '%s' "$HOME/.local/bin/verus"; return 0; fi
  return 1
}
VERUS="$(find_verus || true)"

RC=0
# A SKIP of a guarantee-bearing check is a degraded verdict, not a pass.
SKIPPED_GUARANTEES=()
# Stage-2 G2-gate sub-check verdicts (green|red|skip) — set by the deep-path checks below
# and combined by the [G2] gate section (REQ-9 / AC-9).
S2_AXIOM=skip   # [1′] the stratified axiom probe (shares check [1]'s Lean build)
S2_DRIFT=skip   # [4′] the stratified doc-drift tripwire
S2_DIFF=skip    # [8]  the classifier differential battery
S2_TVSWEEP=skip # [9]  the stratified two-phase TV sweep
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# =============================================================================
#  FAST PATH — the legacy A/B/D existence demo (one program, one mutant).
# =============================================================================
if [ "$FAST" -eq 1 ]; then
  if [ -z "${VERUS:-}" ]; then
    bold "Thermite audit (fast)"
    fail "verus not found — set VERUS_BIN, put 'verus' on PATH, or install to ~/.local/bin/verus."
    note "The L3 proof AND the independent re-check both require the Verus/Z3 prover."
    exit 2
  fi
  bold "Thermite audit (fast) — the existence demo (one program, one mutant)"
  echo "  program : $PROG"
  echo "  prover  : $VERUS ($("$VERUS" --version 2>/dev/null | head -1))"
  echo
  echo "  building forge from source (audit the tool you can read) ..."
  if ! cargo build -q -p forge; then fail "forge build failed"; exit 2; fi
  FORGE="$ROOT/target/debug/forge"
  echo

  bold "(A) the faithful $ITEM should certify L3"
  A_OUT="$("$FORGE" check "$PROG" 2>&1)"
  echo "$A_OUT" | grep -iE "item:|level:|assurance" | sed 's/^/      /'
  if echo "$A_OUT" | grep -qiE "level:[[:space:]]*L3"; then
    pass "$ITEM certified L3 (proven for all inputs)"
  else
    fail "$ITEM did NOT certify L3 (expected L3) — see the output above"; RC=1
  fi
  echo

  bold "(B) the SAME program with an injected bug must be REFUSED"
  BUG="$TMP/${ITEM}_bug.th"
  sed 's/return Some(mid);/return Some(mid + 1);/' "$PROG" > "$BUG"
  if diff -q "$PROG" "$BUG" >/dev/null; then
    fail "could not inject a bug (the 'return Some(mid);' pattern is not in $PROG);"
    note "run the audit on the default binary_search, or adapt the mutation for your program."
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

  bold "(D) the emitted proof must re-verify under THIRD-PARTY Verus (forge NOT involved)"
  if [ ! -f "$GOLDEN" ]; then
    fail "emitted proof file not found: $GOLDEN"; RC=1
  else
    COPY="$TMP/${ITEM}_golden.rs"
    cp "$GOLDEN" "$COPY"
    echo "      proof file : $GOLDEN  (Thermite's emitted Verus, committed)"
    # run from $TMP so verus's output artifact lands in scratch, never the repo tree
    D_OUT="$( ( cd "$TMP" && "$VERUS" "$(basename "$COPY")" ) 2>&1 )"; D_RC=$?
    echo "$D_OUT" | grep -iE "verification results|verified|errors" | sed 's/^/      /'
    if [ "$D_RC" -eq 0 ] && echo "$D_OUT" | grep -qiE "0 errors"; then
      pass "third-party Verus re-verified the proof (0 errors) — forge excluded"
    else
      fail "third-party Verus did NOT verify the emitted proof"; RC=1
    fi
  fi
  echo

  bold "VERDICT (fast)"
  if [ "$RC" -eq 0 ]; then
    printf '  %sFAST AUDIT PASSED%s — L3 certifies the faithful program, REFUSES the buggy one,\n' "$G" "$Z"
    note "and the proof reproduces under independent Verus. This is the EXISTENCE demo;"
    note "run \`make audit\` for the full trust-chain re-derivation (the universal theorem,"
    note "full-corpus TV, the multi-class falsification battery, and the drift tripwire)."
  else
    printf '  %sFAST AUDIT FAILED%s — one or more checks did not hold (see the FAIL lines above).\n' "$R" "$Z"
  fi
  exit "$RC"
fi

# =============================================================================
#  DEEP PATH (default) — re-derive the WHOLE trust chain.
# =============================================================================
bold "Thermite DEEP audit — re-derive the WHOLE trust chain on YOUR machine"
echo "  This is slow (minutes): it rebuilds the Lean spine, runs the per-run TV over the"
echo "  full corpus, and runs the multi-class falsification battery live. Progress per check."
echo "  prover  : ${VERUS:-<none found>} ${VERUS:+($("$VERUS" --version 2>/dev/null | head -1))}"
echo

echo "  building forge from source (audit the tool you can read) ..."
if ! cargo build -q -p forge; then fail "forge build failed"; exit 2; fi
FORGE="$ROOT/target/debug/forge"
echo

# -----------------------------------------------------------------------------
# CHECK 1 — THE UNIVERSAL THEOREM, re-checked locally (the centerpiece).
# -----------------------------------------------------------------------------
bold "[1/5] THE UNIVERSAL THEOREM — re-verified by YOUR Lean kernel"
note "Re-builds the Lean proof spine from source, then \`#print axioms\` all twelve"
note "gated theorems: the v1 and loop spine, relax route, Stage 2 spine, and the"
note "Stage 3 reconstruction probe. PASS iff every axiom list is a subset of"
note "{propext, Classical.choice, Quot.sound} — no sorryAx, no custom axiom."
note "The build + parse is the SHARED scripts/lean-axiom-probe.sh — the same probe the"
note "Lean CI job runs (trust-audit F4), so local and CI cannot drift in what they check."

# detect elan/lake
LAKE=""
if [ -x "$HOME/.elan/bin/lake" ]; then LAKE="$HOME/.elan/bin/lake"
elif command -v lake >/dev/null 2>&1; then LAKE="$(command -v lake)"; fi

if [ -z "$LAKE" ]; then
  skip "elan/lake not found (looked in ~/.elan/bin and PATH)."
  note "CONSEQUENCE: the universal theorem was NOT re-derived locally — its axiom"
  note "footprint is taken on our word, not re-checked by your kernel. (install elan:"
  note "https://github.com/leanprover/elan)"
  SKIPPED_GUARANTEES+=("[1] universal theorem (Lean kernel) — NOT re-derived locally")
else
  export PATH="$(dirname "$LAKE"):$PATH"
  echo "      lake : $LAKE   (building lean/ from source — this is the slow part)"
  # Delegate the build + #print-axioms parse to the shared probe (single source of truth
  # with the Lean CI job). It prints PASS/FAIL per theorem; map its exit into RC.
  if bash "$ROOT/scripts/lean-axiom-probe.sh"; then
    note "MEANING: the ∀-programs faithfulness theorem (lowering_faithful), its three"
    note "T1 soundness pillars (ref_sound / exec_ref_sound / body_ref_sound), the loop"
    note "WHILE-RULE (while_rule), and the two relax-route spine lemmas (r_relax_sound /"
    note "rencode_sound) were re-verified by YOUR Lean kernel just now."
    note "STAGE 3: the permanent LRAT reconstruction theorem was also rebuilt and"
    note "axiom-checked, so the fixed-width replay probe contains no hidden proof hole."
    note "Trust does NOT include our claim of having proven them — you re-checked it."
    note "STAGE 2 ([1′]): the probe ALSO gated the four stratified soundness theorems"
    note "(strat_ref_sound, strat_lowering_faithful, classifier_correct, restrat_conservative)"
    note "— all axiom-clean — so the G2-gate check [1′] is GREEN (see the [G2] section)."
    S2_AXIOM=green
  else
    probe_rc=$?
    if [ "$probe_rc" -eq 2 ]; then
      fail "the Lean spine did not build / the probe could not elaborate (see above)."
      note "CONSEQUENCE: the universal theorem was NOT re-derived locally."
    else
      fail "a probed theorem carries a DISALLOWED axiom (see the FAIL line above)."
    fi
    S2_AXIOM=red
    RC=1
  fi
fi
echo

# -----------------------------------------------------------------------------
# CHECK 2 — FULL-CORPUS cross-validation (not one program).
# -----------------------------------------------------------------------------
bold "[2/5] FULL-CORPUS TRANSLATION-VALIDATION — every admitted program, live Z3"
note "For EVERY .th in conformance/ that forge admits, run \`forge tv\` (contracts),"
note "\`forge exec-tv\` (exec expressions) and \`forge body-tv\` (straight-line bodies)."
note "PASS iff ZERO Divergent across the corpus. Skipped/Unverifiable are counted and"
note "printed (out-of-frozen-subset constructs), not failing."

if [ -z "${VERUS:-}" ]; then
  skip "verus not found — the per-run TV obligations cannot be discharged by Z3."
  note "CONSEQUENCE: the lowering↔reference equivalence was NOT re-checked on this corpus."
  SKIPPED_GUARANTEES+=("[2] full-corpus TV (Z3) — NOT re-checked")
else
  # parse "N ... checked, M faithful, Z DIVERGENT, S skipped, U unverifiable" from each line
  parse_num() { printf '%s' "$1" | grep -oiE "[0-9]+ $2" | head -1 | grep -oE "[0-9]+" || printf '0'; }
  TOT_PROG=0; TOT_OK=0; ADMITTED=0; TOT_CHECKED=0; TOT_FAITHFUL=0
  TOT_DIV=0; TOT_SKIP=0; TOT_UNV=0
  # The skip-reason dedup uses an associative array (bash 4+). The macOS default bash 3.2
  # lacks it; the dedup list is purely informational (TOT_SKIP carries the count), so guard
  # it rather than crash under `set -u` there — the corpus TV verdict is unaffected.
  HAVE_ASSOC=0
  if [ "${BASH_VERSINFO:-0}" -ge 4 ]; then HAVE_ASSOC=1; declare -A SKIP_REASONS=(); fi
  for f in conformance/*.th; do
    TOT_PROG=$((TOT_PROG+1))
    name="$(basename "$f")"
    prog_div=0; prog_admit=1
    for sub in tv exec-tv body-tv; do
      out="$("$FORGE" "$sub" "$f" 2>&1)"; src=$?
      # PARSE-FIRST (do NOT branch on the exit code): forge tv/exec-tv/body-tv exit
      # NONZERO (EXIT_VERIFICATION_FAILURE) precisely WHEN a clause is Divergent — so
      # treating every nonzero exit as "not admitted" would SWALLOW the one finding this
      # gate exists to catch. Authority: forge/src/cli.rs::run_tv (Divergent ⇒ nonzero
      # exit) + render_report's header. Locate the report line ("… N clause(s)/expr(s)/
      # bod… checked, …") wherever it sits; if there is NO report line, forge genuinely
      # refused (usage/parse error) → count as not-admitted for that sub and move on.
      hdr="$(printf '%s' "$out" | grep -iE '[0-9]+ (clause|expr|bod)[a-z()/]* checked' | head -1)"
      if [ -z "$hdr" ]; then
        prog_admit=0
        continue
      fi
      c="$(parse_num "$hdr" "(expr\\(s\\)|clause\\(s\\)|bod(y|ies)) checked")"
      [ -z "$c" ] && c="$(printf '%s' "$hdr" | grep -oE '[0-9]+ (clause|expr|bod)' | head -1 | grep -oE '[0-9]+')"
      ck="$(printf '%s' "$hdr" | grep -oiE '[0-9]+ (clause\(s\)|expr\(s\)|body|bodies) checked' | grep -oE '[0-9]+' | head -1)"
      fa="$(printf '%s' "$hdr" | grep -oiE '[0-9]+ faithful' | grep -oE '[0-9]+' | head -1)"
      dv="$(printf '%s' "$hdr" | grep -oiE '[0-9]+ DIVERGENT' | grep -oE '[0-9]+' | head -1)"
      sk="$(printf '%s' "$hdr" | grep -oiE '[0-9]+ skipped' | grep -oE '[0-9]+' | head -1)"
      uv="$(printf '%s' "$hdr" | grep -oiE '[0-9]+ unverifiable' | grep -oE '[0-9]+' | head -1)"
      TOT_CHECKED=$((TOT_CHECKED + ${ck:-0}))
      TOT_FAITHFUL=$((TOT_FAITHFUL + ${fa:-0}))
      TOT_DIV=$((TOT_DIV + ${dv:-0}))
      TOT_SKIP=$((TOT_SKIP + ${sk:-0}))
      TOT_UNV=$((TOT_UNV + ${uv:-0}))
      prog_div=$((prog_div + ${dv:-0}))
      # collect a short skip reason if present
      if [ "${sk:-0}" -gt 0 ] && [ "$HAVE_ASSOC" -eq 1 ]; then
        reason="$(printf '%s' "$out" | grep -iE 'skipped' | grep -oiE 'OUTSIDE the v1 frozen subset|loop is OUTSIDE|unsupported exec construct|no body|non-scalar' | head -1)"
        [ -n "$reason" ] && SKIP_REASONS["$reason"]=1
      fi
    done
    [ "$prog_admit" -eq 1 ] && ADMITTED=$((ADMITTED+1))
    if [ "$prog_div" -eq 0 ]; then
      TOT_OK=$((TOT_OK+1))
      printf '      %sok%s   %-22s 0 divergent\n' "$G" "$Z" "$name"
    else
      printf '      %sBAD%s  %-22s %s DIVERGENT\n' "$R" "$Z" "$name" "$prog_div"
    fi
  done
  echo
  note "corpus totals: $TOT_PROG programs ($ADMITTED admitted by forge), $TOT_CHECKED obligations checked live by Z3"
  note "               $TOT_FAITHFUL faithful, $TOT_DIV divergent, $TOT_SKIP skipped, $TOT_UNV unverifiable"
  if [ "$HAVE_ASSOC" -eq 1 ] && [ "${#SKIP_REASONS[@]}" -gt 0 ]; then
    note "skip reasons (out-of-frozen-subset, counted not failed):"
    for k in "${!SKIP_REASONS[@]}"; do note "  - $k"; done
  fi
  if [ "$TOT_DIV" -eq 0 ]; then
    pass "ZERO divergent across the whole corpus — the production lowering matched the proven reference encoder on every admitted obligation"
  else
    fail "$TOT_DIV DIVERGENT obligation(s) across the corpus — the lowering disagrees with the reference encoder"
    RC=1
  fi
fi
echo

# -----------------------------------------------------------------------------
# CHECK 3 — THE FALSIFICATION BATTERY (multi-class) + one visible mutant.
# -----------------------------------------------------------------------------
bold "[3/5] THE FALSIFICATION BATTERY — does Z3 CATCH injected infidelities?"
note "A rubber-stamp prover passes everything. These suites inject production-side"
note "infidelities of MANY classes and assert TV (Z3) CATCHES each. Classes exercised:"
note "  contract: wrong-op, cast-paren-drop, byte-view misdispatch, arg-kind (index↔slice),"
note "            wrong-combinator, structural-drop  (thermite-tv::teeth)"
note "  body:     dropped-stmt, reordered-mutation, swapped if-branch, multi-cell projection"
note "            (thermite-tv::body_teeth)"
note "  exec:     wrong-op, nat-coercion-underflow, cast-paren, off-by-one (thermite-tv::exec_teeth)"
note "  loop:     broken invariant (entry/preservation), exit-overclaim (thermite-tv::loop_teeth)"

if [ -z "${VERUS:-}" ]; then
  skip "verus not found — the teeth suites need Z3 to demonstrate the catch."
  note "CONSEQUENCE: the prover's teeth were NOT demonstrated on this machine."
  SKIPPED_GUARANTEES+=("[3] falsification battery (Z3 teeth) — NOT demonstrated")
else
  export VERUS_BIN="$VERUS"
  echo "      running: cargo test -p thermite-tv --test teeth --test body_teeth --test exec_teeth --test loop_teeth"
  if cargo test -q -p thermite-tv --test teeth --test body_teeth --test exec_teeth --test loop_teeth >"$TMP/teeth.log" 2>&1; then
    # surface the per-suite pass counts
    grep -E "test result:" "$TMP/teeth.log" | sed 's/^/      /'
    pass "the falsification battery is GREEN — Z3 caught every injected infidelity class above"
  else
    fail "the falsification battery FAILED — a class of infidelity was NOT caught (or a suite errored)"
    tail -25 "$TMP/teeth.log" | sed 's/^/      /'
    RC=1
  fi
fi
echo

# The ONE visible end-to-end mutant — the legible single demonstration (the old (B)).
bold "      illustration: the SAME program with one wrong line must be REFUSED"
if [ -z "${VERUS:-}" ]; then
  skip "verus absent — the end-to-end mutant illustration is skipped (the battery above is the evidence)."
else
  BUG="$TMP/${ITEM}_bug.th"
  sed 's/return Some(mid);/return Some(mid + 1);/' "$PROG" > "$BUG"
  if diff -q "$PROG" "$BUG" >/dev/null; then
    note "(could not inject the binary_search mutant into $PROG — illustration only, the battery is the evidence)"
  else
    note "injected: $(grep -n 'Some(mid + 1)' "$BUG" | head -1 | sed 's/^[0-9]*:[[:space:]]*//')"
    B_OUT="$("$FORGE" check "$BUG" 2>&1)"
    if echo "$B_OUT" | grep -qiE "level:[[:space:]]*L3"; then
      fail "the buggy program STILL certified L3 — the prover did not catch the end-to-end bug!"; RC=1
    else
      pass "the buggy $ITEM was REFUSED (not L3) — the legible single demonstration"
    fi
  fi
fi
echo

# -----------------------------------------------------------------------------
# CHECK 4 — CORRESPONDENCE DRIFT TRIPWIRE.
# -----------------------------------------------------------------------------
bold "[4/5] CORRESPONDENCE DRIFT TRIPWIRE — is the Rust↔Lean audit still current?"
CORR_DOC=".design/verified/rust-lean-correspondence.md"
note "The Rust reference encoders are tied to their kernel-proven Lean models by an"
note "ARM-BY-ARM audit-by-inspection ($CORR_DOC). That audit pins the"
note "encoder + Lean SHAs it inspected. If a pinned file's CURRENT last-touch commit"
note "differs, the inspection predates the code and the residual is STALE — re-audit required."

if [ ! -f "$CORR_DOC" ]; then
  skip "$CORR_DOC not found — cannot check correspondence drift."
  note "CONSEQUENCE: the Rust↔Lean inspection-tier residual was NOT drift-checked."
  SKIPPED_GUARANTEES+=("[4] correspondence drift — NOT checked")
else
  # The pinned (artifact -> file -> SHA) rows from the doc's "Audited commits" table.
  # Every row whose artifact is a concrete file path goes here (the loop below reads
  # its pinned SHA from the doc and diffs against the file's current last-touch); the
  # `lean/Thermite/**` spine GLOB is not a real path, so it has its own block after.
  declare -a PIN_FILE=(
    "thermite-tv/src/ref_encode.rs"
    "thermite-tv/src/exec_encode.rs"
    "thermite-tv/src/exec_stmt_encode.rs"
    "thermite-spec/src/combinators.rs"
    "forge/src/lean_export.rs"
  )
  # the doc wraps both the path and the SHA in backticks; a literal backtick in a
  # grep pattern inside $(...) would be mis-read as legacy command substitution, so
  # match the row by the (plain) path and pull the first backtick-hex-backtick token
  # via awk against a backtick held in a variable.
  BT="$(printf '\140')"
  pin_sha_for() { # $1 = grep -F needle that selects the row
    grep -F "$1" "$CORR_DOC" \
      | awk -v bt="$BT" '{ if (match($0, bt "[0-9a-f]+" bt)) { s=substr($0,RSTART+1,RLENGTH-2); print s; exit } }'
  }
  DRIFT=0
  for pf in "${PIN_FILE[@]}"; do
    # read the pinned SHA straight from the doc row (`file` | `SHA` (#...))
    pinned="$(pin_sha_for "$pf")"
    cur="$(git log -1 --format=%h -- "$pf" 2>/dev/null)"
    if [ -z "$pinned" ]; then
      skip "$pf — no pinned SHA found in the doc table (cannot compare)"
      DRIFT=1; continue
    fi
    # compare on the shorter length (doc may pin short, git log returns short)
    plen=${#pinned}; clen=${#cur}; n=$(( plen < clen ? plen : clen ))
    if [ "${pinned:0:$n}" = "${cur:0:$n}" ]; then
      pass "$pf — unchanged since the audit (pinned $pinned, current $cur)"
    else
      fail "$pf — DRIFTED: pinned $pinned, current $cur"
      DRIFT=1
    fi
  done
  # the Lean spine SHA (the doc pins `lean/Thermite/**` @ <SHA> in the "Lean spine" row)
  lean_pinned="$(pin_sha_for 'Lean spine')"
  lean_cur="$(git log -1 --format=%h -- lean/Thermite/ 2>/dev/null)"
  if [ -n "$lean_pinned" ]; then
    plen=${#lean_pinned}; clen=${#lean_cur}; n=$(( plen < clen ? plen : clen ))
    if [ "${lean_pinned:0:$n}" = "${lean_cur:0:$n}" ]; then
      pass "lean/Thermite/** — unchanged since the audit (pinned $lean_pinned, current $lean_cur)"
    else
      fail "lean/Thermite/** — DRIFTED: pinned $lean_pinned, current $lean_cur"
      DRIFT=1
    fi
  fi
  if [ "$DRIFT" -eq 0 ]; then
    pass "every pinned artifact is unchanged — the arm-by-arm correspondence audit is CURRENT"
  else
    fail "the Rust↔Lean correspondence audit PREDATES the current encoder/spine — re-audit required"
    note "The inspection-tier residual (Rust encoder ↔ Lean model) is not honestly closed under"
    note "this drift: re-run the arm-by-arm audit and re-pin the SHAs in $CORR_DOC."
    RC=1
  fi
fi
echo

# -----------------------------------------------------------------------------
# CHECK 5 — THIRD-PARTY PROVER RE-CHECK (the legacy (D), kept).
# -----------------------------------------------------------------------------
bold "[5/5] THIRD-PARTY PROVER RE-CHECK — the golden proof, forge EXCLUDED"
note "The committed emitted Verus proof must re-verify under your Verus/Z3 with forge"
note "entirely out of the loop (the most legible single 'the proof is real' check)."
if [ -z "${VERUS:-}" ]; then
  skip "verus not found — the emitted proof cannot be re-verified independently."
  note "CONSEQUENCE: the golden proof was NOT re-verified by a third-party prover."
  SKIPPED_GUARANTEES+=("[5] third-party re-check (Verus) — NOT re-verified")
elif [ ! -f "$GOLDEN" ]; then
  fail "emitted proof file not found: $GOLDEN"; RC=1
else
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

# -----------------------------------------------------------------------------
# CHECK G2 — STAGE-2 STRATIFIED CAGE: the four-check G2 gate (REQ-9 / AC-9).
# -----------------------------------------------------------------------------
# The certificate `trust:` flip for stratified clauses (ref_encode(strat, UNPROVEN) →
# the proven, honestly-scoped form) is GATED on four checks being green in THIS one run:
#   [1′] the axiom probe extended to the four stratified soundness theorems (shares the
#        Lean build of check [1] above) ;
#   [4′] the doc-drift tripwire over the three mirrored Rust files (the stratified
#        classifier / encoder / two-phase TV), .design/verified/strat-rust-lean-correspondence.md ;
#   [8]  the classifier differential battery (`forge strat-tv`) — the Rust classifier held
#        byte-equal to the Lean kernel `admitted` over generated formulae ;
#   [9]  the stratified two-phase TV sweep (`forge strat-faithful-tv`) — syntactic /
#        semantic / timeout phase split, every clause certified.
# The gate itself (`forge g2-gate`) is the TESTED CODE PATH: with G2 declared, ANY red
# check makes it exit non-zero — the flip is mechanically withheld (a flipped certificate
# can never out-run the audit that justifies it). A tool-absent SKIP is INCONCLUSIVE, not
# a green (R-HONEST-3).
bold "[G2] STAGE-2 STRATIFIED CAGE — the four-check G2 gate (REQ-9 / AC-9)"

# [4′] doc-drift tripwire over the three mirrored Rust files.
note "[4′] doc-drift: the three mirrored Rust files (classifier / strat_ref_encode /"
note "     strat_two_phase) are content-pinned + current under the stratified correspondence doc."
if ! command -v python3 >/dev/null 2>&1; then
  skip "[4′] python3 not found — the doc-drift tripwire was not run."
  S2_DRIFT=skip
elif python3 "$ROOT/tooling/doc-drift.py" >"$TMP/docdrift.out" 2>&1 \
     && grep -q "CURRENT  .design/verified/strat-rust-lean-correspondence.md" "$TMP/docdrift.out"; then
  pass "[4′] doc-drift GREEN — the stratified correspondence doc is current (no mirror drift)"
  S2_DRIFT=green
else
  fail "[4′] doc-drift RED — a routed design doc drifted (see below) or the stratified doc is stale"
  grep -E "^DRIFT|^MISSING-PIN|^INVALID-PIN" "$TMP/docdrift.out" | sed 's/^/      /' | head -8
  S2_DRIFT=red
  RC=1
fi

# [8] the classifier differential battery (needs lake for the Lean side; honest SKIP absent).
note "[8] differential battery (\`forge strat-tv\`): the Rust admission classifier held"
note "    byte-equal to the Lean kernel \`Thermite.Strat.Cls.admitted\` over generated formulae."
if [ -z "${LAKE:-}" ]; then
  skip "[8] elan/lake not found — the classifier differential (Lean side) was not run."
  S2_DIFF=skip
else
  if "$FORGE" strat-tv >"$TMP/strattv.out" 2>&1; then
    if grep -qiE "SKIPPED" "$TMP/strattv.out"; then
      skip "[8] strat-TV reported SKIPPED (lake env unavailable for the Lean run)."
      S2_DIFF=skip
    else
      grep -iE "checked|agreement|disagreement|tripwire" "$TMP/strattv.out" | sed 's/^/      /' | head -4
      pass "[8] differential battery GREEN — zero classifier disagreements, zero tripwire"
      S2_DIFF=green
    fi
  else
    fail "[8] differential battery RED — a classifier disagreement or an unknown-on-admitted tripwire"
    tail -8 "$TMP/strattv.out" | sed 's/^/      /'
    S2_DIFF=red
    RC=1
  fi
fi

# [9] the stratified two-phase TV sweep (pure Rust — always runs).
note "[9] two-phase TV sweep (\`forge strat-faithful-tv\`): syntactic / semantic / timeout"
note "    phase split; every stratified clause certified (no divergence, none withheld)."
if "$FORGE" strat-faithful-tv >"$TMP/stratsweep.out" 2>&1; then
  grep -iE "clauses:|PASS" "$TMP/stratsweep.out" | sed 's/^/      /' | head -3
  pass "[9] two-phase TV sweep GREEN — every stratified clause certified"
  S2_TVSWEEP=green
else
  fail "[9] two-phase TV sweep RED — a divergence or a withheld (timeout) clause"
  tail -8 "$TMP/stratsweep.out" | sed 's/^/      /'
  S2_TVSWEEP=red
  RC=1
fi

# The gate itself — the TESTED CODE PATH that mechanically blocks the flip. A tool-absent
# SKIP on any of the four means the gate cannot be evaluated this run (INCONCLUSIVE); a RED
# means the gate is exercised live and FAILS (the mechanical block). All-green flips.
note "[G2] gate (\`forge g2-gate\`): combine [1′][4′][8][9] through g2_flip_permitted —"
note "     the proven (honestly-scoped) flip is permitted iff G2 is declared AND all four green."
if [ "$S2_AXIOM" = skip ] || [ "$S2_DRIFT" = skip ] || [ "$S2_DIFF" = skip ] || [ "$S2_TVSWEEP" = skip ]; then
  skip "[G2] gate INCONCLUSIVE — a gating check SKIPPED (tool absent): \
[1′]=$S2_AXIOM [4′]=$S2_DRIFT [8]=$S2_DIFF [9]=$S2_TVSWEEP"
  note "CONSEQUENCE: the G2 trust flip was NOT re-derived this run (it stays conservative)."
  SKIPPED_GUARANTEES+=("[G2] stratified trust flip — gate not fully evaluated ([1′]=$S2_AXIOM [4′]=$S2_DRIFT [8]=$S2_DIFF [9]=$S2_TVSWEEP)")
else
  v() { [ "$1" = green ] && echo 1 || echo 0; }
  if "$FORGE" g2-gate \
       --axiom-probe "$(v "$S2_AXIOM")" --doc-drift "$(v "$S2_DRIFT")" \
       --differential "$(v "$S2_DIFF")" --two-phase "$(v "$S2_TVSWEEP")" \
       >"$TMP/g2gate.out" 2>&1; then
    grep -iE "effective trust|trust flip permitted|G2 — all four green" "$TMP/g2gate.out" | sed 's/^/      /'
    pass "[G2] the four checks are GREEN in one run — the stratified trust flip is IN EFFECT (G2 reached)"
  else
    grep -iE "BLOCKED|effective trust" "$TMP/g2gate.out" | sed 's/^/      /'
    fail "[G2] the gate BLOCKED the flip — G2 is declared but a gating check is red (the mechanical block)"
    RC=1
  fi
fi
echo

# -----------------------------------------------------------------------------
# CHECK 6 — THE VERDICT + THE RESIDUAL-TRUST STATEMENT.
# -----------------------------------------------------------------------------
bold "VERDICT"
GUARANTEE_SKIPS=${#SKIPPED_GUARANTEES[@]}
if [ "$RC" -eq 0 ] && [ "$GUARANTEE_SKIPS" -eq 0 ]; then
  printf '  %sDEEP AUDIT PASSED%s — every guarantee-bearing link (1-5) was re-derived here.\n' "$G" "$Z"
elif [ "$RC" -eq 0 ] && [ "$GUARANTEE_SKIPS" -gt 0 ]; then
  printf '  %sDEEP AUDIT INCONCLUSIVE%s — no check FAILED, but a guarantee-bearing check SKIPPED:\n' "$Y" "$Z"
  for s in "${SKIPPED_GUARANTEES[@]}"; do printf '       %s- %s%s\n' "$Y" "$s" "$Z"; done
  note "Install the missing tool(s) and re-run for a full re-derivation."
  # INCONCLUSIVE is NOT a pass: a guarantee-bearing link was never re-derived. Exit
  # NONZERO (3, distinct from FAILED's 1) so automation cannot read a skipped-guarantee
  # run as green (R-HONEST-3: no false assurance from an un-run check).
  RC=3
else
  printf '  %sDEEP AUDIT FAILED%s — a guarantee-bearing check did NOT hold (see the FAIL lines above).\n' "$R" "$Z"
  if [ "$GUARANTEE_SKIPS" -gt 0 ]; then
    note "Additionally, guarantee-bearing check(s) were SKIPPED:"
    for s in "${SKIPPED_GUARANTEES[@]}"; do printf '       %s- %s%s\n' "$Y" "$s" "$Z"; done
  fi
fi
echo
bold "THE RESIDUAL TRUST — what you are STILL trusting (everything else re-derived just now)"
note "1. The Lean kernel + its 3 standard axioms {propext, Classical.choice, Quot.sound}"
note "   — check [1] re-ran the kernel on your machine and parsed exactly this axiom set."
note "2. Z3/Verus soundness — the per-run TV equivalences (check [2]) are discharged by Z3."
note "   Supported QF_LIA and QF_BV clauses can move to kernel-checked replay; Z3 remains"
note "   in the trust base for every solver result that has not completed that replay."
note "3. S = the intended meaning — that the Lean denotation \`S\` is what you MEANT the"
note "   program to mean (the spec-to-intent gap; irreducible on any tier)."
note "4. The Rust↔Lean correspondence (inspection tier) — the Rust reference encoders match"
note "   their kernel-proven Lean models by arm-by-arm inspection, pinned + drift-checked in"
note "   check [4] above (.design/verified/rust-lean-correspondence.md)."
note "5. rustc/LLVM — the backend that compiles the emitted Rust to a native binary."
echo
note "This list is what you are trusting. Everything else was re-derived on this machine just now."
exit "$RC"
