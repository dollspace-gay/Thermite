//! L3-grounding conformance for Cluster **C9-A** (crosslink **#108**): plain-`fn`
//! RECURSION — a regular exec `fn` carrying an optional `dec <measure>` clause may
//! call itself, with Verus proving termination via the decreases
//! (`.design/basis/10-recursion-tuples.md` REQ-1..4).
//!
//! These run against the EXTERNAL truths the toolchain does not author for itself:
//! the built `forge` binary's certificate ladder (`forge check`, real verus) and
//! the L1 build+run path (`forge build`, real rustc) — R-CODE-4: the subprocess
//! status is checked, never swallowed.
//!
//! Pins the C9-A deliverables (the GROUNDED forms from the design's Verification
//! section, certified with real `verus 0.2026.05.24`):
//!
//!   * A recursive `fn count_down(n)` with `dec n` and `ens result == 0`
//!     (self-call `count_down(n - 1)`) → **L3** (Verus proves termination from the
//!     `decreases n`) — REQ-1/REQ-3, AC-1.
//!   * The SAME fn recursing on `n` (NOT `n - 1`) with `dec n` → **L0** (Verus
//!     "could not prove termination" — the decreases BITES) — REQ-4, AC-2.
//!   * A self-calling `fn` with NO `dec` and NOT `fx diverge` → a structured
//!     `MissingDecreases` validator error ("recursive function … must have a
//!     decreases clause"), never reaching an L3 cert — REQ-2, AC-3.
//!   * A `fx diverge` recursive `fn` with NO `dec` → **L1** (the #88 exemption —
//!     partial correctness only; termination not claimed) — REQ-2/REQ-4, AC-3.
//!   * A recursive `fn` BUILDS + RUNS (the self-call executes through the L1
//!     runtime-check path) — REQ-3.
//!
//! NON-VACUITY (R-DEFER-9 / `thermite-design.md` §7): the `ens result == 0` is a
//! function of the recursion's fixpoint (a non-terminating or wrong body cannot
//! satisfy it), and the decreases is the ONLY thing standing between the fn and L0
//! — remove it → structured error; weaken it (recurse on `n`) → termination
//! failure. A non-terminating fn cannot be laundered to L3.
//!
//! R-CHAR-3: the expected LEVELS trace to the design (L3 == a discharged verus
//! termination proof; L0 == "could not prove termination"; the no-`dec` reject ==
//! the Verus "recursive function must have a decreases clause" rule mirrored as
//! `MissingDecreases`) — `.design/basis/10-recursion-tuples.md` REQ-1..4 +
//! Verification — NEITHER copied from forge's own output. Runs the BUILT `forge`
//! binary; if verus is absent the L3/L0 cases SKIP LOUDLY (never panic on a
//! missing solver), mirroring `operators_conformance.rs`.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// `true` iff verus is reachable (mirrors `operators_conformance.rs`).
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
        "forge_recur_{tag}_{}_{}.th",
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

// The GROUNDED countdown (design Verification): `count_down(n - 1)`, `dec n`.
const COUNT_DOWN_L3: &str = "fn count_down(n: u64) -> u64\n  \
    req n <= 1000\n  ens result == 0\n  fx pure\n  dec n\n\
    {\n  if n == 0 {\n    0\n  } else {\n    count_down(n - 1)\n  }\n}\n";

// The SAME fn recursing on `n` (non-decreasing) — the decreases does not bite.
const COUNT_DOWN_NONDECREASING: &str = "fn count_down(n: u64) -> u64\n  \
    req n <= 1000\n  ens result == 0\n  fx pure\n  dec n\n\
    {\n  if n == 0 {\n    0\n  } else {\n    count_down(n)\n  }\n}\n";

// A self-calling fn with NO `dec` and NOT `fx diverge` — the mandatory-dec reject.
const COUNT_DOWN_NO_DEC: &str = "fn count_down(n: u64) -> u64\n  \
    req n <= 1000\n  ens result == 0\n  fx pure\n\
    {\n  if n == 0 {\n    0\n  } else {\n    count_down(n - 1)\n  }\n}\n";

// A `fx diverge` recursive fn with NO `dec` — the #88 exemption (L1-capped).
const SPIN_DIVERGE: &str = "fn spin(n: u64) -> u64\n  \
    req true\n  ens result == 0\n  fx diverge\n\
    {\n  if n == 0 {\n    0\n  } else {\n    spin(n - 1)\n  }\n}\n";

// ---------------------------------------------------------------------------
// REQ-1/REQ-3, AC-1: a recursive fn with `dec` certifies L3 (real verus).
// ---------------------------------------------------------------------------

#[test]
fn recursive_fn_with_dec_certifies_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — recursion L3 grounding not exercised.");
        return;
    }
    let certs = check_program("cd_l3", COUNT_DOWN_L3);
    assert_eq!(
        level(&certs, "count_down"),
        "L3",
        "DESIGN 10-recursion-tuples.md REQ-1/REQ-3 + Verification: a recursive `fn` \
         with `dec n` and self-call `count_down(n - 1)` lets Verus prove termination \
         from the emitted `decreases n` → L3. forge: {certs:?}"
    );
}

// ---------------------------------------------------------------------------
// REQ-4, AC-2: a NON-decreasing recursive call → L0 (the decreases BITES).
// ---------------------------------------------------------------------------

#[test]
fn nondecreasing_recursion_is_l0() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — recursion L0 grounding not exercised.");
        return;
    }
    let certs = check_program("cd_l0", COUNT_DOWN_NONDECREASING);
    assert_eq!(
        level(&certs, "count_down"),
        "L0",
        "DESIGN 10-recursion-tuples.md REQ-4 + Verification: `dec n` but recursing on \
         `n` (not `n - 1`) does NOT decrease, so Verus reports \"could not prove \
         termination\" → L0. The decreases is REAL (R-DEFER-9 — no proof cheat). \
         forge: {certs:?}"
    );
}

// ---------------------------------------------------------------------------
// REQ-2, AC-3: a self-call with NO `dec` (and not diverge) → structured error.
// ---------------------------------------------------------------------------

#[test]
fn self_call_without_dec_is_structured_error() {
    // No verus needed — this is rejected by the spec VALIDATOR before the ladder.
    let fixture = std::env::temp_dir().join(format!("forge_recur_nodec_{}.th", std::process::id()));
    std::fs::write(&fixture, COUNT_DOWN_NO_DEC).expect("write fixture");
    let out = Command::new(forge_bin())
        .arg("check")
        .arg(&fixture)
        .output()
        .expect("spawn forge");
    let _ = std::fs::remove_file(&fixture);
    assert!(
        !out.status.success(),
        "DESIGN 10-recursion-tuples.md REQ-2: a recursive `fn` with NO `dec` and NOT \
         `fx diverge` MUST be REJECTED (non-zero exit), never reaching an L3 cert."
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("recursive function") && stderr.contains("must have a decreases clause"),
        "DESIGN 10-recursion-tuples.md REQ-2 + Verification: the validator emits the \
         surface mirror of the Verus rule \"recursive function must have a decreases \
         clause\" (MissingDecreases). stderr:\n{stderr}"
    );
}

// ---------------------------------------------------------------------------
// REQ-2/REQ-4, AC-3: a `fx diverge` recursive fn with NO `dec` → L1 (#88).
// ---------------------------------------------------------------------------

#[test]
fn diverge_recursion_without_dec_is_l1() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — diverge-recursion L1 grounding not exercised.");
        return;
    }
    let certs = check_program("spin_l1", SPIN_DIVERGE);
    assert_eq!(
        level(&certs, "spin"),
        "L1",
        "DESIGN 10-recursion-tuples.md REQ-2/REQ-4 + #88: a `fx diverge` fn is honestly \
         non-terminating and EXEMPT from the mandatory-`dec` rule \
         (`#[verifier::exec_allows_no_decreases_clause]`); it may recurse without a \
         decreases and is L1-capped (partial correctness only), NOT L0. forge: {certs:?}"
    );
}

// ---------------------------------------------------------------------------
// REQ-3: a recursive fn BUILDS + RUNS (the self-call executes at L1).
// ---------------------------------------------------------------------------

#[test]
fn recursive_fn_builds_and_runs() {
    // `forge build` lowers to L1 exec Rust + compiles via rustc; the `run` entry
    // calls `count_down(5)`, executing the self-recursion through the runtime-check
    // path (the `ens result == 0` runtime check passes — no panic).
    let program = "fn count_down(n: u64) -> u64\n  \
        req n <= 1000\n  ens result == 0\n  fx pure\n  dec n\n\
        {\n  if n == 0 {\n    0\n  } else {\n    count_down(n - 1)\n  }\n}\n\n\
        fn run() -> u64\n  req true\n  ens result == 0\n  fx pure\n\
        {\n  count_down(5)\n}\n";
    let fixture = std::env::temp_dir().join(format!("forge_recur_build_{}.th", std::process::id()));
    std::fs::write(&fixture, program).expect("write fixture");
    let out = Command::new(forge_bin())
        .arg("build")
        .arg(&fixture)
        .arg("--entry")
        .arg("run")
        .output()
        .expect("spawn forge build");
    let _ = std::fs::remove_file(&fixture);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "DESIGN 10-recursion-tuples.md REQ-3: a recursive `fn` must BUILD + RUN (the \
         self-call lowers as an ordinary call at L1, no decreases at runtime). \
         stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("L1 (built, runtime-checked)") && stdout.contains("count_down"),
        "DESIGN REQ-3: the L1 build emits the recursive fn + runs the `run` entry. \
         stdout:\n{stdout}"
    );
}
