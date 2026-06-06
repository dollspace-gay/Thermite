//! acto-critic divergence tests for `forge check` on Stage-2 recursion-scheme
//! corpus programs (commit `57d9727`, issue #70 / epic #62).
//!
//! These pin a divergence between the `forge` driver's emitted certificates for
//! `conformance/list_fold.th` and the hand-derived oracle
//! `conformance/adt-schemes/cases.json` (`certify` block). Expected item names +
//! levels trace to the oracle, NEVER to forge's own output (`goal.md` R-CHAR-3).
//!
//! They run the BUILT `forge` binary end-to-end (verus-backed). If verus is
//! absent they SKIP LOUDLY (never panic on a missing solver), matching
//! `divergence_forge.rs` / `check_conformance.rs`.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// `true` iff verus is reachable (mirrors `divergence_forge.rs`).
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

/// Locate the repo-root corpus file regardless of the test's CWD.
fn corpus(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .join("conformance")
        .join(name)
}

/// Run `forge check <path> --json`, returning the parsed array of certificates.
fn check_json(path: &Path) -> Vec<Value> {
    let out = Command::new(forge_bin())
        .arg("check")
        .arg(path)
        .arg("--json")
        .output()
        .expect("spawn forge");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!(
            "forge --json stdout must be one JSON doc: {e}\nstdout:\n{stdout}\nstderr:\n{}",
            String::from_utf8_lossy(&out.stderr)
        )
    });
    value.as_array().expect("array of certs").clone()
}

/// DIVERGENCE — `forge check conformance/list_fold.th` emits THREE certificates
/// for the SAME item name (`len_list`) instead of the three DISTINCT items the
/// oracle requires (`len_list`, `sum_list`, `all_positive`).
///
/// Root cause (observed): for an `Item::SpecFn`, `check::item_subprogram` builds
/// `{adt_decls} + {all spec_items}` — a sub-program that is IDENTICAL for every
/// spec fn in the file (the checked item is not distinguished; it just rides
/// along in `spec_items`). All three spec-fn sub-programs therefore lower to
/// byte-identical Verus, so the proof-cache content-address collides and the
/// stored `len_list` certificate is served for `sum_list` and `all_positive`
/// too. The `sum_list` and `all_positive` certificates are never produced under
/// their own identity.
///
/// Authority: `conformance/adt-schemes/cases.json` `certify` block —
/// `{ "name": "len_list", "level": "L3" }`, `{ "name": "sum_list", "level":
/// "L3" }`, `{ "name": "all_positive", "level": "L3" }` (three distinct items).
/// `.design/basis/02-recursion-schemes.md` AC-1 ("a fold scheme … certifies
/// L3", per the named instance) and `thermite-design.md` §6 ("the certificate …
/// lists every function's level") + §5.3 (per-item isolated sub-program). A
/// certificate whose `item` field is a NEIGHBOR's name violates the §6
/// per-function manifest contract (`goal.md` R-SPEC-2 — the cert is a contract).
/// Tracking: #71
#[test]
#[ignore = "blocker #71 — un-ignore when fixed"]
fn divergence_list_fold_three_distinct_item_certs() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — list_fold scheme cert identity not exercised.");
        return;
    }
    let certs = check_json(&corpus("list_fold.th"));

    // AUTHORITY (cases.json `certify`): the three spec-fn instances each carry
    // their OWN certificate, by name. The `enum List` decl also yields a cert;
    // we assert only that each of the three named spec fns is present with L3.
    for (name, level) in [
        ("len_list", "L3"),
        ("sum_list", "L3"),
        ("all_positive", "L3"),
    ] {
        let cert = certs
            .iter()
            .find(|c| c.get("item").and_then(|v| v.as_str()) == Some(name))
            .unwrap_or_else(|| {
                let names: Vec<&str> = certs
                    .iter()
                    .filter_map(|c| c.get("item").and_then(|v| v.as_str()))
                    .collect();
                panic!(
                    "oracle cases.json requires a certificate for `{name}` (level {level}); \
                     forge emitted certs for items {names:?} (the spec-fn sub-program collision \
                     serves the cached `len_list` cert for the siblings)"
                )
            });
        assert_eq!(
            cert.get("level").and_then(|v| v.as_str()),
            Some(level),
            "oracle: `{name}` certifies {level}; got {cert}"
        );
    }
}

/// COMPANION (un-ignored — a no-divergence guard for the crux). The corpus
/// `list_fold.th` certifies at project level L3: the generated `fold_list` /
/// `for_all_list` schemes + the materialized `fold_bound_list` law verify under
/// real verus. This pins that the scheme engine itself is sound end-to-end (the
/// divergence above is purely the per-item certificate IDENTITY, not a proof
/// failure). Authority: `conformance/adt-schemes/cases.json` `certify` (all L3).
#[test]
fn list_fold_project_assurance_is_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — list_fold project assurance not exercised.");
        return;
    }
    let certs = check_json(&corpus("list_fold.th"));
    // Every emitted certificate is L3 (the oracle certifies all items L3); no
    // item degrades. This holds even under the identity collision (the served
    // cert is itself a genuine L3), so it stays GREEN and is not the divergence.
    assert!(
        !certs.is_empty(),
        "forge must emit at least one certificate for list_fold.th"
    );
    for cert in &certs {
        assert_eq!(
            cert.get("level").and_then(|v| v.as_str()),
            Some("L3"),
            "oracle cases.json certifies every list_fold item L3; got {cert}"
        );
    }
}
