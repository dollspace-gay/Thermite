//! Divergence pin (#127): `callee_takes_string_byteview` (`thermite-lower/src/lower.rs`)
//! gates the `String -> s.data@` byte-view rewrite on a FIXED callee NAME set
//! (`parse_le`/`parse_be`/`all_digits`/`is_digit`/`occurs_at`/`contains_sub`/
//! `count_sep`/`sep_free` — the generated C4/C5/C7 defs). A USER-DEFINED `spec fn`
//! whose name COLLIDES with that set but whose param is a `&String` (the #126
//! String-scanning shape) is MISROUTED: its `&String` self-call argument is rewritten
//! to `.data@` (`Seq<u8>`) instead of being passed through as the `&TString`
//! reference its own lowered signature declares — so the body emits
//! `is_digit(s.data@, ..)` against an `is_digit(&TString, ..)` param (E0308) and the
//! spec fn FAILS to certify.
//!
//! AUTHORITY: `.design/basis/07-strings.md` REQ-4 — "a String-SCANNING `spec fn`
//! (`byte_at`/`len` over a `&String` param) MUST lower correctly and certify L3".
//! The contract is SHAPE-derived ("the SHAPE-derived set whose spec-position
//! `.len()`/`.byte_at(i)`/`.slice(..)` rewrite to the wrapper's SPEC accessors"),
//! NOT name-derived. `thermite-design.md` §6 — L3 == fully-discharged real-verus
//! proof. The #126 commit's own claim ("Keyed on the callee NAME (these names are
//! reserved by the generated defs — no user collision in v0.1)") is the unproven
//! assumption this test refutes: a user spec-fn name lives in the user namespace and
//! the surface does not reserve `is_digit`/`count_sep`/etc.
//!
//! THE DIVERGENCE (R-CHAR-3, not copied from forge's output): two programs that are
//! BYTE-IDENTICAL up to the spec fn's NAME certify DIFFERENTLY — the non-colliding
//! `scan_x` certifies L3 (the #126(A) payoff, the `spec_scan` shape), the colliding
//! `is_digit` certifies L0 (the byte-view misroute). A correct SHAPE-keyed dispatch
//! certifies both at L3.
//!
//! The verus check SKIPS LOUDLY when verus is absent (R-CODE-4); `tests/` is not
//! anti-pattern-gated so `unwrap`/`panic!` are fine (R-APG-2).

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

fn check_program(tag: &str, program: &str) -> Vec<Value> {
    let fixture = std::env::temp_dir().join(format!(
        "forge_byteview_collide_{tag}_{}.th",
        std::process::id()
    ));
    std::fs::write(&fixture, program).expect("write fixture");
    let out = Command::new(forge_bin())
        .arg("check")
        .arg(&fixture)
        .arg("--json")
        .output()
        .expect("spawn forge check");
    let _ = std::fs::remove_file(&fixture);
    let stdout = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str::<Value>(stdout.trim())
        .unwrap_or_else(|e| {
            panic!("[{tag}] forge check --json stdout must be one JSON doc: {e}\nstdout:\n{stdout}")
        })
        .as_array()
        .unwrap_or_else(|| panic!("[{tag}] forge check --json must emit an array"))
        .clone()
}

fn level_of(certs: &[Value], item: &str) -> String {
    certs
        .iter()
        .find(|c| c.get("item").and_then(|v| v.as_str()) == Some(item))
        .unwrap_or_else(|| panic!("no cert for `{item}` in {certs:#?}"))["level"]
        .as_str()
        .unwrap_or_else(|| panic!("cert for `{item}` has no string level"))
        .to_string()
}

/// A String-scanning `spec fn` whose NAME is a placeholder `__NAME__` substituted
/// per-program. The body is the proven `spec_scan`/`spec_line_start` shape (a
/// recursive `byte_at` `\n`-style scan over a `&String` param with a `dec` measure)
/// — the EXACT shape #126 made certify. The only thing that varies between the two
/// programs below is the name.
const TEMPLATE: &str = "\
spec fn __NAME__(s: &String, i: u64, target: u64, acc: u64) -> u64
  dec target - i
{
  if i >= target {
    acc
  } else {
    if s.byte_at(i) == 120 {
      __NAME__(s, i + 1, target, i + 1)
    } else {
      __NAME__(s, i + 1, target, acc)
    }
  }
}
";

#[test]
fn user_spec_fn_named_like_a_generated_byteview_fn_still_certifies_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — #127 byteview name-collision not run.");
        return;
    }

    // CONTROL: a non-colliding name certifies L3 (the #126(A) payoff — a String-
    // scanning spec fn over a `&String` param lowers + proves).
    let control = TEMPLATE.replace("__NAME__", "scan_x");
    let control_certs = check_program("control", &control);
    assert_eq!(
        level_of(&control_certs, "scan_x"),
        "L3",
        "CONTROL: a non-colliding String-scanning spec fn must certify L3 (the #126(A) \
         shape); if this fails the divergence fixture is wrong, not the toolchain:\n{control_certs:#?}"
    );

    // DIVERGENCE: the SAME spec fn, renamed to `is_digit` — a name in the fixed
    // `callee_takes_string_byteview` set — MUST still certify L3 by AUTHORITY
    // (`.design/basis/07-strings.md` REQ-4: the dispatch is SHAPE-derived). It does
    // NOT today (the `&String` self-call arg is misrouted to `.data@`, E0308 -> L0).
    let collide = TEMPLATE.replace("__NAME__", "is_digit");
    let collide_certs = check_program("collide", &collide);
    assert_eq!(
        level_of(&collide_certs, "is_digit"),
        "L3",
        "a user String-scanning spec fn whose name COLLIDES with the generated \
         byte-view set (`is_digit`) must STILL certify L3 — the byte-view dispatch \
         must be keyed on the &String param SHAPE, not the callee NAME (#127):\n{collide_certs:#?}"
    );
}
