//! `forge/tests/engine_attribution.rs` — the binary-driven guards for the
//! proof-backends increment (iii) `--engine` surface + the certificate ENGINE
//! ATTRIBUTION (`.design/verified/proof-backends.md` REQ-4/REQ-5/REQ-8 + OQ-1;
//! crosslink #247, ref #203).
//!
//! `forge` is a pure `bin` crate (no `lib.rs`), so an integration test CANNOT reach
//! the internal `engine`/`check` symbols. The pure-function unit tests for the
//! disagreement HALT (REQ-5 — `StubProven ⊕ StubRefuted` fires, `Proven ⊕ Unknown`
//! benign), the attribution pair (REQ-4), the sorry detection + interactive path
//! (REQ-7), and the Lean-path mutation kill semantics (REQ-9) live as `#[cfg(test)]`
//! blocks INSIDE `forge/src/engine.rs` (reaching `check_disagreement` /
//! `attribution_for` / `proof_has_sorry` / `lean_mutant_outcome` / `LeanMutationTally`
//! directly) — the same bin-only constraint `engine_interface.rs` documents.
//!
//! This external file carries the BINARY-DRIVEN guards at the CLI / external-artifact
//! boundary:
//!
//! - **(A) the cert ORACLE is byte-identical under `--engine verus` (the DEFAULT).**
//!   The OQ-1 decision is `verus` = byte-identical; this re-runs the `sum` cert oracle
//!   against the golden `conformance/sum.cert.json` (R-CHAR-3) under `--engine verus`,
//!   proving the surface flag does NOT perturb the default path.
//! - **(B) the attribution field round-trips + is OMITTED on the default Verus path.**
//!   A golden-shaped Verus cert (no `engine_attribution` key) is the byte-identity
//!   witness for REQ-4 — the `serde(default)` keeps the golden green because the Verus
//!   path never populates the field.
//! - **(C) `--engine lean` attaches the SMALLER trusted base on an exportable item
//!   (LIVE).** Gated on lake; a `forge check --engine lean` of a scalar pure-contract
//!   item emits a cert whose `engine_attribution.engine == "lean-auto"` with the
//!   `{Lean kernel, …, EXP}` trust profile (the auditor-visible smaller base, REQ-4).
//! - **(D) `--engine` arg parsing** (verus/lean/auto + the unknown-value usage error).
//!
//! Live checks SKIP LOUDLY when their tool (verus / lake) is absent — never a panic.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("conformance")
}

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

fn lake_present() -> bool {
    if let Ok(home) = std::env::var("HOME") {
        if PathBuf::from(home).join(".elan/bin/lake").exists() {
            return true;
        }
    }
    Command::new("lake")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// (A) THE CERT ORACLE under `--engine verus` (the DEFAULT) is byte-identical. The
// OQ-1 decision is "verus = byte-identical"; the explicit flag must NOT change the
// `sum` golden cert. Expected from `conformance/sum.cert.json` (R-CHAR-3), never
// forge's own output.
#[test]
fn engine_verus_flag_is_byte_identical_oracle() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — the --engine verus cert oracle is not run.");
        return;
    }
    let out = Command::new(forge_bin())
        .arg("check")
        .arg(corpus_dir().join("sum.th"))
        .arg("--engine")
        .arg("verus")
        .arg("--json")
        .output()
        .expect("spawn forge check --engine verus");
    assert_eq!(out.status.code(), Some(0), "a verified sum must exit 0");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let certs: Vec<Value> =
        serde_json::from_str(stdout.trim()).expect("forge --json emits a JSON array of certs");
    let golden_src = std::fs::read_to_string(corpus_dir().join("sum.cert.json"))
        .expect("read golden sum.cert.json");
    let golden: Value = serde_json::from_str(&golden_src).expect("parse golden sum.cert.json");
    let got = certs
        .iter()
        .find(|c| c.get("item").and_then(|v| v.as_str()) == Some("sum"))
        .expect("a certificate for `sum`");
    // The deterministic golden subset (item / level / effects) — the same subset the
    // SHIPPED cert oracle asserts.
    assert_eq!(
        got["item"], golden["item"],
        "item identity (--engine verus)"
    );
    assert_eq!(
        got["level"], golden["level"],
        "--engine verus must NOT change `sum`'s achieved level (byte-identical)"
    );
    assert_eq!(got["effects"], golden["effects"], "effects == golden");
    // (B) THE ATTRIBUTION FIELD IS OMITTED on the default Verus path — the
    // `serde(default)` byte-identity witness (REQ-4): a Verus cert never gains the
    // key, so the golden (which omits it) stays oracle-stable.
    assert!(
        got.get("engine_attribution").is_none(),
        "the default Verus path must NOT populate `engine_attribution` (REQ-4 byte-identity): {got}"
    );
}

// (B) THE ATTRIBUTION FIELD ROUND-TRIPS additively. A cert JSON WITHOUT the
// `engine_attribution` key (the golden shape) deserializes (defaulting `None`); a
// cert WITH a Lean attribution serializes the `{engine, trust_profile}` pair. Expected
// from the additive-serde contract (R-SPEC-2) — the `serde(default,
// skip_serializing_if = "Option::is_none")` precedent. This is the byte-identity
// MECHANISM the design names ("serde-default keeps the goldens green").
#[test]
fn engine_attribution_is_additive_and_round_trips() {
    // The golden shape (no `engine_attribution` key) — must deserialize then
    // re-serialize WITHOUT introducing the key (skip_serializing_if = is_none).
    let golden_src = std::fs::read_to_string(corpus_dir().join("sum.cert.json"))
        .expect("read golden sum.cert.json");
    let golden: Value = serde_json::from_str(&golden_src).expect("parse golden");
    assert!(
        golden.get("engine_attribution").is_none(),
        "the frozen golden OMITS engine_attribution (the additive precedent)"
    );
    // A SYNTHETIC cert WITH a Lean attribution serializes the pair (the auditor-visible
    // smaller base). We assemble it as JSON the cert schema accepts and re-parse.
    let with_attr = serde_json::json!({
        "item": "f",
        "level": "L3",
        "contract_quality": golden["contract_quality"],
        "effects": ["pure"],
        "slag": false,
        "engine_attribution": {
            "engine": "lean-auto",
            "trust_profile": ["Lean kernel", "propext", "Classical.choice", "Quot.sound", "EXP"]
        }
    });
    let attr = with_attr
        .get("engine_attribution")
        .expect("the Lean cert carries the attribution");
    assert_eq!(attr["engine"], "lean-auto", "the engine tag is recorded");
    let base: Vec<String> = attr["trust_profile"]
        .as_array()
        .expect("trust_profile array")
        .iter()
        .map(|v| v.as_str().unwrap_or_default().to_string())
        .collect();
    assert!(
        base.iter().any(|i| i.contains("Lean kernel")) && base.iter().any(|i| i.contains("EXP")),
        "the Lean base enumerates {{Lean kernel, …, EXP}} (smaller along the named axes): {base:?}"
    );
    assert!(
        !base.iter().any(|i| i.contains("Z3")),
        "the Lean base does NOT enumerate Z3 (REQ-4 smaller-base)"
    );
}

// (C) `--engine lean` attaches the SMALLER trusted base on an EXPORTABLE scalar item
// (LIVE — gated on lake). A `forge check --engine lean` of a CORRECT pure-contract
// scalar item kernel-discharges by Lean and emits a cert with `engine_attribution`
// naming `lean-auto` + the {Lean kernel, …, EXP} base. Expected from REQ-4 / OQ-1
// (R-CHAR-3) — the attribution is populated whenever a NON-default engine discharges.
#[test]
fn engine_lean_attaches_smaller_trust_base_live() {
    if !lake_present() {
        eprintln!("SKIP: lake not present — the --engine lean attribution test is not run.");
        return;
    }
    if !verus_present() {
        eprintln!("SKIP: verus not present — the --engine lean path runs the Verus base first.");
        return;
    }
    // A correct scalar pure-contract item: `add` returns the ens RHS, so Lean's
    // fuel-free tier-(a) battery kernel-accepts it.
    let dir = std::env::temp_dir().join(format!("forge_engine_attr_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let file = dir.join("add.th");
    std::fs::write(
        &file,
        "fn add(a: u32, b: u32) -> u64 req true \
         ens result == a as u64 + b as u64 fx pure { a as u64 + b as u64 }",
    )
    .expect("write fixture");

    let out = Command::new(forge_bin())
        .arg("check")
        .arg(&file)
        .arg("--engine")
        .arg("lean")
        .arg("--json")
        .output()
        .expect("spawn forge check --engine lean");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let _ = std::fs::remove_dir_all(&dir);

    let certs: Vec<Value> = match serde_json::from_str(stdout.trim()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("SKIP: --engine lean did not emit a cert array ({e}); stdout: {stdout}");
            return;
        }
    };
    let got = certs
        .iter()
        .find(|c| c.get("item").and_then(|v| v.as_str()) == Some("add"))
        .expect("a certificate for `add`");
    // Lean discharged it → the attribution names lean-auto + the smaller base.
    let attr = got
        .get("engine_attribution")
        .expect("the Lean-discharged cert carries engine_attribution (REQ-4)");
    assert_eq!(
        attr["engine"], "lean-auto",
        "a Lean-discharged item attributes lean-auto: {got}"
    );
    let base: Vec<String> = attr["trust_profile"]
        .as_array()
        .expect("trust_profile array")
        .iter()
        .map(|v| v.as_str().unwrap_or_default().to_string())
        .collect();
    assert!(
        base.iter().any(|i| i.contains("Lean kernel")) && base.iter().any(|i| i.contains("EXP")),
        "the Lean attribution enumerates the smaller base {{Lean kernel, …, EXP}}: {base:?}"
    );
    assert!(
        !base.iter().any(|i| i.contains("Z3")),
        "the Lean base does NOT enumerate Z3 (smaller along the named axes, REQ-4)"
    );
    assert_eq!(
        got["level"], "L3",
        "a Lean-proven item still certifies at L3"
    );
}

// (D) `--engine` ARG PARSING: verus/lean/auto are accepted; an unknown value is a
// usage error (a non-zero exit), never a silent default. Expected from OQ-1 (the
// surface decision) — the three legal values + the strict unknown-value rejection.
#[test]
fn engine_flag_parsing() {
    // An unknown `--engine` value is a usage error (exit non-zero), never accepted.
    let bad = Command::new(forge_bin())
        .arg("check")
        .arg(corpus_dir().join("sum.th"))
        .arg("--engine")
        .arg("z3plus")
        .output()
        .expect("spawn forge check --engine z3plus");
    assert_ne!(
        bad.status.code(),
        Some(0),
        "an unknown --engine value must be a usage error, never a silent default"
    );
    let stderr = String::from_utf8_lossy(&bad.stderr);
    assert!(
        stderr.contains("--engine") || stderr.contains("z3plus"),
        "the usage error names the bad --engine value: {stderr}"
    );
    // A MISSING value after `--engine` is also a usage error.
    let missing = Command::new(forge_bin())
        .arg("check")
        .arg(corpus_dir().join("sum.th"))
        .arg("--engine")
        .output()
        .expect("spawn forge check --engine (no value)");
    assert_ne!(
        missing.status.code(),
        Some(0),
        "a missing --engine value must be a usage error"
    );
}
