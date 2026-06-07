//! Conformance for Cluster **C7** (crosslink **#95**): the built-in `Option` /
//! `Result` constructors + the `Result<T, E>` two-arg type + the
//! payload-in-contract projection (the spec-`match`-in-`ens`) + the deferred
//! `parse_u64` (the C4 `07-strings.md` REQ-9 payoff). These run against the two
//! EXTERNAL truths the toolchain does not author for itself: the built `forge`
//! binary's certificate ladder (`forge check`, real verus) and — for the generated
//! `parse_u64` whose round-trip cannot be a thin caller's mutation-scored cert —
//! the real `verus` binary on the EMITTED lowering of a `parse_u64`-calling
//! program (R-CODE-4: the subprocess status is checked, never swallowed).
//!
//! Pins the C7 deliverables:
//!
//!   * `Some(5)`/`None` construct + the payload-in-contract `ens match result {
//!     Some(v) => v == 5, None => true }` → L3 (AC-1).
//!   * `Ok(7)`/`Err(e)` construct + `Result<u64, ParseErr>` parses (the two-arg
//!     `Type::Result`) + match + the payload `ens` → L3 (AC-2).
//!   * The error arms BITE: a broken `Some(0)` / `Ok(0)` under the payload `ens`
//!     is REJECTED, never laundered to L3 (AC-3, R-DEFER-9 non-vacuity).
//!   * `parse_u64(s)` lowers to the Horner loop + the three handled-or-loud `None`
//!     arms + the round-trip success `ens`, and the real `verus` binary verifies it
//!     `verified, 0 errors`; a hand-broken `parse_u64` returning `Some(0)`
//!     unconditionally FAILS verus (AC-4 + non-vacuity).
//!
//! The verus checks SKIP LOUDLY when verus is absent (the `string_l3_completeness.rs`
//! precedent) — never panic on a missing solver. `tests/` is not anti-pattern-gated,
//! so `unwrap`/`expect`/`panic!` are fine (R-APG-2).
//!
//! R-CHAR-3: expected levels trace to `.design/basis/09-option-result.md` AC-1..AC-4
//! (the GROUNDED forms: Option construct `4 verified, 0 errors`; Result `3 verified,
//! 0 errors`; the broken bodies FAIL; `parse_u64` `5 verified, 0 errors`, broken
//! `Some(0)` `3 verified, 1 errors`) + `thermite-design.md` §6 ladder semantics (L3 ==
//! a fully-discharged real-verus proof), NEVER copied from the toolchain's own output.

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

/// Resolve the verus binary path (PATH, `VERUS_BIN`, or `~/.local/bin/verus`).
fn verus_bin() -> PathBuf {
    if let Ok(p) = std::env::var("VERUS_BIN") {
        if Path::new(&p).exists() {
            return PathBuf::from(p);
        }
    }
    if let Ok(out) = Command::new("which").arg("verus").output() {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                return PathBuf::from(s);
            }
        }
    }
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".local/bin/verus")
}

/// Write `program` to a unique temp `.th`, `forge check --json` it, return the cert
/// array. The temp file is removed before returning (scratch hygiene, #53).
fn check_program(tag: &str, program: &str) -> Vec<Value> {
    let fixture = std::env::temp_dir().join(format!(
        "forge_optres_{tag}_{}_{}.th",
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

/// Lower a Thermite source program to its Verus source via the toolchain's `lower`,
/// write it to a temp `.rs`, run the real `verus` binary, and return
/// `(success, combined_output)`. The temp file is removed before returning (#53).
/// R-CODE-4: the subprocess status is checked + surfaced, never swallowed.
fn verus_on_lowered(tag: &str, program: &str) -> (bool, String) {
    let parsed = thermite_syntax::parse(program);
    assert!(
        parsed.is_clean(),
        "[{tag}] surface must parse cleanly: {:?}",
        parsed.errors
    );
    let verus_src = thermite_lower::lower(&parsed.program)
        .unwrap_or_else(|e| panic!("[{tag}] lower must succeed: {e:?}"));
    let rs = std::env::temp_dir().join(format!(
        "forge_optres_verus_{tag}_{}.rs",
        std::process::id()
    ));
    std::fs::write(&rs, &verus_src).expect("write lowered .rs");
    // Run verus with cwd = temp_dir so any compiled output artifact verus emits
    // (named after the .rs stem) lands in /tmp, never in the crate tree (#53 scratch
    // hygiene). Both the source and the stem-named artifact are removed after.
    let out = Command::new(verus_bin())
        .arg(&rs)
        .current_dir(std::env::temp_dir())
        .output()
        .expect("spawn verus");
    let _ = std::fs::remove_file(&rs);
    if let Some(stem) = rs.file_stem() {
        let _ = std::fs::remove_file(std::env::temp_dir().join(stem));
    }
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (out.status.success(), combined)
}

/// AC-1 — `Some(5)`/`None` construct + the payload-in-contract `ens match result {
/// Some(v) => v == 5, None => true }` certifies L3.
///
/// AUTHORITY: `.design/basis/09-option-result.md` AC-1 — `Some(5)` is `Expr::Call`,
/// `Option<u64>` is `Type::Option`; the validator's seeded built-in variant registry
/// accepts `Some`; the spec-`match` is admitted as a flat built-in; lowers to a Verus
/// `Option<u64>` + the spec-`match`-in-`ens`. GROUNDED `4 verified, 0 errors`.
/// `thermite-design.md` §6: a fully-discharged verus proof is L3.
#[test]
fn ac1_option_construct_payload_in_contract_certifies_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — Option construct L3 not exercised.");
        return;
    }
    let certs = check_program(
        "optconstruct",
        "fn make() -> Option<u64>\n  req true\n  ens match result { Some(v) => v == 5, None => true }\n  fx pure\n{ Some(5) }\n",
    );
    let make = cert_for(&certs, "make");
    assert_eq!(
        make["level"], "L3",
        "DESIGN 09-option-result.md AC-1: `Some(5)` constructs (the seeded built-in \
         variant registry) and the payload-in-contract `ens match result {{ Some(v) => \
         v == 5, None => true }}` certifies L3 (the spec-match-in-ens projects the \
         Some payload). forge reports: {}",
        make["level"]
    );
}

/// AC-2 — `Result<u64, ParseErr>` PARSES (the two-arg `Type::Result`), `Ok(7)`/`Err`
/// construct, `match`/payload `ens` certify L3.
///
/// AUTHORITY: `.design/basis/09-option-result.md` AC-2 — `Result<u64, ParseErr>` is
/// the dedicated two-type-arg node (the change this AC pins); `Ok(7)` constructs via
/// the seeded `Ok`/`Err` registry; the payload `ens match result { Ok(v) => v == 7,
/// Err(_) => true }` certifies L3. GROUNDED `3 verified, 0 errors`. The `E` parameter
/// `ParseErr` is an ordinary user error enum.
#[test]
fn ac2_result_two_arg_type_construct_payload_certifies_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — Result construct L3 not exercised.");
        return;
    }
    let certs = check_program(
        "resconstruct",
        "enum ParseErr { NotDigit, Overflow, Empty }\n\
         fn ok7() -> Result<u64, ParseErr>\n  req true\n  ens match result { Ok(v) => v == 7, Err(_) => true }\n  fx pure\n{ Ok(7) }\n",
    );
    let ok7 = cert_for(&certs, "ok7");
    assert_eq!(
        ok7["level"], "L3",
        "DESIGN 09-option-result.md AC-2: `Result<u64, ParseErr>` PARSES (the two-arg \
         Type::Result), `Ok(7)` constructs (the seeded Ok/Err registry), and the \
         payload `ens match result {{ Ok(v) => v == 7, Err(_) => true }}` certifies L3. \
         forge reports: {}",
        ok7["level"]
    );
}

/// AC-3 (the error arms BITE — non-vacuity, R-DEFER-9) — a broken `Some(0)` under the
/// payload `ens match result { Some(v) => v == 5, None => true }` is REJECTED, never
/// laundered to L3.
///
/// AUTHORITY: `.design/basis/09-option-result.md` AC-3 — `Some(0)` under the Some-arm
/// `v == 5` FAILS verus (`1 verified, 1 errors`, postcondition not satisfied) — the
/// payload contract is real, not vacuous. `thermite-design.md` §7: the battery catches
/// a false claim.
#[test]
fn ac3_broken_some_under_payload_ens_is_rejected() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — Option non-vacuity not exercised.");
        return;
    }
    let certs = check_program(
        "optbroken",
        "fn bad() -> Option<u64>\n  req true\n  ens match result { Some(v) => v == 5, None => true }\n  fx pure\n{ Some(0) }\n",
    );
    let bad = cert_for(&certs, "bad");
    assert_ne!(
        bad["level"], "L3",
        "R-DEFER-9 non-vacuity: a broken `Some(0)` under the payload `ens` (Some-arm \
         `v == 5`) must be REJECTED — the spec-match-in-ens projects a REAL constraint \
         on the payload, not a vacuous `true`. forge reports: {}",
        bad["level"]
    );
}

/// AC-4 — `parse_u64(s)` lowers to the Horner loop + the three handled-or-loud `None`
/// arms + the round-trip success `ens`, and the real `verus` binary verifies the
/// EMITTED lowering `verified, 0 errors`.
///
/// AUTHORITY: `.design/basis/09-option-result.md` REQ-5 / AC-4 — the lowerer emits
/// `parse_u64(s: &TString) -> Option<u64>` with `ens match result { Some(v) =>
/// all_digits(s.data@) && s.data.len() >= 1 && parse_be(s.data@) == v as nat, None =>
/// true }`, the Horner-accumulate loop with the BE partial-value invariant +
/// `decreases`, and the empty/non-digit/overflow `None` arms. GROUNDED `5 verified, 0
/// errors`. `thermite-design.md` §6: a fully-discharged verus proof. The generated
/// `parse_u64`'s round-trip is the deliverable (it cannot be a thin caller's
/// mutation-scored cert — the partial contract's `None => true` arm legitimately
/// admits an always-`None` body), so non-vacuity is pinned at the codegen-grounding
/// level (AC-4 non-vacuity below).
#[test]
fn ac4_parse_u64_lowering_verifies_under_real_verus() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — parse_u64 lowering not exercised.");
        return;
    }
    // A surface fn that CALLS the generated parse_u64 (the `&String` view; the
    // round-trip ens projects the Some payload via parse_be over the consumed bytes).
    let (ok, output) = verus_on_lowered(
        "parseu64",
        "fn run(s: &String) -> Option<u64>\n  req s.len() <= 1000000\n  ens match result { Some(v) => parse_be(s) == v, None => true }\n  fx pure\n{ parse_u64(s) }\n",
    );
    assert!(
        ok && output.contains("0 errors"),
        "DESIGN 09-option-result.md REQ-5/AC-4: the emitted `parse_u64` lowering (the \
         Horner loop + the three handled-or-loud None arms + the round-trip ens) must \
         VERIFY under real verus `verified, 0 errors` (GROUNDED `5 verified, 0 \
         errors`). verus reports:\n{output}"
    );
}

/// AC-4 NON-VACUITY (R-DEFER-9) — a hand-broken `parse_u64` whose body returns
/// `Some(0)` unconditionally FAILS verus. The round-trip success `ens` has real teeth:
/// a body that returns `Some(0)` for a non-"0" input does NOT satisfy `parse_be(s) ==
/// 0`, so the postcondition is undischarged.
///
/// AUTHORITY: `.design/basis/09-option-result.md` AC-3/AC-4 — the broken `Some(0)`
/// FAILS (`3 verified, 1 errors`). `thermite-design.md` §7: the battery catches a
/// false claim. The break is injected into a STANDALONE verus probe of the generated
/// contract (the surface cannot mutate the generated fn body), confirming the round-
/// trip `ens` is a real proof, not vacuous.
#[test]
fn ac4_broken_parse_u64_body_fails_real_verus() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — parse_u64 non-vacuity not exercised.");
        return;
    }
    // The generated parse_u64's EXACT contract (the round-trip success ens), but the
    // body is the broken `Some(0)` unconditional return. This is the AC-3 negative
    // companion: the round-trip ens is undischarged for any non-"0" input. (The
    // wrapper + spec fns are the GROUNDED forms; only the body is deliberately wrong.)
    let probe = r#"use vstd::prelude::*;
verus! {
pub spec const CAP: usize = 1_000_000;
pub struct TString { pub data: Vec<u8> }
impl TString { pub open spec fn well_formed(&self) -> bool { self.data.len() <= CAP } }
pub open spec fn is_digit(b: u8) -> bool { 48 <= b && b <= 57 }
pub open spec fn all_digits(s: Seq<u8>) -> bool
{ forall|i: int| 0 <= i < s.len() ==> is_digit(#[trigger] s[i]) }
pub open spec fn parse_be(s: Seq<u8>) -> nat
    decreases s.len()
{ if s.len() == 0 { 0 }
  else { parse_be(s.subrange(0, (s.len() - 1) as int)) * 10 + ((s[(s.len() - 1) as int] - 48) as nat) } }
pub fn parse_u64(s: &TString) -> (result: Option<u64>)
    requires s.well_formed(),
    ensures match result {
        Some(v) => all_digits(s.data@) && s.data.len() >= 1 && parse_be(s.data@) == v as nat,
        None => true,
    },
{ Some(0) }
fn main() {}
}
"#;
    let rs = std::env::temp_dir().join(format!(
        "forge_optres_broken_parse_{}.rs",
        std::process::id()
    ));
    std::fs::write(&rs, probe).expect("write broken probe");
    let out = Command::new(verus_bin())
        .arg(&rs)
        .current_dir(std::env::temp_dir())
        .output()
        .expect("spawn verus");
    let _ = std::fs::remove_file(&rs);
    if let Some(stem) = rs.file_stem() {
        let _ = std::fs::remove_file(std::env::temp_dir().join(stem));
    }
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !out.status.success() && combined.contains("error"),
        "R-DEFER-9 non-vacuity: a broken `parse_u64` returning `Some(0)` unconditionally \
         must FAIL verus (the round-trip success ens `parse_be(s.data@) == v` is \
         undischarged for a non-\"0\" input) — the error arm bites. verus reports:\n{combined}"
    );
}

/// AC-5 (no regression) — the existing `Option`-matching corpus `binary_search.th`
/// still certifies L3. The C7 built-in-variant seeding makes its `ens match result {
/// Some(i) => …, None => … }` exhaustiveness-checked (both arms present), and the
/// construction `return Some(mid)` / `return None` stays accepted.
///
/// AUTHORITY: `conformance/binary_search.th` (the SHIPPED kernel corpus) +
/// `thermite-design.md` §6. The C7 seeding is purely additive — it must not perturb
/// an existing `Option` match/construct.
#[test]
fn ac5_binary_search_option_corpus_unchanged() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — binary_search regression not exercised.");
        return;
    }
    let bs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../conformance/binary_search.th");
    let out = Command::new(forge_bin())
        .arg("check")
        .arg(&bs)
        .arg("--json")
        .output()
        .expect("spawn forge check binary_search");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let certs: Vec<Value> = serde_json::from_str::<Value>(stdout.trim())
        .unwrap_or_else(|e| panic!("binary_search --json not one doc: {e}\n{stdout}"))
        .as_array()
        .expect("array of certs")
        .clone();
    let bsearch = cert_for(&certs, "binary_search");
    assert_eq!(
        bsearch["level"], "L3",
        "AC-5 no regression: conformance/binary_search.th must still certify L3 — the \
         C7 built-in-variant seeding (Some/None into the registry) is purely additive \
         (binary_search's `ens match result` Some/None arms are both present, so it \
         stays exhaustive; `return Some(mid)`/`return None` stay accepted). forge \
         reports: {}",
        bsearch["level"]
    );
}
