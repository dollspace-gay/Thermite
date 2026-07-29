//! Divergence test (acto-critic): the covenant pre-stage in `forge/src/check.rs`
//! runs before the `#[slag]` gate, so a `#[slag]` item that also carries a `witness`
//! block is covenant-falsified against its (proof-exempt) stub body and emitted as a
//! `CovenantRefuted` cert with `slag: false` — losing the slag flag/metadata and
//! manufacturing a hard-fail on a proof-exempt item.
//!
//! Authority:
//!   - `.design/forge/slag.md` REQ-2: a VALID `#[slag]` item is "exempt from the L3
//!     proof obligation: `forge check` does not invoke `verus` on it ... The
//!     certificate level is `Level::L1` ... with `slag: true`." The body content is
//!     explicitly "irrelevant to slag certification" (slag.md §"Exact ... fixture
//!     programs": "the body is proof-exempt, so its content is irrelevant").
//!   - `.design/forge/slag.md` REQ-4 (audit visibility): "a slag item is visible in
//!     the certificate ... the existing `Certificate.slag: bool` is set `true`."
//!   - `.design/forge/slag.md` REQ-5: the slag gate runs per ITEM with
//!     validate -> triage(a/b/c) -> emit L1 `slag: true`; "a non-slag item ... is
//!     untouched."
//!
//! Divergence: `check::check_file`'s per-item loop runs the covenant pre-stage
//! (`covenant_gate(analyze_covenant(...))`, the `Item::Fn` block ~check.rs L446) and
//! `continue`s on a `CovenantGate::Refuted` before ever reaching the `gate_fn` slag
//! short-circuit (~check.rs L509). So a `#[slag]` item carrying a `witness` block:
//!   (1) has its DELIBERATE-STUB body executed by the `falsify` driver, and
//!   (2) is emitted as `CovenantRefuted` with `slag: false`,
//! even though the same `#[slag]` item without the witness certifies `L1`, `slag:true`
//! (the stub body is proof-exempt). The covenant pre-stage produces a false verdict
//! shape on a slag item and drops its audit-visibility flag.
//!
//! Control: the same `#[slag]` item with no witness block certifies `L1`, `slag:true`
//! (verified against the live binary in this test), isolating the divergence to the
//! covenant-before-slag ordering.
//!
//! Tracking: filed by the critic (see report). `forge check` resolves the verus
//! version before the covenant short-circuit, so this skips (logged) when verus is
//! absent, mirroring `covenant_conformance.rs`. `tests/` is not anti-pattern-gated.

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

fn write_temp(name: &str, program: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "forge_divcov_slag_{}_{name}.th",
        std::process::id()
    ));
    std::fs::write(&path, program).unwrap_or_else(|e| panic!("write temp {name}: {e}"));
    path
}

fn first_cert(program: &str, name: &str) -> Value {
    let file = write_temp(name, program);
    let out = Command::new(forge_bin())
        .arg("check")
        .arg(&file)
        .arg("--json")
        .output()
        .unwrap_or_else(|e| panic!("spawn forge: {e}"));
    let _ = std::fs::remove_file(&file);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!(
            "forge --json stdout must be one JSON document: {e}\nstdout:\n{stdout}\nstderr:\n{}",
            String::from_utf8_lossy(&out.stderr)
        )
    });
    value
        .as_array()
        .and_then(|a| a.first())
        .cloned()
        .unwrap_or_else(|| panic!("forge --json must emit at least one cert: {value}"))
}

/// A valid `#[slag]` item (all three fields present, non-empty) whose stub body `{ 0 }`
/// does not satisfy `ens result == x` — but per slag.md REQ-2 the body is proof-EXEMPT,
/// so the item certifies L1 `slag: true` regardless. Adding a `witness` block must not
/// turn the proof-exempt stub into a covenant hard-fail nor drop the slag flag.
const SLAG_WITH_WITNESS: &str = "\
#[slag(reason = \"vendored\", owner = \"agent:forge-7\", review = \"required\")]
fn f(x: u32) -> u32
    req true
    ens result == x
    fx pure
{ 0 }

witness { inhabit (5); falsify 10; }
";

/// The control: the same `#[slag]` item with no witness block. It certifies L1,
/// `slag: true` (the proof-exempt stub body is irrelevant — slag.md REQ-2/REQ-4).
const SLAG_NO_WITNESS: &str = "\
#[slag(reason = \"vendored\", owner = \"agent:forge-7\", review = \"required\")]
fn f(x: u32) -> u32
    req true
    ens result == x
    fx pure
{ 0 }
";

#[test]
fn slag_item_keeps_its_slag_flag_under_a_covenant_block() {
    if !verus_present() {
        eprintln!(
            "SKIP: verus not available — `forge check` resolves the verus version before \
             the covenant short-circuit (set VERUS_BIN or install verus on PATH)."
        );
        return;
    }

    // Control (slag.md REQ-2/REQ-4): the same item without a witness certifies L1,
    // slag:true — its stub body is proof-exempt, never falsified.
    let control = first_cert(SLAG_NO_WITNESS, "nowitness");
    assert_eq!(
        control["slag"],
        Value::from(true),
        "control: a valid #[slag] item certifies with slag:true (slag.md REQ-4): {control}"
    );
    assert_eq!(
        control["level"],
        Value::from("L1"),
        "control: a valid #[slag] item certifies L1 (slag.md REQ-2): {control}"
    );

    // Authority (slag.md REQ-2/REQ-4/REQ-5): a #[slag] item is proof-exempt and its
    // certificate carries slag:true. Adding a `witness` block does not strip the slag
    // identity. The covenant pre-stage (check.rs, the Item::Fn covenant block that runs
    // before the gate_fn slag short-circuit) executes the proof-exempt stub body, emits
    // a CovenantRefuted, and `continue`s — so the cert reports slag:false (the slag
    // flag/metadata are lost) on an item that IS slag.
    let cert = first_cert(SLAG_WITH_WITNESS, "withwitness");
    assert_eq!(
        cert["slag"],
        Value::from(true),
        "DIVERGENCE (slag.md REQ-4): a #[slag] item must carry slag:true in its \
         certificate; the covenant pre-stage ran before the slag gate and dropped the \
         slag flag, reporting slag:false on a proof-exempt item: {cert}"
    );
}
