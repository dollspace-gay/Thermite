//! Divergence pin (crosslink #92, the diagnostics half) — a per-item verus
//! harness was named after the first item of the woven sub-program rather than
//! the item it checks, so a failure reported its source location under a
//! sibling's name.
//!
//! `run_verus` took its scratch-dir/`.rs` stem from
//! `program.items.first().map(|i| i.name())`. `item_subprogram` weaves the ADT
//! decls and spec fns first and pushes the checked item last, so any item with
//! anything woven ahead of it reported diagnostics under the wrong name:
//!
//!   spec fn helper(n: u64) -> u64 { n }
//!   fn bad(x: u64) -> u64 ens result == helper(x) + 1 { x }   // cannot hold
//!
//! `bad`'s sub-program is `[helper, bad]`, so `bad`'s failed postcondition was
//! reported at `helper_check.rs:13:9` — naming a function that certified L3.
//!
//! This is not cosmetic. It is what made #92 look like two bugs instead of one:
//! the reporter saw `E0425` under `/tmp/forge_is_owner_check_*/is_owner_check.rs`
//! while `is_owner` certified L3, and reasonably concluded that a harness which
//! failed to compile could coexist with an L3 on the same item — i.e. that the
//! L3 refusal path had a hole. It does not. The failing harness belonged to
//! `Unused` (correctly L0); only its NAME pointed at `is_owner`. A diagnostic
//! that misattributes its subject sends the next reader hunting a defect that
//! is not there.
//!
//! The fix drops the sub-program from `run_verus` entirely and passes the
//! checked item's name, so the harness cannot name a merely-woven member even by
//! accident.
//!
//! The authority (R-CHAR-3): `.design/forge/check.md` REQ-4 requires each failing
//! obligation to carry "the obligation description and the concrete failure
//! witness … the failed obligation and its source position" — a position under a
//! sibling's filename does not identify the subject. AC-4 (the crate-name gotcha:
//! stem has no `.`) is preserved and independently pinned by
//! `crate_stem_has_no_dot_and_is_valid`. The expected stem `bad_check` is derived
//! from the fixture's own item name plus the documented `<stem>.rs` scheme
//! (Amendment item 6), never copied from the toolchain's output.
//!
//! Verus check skips with an eprintln when verus is absent (`editor_runs.rs`
//! precedent). `tests/` is not anti-pattern-gated (R-APG-2).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// `true` iff verus is reachable (mirrors `divergence_multi_adt_subprogram.rs`).
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

/// A spec fn (woven first) plus an exec fn whose `ens` cannot hold. `bad`'s
/// sub-program is `[helper, bad]`, so pre-#92 its diagnostics were filed under
/// `helper_check.rs`.
const WOVEN_AHEAD_PROGRAM: &str = "\
spec fn helper(n: u64) -> u64
  dec n
{
  n
}

fn bad(x: u64) -> u64
  req true
  ens result == helper(x) + 1
  fx  pure
{
  x
}
";

fn check_program(tag: &str, program: &str) -> Vec<Value> {
    let fixture = std::env::temp_dir().join(format!(
        "forge_harness_naming_{tag}_{}.th",
        std::process::id()
    ));
    std::fs::write(&fixture, program).expect("write fixture");
    let out = Command::new(forge_bin())
        .arg("check")
        .arg(&fixture)
        // This test pins the generated Verus harness filename. Automatic EPR
        // countermodels carry canonical clause addresses instead, so select the
        // backend whose diagnostic path is under test.
        .arg("--engine")
        .arg("verus")
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

/// Every `location` string across an item's obligations.
fn locations_of(certs: &[Value], item: &str) -> Vec<String> {
    certs
        .iter()
        .find(|c| c.get("item").and_then(|v| v.as_str()) == Some(item))
        .unwrap_or_else(|| panic!("no cert for `{item}` in {certs:#?}"))
        .get("obligations")
        .and_then(|o| o.as_array())
        .unwrap_or_else(|| panic!("cert for `{item}` has no obligations array"))
        .iter()
        .filter_map(|o| o.get("location").and_then(|v| v.as_str()))
        .map(|s| s.to_string())
        .collect()
}

#[test]
fn a_failing_obligation_is_located_in_the_checked_items_harness() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — harness-naming pin not run.");
        return;
    }
    let certs = check_program("woven_ahead", WOVEN_AHEAD_PROGRAM);
    let locs = locations_of(&certs, "bad");
    assert!(
        !locs.is_empty(),
        "`bad`'s ens cannot hold, so it must carry a located failed obligation \
         (.design/forge/check.md REQ-4); got {locs:?}"
    );
    for loc in &locs {
        assert!(
            loc.starts_with("bad_check."),
            "a failed obligation must be located in the harness of the item under \
             check (`bad_check.rs`), not in a merely-woven sibling's; got `{loc}`"
        );
        // The specific pre-#92 misattribution: `helper` is woven first and
        // certifies L3, yet lent its name to `bad`'s failure.
        assert!(
            !loc.contains("helper"),
            "the harness must not be named after a woven spec fn; got `{loc}`"
        );
    }
}

#[test]
fn the_woven_spec_fn_still_certifies_and_owns_its_own_harness() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — harness-naming sibling check not run.");
        return;
    }
    // The other half of the misread: `helper` is fine. Pinning it here makes the
    // pair explicit — one item failed, a different item was named, and the two
    // facts were easy to fuse into a phantom "L3 coexisting with a broken
    // harness" bug.
    let certs = check_program("sibling", WOVEN_AHEAD_PROGRAM);
    let level = certs
        .iter()
        .find(|c| c.get("item").and_then(|v| v.as_str()) == Some("helper"))
        .expect("cert for helper")["level"]
        .as_str()
        .expect("string level");
    assert_eq!(
        level, "L3",
        "the woven spec fn certifies on its own merits (.design/forge/check.md REQ-5)"
    );
}
