//! Divergence (CLOSED, blocker #77): the v1 type-level IFC guarantee (Stage 6,
//! issue #76) WAS bypassable via direct struct construction (`Expr::StructLit`).
//! The acto-critic pinned the hole as three `#[ignore]`d failing tests; the
//! `#[sealed]` ABSTRACTION BARRIER (REQ-8) now CLOSES it, so these tests are
//! UN-IGNORED and assert the bypass is REJECTED at validation.
//!
//! THE HOLE (was live; now closed). The clean types (`Sql`/`Public`/`Authorized`)
//! are Stage-1 newtype structs with ACCESSIBLE fields, so a caller could MINT one
//! directly from a marked value WITHOUT the declared `#[boundary]` door:
//!
//! ```text
//! fn bypass_query(input: Tainted) -> u64 ... { query(Sql { stmt: input.raw }) }
//! ```
//!
//! The `Sql { stmt: input.raw }` `StructLit` laundered the `Tainted` payload into
//! a `Sql` outside the `parameterize` door, the sink accepted it by type, and the
//! function certified **L3**. THE FIX (REQ-8): the clean types are `#[sealed]`, and
//! the validator REJECTS any `Expr::StructLit` of a `#[sealed]` struct with
//! `SpecError::SealedConstruction`. `forge check` on a bypass program now FAILS at
//! validation (a whole-program spec error — exit non-zero, the `SealedConstruction`
//! diagnostic on stderr, NO L3 certificate ever emitted), exactly as every other
//! validator reject (`NonExhaustiveMatch`, `UnknownField`, …) does. The door is the
//! ONLY launder point — its body is foreign (`external_body`), with no in-language
//! `StructLit`, so the safe doored path (`query(parameterize(input))`) is unaffected
//! (still L3, see `provenance_conformance.rs`).
//!
//! Authority (`.design/basis/06-provenance-and-sinks.md`):
//!   - REQ-8: a `#[sealed]` clean type "CANNOT be constructed by a `StructLit`
//!     anywhere in Thermite code, so the ONLY way to obtain one is through its
//!     `#[boundary]` door". `query(Sql { … })` is `SpecError::SealedConstruction`.
//!   - REQ-2: "No mark-change exists outside a door … TRUE only because the clean
//!     types are `#[sealed]` (REQ-8)." The StructLit launder is closed.
//!   - AC-7: these three `#[ignore]`d tests are UN-IGNORED and PASS — each launder
//!     yields `SealedConstruction` and does NOT certify L3.
//!   - The handled-or-loud law: the un-doored flow is now the LOUDEST tooth — a
//!     compile-time SCREAM (validation reject), never a silent L3.
//!   - `goal.md` R-DEFER-9: no un-doored marked→clean launder.
//!
//! These do NOT need verus: the launder dies at the VALIDATOR (before any proof),
//! so they assert the spec-reject directly and run regardless of solver presence.

use std::path::PathBuf;
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

/// The captured outcome of `forge check <program> --json` on a sealed-launder
/// program: the exit code, the combined stdout, and stderr.
struct CheckOutcome {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

/// Run `forge check <program> --json` and capture exit code + stdout + stderr.
fn run_check(program: &str, file: &str) -> CheckOutcome {
    let path = write_temp(file, program);
    let out = Command::new(forge_bin())
        .arg("check")
        .arg(&path)
        .arg("--json")
        .output()
        .unwrap_or_else(|e| panic!("spawn forge: {e}"));
    let _ = std::fs::remove_file(&path);
    CheckOutcome {
        code: out.status.code(),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

/// Assert a `#[sealed]` `StructLit` launder is REJECTED at validation (REQ-8,
/// AC-7): the run FAILS (non-zero exit), the `SealedConstruction` barrier
/// diagnostic for `sealed_ty` is surfaced (never swallowed), and NO certificate
/// for the laundering `item` ever reaches `L3` (the security guarantee). A
/// validator reject is a whole-program spec error, so `forge --json` emits NO
/// cert array on stdout — the door-bypass dies BEFORE any per-item proof, exactly
/// like every other validator reject. Expectations are hand-derived from
/// `.design/basis/06-provenance-and-sinks.md` REQ-8/AC-7 (R-CHAR-3), never read
/// back from the toolchain's own output.
fn assert_sealed_launder_rejected(outcome: &CheckOutcome, item: &str, sealed_ty: &str) {
    assert_ne!(
        outcome.code,
        Some(0),
        "IFC HOLE: a `#[sealed]` `{sealed_ty}` minted via `StructLit` in `{item}` was \
         ACCEPTED (exit 0) — the door-bypass certifies. REQ-8 requires the door be the \
         ONLY launder point; the un-doored flow must be a compile-time SCREAM."
    );
    let diag = format!("{}{}", outcome.stdout, outcome.stderr);
    assert!(
        diag.contains("sealed") && diag.contains(sealed_ty),
        "the reject must be the `SealedConstruction` barrier naming `{sealed_ty}` (REQ-8) — \
         got:\nstdout:\n{}\nstderr:\n{}",
        outcome.stdout,
        outcome.stderr
    );
    // No L3 certificate for the laundering fn may be emitted. A validator reject
    // emits no cert array at all; if a (future) regression emitted one, it must
    // NOT carry the laundering item at L3.
    if let Ok(Value::Array(certs)) = serde_json::from_str::<Value>(outcome.stdout.trim()) {
        for cert in &certs {
            if cert.get("item").and_then(|v| v.as_str()) == Some(item) {
                assert_ne!(
                    cert["level"],
                    Value::from("L3"),
                    "SECURITY GUARANTEE: the door-bypass `{item}` MUST NEVER certify L3 (REQ-8)"
                );
            }
        }
    }
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
#[sealed] struct Sql { stmt: u64 }

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
#[sealed] struct Public { val: u64 }

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
#[sealed] struct Authorized { id: u64 }

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
/// AUTHORITY (06-provenance-and-sinks.md REQ-8/AC-7): `Sql` is `#[sealed]`, so a
/// `StructLit` minting it is `SpecError::SealedConstruction` — the door
/// (`parameterize`) is the ONLY launder point. The tainted payload reaching the
/// SQL sink as a clean `Sql` via a struct literal MUST be REJECTED at validation,
/// never certify L3. FIXED by blocker #77 (the `#[sealed]` barrier) — un-ignored.
#[test]
fn taint_structlit_bypass_must_not_certify_l3() {
    let outcome = run_check(TAINT_BYPASS, "taint_bypass");
    assert_sealed_launder_rejected(&outcome, "bypass_query", "Sql");
}

/// SECRET bypass — `emit(Public { val: s.val })` from a `Secret s`.
///
/// AUTHORITY (06-provenance-and-sinks.md Axis 2 / REQ-8/AC-7): `Public` is
/// `#[sealed]`, so a `StructLit` minting it is `SealedConstruction` — `declassify`
/// is the ONLY release door. A `Public` struct literal reading a secret payload is
/// an un-audited release REJECTED at validation, never L3. FIXED by #77 —
/// un-ignored.
#[test]
fn secret_structlit_bypass_must_not_certify_l3() {
    let outcome = run_check(SECRET_BYPASS, "secret_bypass");
    assert_sealed_launder_rejected(&outcome, "bypass_emit", "Public");
}

/// CAPABILITY bypass — `delete(Authorized { id: u.id })` from a `User u`.
///
/// AUTHORITY (06-provenance-and-sinks.md Axis 3 / REQ-8/AC-7): `Authorized` is
/// `#[sealed]`, so a `StructLit` forging it is `SealedConstruction` — `authorize`
/// is the ONLY `Authorized` producer. A forged capability via a struct literal is
/// REJECTED at validation, never L3. FIXED by #77 — un-ignored.
#[test]
fn capability_structlit_bypass_must_not_certify_l3() {
    let outcome = run_check(CAP_BYPASS, "cap_bypass");
    assert_sealed_launder_rejected(&outcome, "bypass_delete", "Authorized");
}
