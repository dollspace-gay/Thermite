//! `forge/tests/goal_repl.rs` — conformance for the goal-state REPL increments
//! (i)+(ii) (#193; `.design/forge/goal-repl.md` REQ-1/REQ-2/REQ-3/REQ-7). Drives
//! the real `forge` binary against the corpus:
//!
//! - `forge battery conformance/sum.th` reports the SAME §7 verdict that `forge
//!   check` computes internally — a VIEW, NEVER a re-derivation (AC-1). The
//!   non-vacuous BOOLEANS are oracle fields anchored to `conformance/sum.cert.json`
//!   (`oracle_subset`); the mutation KILL-RATIO is tool-computed and EXCLUDED from
//!   the cert oracle (`conformance/README.md` — the golden `17/18` is illustrative,
//!   not enforced), so its fidelity is asserted CROSS-VERB (battery ratio == check
//!   ratio from the same binary), NEVER literal-copied (R-CHAR-3).
//! - `forge goal conformance/sum.th sum` renders the §5.1 GOAL STATE: the `given`
//!   (`req`) + `want` (`ens`) source text and `ALL GOALS DISCHARGED` (AC-2).
//! - `forge edit` resolves a semantic address, splices the replacement clause at
//!   its span, re-emits, and re-checks (REQ-3 / AC-4).
//! - a bad address is an honest structured error, never a panic (REQ-7 / AC-4).
//!
//! The verus-backed assertions SKIP LOUDLY when verus is absent (the
//! `acceptance_programs` convention); the address / bad-address paths do not need
//! verus and always run.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

fn sum_th() -> PathBuf {
    repo_root().join("conformance/sum.th")
}

fn binary_search_th() -> PathBuf {
    repo_root().join("conformance/binary_search.th")
}

/// `true` iff verus is reachable (mirrors `acceptance_programs.rs`).
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

struct Run {
    stdout: String,
    success: bool,
}

fn run_forge(args: &[&str]) -> Run {
    let out = Command::new(forge_bin())
        .args(args)
        .output()
        .expect("spawn forge");
    Run {
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        success: out.status.success(),
    }
}

/// `forge check <file> --json`, returning the `sum` cert's mutation kill-ratio
/// (the tool-computed, oracle-EXCLUDED value the battery view must mirror).
fn check_sum_mutants_killed() -> String {
    let out = Command::new(forge_bin())
        .args(["check", sum_th().to_str().unwrap(), "--json"])
        .output()
        .expect("spawn forge check");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let certs: serde_json::Value = serde_json::from_str(&stdout).expect("forge check --json array");
    for c in certs.as_array().expect("cert array") {
        if c["item"] == "sum" {
            return c["contract_quality"]["mutants_killed"]
                .as_str()
                .expect("mutants_killed string")
                .to_string();
        }
    }
    panic!("no `sum` cert in forge check output:\n{stdout}");
}

// REQ-1 / AC-1: `forge battery` is a VIEW over the SAME verdict `forge check`
// computes. The non-vacuous BOOLEANS are oracle fields (anchored to the golden
// sum.cert.json: tautology=false, vacuous_precondition=false). The mutation ratio
// is tool-computed + oracle-EXCLUDED, so its fidelity is asserted CROSS-VERB
// (battery ratio == check ratio from the same binary), NEVER literal-copied
// (R-CHAR-3).
#[test]
fn battery_view_matches_check_verdicts() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — `forge battery` verdict not exercised (#193 AC-1).");
        return;
    }
    let run = run_forge(&["battery", sum_th().to_str().unwrap(), "sum"]);
    assert!(run.success, "forge battery should query successfully");
    // Oracle field (golden sum.cert.json): the contract is non-vacuous.
    assert!(
        run.stdout.contains("non-vacuous"),
        "battery view must report non-vacuous (golden oracle field); got:\n{}",
        run.stdout
    );
    // View fidelity: the battery ratio is EXACTLY the ratio forge check reports
    // for the same item (a VIEW, not a re-derivation). Cross-derived from the same
    // binary; never a literal copy of the illustrative golden `17/18`.
    let check_ratio = check_sum_mutants_killed();
    assert!(
        run.stdout
            .contains(&format!("mutants killed: {check_ratio}")),
        "battery view must mirror forge check's kill-ratio `{check_ratio}`; got:\n{}",
        run.stdout
    );
}

// REQ-2 / AC-2: `forge goal sum` renders the §5.1 GOAL STATE — given (`req`) +
// want (`ens`) source text and ALL GOALS DISCHARGED for the clean corpus item.
#[test]
fn goal_render_discharged_for_sum() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — `forge goal` discharge not exercised (#193 AC-2).");
        return;
    }
    let run = run_forge(&["goal", sum_th().to_str().unwrap(), "sum"]);
    assert!(run.success, "forge goal should query successfully");
    assert!(
        run.stdout.contains("GOAL STATE — sum"),
        "goal render must head with the item; got:\n{}",
        run.stdout
    );
    // given/want from the parsed contract (the §5.1 four-part view).
    assert!(
        run.stdout.contains("given: xs.len() <= 1_000_000"),
        "goal render must show the req clause as `given`; got:\n{}",
        run.stdout
    );
    assert!(
        run.stdout.contains("result == spec_sum(xs)"),
        "goal render must show the ens clause as `want`; got:\n{}",
        run.stdout
    );
    assert!(
        run.stdout.contains("ALL GOALS DISCHARGED"),
        "a clean L3 item renders ALL GOALS DISCHARGED; got:\n{}",
        run.stdout
    );
    assert!(
        run.stdout.contains("certified L3"),
        "the discharged render carries the L3 level; got:\n{}",
        run.stdout
    );
    // The §5.1 contract-score line (the inline battery view). The ratio mirrors
    // forge check's tool-computed value (oracle-excluded — cross-verb, R-CHAR-3).
    let check_ratio = check_sum_mutants_killed();
    assert!(
        run.stdout
            .contains(&format!("mutants killed {check_ratio}")),
        "the goal render carries the §7 contract score inline (`{check_ratio}`); got:\n{}",
        run.stdout
    );
}

// REQ-3 / AC-4: `forge edit <addr> --replace <code>` resolves the address,
// splices the new clause at its span, re-emits, and the re-emitted file
// re-parses + re-checks. We operate on a TEMP COPY (the verb mutates the file in
// place) so the corpus stays pristine.
#[test]
fn edit_splices_clause_and_rechecks() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — `forge edit` re-check not exercised (#193 AC-4).");
        return;
    }
    let tmp = std::env::temp_dir().join(format!("forge_goal_edit_{}.th", std::process::id()));
    std::fs::copy(binary_search_th(), &tmp).expect("copy binary_search.th to temp");

    // Replace inv#2 with a semantically-equivalent reformulation (keeps the proof
    // sound, exercises the splice + re-check). The replacement is well-formed
    // Thermite inv-clause source text.
    let run = run_forge(&[
        "edit",
        tmp.to_str().unwrap(),
        "binary_search.loop#1.inv#2",
        "--replace",
        "forall_below(haystack, lo, |x| x < needle)",
    ]);
    assert!(
        run.success,
        "forge edit + re-check should succeed; got:\n{}",
        run.stdout
    );
    assert!(
        run.stdout.contains("GOAL STATE — binary_search"),
        "edit prints the re-checked goal state; got:\n{}",
        run.stdout
    );

    // The file on disk now carries the spliced clause (the in-place edit).
    let after = std::fs::read_to_string(&tmp).expect("read edited temp file");
    assert!(
        after.contains("forall_below(haystack, lo, |x| x < needle)"),
        "the spliced clause must be present in the re-emitted file; got:\n{after}"
    );
    let _ = std::fs::remove_file(&tmp);
}

// REQ-7 / AC-4: a bad address is an honest structured error + a non-success exit,
// never a panic. Does NOT need verus (the resolver rejects before any check).
#[test]
fn edit_bad_address_is_honest_error() {
    let tmp = std::env::temp_dir().join(format!("forge_goal_badaddr_{}.th", std::process::id()));
    std::fs::copy(binary_search_th(), &tmp).expect("copy binary_search.th to temp");

    let out = Command::new(forge_bin())
        .args([
            "edit",
            tmp.to_str().unwrap(),
            "binary_search.loop#9",
            "--replace",
            "lo <= hi",
        ])
        .output()
        .expect("spawn forge edit");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "a bad address must NOT exit success; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("no such address `binary_search.loop#9`"),
        "the error must name the unresolvable address; stderr:\n{stderr}"
    );
    // The file must be UNTOUCHED — a failed resolve never mutates the source.
    let after = std::fs::read_to_string(&tmp).expect("read temp file");
    let before = std::fs::read_to_string(binary_search_th()).expect("read corpus");
    assert_eq!(after, before, "a failed edit must not mutate the file");
    let _ = std::fs::remove_file(&tmp);
}

// REQ-2 / REQ-7: `forge goal` on an unknown item is an honest Usage error naming
// the known items, never a silent empty render. No verus needed (selection
// happens after the check; but check needs verus, so guard).
#[test]
fn goal_unknown_item_is_honest_error() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — `forge goal` unknown-item path not exercised.");
        return;
    }
    let out = Command::new(forge_bin())
        .args(["goal", sum_th().to_str().unwrap(), "no_such_item"])
        .output()
        .expect("spawn forge goal");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "unknown item must not exit success");
    assert!(
        stderr.contains("no checked item named `no_such_item`"),
        "the error must name the missing item; stderr:\n{stderr}"
    );
}
