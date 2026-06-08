//! The LIVE conformance oracle for forge's §7 EQUIVALENT-MUTANT EXCLUSION
//! (`.design/forge/equivalent-mutants.md`, crosslink #101). It drives the BUILT
//! `forge` binary (mirroring `mutation_conformance.rs`) over hand-authored
//! forced-output fixtures and asserts the verdict FLIP the exclusion produces.
//!
//! Every case issues REAL per-survivor verus equivalence queries (the exclusion
//! is gated on a Verus PROOF — R-DEFER-9), so each case needs verus and SKIPS
//! LOUDLY when verus is absent, never panics.
//!
//! Expected verdicts are HAND-DERIVED from the design's *Ground the path*
//! (R-CHAR-3 — never read back from forge's own output):
//!   - AC-1: `clamp_zero` (`req x == 0 ens result == 0 { let y = x + 0; y }`)
//!     was `WeakContract 1/3` BEFORE #101; the early-`return 0` AND the `x - 0`
//!     binop-flip survivors are PROVED equivalent to `x + 0` under `x == 0`, so
//!     they DROP from the denominator → `1/1 = 1.0 >= 0.60` → CERTIFIES L3.
//!   - AC-2: `loose` (`req x <= 100 ens result <= 1000`) has a SURVIVING
//!     early-`return 0` mutant that is NOT equivalent (x = 5 distinguishes), so
//!     its query FAILS → it STAYS counted → STILL `WeakContract` (NOT laundered).
//!   - AC-3 / AC-5: `refuse` (`req x == 0 ens result == 0 { x }`) has a sole
//!     early-`return 0` survivor that IS proved equivalent → excluded → `0/0` →
//!     the #48 backstop STILL gates `WeakContract` (no vacuous `1.0` pass).
//!   - AC-4: a genuinely-KILLED-mutant fixture (`add`, a strong contract) is
//!     UNCHANGED — the equivalence query runs only on survivors.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

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

fn unique() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    N.fetch_add(1, Ordering::Relaxed)
}

fn write_temp(name: &str, program: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "forge_equiv_{}_{}_{name}.th",
        std::process::id(),
        unique()
    ));
    std::fs::write(&path, program).unwrap_or_else(|e| panic!("write temp {name}: {e}"));
    path
}

fn unique_cache_dir() -> PathBuf {
    std::env::temp_dir().join(format!(
        "forge_equiv_cache_{}_{}",
        std::process::id(),
        unique()
    ))
}

/// Run `forge check <file> --json`, returning (exit_code, certs).
fn run_check_json(file: &Path) -> (Option<i32>, Vec<Value>) {
    let cache_dir = unique_cache_dir();
    let _ = std::fs::remove_dir_all(&cache_dir);
    let out = Command::new(forge_bin())
        .arg("check")
        .arg(file)
        .arg("--json")
        .env("FORGE_CACHE_DIR", &cache_dir)
        .output()
        .unwrap_or_else(|e| panic!("spawn forge: {e}"));
    let _ = std::fs::remove_dir_all(&cache_dir);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!(
            "forge --json stdout must be one JSON document: {e}\nstdout:\n{stdout}\nstderr:\n{}",
            String::from_utf8_lossy(&out.stderr)
        )
    });
    let arr = value
        .as_array()
        .unwrap_or_else(|| panic!("forge --json must emit a JSON array: {value}"))
        .clone();
    (out.status.code(), arr)
}

fn cert_for<'a>(certs: &'a [Value], item: &str) -> &'a Value {
    certs
        .iter()
        .find(|c| c.get("item").and_then(|i| i.as_str()) == Some(item))
        .unwrap_or_else(|| panic!("no `{item}` cert in {certs:?}"))
}

fn mutants_killed(cert: &Value) -> String {
    cert.get("contract_quality")
        .and_then(|q| q.get("mutants_killed"))
        .and_then(|m| m.as_str())
        .unwrap_or_else(|| panic!("cert missing mutants_killed: {cert}"))
        .to_string()
}

fn parse_ratio(mk: &str) -> (u64, u64) {
    let (k, n) = mk
        .split_once('/')
        .unwrap_or_else(|| panic!("mutants_killed `{mk}` is not `K/N`"));
    (
        k.parse().unwrap_or_else(|_| panic!("bad killed in `{mk}`")),
        n.parse().unwrap_or_else(|_| panic!("bad scored in `{mk}`")),
    )
}

fn level(cert: &Value) -> String {
    cert.get("level")
        .and_then(|l| l.as_str())
        .unwrap_or("")
        .to_string()
}

fn reject_cause(cert: &Value) -> Option<String> {
    cert.get("reject")
        .and_then(|r| if r.is_null() { None } else { Some(r) })
        .and_then(|r| r.get("cause"))
        .and_then(|c| c.as_str())
        .map(|s| s.to_string())
}

const CLAMP_ZERO: &str = "fn clamp_zero(x: u64) -> u64\n    req x == 0\n    ens result == 0\n    fx pure\n{\n    let y: u64 = x + 0;\n    y\n}\n";

const LOOSE: &str = "fn loose(x: u64) -> u64\n    req x <= 100\n    ens result <= 1000\n    fx pure\n{\n    let y: u64 = x + 0;\n    y\n}\n";

const REFUSE: &str =
    "fn refuse(x: u64) -> u64\n    req x == 0\n    ens result == 0\n    fx pure\n{\n    x\n}\n";

const ADD: &str = "fn add(a: u64, b: u64) -> u64\n    req a <= 10 && b <= 10\n    ens result == a + b\n    fx pure\n{\n    let s: u64 = a + b;\n    s\n}\n";

/// AC-1: the equivalent-mutant exclusion FLIPS `clamp_zero` from the pre-#101
/// `WeakContract 1/3` to a certifying `L3` `1/1` — the two proved-equivalent
/// survivors (early-`return 0`, `x - 0` flip) drop from the denominator.
#[test]
fn ac1_forced_output_excludes_equivalents_and_certifies() {
    if !verus_present() {
        eprintln!("SKIP ac1: verus absent");
        return;
    }
    let path = write_temp("clamp_zero", CLAMP_ZERO);
    let (code, certs) = run_check_json(&path);
    let cert = cert_for(&certs, "clamp_zero");
    assert_eq!(
        code,
        Some(0),
        "AC-1: clamp_zero must certify (exit 0) after exclusion; cert: {cert}"
    );
    assert_eq!(
        level(cert),
        "L3",
        "AC-1: clamp_zero must certify L3 after exclusion; cert: {cert}"
    );
    assert_eq!(
        reject_cause(cert),
        None,
        "AC-1: clamp_zero must NOT be WeakContract after exclusion; cert: {cert}"
    );
    let mk = mutants_killed(cert);
    let (killed, scored) = parse_ratio(&mk);
    // The DROPPED equivalents reduce the denominator: at least one mutant killed,
    // and the surviving denominator carries NO un-killed mutant (kill_ratio 1.0).
    assert!(
        killed >= 1 && killed == scored,
        "AC-1: after excluding the proved-equivalent survivors the ratio is 1.0 \
         (every remaining scored mutant killed); got `{mk}`"
    );
    let _ = std::fs::remove_file(&path);
}

/// AC-2 (the soundness line, R-DEFER-9): a genuinely-weak contract's
/// DISTINGUISHING survivor FAILS the equivalence query → STAYS counted → STILL
/// `WeakContract`. The exclusion does NOT launder it.
#[test]
fn ac2_weak_contract_survivor_stays_counted() {
    if !verus_present() {
        eprintln!("SKIP ac2: verus absent");
        return;
    }
    let path = write_temp("loose", LOOSE);
    let (code, certs) = run_check_json(&path);
    let cert = cert_for(&certs, "loose");
    assert_ne!(
        code,
        Some(0),
        "AC-2: a genuinely-weak contract must be GATED (non-zero exit); cert: {cert}"
    );
    assert_eq!(
        level(cert),
        "L0",
        "AC-2: loose stays L0 (its distinguishing survivor is not excluded); cert: {cert}"
    );
    assert_eq!(
        reject_cause(cert).as_deref(),
        Some("WeakContract"),
        "AC-2: loose stays WeakContract — the distinguishing early-return survivor \
         is NEVER excluded (R-DEFER-9); cert: {cert}"
    );
    // The surviving denominator is below floor: a counted survivor remains.
    let (killed, scored) = parse_ratio(&mutants_killed(cert));
    assert!(
        scored > 0 && (killed as f64) / (scored as f64) < 0.60,
        "AC-2: loose's ratio stays below the 0.60 floor; got {killed}/{scored}"
    );
    let _ = std::fs::remove_file(&path);
}

/// AC-3 / AC-5 (no vacuous pass): `refuse`'s sole survivor is proved equivalent
/// → excluded → `0/0` → the #48 backstop STILL gates `WeakContract`. Exclusion
/// never opens a vacuous `1.0` pass for a fn the battery could not exercise.
#[test]
fn ac3_all_equivalent_reduces_to_zero_over_zero_still_gated() {
    if !verus_present() {
        eprintln!("SKIP ac3: verus absent");
        return;
    }
    let path = write_temp("refuse", REFUSE);
    let (code, certs) = run_check_json(&path);
    let cert = cert_for(&certs, "refuse");
    assert_ne!(
        code,
        Some(0),
        "AC-3: refuse (0/0 after exclusion) must STILL be gated; cert: {cert}"
    );
    assert_eq!(
        reject_cause(cert).as_deref(),
        Some("WeakContract"),
        "AC-3: refuse reduces to 0/0 → the #48 backstop STILL gates WeakContract \
         (no vacuous 1.0 pass); cert: {cert}"
    );
    assert_eq!(
        mutants_killed(cert),
        "0/0",
        "AC-3: the sole proved-equivalent survivor leaves the denominator EMPTY \
         (0/0), not a spurious 1/1; cert: {cert}"
    );
    let _ = std::fs::remove_file(&path);
}

/// AC-4: a strong contract whose mutants are genuinely KILLED is UNCHANGED — the
/// equivalence query runs ONLY on survivors, so a killed mutant is never excluded
/// and the verdict is the same as before #101.
#[test]
fn ac4_killed_mutants_unaffected() {
    if !verus_present() {
        eprintln!("SKIP ac4: verus absent");
        return;
    }
    let path = write_temp("add", ADD);
    let (code, certs) = run_check_json(&path);
    let cert = cert_for(&certs, "add");
    assert_eq!(
        code,
        Some(0),
        "AC-4: the strong `add` contract certifies L3 unchanged; cert: {cert}"
    );
    assert_eq!(level(cert), "L3", "AC-4: `add` certifies L3; cert: {cert}");
    let (killed, scored) = parse_ratio(&mutants_killed(cert));
    assert!(
        killed >= 1 && (killed as f64) / (scored as f64) >= 0.60,
        "AC-4: `add` kills its mutants at or above floor (unchanged); got {killed}/{scored}"
    );
    let _ = std::fs::remove_file(&path);
}

/// Determinism (REQ-6): scoring the SAME forced-output fixture twice yields the
/// byte-identical reduced `mutants_killed` (the exclusion verdict is a cached,
/// deterministic verus proof).
#[test]
fn req6_exclusion_is_deterministic() {
    if !verus_present() {
        eprintln!("SKIP req6: verus absent");
        return;
    }
    let path = write_temp("clamp_zero_det", CLAMP_ZERO);
    let (_, certs1) = run_check_json(&path);
    let (_, certs2) = run_check_json(&path);
    let mk1 = mutants_killed(cert_for(&certs1, "clamp_zero"));
    let mk2 = mutants_killed(cert_for(&certs2, "clamp_zero"));
    assert_eq!(
        mk1, mk2,
        "REQ-6: the reduced mutants_killed must be byte-identical across runs"
    );
    let _ = std::fs::remove_file(&path);
}
