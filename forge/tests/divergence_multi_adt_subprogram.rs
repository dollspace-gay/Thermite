//! Divergence pin (crosslink #92) — no file declaring more than one ADT could
//! certify, because `forge::check`'s referrer seed for `reachable_adt_deps` was a
//! strict subset of what `item_subprogram` weaves.
//!
//! Every arm of `item_subprogram` weaves `item_spec_items`, but only the
//! `Item::SpecFn` arm seeded those spec fns as ADT referrers. An ADT reachable
//! only through a woven spec fn was therefore absent from the emitted sub-program
//! and verus could not resolve it. Three surfaces, all live-confirmed:
//!
//!   1. a checked `struct`/`enum`: `collect_item_adt_refs` is inert on an ADT decl
//!      (field types are followed by the type-graph fixed point instead), so
//!      `adt_deps` came out empty for every ADT item while the arm wove the file's
//!      whole `spec_items` — a spec fn naming a second ADT dangled, the item landed
//!      L0 on `E0425 cannot find type`, and `project assurance` was FAILED;
//!   2. a checked `fn` whose contract and body name no ADT that a woven spec fn
//!      takes: the solver-vacuity harness failed to elaborate, which
//!      `vacuity_solver::interpret_summary` refuses as undetermined — a `ForgeError`
//!      aborting the whole run;
//!   3. a checked `spec fn`: already correct, which is why a single-ADT corpus
//!      never exercised the gap.
//!
//! One ADT masks all of it: the only ADT is the checked item, which the ADT arm
//! pushes itself, so the decl is present by construction. This is the same
//! under-approximated-closure class `.design/verified/proof-backends.md` records
//! for `reachable_spec_fn_deps` walking `decl.body` without `decl.dec`.
//!
//! The authority (R-CHAR-3): expected level L3 is the design contract —
//! `.design/forge/check.md` REQ-5 (L3 iff verus reports 0 errors) +
//! `thermite-design.md` §6 (L3 == a fully-discharged real-verus proof) +
//! `.design/basis/01-adts.md` REQ-2/REQ-9 (enum -> Verus enum, match -> Verus
//! match). The corpus anchor is `conformance/multi_adt.th` +
//! `conformance/multi_adt.cert.json` (hand-derived). Adding a declaration that
//! the rest of the file does not reference cannot change what is provable about
//! the other items, so the single-ADT twin's levels ARE the oracle for the
//! two-ADT file. Expected values never copied from the toolchain's output.
//!
//! Verus checks skip with an eprintln when verus is absent (`editor_runs.rs`
//! precedent). `tests/` is not anti-pattern-gated, so `unwrap`/`panic!` are fine
//! (R-APG-2).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// `true` iff verus is reachable (mirrors `divergence_struct_inv_spec_fn_subprogram.rs`).
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

/// Run `forge check --json` over an inline program. Panics with the captured
/// stderr when the run aborts, which is itself surface 2's pre-fix symptom (a
/// `ForgeError` from the vacuity harness, not a cert array).
fn check_program(tag: &str, program: &str) -> Vec<Value> {
    let fixture = std::env::temp_dir().join(format!(
        "forge_multi_adt_subprog_{tag}_{}.th",
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
            panic!(
                "[{tag}] forge check --json stdout must be one JSON doc: {e}\nstdout:\n{stdout}\nstderr:\n{}",
                String::from_utf8_lossy(&out.stderr)
            )
        })
        .as_array()
        .unwrap_or_else(|| panic!("[{tag}] forge check --json must emit an array of certs"))
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

/// Surface 1, the reported repro: two enums where a spec fn names only the first.
/// `Unused` is declared and never mentioned again; pre-#92 it certified L0 with
/// `E0425 cannot find type Role` — the dangling reference belonging to `is_owner`,
/// which the ADT arm wove into `Unused`'s sub-program without `Role`.
const SPARE_DECL_PROGRAM: &str = "\
enum Role { Owner, Player }
enum Unused { A, B }

spec fn is_owner(r: Role) -> bool
  dec r
{
  match r { Role::Owner => true, Role::Player => false }
}

fn check(r: Role) -> bool
  req true
  ens result == is_owner(r)
  fx  pure
{
  match r { Role::Owner => true, Role::Player => false }
}
";

/// The single-ADT twin of [`SPARE_DECL_PROGRAM`], byte-identical but for the
/// spare `enum Unused` line. Its levels are the oracle for the two-ADT file.
const SINGLE_ADT_TWIN: &str = "\
enum Role { Owner, Player }

spec fn is_owner(r: Role) -> bool
  dec r
{
  match r { Role::Owner => true, Role::Player => false }
}

fn check(r: Role) -> bool
  req true
  ens result == is_owner(r)
  fx  pure
{
  match r { Role::Owner => true, Role::Player => false }
}
";

/// Surface 2: the checked exec fn `g` names no ADT at all, but the woven
/// `spec_items` include `p`, whose parameter is an `enum E`. Pre-#92 the
/// solver-vacuity harness failed to elaborate and the whole run aborted with a
/// `ForgeError` rather than emitting certs.
const EXEC_FN_UNMENTIONED_ADT: &str = "\
enum E { A, B }

spec fn p(e: E) -> bool
  dec e
{
  match e { E::A => true, E::B => false }
}

fn g(x: u64) -> u64
  req x < 10
  ens result == x
  fx  pure
{
  x
}
";

#[test]
fn spare_adt_decl_does_not_break_its_siblings() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — multi-ADT sub-program cert not run.");
        return;
    }
    let certs = check_program("spare", SPARE_DECL_PROGRAM);

    // .design/forge/check.md REQ-5 + thermite-design.md §6: a correct source
    // certifies L3. `Unused` carries no contract to discharge, exactly as `Role`
    // does; both are enum decls lowering to a Verus enum (01-adts REQ-9).
    assert_eq!(
        level_of(&certs, "Role"),
        "L3",
        "the referenced enum must certify"
    );
    assert_eq!(
        level_of(&certs, "Unused"),
        "L3",
        "an enum the file never references must certify — its sub-program weaves \
         the file's spec fns, so it must also weave the ADTs those spec fns name"
    );
    assert_eq!(level_of(&certs, "is_owner"), "L3");
    assert_eq!(level_of(&certs, "check"), "L3");
}

#[test]
fn a_spare_adt_decl_changes_no_sibling_level() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — multi-ADT twin comparison not run.");
        return;
    }
    // The oracle: adding a declaration nothing references cannot change what is
    // provable about the other items, so every shared item's level must match its
    // single-ADT twin. This is the R-CHAR-3 anchor for the levels above — the twin
    // is the external truth, not forge's own output on the two-ADT file.
    let twin = check_program("twin", SINGLE_ADT_TWIN);
    let spare = check_program("spare_cmp", SPARE_DECL_PROGRAM);
    for item in ["Role", "is_owner", "check"] {
        assert_eq!(
            level_of(&spare, item),
            level_of(&twin, item),
            "`{item}`'s level must be invariant under adding an unreferenced decl"
        );
    }
}

#[test]
fn exec_fn_certifies_when_a_woven_spec_fn_names_an_unmentioned_adt() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — vacuity-harness weave not run.");
        return;
    }
    // Pre-#92 this did not merely mis-level: `forge check` aborted with
    // "a solver-vacuity harness failed to compile/elaborate before verification".
    // `check_program` panics on a non-JSON stdout, so reaching the asserts at all
    // is half the pin.
    let certs = check_program("exec", EXEC_FN_UNMENTIONED_ADT);
    assert_eq!(level_of(&certs, "g"), "L3", "the exec fn must certify");
    assert_eq!(level_of(&certs, "E"), "L3");
    assert_eq!(level_of(&certs, "p"), "L3");
}

#[test]
fn multi_adt_corpus_program_certifies_end_to_end() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — multi_adt corpus cert not run.");
        return;
    }
    // The corpus anchor: conformance/multi_adt.th is the first corpus program
    // declaring more than one ADT. Its golden pins `authorize` (the fn-only
    // golden convention); the ADT items are pinned here.
    let corpus = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("conformance")
        .join("multi_adt.th");
    let out = Command::new(forge_bin())
        .arg("check")
        .arg(&corpus)
        .arg("--json")
        .output()
        .expect("spawn forge check");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let certs = serde_json::from_str::<Value>(stdout.trim())
        .unwrap_or_else(|e| {
            panic!(
                "forge check --json on the corpus program must be one JSON doc: {e}\nstdout:\n{stdout}\nstderr:\n{}",
                String::from_utf8_lossy(&out.stderr)
            )
        })
        .as_array()
        .expect("array of certs")
        .clone();
    for item in ["Role", "Action", "may_ban", "is_ban", "authorize"] {
        assert_eq!(
            level_of(&certs, item),
            "L3",
            "every item of the multi-ADT corpus program must certify L3"
        );
    }
}
