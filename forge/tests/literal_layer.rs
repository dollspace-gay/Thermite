//! L3-grounding conformance for the LITERAL LAYER — crosslink #91 cluster 1.
//!
//! Cluster 1 of the primitive-completeness buildout adds the missing string-escape
//! forms (`\r`, `\0`, `\xNN` — the ANSI/control bytes the editor needs). This test
//! certifies, against the REAL verus binary, that each escape decodes to its
//! control byte and that byte flows end-to-end through the existing `String`
//! literal lowering (`thermite-lower::lower` `Expr::StrLit` → byte-`push`) so that
//! `"\x1b".byte_at(0) == 27`, `"\r".byte_at(0) == 13`, `"\0".byte_at(0) == 0`
//! certify L3 (`.design/basis/07-strings.md` REQ-6 escape table + REQ-2 byte model,
//! `thermite-design.md` §6 ladder: a fully-discharged real-verus proof is L3).
//!
//! NON-VACUITY (R-DEFER-9 / `thermite-design.md` §7): the control byte is observed
//! through a `result == (n == <CODE>)` contract — body `LIT.byte_at(0) == n` — so a
//! deliberately-wrong body (`return false`, or a different byte) is KILLED by the
//! §7 mutation battery (the proof requires the literal byte to be EXACTLY the
//! control code for all `n`). A naive `ens result == 0` for `\0` is correctly
//! REJECTED by the same battery (the `return 0` mutant survives), which is why the
//! grounding uses the equality-against-parameter form, not a bare constant `ens`.
//!
//! R-CHAR-3: the expected BYTES are the ANSI/ASCII control-code symbolic constants
//! (ESC == 27, CR == 13, NUL == 0), and the expected LEVELS trace to §6 (L3 == a
//! discharged verus proof); NEITHER is copied from forge's own output. Runs the
//! BUILT `forge` binary; if verus is absent it SKIPS LOUDLY (never panics on a
//! missing solver), mirroring `string_l3_completeness.rs` / `divergence_strings.rs`.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// `true` iff verus is reachable (mirrors `string_l3_completeness.rs`).
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
        "forge_litlayer_{tag}_{}_{}.th",
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

/// A `fn` certifying that the escape literal `lit`'s first byte equals the control
/// code `code`, via the non-vacuous `result == (n == code)` / `lit.byte_at(0) == n`
/// form (R-DEFER-9: the §7 battery kills a wrong byte). Constructing the literal
/// allocates, so the fn carries `fx alloc` (07-strings.md REQ-4).
fn escape_eq_program(name: &str, lit: &str, code: u64) -> String {
    format!(
        "fn {name}(n: u64) -> bool\n  req true\n  ens result == (n == {code})\n  fx alloc\n{{ \"{lit}\".byte_at(0) == n }}\n"
    )
}

/// `\x1b` (ANSI ESC) decodes to byte 27 and certifies L3: the escape-table
/// addition flows the ESC byte through the `String`-literal lowering so
/// `"\x1b".byte_at(0)` is provably 27 (07-strings.md REQ-6; §6 L3).
#[test]
fn escape_x1b_certifies_l3_byte_27() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — \\x1b L3 grounding not exercised.");
        return;
    }
    let certs = check_program("x1b", &escape_eq_program("esc_is_27", "\\x1b", 27));
    let c = cert_for(&certs, "esc_is_27");
    assert_eq!(
        c["level"], "L3",
        "DESIGN 07-strings.md REQ-6: `\\x1b` decodes to ESC == 27; `\"\\x1b\".byte_at(0) == n` \
         iff `n == 27` certifies L3 (non-vacuous, §7 battery). forge reports: {}",
        c["level"]
    );
    assert_eq!(
        c["effects"],
        serde_json::json!(["alloc"]),
        "DESIGN REQ-4: materializing a String literal carries fx alloc."
    );
}

/// `\r` (carriage return) decodes to byte 13 and certifies L3.
#[test]
fn escape_cr_certifies_l3_byte_13() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — \\r L3 grounding not exercised.");
        return;
    }
    let certs = check_program("cr", &escape_eq_program("cr_is_13", "\\r", 13));
    let c = cert_for(&certs, "cr_is_13");
    assert_eq!(
        c["level"], "L3",
        "DESIGN 07-strings.md REQ-6: `\\r` decodes to CR == 13. forge reports: {}",
        c["level"]
    );
}

/// `\0` (NUL) decodes to byte 0 and certifies L3 — via the equality-against-
/// parameter form (a bare `ens result == 0` is correctly REJECTED by the §7
/// battery because the `return 0` mutant survives; the non-vacuous form pins the
/// literal byte to EXACTLY 0 for all `n`).
#[test]
fn escape_nul_certifies_l3_byte_0() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — \\0 L3 grounding not exercised.");
        return;
    }
    let certs = check_program("nul", &escape_eq_program("nul_is_0", "\\0", 0));
    let c = cert_for(&certs, "nul_is_0");
    assert_eq!(
        c["level"], "L3",
        "DESIGN 07-strings.md REQ-6: `\\0` decodes to NUL == 0; the `result == (n == 0)` \
         form pins it non-vacuously (the bare `ens result == 0` is correctly killed by \
         the §7 mutation battery). forge reports: {}",
        c["level"]
    );
}

/// NON-VACUITY (R-DEFER-9) — the escape byte is LOAD-BEARING: a contract claiming
/// the WRONG control code (`\x1b` == 99 instead of 27) does NOT certify L3. The
/// proof requires the literal to decode to the EXACT byte; a wrong claim leaves the
/// `ens` undischarged → not L3.
#[test]
fn escape_byte_is_load_bearing_wrong_code_not_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — escape non-vacuity not exercised.");
        return;
    }
    // body decodes to 27, but the contract claims the byte is 99.
    let certs = check_program(
        "wrong",
        "fn wrong(n: u64) -> bool\n  req true\n  ens result == (n == 99)\n  fx alloc\n{ \"\\x1b\".byte_at(0) == n }\n",
    );
    let c = cert_for(&certs, "wrong");
    assert_ne!(
        c["level"], "L3",
        "R-DEFER-9: `\\x1b` decodes to 27, NOT 99 — a contract claiming 99 must NOT \
         certify L3 (the escape byte is load-bearing, not laundered). forge reports: {}",
        c["level"]
    );
}
