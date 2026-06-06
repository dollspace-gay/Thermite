//! Divergence: the v1 type-level IFC guarantee (Stage 6, issue #76, commit
//! `ba4af4b`) is BYPASSABLE via direct struct construction (`Expr::StructLit`).
//! Authored by acto-critic — each test pins the divergence as a FAILING assertion
//! of the AUTHORITY's required behavior and is `#[ignore]`d behind the tracking
//! blocker so `main` stays CI-green (the fixer un-ignores when the barrier lands).
//!
//! THE HOLE (CONFIRMED live against the real `forge` binary). The clean types
//! (`Sql`/`Public`/`Authorized`) are ordinary Stage-1 newtype structs with
//! ACCESSIBLE fields, so a caller can MINT one directly from a marked value
//! WITHOUT the declared `#[boundary]` door:
//!
//! ```text
//! fn bypass_query(input: Tainted) -> u64 ... { query(Sql { stmt: input.raw }) }
//! ```
//!
//! The `Sql { stmt: input.raw }` `StructLit` launders the `Tainted` payload into a
//! `Sql` outside the `parameterize` door, the sink accepts it by type, and the
//! function certifies **L3** (exit 0). Tainted data flows into the SQL sink, fully
//! certified, with no sanitizer reached. This VIOLATES the design's central claim.
//!
//! Authority (`.design/basis/06-provenance-and-sinks.md`):
//!   - REQ-2: "No mark-change exists outside a door — a value's mark is fixed at
//!     construction (the struct literal) and changeable only by passing a door's
//!     return type." The StructLit here CHANGES the effective mark (Tainted payload
//!     → a clean `Sql`) OUTSIDE any door — directly contradicting REQ-2.
//!   - The model section: "DOORS (the only mark-changing operations)" / "the only
//!     door from `Tainted` to `Sql` is `parameterize`". Here a `StructLit` is a
//!     SECOND, undeclared, un-audited launder point.
//!   - The handled-or-loud law: a forbidden flow is HANDLED (through a door) or a
//!     compile-time SCREAM. This flow is NEITHER — it is silently certified L3.
//!   - `conformance/provenance/cases.json`: "the ONLY door from a marked type to a
//!     clean type is the declared #[boundary] door"; the doors are "exactly where a
//!     marked value is laundered to clean". A StructLit launder is NOT a door.
//!   - `goal.md` R-DEFER-9: a marked value reaches the sink as clean without the
//!     door — the obligation (no un-doored marked→clean flow) is silently dropped.
//!
//! ROOT CAUSE: the clean types are ordinary structs with accessible fields; v1's
//! "emergent type-level enforcement" has NO abstraction barrier (no sealed /
//! door-only-constructible clean type, and no v1.1 dataflow propagation), so a
//! `StructLit` of a clean type reading a marked field is accepted. v1 catches the
//! NAIVE form (`query(input)` is L0 — see `provenance_conformance.rs`) but NOT the
//! StructLit launder. Tracking: blocker #77.
//!
//! These run a real verus proof (the bypass body proves its equality contract), so
//! they SKIP LOUDLY if verus is absent — never panic on a missing solver (mirrors
//! `provenance_conformance.rs`).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

fn unique() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    N.fetch_add(1, Ordering::Relaxed)
}

fn write_temp(name: &str, program: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "forge_divprov_{}_{}_{name}.th",
        std::process::id(),
        unique()
    ));
    std::fs::write(&path, program).unwrap_or_else(|e| panic!("write temp {name}: {e}"));
    path
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

/// Run `forge check <program> --json` and return the certificate for `item`.
fn cert_for(program: &str, file: &str, item: &str) -> Value {
    let path = write_temp(file, program);
    let out = Command::new(forge_bin())
        .arg("check")
        .arg(&path)
        .arg("--json")
        .output()
        .unwrap_or_else(|e| panic!("spawn forge: {e}"));
    let _ = std::fs::remove_file(&path);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!(
            "forge --json must emit one JSON document: {e}\nstdout:\n{stdout}\nstderr:\n{}",
            String::from_utf8_lossy(&out.stderr)
        )
    });
    value
        .as_array()
        .and_then(|certs| {
            certs
                .iter()
                .find(|c| c.get("item").and_then(|v| v.as_str()) == Some(item))
                .cloned()
        })
        .unwrap_or_else(|| panic!("no certificate for item `{item}`\nstdout:\n{stdout}"))
}

// ---------------------------------------------------------------------------
// The three bypass programs (one per IFC axis). The marked value's payload is
// laundered into the clean type via a DIRECT `Expr::StructLit`, NOT through the
// declared `#[boundary]` door. Each program is a self-contained `.th` source.
// ---------------------------------------------------------------------------

/// TAINT axis: `Sql { stmt: input.raw }` launders a `Tainted` into the SQL sink's
/// clean type without `parameterize`. The SQLi-un-typeable centerpiece is hollow.
const TAINT_BYPASS: &str = r#"
struct Tainted { raw: u64 }
struct Sql { stmt: u64 }

#[boundary("ifc::query")] fn query(q: Sql) -> u64
  req true
  ens result == q.stmt
  fx  net(db)
  ;

fn bypass_query(input: Tainted) -> u64
  req true
  ens result == input.raw
  fx  net(db)
{
  query(Sql { stmt: input.raw })
}
"#;

/// SECRET axis: `Public { val: s.val }` launders a `Secret` into the public sink's
/// clean type without `declassify`. The secret reaches `emit` un-declassified.
const SECRET_BYPASS: &str = r#"
struct Secret { val: u64 }
struct Public { val: u64 }

#[boundary("ifc::emit")] fn emit(p: Public) -> u64
  req true
  ens result == p.val
  fx  write(log)
  ;

fn bypass_emit(s: Secret) -> u64
  req true
  ens result == s.val
  fx  write(log)
{
  emit(Public { val: s.val })
}
"#;

/// CAPABILITY axis: `Authorized { id: u.id }` forges the capability token without
/// `authorize`. The protected op `delete` runs on an unauthorized `User`.
const CAP_BYPASS: &str = r#"
struct User { id: u64 }
struct Authorized { id: u64 }

#[boundary("ifc::delete")] fn delete(c: Authorized) -> u64
  req true
  ens result == c.id
  fx  write(db)
  ;

fn bypass_delete(u: User) -> u64
  req true
  ens result == u.id
  fx  write(db)
{
  delete(Authorized { id: u.id })
}
"#;

/// TAINT bypass — `query(Sql { stmt: input.raw })` from a `Tainted input`.
///
/// AUTHORITY (06-provenance-and-sinks.md REQ-2): a mark changes "only by passing a
/// door's return type"; "No mark-change exists outside a door." The only
/// `Tainted -> Sql` door is `parameterize`. A `StructLit` is NOT a door, so the
/// tainted payload reaching the SQL sink as a clean `Sql` MUST NOT certify L3 — it
/// must be the same compile-time SCREAM (`L0`) as the naive `query(input)`.
///
/// CURRENT (commit ba4af4b, real forge): `bypass_query` certifies **L3** (exit 0).
/// The SQLi-un-typeable centerpiece is hollow. Tracking: blocker #77.
#[test]
#[ignore = "divergence: StructLit launders Tainted->Sql outside the parameterize door, certifies L3 (must be L0); blocker #77 — un-ignore when fixed"]
fn taint_structlit_bypass_must_not_certify_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — the StructLit-launder taint axis not run.");
        return;
    }
    let cert = cert_for(TAINT_BYPASS, "taint_bypass", "bypass_query");
    assert_ne!(
        cert["level"],
        Value::from("L3"),
        "IFC HOLE: a Tainted laundered to Sql via `StructLit` (NOT the parameterize \
         door) reaches the SQL sink and certifies L3 — SQLi is laundered. \
         06-provenance-and-sinks.md REQ-2 requires the door be the ONLY launder \
         point; the un-doored flow must be a compile-time SCREAM (L0)."
    );
    assert_eq!(
        cert["level"],
        Value::from("L0"),
        "the un-doored Tainted->Sql StructLit flow must be rejected (L0), like the \
         naive `query(input)` path (which IS L0 — see provenance_conformance.rs)."
    );
}

/// SECRET bypass — `emit(Public { val: s.val })` from a `Secret s`.
///
/// AUTHORITY (06-provenance-and-sinks.md Axis 2 / REQ-2): "A `Secret` ... cannot
/// reach a PUBLIC output ... without an explicit, AUDITED `declassify` door." The
/// only `Secret -> Public` door is `declassify`. A `StructLit` minting a `Public`
/// from a secret payload is an un-audited release — it MUST NOT certify L3.
///
/// CURRENT: `bypass_emit` certifies **L3** (exit 0) — the secret is laundered to a
/// public sink with no `declassify` in the TCB. Tracking: blocker #77.
#[test]
#[ignore = "divergence: StructLit launders Secret->Public outside the declassify door, certifies L3 (must be L0); blocker #77 — un-ignore when fixed"]
fn secret_structlit_bypass_must_not_certify_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — the StructLit-launder secret axis not run.");
        return;
    }
    let cert = cert_for(SECRET_BYPASS, "secret_bypass", "bypass_emit");
    assert_ne!(
        cert["level"],
        Value::from("L3"),
        "IFC HOLE: a Secret laundered to Public via `StructLit` (NOT the declassify \
         door) reaches the public sink `emit` and certifies L3 — the secret is \
         released with no audited declassify. 06-provenance-and-sinks.md Axis 2 \
         requires every release pass the declassify door; this must be L0."
    );
    assert_eq!(
        cert["level"],
        Value::from("L0"),
        "the un-doored Secret->Public StructLit flow must be rejected (L0), like the \
         naive `emit(s)` leak path (which IS L0)."
    );
}

/// CAPABILITY bypass — `delete(Authorized { id: u.id })` from a `User u`.
///
/// AUTHORITY (06-provenance-and-sinks.md Axis 3 / REQ-2): the protected op demands
/// an `Authorized` token that ONLY the `authorize` door produces — "the op is
/// un-callable without it." A `StructLit` minting `Authorized` from a raw `User`
/// forges the capability outside the auth check; it MUST NOT certify L3.
///
/// CURRENT: `bypass_delete` certifies **L3** (exit 0) — the capability is forged,
/// the protected `delete` runs on an unauthorized `User`. Tracking: blocker #77.
#[test]
#[ignore = "divergence: StructLit forges Authorized from User outside the authorize door, certifies L3 (must be L0); blocker #77 — un-ignore when fixed"]
fn capability_structlit_bypass_must_not_certify_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — the StructLit-launder capability axis not run.");
        return;
    }
    let cert = cert_for(CAP_BYPASS, "cap_bypass", "bypass_delete");
    assert_ne!(
        cert["level"],
        Value::from("L3"),
        "IFC HOLE: an Authorized token forged from a raw User via `StructLit` (NOT \
         the authorize door) lets the protected `delete` certify L3 — missing-authz \
         / IDOR is forgeable. 06-provenance-and-sinks.md Axis 3 requires authorize \
         be the ONLY Authorized producer; this must be L0."
    );
    assert_eq!(
        cert["level"],
        Value::from("L0"),
        "the un-doored User->Authorized StructLit forge must be rejected (L0), like \
         the naive `delete(u)` path (which IS L0)."
    );
}
