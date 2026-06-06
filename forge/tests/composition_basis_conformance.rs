//! Stage 5 — compositional reasoning cert oracle (Basis epic #62, crosslink #75;
//! `.design/basis/05-composition.md`). This is a CONFORMANCE DEMONSTRATION: it
//! confirms the SHIPPED machinery (#52 contract composition, #15/#60 assurance
//! aggregation) COMPOSES the basis stages, against the hand-derived oracle
//! `conformance/composition-basis/cases.json` (R-CHAR-3 — expected values trace to
//! the oracle + `.design/basis/05-composition.md` §9, NEVER copied from forge's
//! own output). NO new toolchain code: the SHIPPED laws already carry the
//! capability; this test is the evidence row per the doc's TEST-FIRST resolution.
//!
//! The four laws exercised over `conformance/compose_demo.th`:
//!
//! - LAW 1 — CONTRACT composition (#52, REQ-1): `pipeline = step_double(step_inc(x))`
//!   discharges LOCALLY — `step_inc`'s `ens result == x + 1` (with `x < 99`)
//!   establishes `step_double`'s `req y < 100`, and `pipeline` proves its own `ens
//!   result == 2 * (x + 1)` using ONLY the two callees' contracts, never their
//!   bodies. `pipeline`/`step_inc`/`step_double` certify L3, end_to_end, pure.
//!
//! - LAW 1 through a BOUNDARY (#52, REQ-1): `read_then_inc` composes THROUGH the
//!   `#[boundary] read_small` (`external_body` assumable signature): it discharges
//!   `read_small`'s contract and proves its own `ens` from the assumed
//!   `Some(v) => v < 256`. Certifies L3, to_boundary, via `read_small` (the boundary
//!   itself stays L1). The shape-only effect contract has an unconstrained `None`
//!   arm, so the oracle pins `--mutation-floor 0` (Stage 3 caveat); the COMPOSITION
//!   is sound regardless (the woven contract resolves the call).
//!
//! - LAWS 2 & 3 — ASSURANCE + TCB aggregation (#15/#60, REQ-2/REQ-3): `forge audit`
//!   over the file (which mixes an end_to_end part `pipeline` + a to_boundary part
//!   `read_then_inc`) reports the PROJECT scope as the genuine MIN — `to_boundary`
//!   listing the `read_small` crossing, NEVER over-claimed as end_to_end — and the
//!   TCB enumerates `read_small`'s boundary contract ∪ the toolchain (R-DEFER-9).
//!
//! Scheme-fusion (REQ-5) + invariant-conjunction (REQ-6) are NOT exercised here:
//! the basis oracle carries no fold/`Vec<Account>` case, and a probe against the
//! existing machinery shows they await their prerequisite stages (Stage 2 scheme
//! lowering, Stage 1/4 ADT/collection lowering) — they stay NOT-STARTED.
//!
//! These SKIP LOUDLY if verus is absent (a real verus proof underlies each cert),
//! mirroring `composition_conformance.rs` / `audit_conformance.rs`.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// The Stage-5 oracle (R-CHAR-3 external truth) — `conformance/composition-basis/cases.json`.
fn oracle() -> Value {
    let path = repo_root()
        .join("conformance")
        .join("composition-basis")
        .join("cases.json");
    let src = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read composition-basis cases.json: {e}"));
    serde_json::from_str(&src).unwrap_or_else(|e| panic!("parse composition-basis cases.json: {e}"))
}

/// The Stage-5 demo program — `conformance/compose_demo.th` (the orchestrator's
/// read-only fixture; NEVER edited, R-CHAR-3).
fn compose_demo() -> PathBuf {
    repo_root().join("conformance").join("compose_demo.th")
}

/// `true` iff verus can be located — mirrors `composition_conformance.rs`. The
/// Stage-5 composition certs are real verus proofs (each step L3-proves through
/// the next step's contract), so the prover must be present.
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

/// Run `forge check <file> --json --mutation-floor 0`, returning (exit_code, cert
/// array, stderr). The oracle's `read_then_inc` carries `"mutation_floor": 0.0`
/// (the Stage-3 shape-only effect contract leaves the `None` arm unconstrained),
/// so the demonstration runs at the floor the oracle pins — the COMPOSITION law
/// (the woven contract resolving the boundary call) is what is under test.
fn run_check_floor0(file: &Path) -> (Option<i32>, Vec<Value>, String) {
    let out = Command::new(forge_bin())
        .arg("check")
        .arg(file)
        .arg("--json")
        .arg("--mutation-floor")
        .arg("0")
        .output()
        .unwrap_or_else(|e| panic!("spawn forge check: {e}"));
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let certs = serde_json::from_str::<Value>(stdout.trim())
        .ok()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();
    (out.status.code(), certs, stderr)
}

/// Run `forge audit <file> --json`, returning (exit_code, manifest, stderr). The
/// audit runs at the pinned default budget (the `--mutation-floor` lever is NOT
/// exposed on `audit` by design — `forge/src/cli.rs` `run_audit`); the oracle's
/// `project_aggregation` row pins only the project SCOPE + the TCB crossing, which
/// hold at the default floor.
fn run_audit(file: &Path) -> (Option<i32>, Value, String) {
    let out = Command::new(forge_bin())
        .arg("audit")
        .arg(file)
        .arg("--json")
        .output()
        .unwrap_or_else(|e| panic!("spawn forge audit: {e}"));
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let manifest = serde_json::from_str::<Value>(stdout.trim()).unwrap_or_else(|e| {
        panic!("forge audit --json must be one JSON doc: {e}\nstdout:\n{stdout}\nstderr:\n{stderr}")
    });
    (out.status.code(), manifest, stderr)
}

fn find_cert<'a>(certs: &'a [Value], item: &str) -> &'a Value {
    certs
        .iter()
        .find(|c| c.get("item").and_then(|v| v.as_str()) == Some(item))
        .unwrap_or_else(|| panic!("no certificate for item `{item}` in {certs:?}"))
}

/// The set of effect labels a cert records, as owned strings (oracle compares as a
/// set; `compose_demo.th` uses a single label per fn).
fn effects(cert: &Value) -> Vec<String> {
    cert["effects"]
        .as_array()
        .expect("effects array")
        .iter()
        .map(|e| e.as_str().expect("effect label").to_string())
        .collect()
}

// LAW 1 (CONTRACT composition, #52 REQ-1): the pure pipeline composes LOCALLY.
// `pipeline = step_double(step_inc(x))` proves L3 + end_to_end + pure using ONLY
// each callee's contract (step_inc's `ens` discharges step_double's `req`); the
// two leaves are L3. Anchored to the oracle's `contract_composition` array.
#[test]
fn contract_composition_pipeline_discharges_locally() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — Stage-5 contract-composition oracle not run.");
        return;
    }
    let oracle = oracle();
    let (code, certs, stderr) = run_check_floor0(&compose_demo());
    assert_eq!(
        code,
        Some(0),
        "compose_demo.th certifies at --mutation-floor 0 (the pure closure + the \
         to-boundary chain all reach L3); stderr:\n{stderr}"
    );

    for case in oracle["contract_composition"]
        .as_array()
        .expect("contract_composition array")
    {
        let name = case["name"].as_str().expect("name");
        let expect_level = case["level"].as_str().expect("level");
        let expect_effects = case["effects"]
            .as_array()
            .expect("effects")
            .iter()
            .map(|e| e.as_str().expect("effect").to_string())
            .collect::<Vec<_>>();

        let cert = find_cert(&certs, name);
        assert_eq!(
            cert["level"],
            Value::from(expect_level),
            "`{name}` certifies {expect_level} (composition discharges through the contracts, #52)"
        );
        assert_eq!(
            effects(cert),
            expect_effects,
            "`{name}` records the oracle effect row {expect_effects:?}"
        );

        // The pure closure reaches NO boundary/slag → END-TO-END (the §9
        // "verified, period" scope). The oracle pins `scope: end_to_end` for the
        // top-level `pipeline`; the leaves omit `scope` (an end-to-end default).
        if let Some(expect_scope) = case.get("scope").and_then(|v| v.as_str()) {
            assert_eq!(expect_scope, "end_to_end", "oracle pins end_to_end here");
            let scope_e2e = match cert.get("assurance_scope") {
                None | Some(Value::Null) => true,
                Some(s) => s.get("kind").and_then(|v| v.as_str()) == Some("end_to_end"),
            };
            assert!(
                scope_e2e,
                "`{name}` is END-TO-END (its closure reaches no boundary/slag): {cert:?}"
            );
        }
        assert_eq!(
            cert["boundary"],
            Value::from(false),
            "`{name}` is not a boundary fn (a pure composed step)"
        );
    }
}

// LAW 1 through a BOUNDARY (#52 REQ-1) + the per-fn scope: `read_then_inc`
// composes THROUGH `read_small`. It certifies L3 (proving its own `ens` from the
// assumed boundary `ens`, never re-proving the foreign body) with scope
// to_boundary via `read_small`; the boundary fn itself stays L1 + boundary.
// Anchored to the oracle's `boundary_composition_and_aggregation` array.
#[test]
fn boundary_composition_read_then_inc_composes_through_the_boundary() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — Stage-5 boundary-composition oracle not run.");
        return;
    }
    let oracle = oracle();
    let (code, certs, stderr) = run_check_floor0(&compose_demo());
    assert_eq!(
        code,
        Some(0),
        "compose_demo.th certifies; stderr:\n{stderr}"
    );

    for case in oracle["boundary_composition_and_aggregation"]
        .as_array()
        .expect("boundary_composition_and_aggregation array")
    {
        let name = case["name"].as_str().expect("name");
        let expect_level = case["level"].as_str().expect("level");
        let cert = find_cert(&certs, name);

        assert_eq!(
            cert["level"],
            Value::from(expect_level),
            "`{name}` certifies {expect_level}"
        );

        if let Some(expect_via) = case.get("via").and_then(|v| v.as_str()) {
            // The caller crosses a boundary → to_boundary via the crossing (#17).
            assert_eq!(
                cert["assurance_scope"]["kind"],
                Value::from("to_boundary"),
                "`{name}` is to_boundary (its closure crosses `{expect_via}`)"
            );
            assert_eq!(
                cert["assurance_scope"]["via"],
                Value::from(expect_via),
                "`{name}` records the oracle crossing `{expect_via}`"
            );
            // L3 THROUGH the contract: the caller is fully proved, NOT laundered —
            // it has a discharged obligation, no failed obligation.
            assert_eq!(
                cert["boundary"],
                Value::from(false),
                "`{name}` is the composing caller (a regular fn, not the boundary itself)"
            );
            let obligations = cert["obligations"].as_array().expect("obligations array");
            assert!(
                obligations
                    .iter()
                    .all(|o| o["status"].as_str() != Some("failed")),
                "`{name}` carries NO failed obligation — it proves THROUGH the contract (#52)"
            );
        }

        if case.get("boundary").and_then(|v| v.as_bool()) == Some(true) {
            // The boundary primitive stays L1 + boundary (the §16 path, untouched).
            assert_eq!(
                cert["boundary"],
                Value::from(true),
                "`{name}` keeps boundary == true (the effect primitive)"
            );
            assert_eq!(
                cert["level"],
                Value::from("L1"),
                "the boundary primitive `{name}` stays L1 (contract enforced, body trusted)"
            );
        }
    }
}

// LAWS 2 & 3 (ASSURANCE + TCB aggregation, #15/#60 REQ-2/REQ-3): the audit
// manifest aggregates the project HONESTLY. compose_demo.th mixes an end_to_end
// part (`pipeline`) and a to_boundary part (`read_then_inc`), so the PROJECT scope
// is the genuine MIN — `to_boundary` listing the `read_small` crossing, NEVER
// over-claimed as end_to_end — and the TCB enumerates `read_small`'s boundary
// contract ∪ the toolchain (nothing fiat-trusted omitted, R-DEFER-9). Anchored to
// the oracle's `project_aggregation` array.
#[test]
fn project_aggregation_is_the_honest_min_over_parts() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — Stage-5 project-aggregation oracle not run.");
        return;
    }
    let oracle = oracle();
    let case = oracle["project_aggregation"]
        .as_array()
        .expect("project_aggregation array")
        .first()
        .expect("one project_aggregation case");
    let expect_scope = case["expect_project_scope"]
        .as_str()
        .expect("project scope");
    let expect_tcb_boundary = case["expect_tcb_boundary_contains"]
        .as_array()
        .expect("expect_tcb_boundary_contains")
        .iter()
        .map(|v| v.as_str().expect("boundary name").to_string())
        .collect::<Vec<_>>();

    let (_code, manifest, stderr) = run_audit(&compose_demo());
    assert!(
        stderr.is_empty() || stderr.lines().all(|l| l.trim().is_empty()),
        "forge audit emits no error chatter on stderr:\n{stderr}"
    );
    assert_eq!(
        manifest["manifest_version"],
        Value::from("v1"),
        "the stable v1 audit-manifest format tag"
    );

    // The genuine MIN-OVER-PARTS scope: a mixed project is to_boundary (NOT
    // over-claimed end_to_end — the no-over-claim guarantee, R-DEFER-9 / #60).
    assert_eq!(
        manifest["project_assurance"]["scope"]["kind"],
        Value::from(expect_scope),
        "the project scope is the MIN over parts — a mixed end_to_end+to_boundary \
         project is reported {expect_scope}, never over-claimed end_to_end (§5.2/#15)"
    );
    // The crossing is enumerated in the project scope.
    let crossings: Vec<String> = manifest["project_assurance"]["scope"]["crossings"]
        .as_array()
        .expect("project scope crossings array")
        .iter()
        .map(|c| c.as_str().expect("crossing name").to_string())
        .collect();
    for boundary in &expect_tcb_boundary {
        assert!(
            crossings.contains(boundary),
            "the project scope lists the `{boundary}` crossing (got {crossings:?})"
        );
    }

    // The TCB enumerates the boundary contract ∪ the toolchain (REQ-3, R-DEFER-9).
    let tcb_boundary: Vec<String> = manifest["tcb"]["boundary_contracts"]
        .as_array()
        .expect("tcb boundary_contracts array")
        .iter()
        .map(|c| {
            c["name"]
                .as_str()
                .expect("boundary contract name")
                .to_string()
        })
        .collect();
    for boundary in &expect_tcb_boundary {
        assert!(
            tcb_boundary.contains(boundary),
            "the TCB enumerates the `{boundary}` boundary contract (got {tcb_boundary:?})"
        );
    }
    // The boundary contract records its foreign target (the §9 enumerated crossing).
    let read_small = manifest["tcb"]["boundary_contracts"]
        .as_array()
        .expect("boundary_contracts")
        .iter()
        .find(|c| c["name"] == "read_small")
        .expect("read_small boundary contract enumerated in the TCB");
    assert_eq!(
        read_small["target"],
        Value::from("os::read_small"),
        "the TCB records `read_small`'s foreign target os::read_small"
    );
    // The toolchain is ALWAYS present (the irreducible residue, §9).
    assert!(
        manifest["tcb"]["toolchain"]["verus"]
            .as_str()
            .map(|s| !s.is_empty())
            .unwrap_or(false),
        "the toolchain verus version is present in the TCB"
    );
}
