//! acto-critic divergence tests for `forge check` on the bounded-`String` corpus
//! (commit `b8c3bf7`, Basis Stage 7 / issue #79).
//!
//! Each test pins a divergence (or CONFIRMS a genuine guarantee) between the LIVE
//! per-item `forge check` certificate and the authority chain
//! (`.design/basis/07-strings.md`, the hand-derived oracle
//! `conformance/string/cases.json`, `thermite-design.md` §6/§7). Expected values
//! trace to the oracle / design, NEVER to forge's own output (`goal.md` R-CHAR-3).
//!
//! These run the BUILT `forge` binary end-to-end (verus-backed). If verus is
//! absent they SKIP LOUDLY (never panic on a missing solver), matching
//! `divergence_collections.rs` / `divergence_forge.rs`.
//!
//! ROOT CAUSE (the headline pinned below): the builder's `string_conformance.rs`
//! tests exercise only WHOLE-PROGRAM `thermite_lower::lower` + a direct `verus`
//! run plus the raw `cases.json` text — they NEVER run the per-item
//! `forge::check::check_file` ladder. On that real path `join` (a `String` return)
//! PROVES at verus (the golden `tests/golden/lower/string_demo.verus.rs` is
//! `11 verified, 0 errors`), but #12 MUTATION SCORING cannot synthesize an
//! early-return mutant for a `Type::String` return type: `forge::mutation::
//! early_return_value` has a `Type::Vec` arm (#74) and a `Type::Ref`-slice arm
//! (#48) but NO `Type::String` arm, and `zero_value_for` has no `String` case, so
//! `join`'s body (`a.concat(b)`, no binop / off-by-one / branch site) yields ZERO
//! mutants → `0/0` → the #48 anti-Goodhart backstop gates it `WeakContract` →
//! `Level::L0`. The oracle says `join` → L3 alloc. This is the EXACT #74 class
//! (the `Type::Vec` mutation-synthesis gap), unfixed for `Type::String`.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// `true` iff verus is reachable (mirrors `divergence_collections.rs`).
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

/// DIVERGENCE (THE HEADLINE) — `join` (a `String`-returning `concat`) PROVES at
/// verus (L3) on the per-item path, but the #12 mutation scorer cannot synthesize
/// a mutant for its `Type::String` return type (`0/0`), so the #48 backstop gates
/// the L3-proved cert to a `WeakContract` reject → `Level::L0`.
///
/// AUTHORITY: `conformance/string/cases.json` (the hand-derived R-CHAR-3 oracle) —
/// `{ "name": "join", "level": "L3", "effects": ["alloc"] }` ("concat: req
/// a.len()+b.len() <= 1_000_000; ens result.len() == a.len()+b.len() by concat's
/// spec. fx alloc ... L3."). Also `.design/basis/07-strings.md` AC-3 ("`forge
/// check` certifies `join` L3 with `effects: [alloc]`") and REQ-4 (the bounded
/// `concat` with the length identity). The verus proof DOES succeed — the golden
/// `tests/golden/lower/string_demo.verus.rs` is `11 verified, 0 errors` — so the
/// L0 is NOT a verus/composition failure; it is the mutation-gate divergence.
///
/// Tracking: #80
#[ignore = "blocker #80 — un-ignore when fixed"]
#[test]
fn divergence_join_l3_not_mutation_gated_l0() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — join per-item certification not exercised.");
        return;
    }
    let certs = check_json_file(&corpus_path("string_demo.th"));
    let join = cert_for(&certs, "join");
    // ORACLE (cases.json): join certifies L3 with fx alloc.
    assert_eq!(
        join["level"], "L3",
        "ORACLE conformance/string/cases.json: join -> L3 (the bounded concat; \
         verus PROVES it, golden string_demo.verus.rs is `11 verified, 0 errors`). \
         forge check reports: {} (mutants_killed={}, reject={}). ROOT CAUSE: \
         mutation::early_return_value has no Type::String arm (the #74 Type::Vec \
         gap, unfixed for String) -> 0/0 -> WeakContract gate.",
        join["level"],
        join.get("contract_quality")
            .and_then(|q| q.get("mutants_killed"))
            .unwrap_or(&Value::Null),
        join.get("reject")
            .and_then(|r| r.get("cause"))
            .unwrap_or(&Value::Null),
    );
    assert_eq!(
        join["effects"],
        serde_json::json!(["alloc"]),
        "ORACLE: join carries fx alloc (the concatenated TString allocates — the \
         Stage-1 Alloc effect)."
    );
}

/// CONFIRMATION (NOT a divergence — this MUST pass today) — `greeting_len` /
/// `first_byte` / `literal_len` certify exactly as the oracle says. This pins that
/// the three non-`String`-return string items are honest on the live ladder (only
/// `join`, the `String`-return, is mutation-gate-broken), so the headline test
/// isolates the `Type::String` gap rather than a whole-stage failure.
///
/// AUTHORITY: `conformance/string/cases.json` — greeting_len/first_byte L3 pure;
/// literal_len L3 alloc.
#[test]
fn confirm_string_non_join_items_certify_per_oracle() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — string non-join certification not exercised.");
        return;
    }
    let certs = check_json_file(&corpus_path("string_demo.th"));

    let gl = cert_for(&certs, "greeting_len");
    assert_eq!(gl["level"], "L3", "ORACLE: greeting_len -> L3");
    assert_eq!(gl["effects"], serde_json::json!(["pure"]));

    let fb = cert_for(&certs, "first_byte");
    assert_eq!(fb["level"], "L3", "ORACLE: first_byte -> L3 (no-OOB byte_at)");
    assert_eq!(fb["effects"], serde_json::json!(["pure"]));

    let ll = cert_for(&certs, "literal_len");
    assert_eq!(
        ll["level"], "L3",
        "ORACLE: literal_len -> L3 (the Expr::StrLit \"hello\" -> 5 bytes, len()==5)"
    );
    assert_eq!(ll["effects"], serde_json::json!(["alloc"]));
}

/// CONFIRMATION (NOT a divergence — this MUST pass today) — the no-OOB `byte_at`
/// is GENUINELY bounds-checked (the editor's core safety, R-DEFER-9 non-vacuity),
/// not a no-op laundered to L3:
///   (1) `first_byte` (`req s.len() > 0` discharges `byte_at(0)`'s `0 < len`) -> L3;
///   (2) the OOB negative — `byte_at(0)` with NO `req s.len() > 0` — leaves
///       `byte_at`'s `i < len` precondition undischarged -> verus FAILS -> L0;
///   (3) an OFF-BY-ONE bound — `req i <= s.len()` then `s.byte_at(i)` — still
///       leaves `i < len` undischarged (`i == len` is OOB) -> verus FAILS -> L0.
/// The (2)/(3) L0s are verus `precondition not satisfied` failures (a genuine
/// bounds-check), NOT a mutation-gate / no-op. This is NOT `#[ignore]`d — it passes
/// against `b8c3bf7` (the honest behavior); if a future change launders the bound
/// it goes red.
///
/// AUTHORITY: `conformance/string/cases.json` — `first_byte` -> L3/pure; the
/// `reject` entry `oob_byte_at_no_req` -> L0 ("a missing bound is caught, not
/// laundered to L3"). `.design/basis/07-strings.md` AC-2/AC-4 (the no-OOB `byte_at`
/// is real; the unguarded form FAILS verus).
#[test]
fn confirm_byte_at_bound_is_load_bearing() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — byte_at bound-check not exercised.");
        return;
    }
    // (1) the faithful guarded accessor: L3 / pure.
    let certs = check_json_file(&corpus_path("string_demo.th"));
    let fb = cert_for(&certs, "first_byte");
    assert_eq!(
        fb["level"], "L3",
        "ORACLE: first_byte -> L3 (req s.len()>0 discharges byte_at(0)'s bound)."
    );

    // (2) the OOB negative — the oracle's `oob_byte_at_no_req.program` (R-CHAR-3):
    //     no `req s.len() > 0` -> byte_at's `0 < len` undischarged -> L0.
    // (3) an OFF-BY-ONE bound `req i <= s.len()` -> `i < len` undischarged
    //     (`i == len` is OOB) -> L0 (the bound is genuinely load-bearing).
    let fixture =
        std::env::temp_dir().join(format!("forge_div_str_oob_{}.th", std::process::id()));
    std::fs::write(
        &fixture,
        "fn oob_byte_at_no_req(s: String) -> u64\n  req true\n  ens result == s.byte_at(0)\n  fx  pure\n{\n  s.byte_at(0)\n}\n\nfn oob_byte_at_offbyone(s: String, i: usize) -> u64\n  req i <= s.len()\n  ens result == s.byte_at(i)\n  fx  pure\n{\n  s.byte_at(i)\n}\n",
    )
    .expect("write fixture");
    let certs = check_json_file(&fixture);
    let _ = std::fs::remove_file(&fixture);

    let no_req = cert_for(&certs, "oob_byte_at_no_req");
    assert_eq!(
        no_req["level"], "L0",
        "ORACLE oob_byte_at_no_req reject: no `req s.len() > 0` -> byte_at(0) \
         unproven in-bounds -> L0 (caught, not laundered). forge reports: {}",
        no_req["level"]
    );
    let off = cert_for(&certs, "oob_byte_at_offbyone");
    assert_eq!(
        off["level"], "L0",
        "R-DEFER-9 non-vacuity: an off-by-one `req i <= s.len()` does NOT discharge \
         byte_at's `i < len` (`i == len` is OOB) -> L0 (the no-OOB bound is \
         genuinely load-bearing, not laundered). forge reports: {}",
        off["level"]
    );
}
