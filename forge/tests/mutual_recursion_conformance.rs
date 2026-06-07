//! L3-grounding conformance for Cluster **C11** (crosslink **#121**, epic
//! **#113**): MUTUAL recursion — a call-graph cycle of exec `fn`s (`a -> b -> a`,
//! `a -> b -> c -> a`, …) where every member carries a `dec` measure is a Verus
//! mutual-`decreases` group and certifies **L3**
//! (`.design/basis/12-mutual-recursion.md` REQ-1..4).
//!
//! C9 (#108) shipped direct self-recursion and cleanly **L0-rejected** any mutual
//! cycle (`forge::check`'s `mutual_recursion_cycle_fns`, #110). C11 REFINES that
//! blanket reject to CONDITIONAL: a cycle whose members all carry `dec` (or
//! declare `fx diverge`) FALLS THROUGH to the normal per-item lower/verus ladder,
//! where the existing C9 source-order single-`verus!`-block emission presents
//! Verus a valid mutual-decreases group → Verus proves termination across the
//! cycle → L3. The reject fires ONLY when a non-diverge cycle member LACKS `dec`
//! (renamed cause `MutualRecursionMissingDecreases`).
//!
//! These run against the EXTERNAL truth the toolchain does not author for itself:
//! the built `forge` binary's certificate ladder (`forge check`, real verus) —
//! R-CODE-4: the subprocess status is checked, never swallowed.
//!
//! Pins the C11 ACs (the GROUNDED forms from the design's Verification section,
//! certified with real `verus 0.2026.05.24`):
//!
//!   * AC-1: an exec mutual PAIR `is_even(n)`/`is_odd(n)`, each `dec n`, each
//!     cross-calling the other on `n - 1`, each with a NON-VACUOUS `ens`
//!     (`result == (n % 2 == 0)` / `== (n % 2 == 1)`) → **L3** for BOTH (Verus
//!     proves the mutual-decreases group; the partner is woven into each member's
//!     §5.3 sub-program by the existing `reachable_fn_deps`).
//!   * AC-2: a mutual pair whose cross-call does NOT decrease the measure
//!     (`ping(n)` calls `pong(n)`, both `dec n`) → **L0** ("could not prove
//!     termination" — the decreases BITES, the SAME shape as the single-fn
//!     non-decreasing L0).
//!   * AC-3: a mutual pair where one member LACKS `dec` (and is not `fx diverge`)
//!     → **L0** rejected at `forge::check` with cause
//!     `MutualRecursionMissingDecreases` (a clean cert verdict, never the raw
//!     Verus VIR-error abort); the WHOLE non-diverge cycle is rejected.
//!   * AC-4: a `fx diverge` mutual cycle → **L1** (the #88 honesty exemption — a
//!     diverge member is never rejected for missing `dec`).
//!   * AC-5: a 3-cycle `step_a -> step_b -> step_c -> step_a`, each `dec n` cross
//!     `n - 1` → **L3** for all three (v1 is n-cycles, not pairs-only).
//!
//! NON-VACUITY (R-DEFER-9 / `thermite-design.md` §7): the AC-1 `ens` is tied to
//! `n % 2` (a wrong body cannot satisfy it), and the `dec` is the ONLY thing
//! between the cycle and L0 — remove it from a member → the missing-dec reject
//! (AC-3); weaken it (cross-call on `n`) → Verus termination failure (AC-2). A
//! non-terminating mutual cycle cannot be laundered to L3.
//!
//! R-CHAR-3: the expected LEVELS trace to the design (L3 == a discharged Verus
//! mutual-`decreases` proof; L0 == "could not prove termination" / the
//! missing-dec reject; L1 == the `fx diverge` cap) —
//! `.design/basis/12-mutual-recursion.md` REQ-1..4 + Verification + AC-1..AC-5 —
//! NEITHER copied from forge's own output. Runs the BUILT `forge` binary; if
//! verus is absent the L3/L0-verus cases SKIP LOUDLY (never panic on a missing
//! solver), mirroring `recursion_conformance.rs`.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// `true` iff verus is reachable (mirrors `recursion_conformance.rs`).
fn verus_present() -> bool {
    if let Ok(p) = std::env::var("VERUS_BIN") {
        if Path::new(&p).exists() {
            return true;
        }
    }
    if let Ok(out) = Command::new("which").arg("verus").output() {
        if out.status.success() && !String::from_utf8_lossy(&out.stdout).trim().is_empty() {
            return true;
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        if PathBuf::from(home).join(".local/bin/verus").exists() {
            return true;
        }
    }
    false
}

/// Write `program` to a temp `.th`, `forge check --json` it, return the cert array.
/// The temp file is removed before returning (scratch hygiene, #53).
fn check_program(tag: &str, program: &str) -> Vec<Value> {
    let fixture = std::env::temp_dir().join(format!(
        "forge_mutual_{tag}_{}_{}.th",
        std::process::id(),
        tag.len()
    ));
    std::fs::write(&fixture, program).expect("write fixture");
    let out = Command::new(forge_bin())
        .arg("check")
        .arg(&fixture)
        .arg("--json")
        .output()
        .expect("spawn forge");
    let _ = std::fs::remove_file(&fixture);
    let stdout = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str::<Value>(stdout.trim())
        .unwrap_or_else(|e| {
            panic!(
                "forge --json stdout must be one JSON doc: {e}\nstdout:\n{stdout}\nstderr:\n{}",
                String::from_utf8_lossy(&out.stderr)
            )
        })
        .as_array()
        .expect("array of certs")
        .clone()
}

fn cert_for<'a>(certs: &'a [Value], item: &str) -> &'a Value {
    certs
        .iter()
        .find(|c| c.get("item").and_then(|v| v.as_str()) == Some(item))
        .unwrap_or_else(|| panic!("no cert for `{item}` in {certs:?}"))
}

fn level(certs: &[Value], item: &str) -> String {
    cert_for(certs, item)["level"]
        .as_str()
        .unwrap_or("<none>")
        .to_string()
}

fn reject_cause(certs: &[Value], item: &str) -> String {
    cert_for(certs, item)
        .get("reject")
        .and_then(|r| r.get("cause"))
        .and_then(|v| v.as_str())
        .unwrap_or("<none>")
        .to_string()
}

// AC-1 (GROUNDED): the dec-complete mutual pair. Each member `dec n`, cross-calls
// the other on `n - 1`, NON-VACUOUS `ens` tied to `n % 2`. Verus proves the
// mutual-decreases group → L3 (raw verus `2 verified, 0 errors`).
const EVEN_ODD_L3: &str = "fn is_even(n: u64) -> bool\n  \
    req n <= 1000\n  ens result == (n % 2 == 0)\n  fx pure\n  dec n\n\
    {\n  if n == 0 { true } else { is_odd(n - 1) }\n}\n\n\
    fn is_odd(n: u64) -> bool\n  \
    req n <= 1000\n  ens result == (n % 2 == 1)\n  fx pure\n  dec n\n\
    {\n  if n == 0 { false } else { is_even(n - 1) }\n}\n";

// AC-2 (GROUNDED): both members `dec n`, but the cross-call does NOT decrease
// (`ping(n)` calls `pong(n)`, not `pong(n - 1)`). Reaches Verus (every member
// HAS `dec`, so NOT caught by the missing-dec reject) → "could not prove
// termination" → L0.
const PING_PONG_NONDECREASING: &str = "fn ping(n: u64) -> u64\n  \
    req n <= 1000\n  ens result == n\n  fx pure\n  dec n\n\
    {\n  if n == 0 { 0 } else { pong(n) }\n}\n\n\
    fn pong(n: u64) -> u64\n  \
    req n <= 1000\n  ens result == n\n  fx pure\n  dec n\n\
    {\n  if n == 0 { 0 } else { ping(n) }\n}\n";

// AC-3 (GROUNDED): `is_even` LACKS `dec` (and is not `fx diverge`); `is_odd` has
// it. The cycle is termination-INCOMPLETE → rejected at `forge::check` (BEFORE
// verus) with cause `MutualRecursionMissingDecreases`. The WHOLE non-diverge
// cycle is rejected (both members), since Verus would reject the entire group.
const EVEN_ODD_MISSING_DEC: &str = "fn is_even(n: u64) -> bool\n  \
    req n <= 1000\n  ens result == (n % 2 == 0)\n  fx pure\n\
    {\n  if n == 0 { true } else { is_odd(n - 1) }\n}\n\n\
    fn is_odd(n: u64) -> bool\n  \
    req n <= 1000\n  ens result == (n % 2 == 1)\n  fx pure\n  dec n\n\
    {\n  if n == 0 { false } else { is_even(n - 1) }\n}\n";

// AC-4: a `fx diverge` mutual cycle. Both members are honestly non-terminating
// (cross-call on `n + 1`, no `dec`) → the #88 exemption: never rejected for
// missing `dec`, lowered with `#[verifier::exec_allows_no_decreases_clause]`,
// L1-capped (partial correctness).
const DIVERGE_CYCLE_L1: &str = "fn loop_a(n: u64) -> u64\n  \
    req true\n  ens result == 0\n  fx diverge\n\
    {\n  if n == 0 { 0 } else { loop_b(n + 1) }\n}\n\n\
    fn loop_b(n: u64) -> u64\n  \
    req true\n  ens result == 0\n  fx diverge\n\
    {\n  if n == 0 { 0 } else { loop_a(n + 1) }\n}\n";

// AC-5 (GROUNDED): a 3-cycle `step_a -> step_b -> step_c -> step_a`, each member
// `dec n` cross-calling on `n - 1`. Verus proves the mutual-decreases group for
// the whole SCC → L3 for all three (v1 is n-cycles, not pairs-only).
const THREE_CYCLE_L3: &str = "fn step_a(n: u64) -> u64\n  \
    req n <= 1000\n  ens result == 0\n  fx pure\n  dec n\n\
    {\n  if n == 0 { 0 } else { step_b(n - 1) }\n}\n\n\
    fn step_b(n: u64) -> u64\n  \
    req n <= 1000\n  ens result == 0\n  fx pure\n  dec n\n\
    {\n  if n == 0 { 0 } else { step_c(n - 1) }\n}\n\n\
    fn step_c(n: u64) -> u64\n  \
    req n <= 1000\n  ens result == 0\n  fx pure\n  dec n\n\
    {\n  if n == 0 { 0 } else { step_a(n - 1) }\n}\n";

// ---------------------------------------------------------------------------
// AC-1 (REQ-1/REQ-3): a dec-complete mutual pair certifies L3 (real verus).
// ---------------------------------------------------------------------------

#[test]
fn dec_complete_mutual_pair_certifies_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — mutual-recursion L3 grounding not exercised.");
        return;
    }
    let certs = check_program("evenodd_l3", EVEN_ODD_L3);
    for item in ["is_even", "is_odd"] {
        assert_eq!(
            level(&certs, item),
            "L3",
            "DESIGN 12-mutual-recursion.md REQ-1/REQ-3 + AC-1: a mutual pair where EVERY \
             member carries `dec n` and cross-calls on `n - 1` is a Verus \
             mutual-`decreases` group; Verus proves the cycle terminates → `{item}` L3 \
             (NOT the MutualRecursionMissingDecreases reject — the C11 conditional \
             fall-through). forge: {certs:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// AC-2 (REQ-2/REQ-4): both members have `dec` but the cycle does NOT decrease
// → L0 (Verus "could not prove termination" — the decreases BITES).
// ---------------------------------------------------------------------------

#[test]
fn nondecreasing_mutual_cycle_is_l0() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — mutual-recursion L0 grounding not exercised.");
        return;
    }
    let certs = check_program("pingpong_l0", PING_PONG_NONDECREASING);
    for item in ["ping", "pong"] {
        assert_eq!(
            level(&certs, item),
            "L0",
            "DESIGN 12-mutual-recursion.md REQ-2/REQ-4 + AC-2: both members have `dec n` \
             (so NOT caught by the missing-dec reject), but the cross-call is on `n` (not \
             `n - 1`) so the measure does NOT decrease across the cycle → Verus \"could \
             not prove termination\" → `{item}` L0. The `dec` is REAL (R-DEFER-9 — no \
             proof cheat). forge: {certs:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// AC-3 (REQ-2): a member LACKS `dec` → clean L0 reject (the REFINED cause), the
// whole non-diverge cycle, BEFORE verus.
// ---------------------------------------------------------------------------

#[test]
fn mutual_cycle_missing_dec_is_rejected_l0() {
    // No verus needed — this is rejected at `forge::check` BEFORE lowering.
    let certs = check_program("evenodd_nodec", EVEN_ODD_MISSING_DEC);
    for item in ["is_even", "is_odd"] {
        assert_eq!(
            level(&certs, item),
            "L0",
            "DESIGN 12-mutual-recursion.md REQ-2 + AC-3: a mutual cycle where a member \
             lacks `dec` is rejected at `forge::check` as a clean L0 cert (the WHOLE \
             non-diverge cycle), never reaching verus → `{item}` L0. forge: {certs:?}"
        );
        assert_eq!(
            reject_cause(&certs, item),
            "MutualRecursionMissingDecreases",
            "DESIGN 12-mutual-recursion.md REQ-2 + AC-3 + OQ-1: the refined reject cause is \
             `MutualRecursionMissingDecreases` (the #110 `MutualRecursionUnsupported` \
             blanket cause is renamed — the only remaining mutual reject IS missing-dec). \
             forge: {certs:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// AC-4 (REQ-2 / #88): a `fx diverge` mutual cycle → L1 (exemption preserved).
// ---------------------------------------------------------------------------

#[test]
fn diverge_mutual_cycle_is_l1() {
    let certs = check_program("diverge_cycle", DIVERGE_CYCLE_L1);
    for item in ["loop_a", "loop_b"] {
        assert_eq!(
            level(&certs, item),
            "L1",
            "DESIGN 12-mutual-recursion.md AC-4 + #88: a `fx diverge` mutual-cycle member \
             is EXEMPT from the missing-dec reject (honestly non-terminating, lowers with \
             `#[verifier::exec_allows_no_decreases_clause]`, L1-capped — partial \
             correctness) → `{item}` L1, NEVER L0-rejected. forge: {certs:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// AC-5 (REQ-1/REQ-4): a 3-cycle with `dec` on every member → L3 (n-cycles).
// ---------------------------------------------------------------------------

#[test]
fn dec_complete_three_cycle_certifies_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — 3-cycle L3 grounding not exercised.");
        return;
    }
    let certs = check_program("threecycle_l3", THREE_CYCLE_L3);
    for item in ["step_a", "step_b", "step_c"] {
        assert_eq!(
            level(&certs, item),
            "L3",
            "DESIGN 12-mutual-recursion.md REQ-1/REQ-4 + AC-5: a 3-cycle \
             `step_a -> step_b -> step_c -> step_a` where every member carries `dec n` and \
             every cross-call decreases is a Verus mutual-`decreases` group over the whole \
             SCC → `{item}` L3 (v1 is n-cycles, not pairs-only). forge: {certs:?}"
        );
    }
}
