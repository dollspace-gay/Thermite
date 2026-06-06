//! acto-critic divergence tests for `forge check` on the bounded-`Vec` corpus
//! (commit `a48d2a1`, Basis Stage 4 / issue #73).
//!
//! Each test pins a divergence between the LIVE per-item `forge check` certificate
//! and the authority chain (`.design/basis/04-collections.md`, the hand-derived
//! oracle `conformance/collections/cases.json`, `thermite-design.md` §5.3/§6/§7).
//! Expected values trace to the oracle / design, NEVER to forge's own output
//! (`goal.md` R-CHAR-3).
//!
//! These run the BUILT `forge` binary end-to-end (verus-backed). If verus is
//! absent they SKIP LOUDLY (never panic on a missing solver), matching
//! `divergence_forge.rs` / `check_conformance.rs`.
//!
//! ROOT CAUSE (pinned below): the builder's `collections_conformance.rs` test
//! exercises only the WHOLE-PROGRAM `thermite_lower::lower` + a direct `verus`
//! run, NOT the per-item `forge::check::check_file` path (the §5.3
//! `item_subprogram` pipeline). On that real path `push_one` PROVES at verus
//! (`4 verified, 0 errors`, confirmed against the golden), but #12 MUTATION
//! SCORING (`forge::mutation::early_return_value` / `zero_value_for`) cannot
//! synthesize an early-return mutant for a `Vec<u64>` (`Type::Vec`) return type,
//! yielding a `0/0` score that the #48 backstop gates to a `WeakContract`
//! reject → `Level::L0`. The oracle says `push_one` → L3. (Same class of gap as
//! the Stage-1c deposit-L0 #68 bug: a test that lowered whole-program rather than
//! running the per-item `forge check` ladder.)

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// `true` iff verus is reachable (mirrors `divergence_forge.rs`).
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

/// The corpus directory (workspace root `/conformance`).
fn corpus_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .join("conformance")
        .join(name)
}

/// Run `forge check <file> --json`, returning the parsed array of certificates.
fn check_json_file(path: &Path) -> Vec<Value> {
    let out = Command::new(forge_bin())
        .arg("check")
        .arg(path)
        .arg("--json")
        .output()
        .expect("spawn forge");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!(
            "forge --json stdout must be one JSON doc: {e}\nstdout:\n{stdout}\nstderr:\n{}",
            String::from_utf8_lossy(&out.stderr)
        )
    });
    value.as_array().expect("array of certs").clone()
}

fn cert_for<'a>(certs: &'a [Value], item: &str) -> &'a Value {
    certs
        .iter()
        .find(|c| c.get("item").and_then(|v| v.as_str()) == Some(item))
        .unwrap_or_else(|| panic!("no cert for `{item}` in {certs:?}"))
}

/// DIVERGENCE (THE HEADLINE) — the capacity-preserving `push_one` PROVES at verus
/// (L3) on the per-item path, but the #12 mutation scorer cannot synthesize a
/// mutant for its `Vec<u64>` return type (`0/0`), so the #48 backstop gates the
/// L3-proved cert to a `WeakContract` reject → `Level::L0`.
///
/// AUTHORITY: `conformance/collections/cases.json` (the hand-derived R-CHAR-3
/// oracle) — `{ "name": "push_one", "level": "L3", "effects": ["alloc"] }`. Also
/// `.design/basis/04-collections.md` AC-1 ("running the real `verus` binary on
/// the emitted output exits 0 with `N verified, 0 errors` ... the emitted
/// certificate matches `vec_accum.cert.json` (L3, non-vacuous)") and REQ-5
/// (the capacity-preserving push). The verus proof DOES succeed — the golden
/// `tests/golden/lower/vec_demo.verus.rs` is `4 verified, 0 errors` — so the L0
/// is NOT a verus/composition failure; it is the mutation-gate divergence.
///
/// Tracking: #74
#[test]
fn divergence_push_one_l3_not_mutation_gated_l0() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — push_one per-item certification not exercised.");
        return;
    }
    let certs = check_json_file(&corpus_path("vec_demo.th"));
    let push_one = cert_for(&certs, "push_one");
    // ORACLE (cases.json): push_one certifies L3 with fx alloc.
    assert_eq!(
        push_one["level"], "L3",
        "ORACLE conformance/collections/cases.json: push_one -> L3 (the \
         capacity-preserving push; verus PROVES it, golden vec_demo.verus.rs is \
         `4 verified, 0 errors`). forge check reports: {}",
        push_one["level"]
    );
    assert_eq!(
        push_one["effects"],
        serde_json::json!(["alloc"]),
        "ORACLE: push_one carries fx alloc (the Stage-1 Alloc heap, generalized)."
    );
}

/// CONFIRMATION (NOT a divergence — this MUST pass today) — `checked_get` is the
/// FAITHFUL no-OOB accessor: it certifies L3 / pure, AND the `req i < v.len()` is
/// GENUINELY load-bearing. An off-by-one bound `req i <= v.len()` leaves `get`'s
/// index precondition undischarged → L0 (NOT laundered). This test asserts both
/// the faithful L3 and the genuine bound; it pins that the accessor is real, not
/// a no-op. It is NOT `#[ignore]`d because it passes against `a48d2a1` (the
/// honest behavior) — if a future change launders the bound this goes red.
///
/// AUTHORITY: `conformance/collections/cases.json` — `checked_get` -> L3/pure
/// ("req i < v.len() discharges Vec::get's bound precondition"); the `reject`
/// entry `oob_get_no_req` -> L0 ("a missing bound is caught, not laundered to
/// L3"). `.design/basis/04-collections.md` AC-1 (the no-OOB `get` is real).
#[test]
fn confirm_checked_get_bound_is_load_bearing() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — checked_get bound-check not exercised.");
        return;
    }
    // (1) the faithful accessor: L3 / pure.
    let certs = check_json_file(&corpus_path("vec_demo.th"));
    let cg = cert_for(&certs, "checked_get");
    assert_eq!(
        cg["level"], "L3",
        "ORACLE cases.json: checked_get -> L3 (req i < v.len() discharges get's bound)."
    );
    assert_eq!(cg["effects"], serde_json::json!(["pure"]));

    // (2) the bound is GENUINELY load-bearing: an off-by-one `req i <= v.len()`
    // (the design's `oob_get_no_req`-class negative, R-DEFER-9 non-vacuity) leaves
    // get's `i < len` precondition undischarged -> L0 (caught, not laundered).
    let fixture =
        std::env::temp_dir().join(format!("forge_div_offbyone_{}.th", std::process::id()));
    std::fs::write(
        &fixture,
        "fn oob_get_offbyone(v: Vec<u64>, i: usize) -> u64\n  req i <= v.len()\n  ens result == v.get(i)\n  fx  pure\n{\n  v.get(i)\n}\n",
    )
    .expect("write fixture");
    let certs = check_json_file(&fixture);
    let _ = std::fs::remove_file(&fixture);
    let off = cert_for(&certs, "oob_get_offbyone");
    assert_eq!(
        off["level"], "L0",
        "R-DEFER-9 non-vacuity (cases.json oob_get reject class): an off-by-one \
         `req i <= v.len()` does NOT discharge get's `i < len` -> L0 (the bound is \
         genuinely load-bearing, not laundered). forge reports: {}",
        off["level"]
    );
}
