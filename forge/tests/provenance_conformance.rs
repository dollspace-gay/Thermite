//! Stage 6 — security-by-construction / information-flow control (IFC) cert
//! oracle (`.design/basis/06-provenance-and-sinks.md` AC-1/AC-2/AC-3/AC-5;
//! `conformance/provenance/cases.json` + `conformance/provenance_demo.th`).
//!
//! This drives the BUILT `forge` binary with `check --json` / `audit --json` over
//! the READ-ONLY corpus program `conformance/provenance_demo.th` (NEVER copied or
//! edited — R-CHAR-3) and asserts the v1 TYPE-LEVEL IFC guarantee, which is
//! EMERGENT from SHIPPED machinery (Stage-1 ADT wrapper structs + the `#[boundary]`
//! door/sink form + the #52 compose-through + the existing lower→verus type-check)
//! — NO new toolchain code. The expected levels/scopes/TCB are hand-derived in
//! `cases.json` from the flow rules + verus/type semantics, NEVER copied from
//! forge's own output (R-CHAR-3).
//!
//! THE CENTERPIECE (the security guarantee, asserted STRONGLY): the three careless
//! / dipshit paths — `careless_query` (raw `Tainted` → `query(Sql)`), `leak` (raw
//! `Secret` → `emit(Public)`), `unauth_delete` (raw `User` → `delete(Authorized)`)
//! — do NOT certify. Each is a TYPE MISMATCH the lower→verus type-check rejects
//! (`error[E0308]: expected <clean>, found <marked>`), so the cert is `L0` and the
//! function NEVER reaches `L3`. SQL-injection / secret-leak / missing-authz are
//! un-typeable: the dipshit path does not compile to a verified artifact. The SAME
//! flows routed through a door (`parameterize` / `declassify` / `authorize`)
//! type-check and certify `L3` to-the-boundary. `forge audit` enumerates the doors
//! as the grep-complete security TCB.
//!
//! The mark-PROPAGATION engine (taint through DERIVED values — `let y = f(x)`) is
//! v1.1 (REQ-4, AC-4): a NEW validator-dataflow pass, NOT built here and NOT
//! exercised by this oracle (the v1 type-level slice rejects a DIRECT
//! `query(input)` by the sink's parameter type alone — GROUNDED).
//!
//! These run a real verus proof (the doored callers L3-prove against the assumed
//! door contracts), so they SKIP LOUDLY if verus is absent — never panic on a
//! missing solver, mirroring `composition_conformance.rs` / `audit_conformance.rs`.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

fn conformance_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("conformance")
}

/// The READ-ONLY corpus program all axes live in (R-CHAR-3 — never edited).
fn demo_path() -> PathBuf {
    conformance_dir().join("provenance_demo.th")
}

/// Load the IFC cases oracle JSON (R-CHAR-3 external truth — hand-derived).
fn cases() -> Value {
    let path = conformance_dir().join("provenance").join("cases.json");
    let src = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read provenance cases.json: {e}"));
    serde_json::from_str(&src).unwrap_or_else(|e| panic!("parse provenance cases.json: {e}"))
}

/// `true` iff verus can be located — mirrors `composition_conformance.rs`. The
/// doored callers L3-prove against the assumed door contracts, so the prover must
/// be present; `forge check` also resolves the verus version up-front for the
/// proof cache.
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

/// Run `forge check <file> --json`, returning (exit_code, cert array, stderr). The
/// careless paths make the project FAIL (exit 1), so the caller asserts the
/// per-item levels, not the project exit alone.
fn run_check_json(file: &Path) -> (Option<i32>, Vec<Value>, String) {
    let out = Command::new(forge_bin())
        .arg("check")
        .arg(file)
        .arg("--json")
        .output()
        .unwrap_or_else(|e| panic!("spawn forge: {e}"));
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let certs = serde_json::from_str::<Value>(stdout.trim())
        .unwrap_or_else(|e| {
            panic!("forge check --json must be one JSON doc: {e}\nstdout:\n{stdout}")
        })
        .as_array()
        .cloned()
        .unwrap_or_else(|| panic!("forge check --json must be a cert array\nstdout:\n{stdout}"));
    (out.status.code(), certs, stderr)
}

/// Run `forge audit <file> --json`, returning (exit_code, manifest, stderr).
fn run_audit_json(file: &Path) -> (Option<i32>, Value, String) {
    let out = Command::new(forge_bin())
        .arg("audit")
        .arg(file)
        .arg("--json")
        .output()
        .unwrap_or_else(|e| panic!("spawn forge: {e}"));
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let manifest = serde_json::from_str::<Value>(stdout.trim()).unwrap_or_else(|e| {
        panic!("forge audit --json must be one JSON doc: {e}\nstdout:\n{stdout}")
    });
    (out.status.code(), manifest, stderr)
}

fn find_cert<'a>(certs: &'a [Value], item: &str) -> &'a Value {
    certs
        .iter()
        .find(|c| c.get("item").and_then(|v| v.as_str()) == Some(item))
        .unwrap_or_else(|| {
            let names: Vec<&str> = certs
                .iter()
                .filter_map(|c| c.get("item").and_then(|v| v.as_str()))
                .collect();
            panic!("no certificate for item `{item}` in {names:?}")
        })
}

/// Assert a careless / dipshit path is REJECTED at the TYPE level: `L0`, NEVER
/// `L3` (the security guarantee), with a FAILED obligation whose verus diagnostic
/// is the `E0308` type mismatch (the marked type is not the clean type, and only
/// the door produces the clean type). `marked`/`clean` are the oracle's expected
/// type-mismatch operands.
fn assert_careless_rejected(certs: &[Value], item: &str) {
    let cert = find_cert(certs, item);

    // The CENTERPIECE assertion — the dipshit path does NOT certify and NEVER
    // reaches L3 (un-typeable: SQLi / secret-leak / missing-authz cannot compile to
    // a verified artifact). Hand-derived oracle: expect_level == "L0".
    assert_eq!(
        cert["level"],
        Value::from("L0"),
        "`{item}` is the careless/dipshit path: a raw marked value at a clean-type \
         sink → TYPE MISMATCH → L0 (does NOT certify)"
    );
    assert_ne!(
        cert["level"],
        Value::from("L3"),
        "SECURITY GUARANTEE: the careless `{item}` MUST NEVER reach L3 — a certified \
         dipshit path would be a critical IFC hole"
    );

    // It is rejected by the lower→verus TYPE-CHECK (a FAILED obligation), NOT a
    // §7.1 triage reject (the careless body is well-formed; the type is the rule).
    let obligations = cert["obligations"].as_array().expect("obligations array");
    let failed = obligations
        .iter()
        .find(|o| o.get("status").and_then(|v| v.as_str()) == Some("failed"))
        .unwrap_or_else(|| {
            panic!("`{item}` must carry a FAILED verus obligation: {obligations:?}")
        });
    let diag = failed
        .get("diagnostic")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("`{item}` failed obligation must carry a verus diagnostic"));
    assert!(
        diag.contains("E0308") && diag.contains("mismatched types"),
        "`{item}` is rejected by the verus TYPE-CHECK (E0308 mismatched types — the \
         marked type is not the clean type the sink demands); diagnostic:\n{diag}"
    );
}

/// Assert a safe / doored path CERTIFIES `L3` to-the-boundary via the named sink:
/// the marked value is laundered to the clean type through the door, the sink
/// accepts it, and the caller proves THROUGH the door+sink contracts.
fn assert_safe_certifies(certs: &[Value], item: &str, expect_via: &str) {
    let cert = find_cert(certs, item);
    assert_eq!(
        cert["level"],
        Value::from("L3"),
        "`{item}` routes the marked value through its door → type-checks → proves \
         L3 (the default floor; the equality contract is mutation-killable)"
    );
    // #17 scope ⊥ level: L3 AND to_boundary via the reached sink/door.
    assert_eq!(
        cert["assurance_scope"]["kind"],
        Value::from("to_boundary"),
        "`{item}` is verified to-the-boundary (its closure crosses a door/sink)"
    );
    assert_eq!(
        cert["assurance_scope"]["via"],
        Value::from(expect_via),
        "`{item}` records the oracle crossing `via` (`{expect_via}`)"
    );
}

/// Iterate an oracle axis (an array of `{name, expect_level, scope?}` cases),
/// asserting careless (`L0`) vs safe (`L3` to_boundary) per the hand-derived oracle.
/// `expect_via` maps a safe case's fn to its reached sink/door (the #17 crossing).
fn assert_axis(certs: &[Value], axis: &Value, expect_via: &dyn Fn(&str) -> &'static str) {
    for case in axis.as_array().expect("axis is a cases array") {
        let name = case["name"].as_str().expect("case name");
        let expect_level = case["expect_level"].as_str().expect("expect_level");
        match expect_level {
            "L0" => assert_careless_rejected(certs, name),
            "L3" => {
                assert_eq!(
                    case["scope"].as_str(),
                    Some("to_boundary"),
                    "the oracle's safe `{name}` is to_boundary"
                );
                assert_safe_certifies(certs, name, expect_via(name));
            }
            other => panic!("unexpected oracle expect_level `{other}` for `{name}`"),
        }
    }
}

// AC-1 (the CENTERPIECE — the SQLi program does NOT compile): `careless_query`
// (raw `Tainted` → `query(Sql)`) is `L0` (E0308: expected Sql, found Tainted),
// NEVER L3; `safe_query` (through `parameterize`) is `L3` to_boundary via `query`.
// Anchored to `cases.json` `centerpiece_sqli` (R-CHAR-3).
#[test]
fn centerpiece_sqli_careless_is_l0_safe_is_l3() {
    if !verus_present() {
        eprintln!(
            "SKIP: verus not available — the SQLi un-typeable centerpiece is a real \
             verus type-check + proof, not run."
        );
        return;
    }
    let oracle = cases();
    let (_code, certs, stderr) = run_check_json(&demo_path());
    assert!(
        !certs.is_empty(),
        "forge check emitted certs over the demo; stderr:\n{stderr}"
    );
    assert_axis(&certs, &oracle["centerpiece_sqli"], &|name| match name {
        "safe_query" => "query",
        other => panic!("no via mapping for safe sqli case `{other}`"),
    });
}

// AC-2 (a `Secret` reaching `emit` does NOT compile): `leak` (raw `Secret` →
// `emit(Public)`) is `L0` (E0308: expected Public, found Secret), NEVER L3;
// `safe_emit` (through `declassify`) is `L3` to_boundary via `emit`. Anchored to
// `cases.json` `secret_leak` (R-CHAR-3).
#[test]
fn secret_leak_careless_is_l0_safe_is_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — the secret-leak axis not run.");
        return;
    }
    let oracle = cases();
    let (_code, certs, stderr) = run_check_json(&demo_path());
    assert!(
        !certs.is_empty(),
        "forge check emitted certs; stderr:\n{stderr}"
    );
    assert_axis(&certs, &oracle["secret_leak"], &|name| match name {
        "safe_emit" => "emit",
        other => panic!("no via mapping for safe secret case `{other}`"),
    });
}

// AC-3 (a protected op without `Authorized` does NOT compile): `unauth_delete`
// (raw `User` → `delete(Authorized)`) is `L0` (E0308: expected Authorized, found
// User), NEVER L3; `safe_delete` (through `authorize`) is `L3` to_boundary via
// `delete`. Anchored to `cases.json` `missing_capability` (R-CHAR-3).
#[test]
fn missing_capability_careless_is_l0_safe_is_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — the capability axis not run.");
        return;
    }
    let oracle = cases();
    let (_code, certs, stderr) = run_check_json(&demo_path());
    assert!(
        !certs.is_empty(),
        "forge check emitted certs; stderr:\n{stderr}"
    );
    assert_axis(&certs, &oracle["missing_capability"], &|name| match name {
        "safe_delete" => "delete",
        other => panic!("no via mapping for safe capability case `{other}`"),
    });
}

// AC-1/AC-2/AC-3 cross-axis: ALL THREE careless paths are L0, NONE is L3 — the
// strong, consolidated security guarantee (one assertion that the dipshit path is
// un-certifiable across every axis). Hand-derived from `cases.json` (the L0 cases).
#[test]
fn no_careless_path_ever_certifies() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — the cross-axis security guarantee not run.");
        return;
    }
    let oracle = cases();
    let (_code, certs, stderr) = run_check_json(&demo_path());
    assert!(
        !certs.is_empty(),
        "forge check emitted certs; stderr:\n{stderr}"
    );

    let mut careless: Vec<&str> = Vec::new();
    for axis in ["centerpiece_sqli", "secret_leak", "missing_capability"] {
        for case in oracle[axis].as_array().expect("axis array") {
            if case["expect_level"].as_str() == Some("L0") {
                careless.push(case["name"].as_str().expect("name"));
            }
        }
    }
    // The oracle declares exactly the three dipshit paths (one per axis).
    assert_eq!(
        careless.len(),
        3,
        "the oracle declares exactly the three careless paths (taint/secret/capability)"
    );
    for item in &careless {
        let cert = find_cert(&certs, item);
        assert_ne!(
            cert["level"],
            Value::from("L3"),
            "SECURITY GUARANTEE: the careless `{item}` MUST NEVER certify L3 \
             (a certified dipshit path is a critical IFC hole)"
        );
        assert_eq!(
            cert["level"],
            Value::from("L0"),
            "the careless `{item}` is L0 — the forbidden flow does not compile"
        );
    }
}

// AC-5 (the doors/sinks are the security TCB): the six doors/sinks
// (parameterize/query/declassify/emit/authorize/delete) each certify `L1` +
// boundary (foreign bodies, assumed contracts); `forge audit` enumerates them in
// the `tcb` `boundary_contracts`, and the THREE laundering doors
// (parameterize/declassify/authorize) are present — `grep declassify` = the
// manifest's secret-release list. Anchored to `cases.json`
// `doors_and_sinks_are_tcb` (R-CHAR-3).
#[test]
fn doors_and_sinks_are_l1_boundary_and_the_audit_tcb() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — the door/sink TCB oracle not run.");
        return;
    }
    let oracle = cases();
    let case = oracle["doors_and_sinks_are_tcb"]
        .as_array()
        .expect("doors_and_sinks_are_tcb array")
        .iter()
        .find(|c| c["name"] == "boundary_doors")
        .expect("boundary_doors case")
        .clone();
    let expect_l1: Vec<&str> = case["expect_l1_boundary"]
        .as_array()
        .expect("expect_l1_boundary")
        .iter()
        .map(|v| v.as_str().expect("door name"))
        .collect();
    let expect_tcb: Vec<&str> = case["expect_tcb_boundary_contains"]
        .as_array()
        .expect("expect_tcb_boundary_contains")
        .iter()
        .map(|v| v.as_str().expect("tcb name"))
        .collect();

    // Each door/sink certifies L1 + boundary (the §16 path, not L3 — no verus on a
    // foreign body).
    let (_code, certs, stderr) = run_check_json(&demo_path());
    assert!(
        !certs.is_empty(),
        "forge check emitted certs; stderr:\n{stderr}"
    );
    for door in &expect_l1 {
        let cert = find_cert(&certs, door);
        assert_eq!(
            cert["level"],
            Value::from("L1"),
            "the door/sink `{door}` certifies L1 (foreign body, assumed contract)"
        );
        assert_eq!(
            cert["boundary"],
            Value::from(true),
            "the door/sink `{door}` is a boundary fn"
        );
    }

    // `forge audit` enumerates the doors as the security TCB. The demo file holds
    // BOTH the safe (doored) and the careless paths, so the PROJECT assurance is
    // FAILED (the careless fns are L0) and `forge audit` exits 1 — but it still
    // emits the complete manifest on stdout, and the door enumeration is what the
    // TCB oracle asserts (NOT a clean project exit; the careless paths SHOULD fail).
    let (_audit_code, manifest, _audit_stderr) = run_audit_json(&demo_path());
    assert_eq!(
        manifest["manifest_version"],
        Value::from("v1"),
        "forge audit emits the v1 manifest even when careless fns fail the project"
    );
    assert_eq!(
        manifest["project_assurance"]["level"]["kind"],
        Value::from("failed"),
        "the demo project assurance is FAILED — the careless paths do not certify \
         (the security guarantee at the project level)"
    );
    let bnd_names: Vec<String> = manifest["tcb"]["boundary_contracts"]
        .as_array()
        .expect("boundary_contracts array")
        .iter()
        .map(|c| c["name"].as_str().expect("bnd name").to_string())
        .collect();
    // The grep-complete security TCB enumerates the laundering doors (R-DEFER-9 —
    // nothing fiat-trusted omitted; `grep declassify` = the manifest's list).
    for door in &expect_tcb {
        assert!(
            bnd_names.iter().any(|n| n == door),
            "the audit TCB `boundary_contracts` must enumerate the door `{door}` \
             (the audited security trust point); got {bnd_names:?}"
        );
    }
    // The enumerated `declassify` records its contract (name + target + req + ens +
    // fx) — every secret-release is a named, contracted door.
    let declassify = manifest["tcb"]["boundary_contracts"]
        .as_array()
        .expect("boundary_contracts array")
        .iter()
        .find(|c| c["name"] == "declassify")
        .expect("declassify door in the TCB");
    assert_eq!(
        declassify["target"],
        Value::from("ifc::declassify"),
        "the declassify door records its foreign target"
    );
}
