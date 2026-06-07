//! L3-grounding conformance for the OPERATOR + LITERAL layer — crosslink
//! #92 (clusters 1-remainder + 2 of the primitive-completeness buildout).
//!
//! Cluster 2 of the buildout adds the integer OPERATORS `% << >> & | ^ !` and folds
//! in the char/hex/binary LITERAL forms (`'A'` / `0x1b` / `0b101`) from cluster 1.
//! This test certifies, against the REAL `verus` binary, that:
//!
//!   - each operator (`%`/`<<`/`>>`/`&`/`|`/`^`/`!`) lowers and certifies L3 with a
//!     NON-VACUOUS `ens` (`.design/syntax/ast.md` REQ-10, §6 ladder: a fully-
//!     discharged real-verus proof is L3);
//!   - the PARTIAL operators (`%`, and the existing `/`; the shifts `<<`/`>>`) carry
//!     their §7 PROOF obligation (ast.md REQ-11): `a % b` WITH `req b != 0` certifies
//!     L3, but WITHOUT it is L0 ("possible division by zero"); `a << k` WITH `req
//!     k < 64` certifies L3, but unbounded is L0 ("possible bit shift");
//!   - the literal VALUES are exact: `'A'` == 65, `0x1b` == 27, `0b101` == 5 each
//!     certify their non-vacuous `ens result == <decimal>` at L3, and a WRONG code
//!     (`'A'` claimed 66) does NOT certify L3;
//!   - the pinned PRECEDENCE is realized end-to-end: `a % b + 1` groups `(a % b) + 1`
//!     (`surface-grammar.md` REQ-10) — the proof certifies the expected value.
//!
//! NON-VACUITY (R-DEFER-9 / `thermite-design.md` §7): every `ens` observes the
//! operator/literal through `result == <expr>` (a function of the inputs), so a
//! deliberately-wrong body or a wrong-code claim is KILLED by the §7 battery — the
//! §7 vacuity gate (which rejects `ens true`) is respected.
//!
//! R-CHAR-3: the expected VALUES are the design's symbolic constants (the ASCII
//! code for `'A'` is 65, `0x1b` == 27, `0b101` == 5 — `lexer.md` AC-7/AC-8 /
//! `ast.md` Verification) and the expected LEVELS trace to §6 (L3 == a discharged
//! verus proof; L0 == an undischarged obligation); NEITHER is copied from forge's
//! own output. Runs the BUILT `forge` binary; if verus is absent it SKIPS LOUDLY
//! (never panics on a missing solver), mirroring `literal_layer.rs`.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// `true` iff verus is reachable (mirrors `literal_layer.rs`).
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
        "forge_ops_{tag}_{}_{}.th",
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

// ---------------------------------------------------------------------------
// Operators that certify L3 with a non-vacuous ens.
// ---------------------------------------------------------------------------

/// `%` WITH `req b != 0` certifies L3 (ast.md REQ-10/REQ-11; AC-6).
#[test]
fn rem_with_nonzero_req_certifies_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — `%` L3 grounding not exercised.");
        return;
    }
    let certs = check_program(
        "rem_ok",
        "fn modulo(a: u64, b: u64) -> u64\n  req b != 0\n  ens result == a % b\n  fx pure\n{ a % b }\n",
    );
    assert_eq!(
        level(&certs, "modulo"),
        "L3",
        "DESIGN ast.md REQ-11: `a % b` WITH `req b != 0` discharges the div-by-zero \
         obligation and certifies L3. forge: {:?}",
        certs
    );
}

/// `<<` WITH `req k < 64` certifies L3; `&`/`|`/`^` certify L3 (AC-6).
#[test]
fn shifts_and_bitwise_certify_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — shift/bitwise L3 grounding not exercised.");
        return;
    }
    // `<<` with the shift-bound obligation discharged.
    let shl = check_program(
        "shl_ok",
        "fn lshift(a: u64, k: u64) -> u64\n  req k < 64\n  ens result == a << k\n  fx pure\n{ a << k }\n",
    );
    assert_eq!(
        level(&shl, "lshift"),
        "L3",
        "DESIGN ast.md REQ-11: `a << k` WITH `req k < 64` certifies L3. forge: {shl:?}"
    );
    // `>>` with the same bound.
    let shr = check_program(
        "shr_ok",
        "fn rshift(a: u64, k: u64) -> u64\n  req k < 64\n  ens result == a >> k\n  fx pure\n{ a >> k }\n",
    );
    assert_eq!(
        level(&shr, "rshift"),
        "L3",
        "DESIGN ast.md REQ-11: `a >> k` WITH `req k < 64` certifies L3. forge: {shr:?}"
    );
    // `&` — total, no obligation.
    let and = check_program(
        "band_ok",
        "fn band(a: u64, b: u64) -> u64\n  req true\n  ens result == a & b\n  fx pure\n{ a & b }\n",
    );
    assert_eq!(
        level(&and, "band"),
        "L3",
        "DESIGN: `a & b` certifies L3. forge: {and:?}"
    );
    // `|`
    let or = check_program(
        "bor_ok",
        "fn bor(a: u64, b: u64) -> u64\n  req true\n  ens result == a | b\n  fx pure\n{ a | b }\n",
    );
    assert_eq!(
        level(&or, "bor"),
        "L3",
        "DESIGN: `a | b` certifies L3. forge: {or:?}"
    );
    // `^`
    let xor = check_program(
        "bxor_ok",
        "fn bxor(a: u64, b: u64) -> u64\n  req true\n  ens result == a ^ b\n  fx pure\n{ a ^ b }\n",
    );
    assert_eq!(
        level(&xor, "bxor"),
        "L3",
        "DESIGN: `a ^ b` certifies L3. forge: {xor:?}"
    );
}

/// The prefix `!` certifies L3 on BOTH a `bool` (logical-not) and a `u64`
/// (bitwise-not) operand — the ONE `UnaryOp::Not` resolved per type by Verus's
/// type-directed `!` (ast.md OQ-4; AC-6).
#[test]
fn unary_not_certifies_l3_per_type() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — `!` L3 grounding not exercised.");
        return;
    }
    // logical-not on bool.
    let lnot = check_program(
        "lnot_ok",
        "fn lnot(flag: bool) -> bool\n  req true\n  ens result == !flag\n  fx pure\n{ !flag }\n",
    );
    assert_eq!(
        level(&lnot, "lnot"),
        "L3",
        "DESIGN ast.md OQ-4: `!flag` (logical-not on bool) certifies L3. forge: {lnot:?}"
    );
    // bitwise-not on u64.
    let bnot = check_program(
        "bnot_ok",
        "fn bnot(bits: u64) -> u64\n  req true\n  ens result == !bits\n  fx pure\n{ !bits }\n",
    );
    assert_eq!(
        level(&bnot, "bnot"),
        "L3",
        "DESIGN ast.md OQ-4: `!bits` (bitwise-not on u64) certifies L3. forge: {bnot:?}"
    );
}

// ---------------------------------------------------------------------------
// Partiality bites: the obligation is NOT optional (R-DEFER-9).
// ---------------------------------------------------------------------------

/// `%` WITHOUT `req b != 0` is L0 — the divide-by-zero obligation BITES (ast.md
/// REQ-11; AC-6). The same teeth as the existing `/`.
#[test]
fn rem_without_nonzero_req_is_l0() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — `%` partiality not exercised.");
        return;
    }
    let certs = check_program(
        "rem_bad",
        "fn modulo(a: u64, b: u64) -> u64\n  req true\n  ens result == a % b\n  fx pure\n{ a % b }\n",
    );
    assert_eq!(
        level(&certs, "modulo"),
        "L0",
        "DESIGN ast.md REQ-11 (R-DEFER-9): `a % b` WITHOUT `req b != 0` leaves the \
         div-by-zero obligation undischarged → L0, NOT laundered to L3. forge: {certs:?}"
    );
}

/// `<<` WITHOUT a bounded shift amount is L0 — the shift-bound obligation BITES.
#[test]
fn shift_without_bound_is_l0() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — shift partiality not exercised.");
        return;
    }
    let certs = check_program(
        "shl_bad",
        "fn lshift(a: u64, k: u64) -> u64\n  req true\n  ens result == a << k\n  fx pure\n{ a << k }\n",
    );
    assert_eq!(
        level(&certs, "lshift"),
        "L0",
        "DESIGN ast.md REQ-11 (R-DEFER-9): `a << k` WITHOUT `req k < 64` leaves the \
         shift-bound obligation undischarged → L0. forge: {certs:?}"
    );
}

// ---------------------------------------------------------------------------
// Char / hex / binary literals: exact value + non-vacuity.
// ---------------------------------------------------------------------------

/// `'A'` == 65, `0x1b` == 27, `0b101` == 5 — each certifies its exact-value `ens`
/// at L3 (lexer.md AC-7/AC-8; the values are the design's symbolic constants).
#[test]
fn char_hex_binary_literals_certify_exact_value_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — literal-value L3 grounding not exercised.");
        return;
    }
    // char 'A' == 65 (the byte/u8 model; the value flows through Expr::IntLit).
    let ch = check_program(
        "char_a",
        "fn char_a() -> u64\n  req true\n  ens result == 65\n  fx pure\n{ 'A' }\n",
    );
    assert_eq!(
        level(&ch, "char_a"),
        "L3",
        "DESIGN lexer.md AC-8: `'A'` == 65 (byte value) certifies L3. forge: {ch:?}"
    );
    // hex 0x1b == 27.
    let hex = check_program(
        "hex_1b",
        "fn hex_1b() -> u64\n  req true\n  ens result == 27\n  fx pure\n{ 0x1b }\n",
    );
    assert_eq!(
        level(&hex, "hex_1b"),
        "L3",
        "DESIGN lexer.md AC-7: `0x1b` == 27 certifies L3. forge: {hex:?}"
    );
    // binary 0b101 == 5.
    let bin = check_program(
        "bin_101",
        "fn bin_101() -> u64\n  req true\n  ens result == 5\n  fx pure\n{ 0b101 }\n",
    );
    assert_eq!(
        level(&bin, "bin_101"),
        "L3",
        "DESIGN lexer.md AC-7: `0b101` == 5 certifies L3. forge: {bin:?}"
    );
}

/// NON-VACUITY (R-DEFER-9) — the char byte is LOAD-BEARING: a contract claiming the
/// WRONG code (`'A'` == 66 instead of 65) does NOT certify L3 (lexer.md AC-8).
#[test]
fn char_literal_value_is_load_bearing_wrong_code_not_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — literal non-vacuity not exercised.");
        return;
    }
    let certs = check_program(
        "char_wrong",
        "fn char_wrong() -> u64\n  req true\n  ens result == 66\n  fx pure\n{ 'A' }\n",
    );
    assert_ne!(
        level(&certs, "char_wrong"),
        "L3",
        "R-DEFER-9: `'A'` is 65, NOT 66 — a contract claiming 66 must NOT certify L3 \
         (the literal value is load-bearing). forge: {certs:?}"
    );
}

// ---------------------------------------------------------------------------
// Precedence is realized end-to-end (surface-grammar.md REQ-10).
// ---------------------------------------------------------------------------

/// `a % b + 1` groups as `(a % b) + 1` (the pinned precedence: `%` tighter than
/// `+`). The proof certifies the EXPECTED value — a mis-grouping `a % (b + 1)`
/// would change the function and fail the exact-value `ens`.
#[test]
fn precedence_rem_binds_tighter_than_add() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — precedence grounding not exercised.");
        return;
    }
    let certs = check_program(
        "prec",
        "fn prec(a: u64, b: u64) -> u64\n  req b != 0\n  ens result == (a % b) + 1\n  fx pure\n{ a % b + 1 }\n",
    );
    assert_eq!(
        level(&certs, "prec"),
        "L3",
        "DESIGN surface-grammar.md REQ-10: `a % b + 1` groups `(a % b) + 1` (`%` > `+`); \
         the exact-value `ens` certifies L3. forge: {certs:?}"
    );
}
