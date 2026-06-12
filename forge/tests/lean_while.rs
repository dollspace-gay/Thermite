//! `forge/tests/lean_while.rs` — the integration-level oracle suite for the WHILE-BODY
//! Lean exporter (`.design/verified/proof-backends.md` REQ-11 / §4.2, increment (v-b),
//! blocker #264). The v1 `while` shape exports the FIVE per-item obligations + the TWO
//! GENERATOR-PROVED composed theorems (`while_compose` / `loopDenote_exits_of_dec`); a
//! LINEAR-family item certifies L3-via-lean-auto end-to-end.
//!
//! `forge` is a BINARY crate (no lib target), so the in-process `LeanEngine` API is NOT
//! reachable from an integration test — the live in-process verdicts (O-1 strong, O-5
//! the narrowed boundary) live as `#[cfg(test)]` unit tests in `forge/src/engine.rs`
//! (`live_while_body_item_is_honest`, `while_body_item_refuses_export`,
//! `while_refusal_inventory_is_structured`, `live_while_true_vacuity_is_not_proven`) and
//! are CROSS-REFERENCED here, not duplicated. This integration file carries the oracles
//! that DO live at the binary / external-artifact boundary — the `forge check … --engine
//! lean --json` certificate shape:
//!
//! - **O-1** the LINEAR-family `count` item certifies L3 via lean-auto through the CLI
//!   (lake-gated, LOUD skip); an HONEST assertion on `conformance/sum.th` (its
//!   recursive-registry `ens result == spec_sum(xs)` is the §4 interactive residual — it
//!   does NOT certify L3-via-lean, the honest landing per R-1).
//! - **O-2** the FULL refusal matrix — every OUT-of-v1 shape does NOT certify L3-via-lean
//!   (it is the honest `Unknown`/degrade skip, never a false verdict).
//! - **O-3** the while-true vacuity fixture is NEVER `Proven` L3-via-lean (the §4.2.3
//!   termination-vacuity gate — the conjoined `_converges` obligation has teeth).
//! - **O-4** an in-grammar while-body mutant is ATTEMPTED (REQ-11.7) — the certificate's
//!   mutation report says "against lean" with a non-zero attempted denominator, NOT the
//!   `UntestedAgainstLean`/`0/0` backstop that every while mutant hit pre-(v-b).
//!
//! Expected values are hand-derived from §4.2 (R-CHAR-3), never copied from forge's
//! output.

use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("conformance")
}

/// `true` when `lake` is available (the live Lean engine needs it). A LOUD skip
/// otherwise (the suite never silently passes on a missing lake — R-DEFER-3).
fn lake_present() -> bool {
    if let Ok(home) = std::env::var("HOME") {
        if PathBuf::from(home).join(".elan/bin/lake").exists() {
            return true;
        }
    }
    Command::new("lake")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Write a `.th` fixture into a scratch dir under the system temp (a process-unique
/// name) and return its path. The caller cleans it up.
fn write_fixture(name: &str, src: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "thermite-lean-while-{}-{}",
        std::process::id(),
        name
    ));
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(format!("{name}.th"));
    std::fs::write(&path, src).expect("write fixture");
    path
}

/// Run `forge check <path> --engine lean --json` and parse the certificate array. An
/// OUT-of-grammar shape may fail BEFORE the engine path (e.g. the L3 Verus lowering
/// rejects a nested loop) and emit a non-JSON error to stdout — that is itself a
/// NON-certification (no L3-via-lean), so a non-array stdout yields an EMPTY cert.
fn check_lean_required(path: &PathBuf) -> Vec<Value> {
    let cert = check_lean_opt(path);
    cert.unwrap_or_else(|| panic!("forge --json must emit a cert array for {path:?}"))
}

/// As [`check_lean_required`], but `None` when forge emitted no JSON array (the
/// not-certified-via-lean case for an OUT-of-grammar shape).
fn check_lean_opt(path: &PathBuf) -> Option<Vec<Value>> {
    let out = Command::new(forge_bin())
        .arg("check")
        .arg(path)
        .arg("--engine")
        .arg("lean")
        .arg("--json")
        .output()
        .expect("spawn forge check --engine lean");
    let stdout = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str::<Vec<Value>>(&stdout).ok()
}

/// The certificate level of the first item (`"L3"`/`"L2"`/`"L1"`/`"L0"`), or `"<none>"`.
fn level_of(cert: &[Value]) -> String {
    cert.first()
        .and_then(|c| c.get("level"))
        .and_then(|l| l.as_str())
        .unwrap_or("<none>")
        .to_string()
}

/// Whether the first item's discharge engine is the Lean engine (the
/// `engine_attribution.engine == "lean-auto"`/`"lean-interactive"` field). A `None`
/// attribution (the default Verus path) is NOT a Lean certification.
fn certified_via_lean(cert: &[Value]) -> bool {
    cert.first()
        .and_then(|c| c.get("engine_attribution"))
        .and_then(|a| a.get("engine"))
        .and_then(|e| e.as_str())
        .map(|e| e.starts_with("lean"))
        .unwrap_or(false)
}

const COUNT_SRC: &str = "\
fn count(n: u64) -> u64
  req n <= 1000
  ens result == n
  fx pure
{
  let mut lo: u64 = 0;
  while lo < n
    inv lo <= n
    dec n - lo
  {
    lo = lo + 1;
  }
  lo
}
";

// ════════════════════════════════════════════════════════════════════════════════
// O-1 (STRONG) — the LINEAR-family item certifies L3 via lean-auto through the CLI.
// ════════════════════════════════════════════════════════════════════════════════

#[test]
fn count_certifies_l3_via_lean_auto() {
    if !lake_present() {
        eprintln!(
            "SKIP: lake not present — the live L3-via-lean count certification is not run \
             (install lean/elan to exercise the §4.2.4 obligation set)."
        );
        return;
    }
    let path = write_fixture("count", COUNT_SRC);
    let cert = check_lean_required(&path);
    let _ = std::fs::remove_dir_all(path.parent().unwrap());

    // Expected from §4.2.4 (R-CHAR-3): the L1 linear family closes all 5 per-item
    // obligations + both composed theorems → L3 via lean-auto. NEVER a lower level on a
    // genuinely-terminating, contract-correct body.
    assert_eq!(
        level_of(&cert),
        "L3",
        "the LINEAR-family `count` item must certify L3 (both composed theorems \
         kernel-accept — R-1): {cert:#?}"
    );
    assert!(
        certified_via_lean(&cert),
        "the certification is attributed to the Lean engine ({{Lean kernel + 3 axioms, \
         EXP}} — REQ-4): {cert:#?}"
    );
}

// ════════════════════════════════════════════════════════════════════════════════
// O-1 (HONEST) — `conformance/sum.th` does NOT certify L3-via-lean (the §4 residual).
// ════════════════════════════════════════════════════════════════════════════════

#[test]
fn sum_does_not_certify_l3_via_lean_recursive_residual() {
    if !lake_present() {
        eprintln!("SKIP: lake not present — the sum.th lean-engine landing is not run.");
        return;
    }
    let sum = corpus_dir().join("sum.th");
    let cert = check_lean_required(&sum);
    // sum.th's `ens result == spec_sum(xs)` is a RECURSIVE-registry contract clause (the
    // recursive `spec_sum` spec-fn) — the §4 stabilized form is the INTERACTIVE residual,
    // so the AUTO Lean path REFUSES it (the contract-tier gate), and it does NOT certify
    // L3 via the Lean engine. Expected from §4.2.1 / REQ-7 (R-CHAR-3): the honest landing
    // is NOT-L3-via-lean (it degrades / falls to Verus). NEVER a false L3-via-lean.
    assert!(
        !(level_of(&cert) == "L3" && certified_via_lean(&cert)),
        "sum.th must NOT certify L3 via the Lean AUTO engine — its recursive-registry \
         `ens` is the §4 interactive residual (the honest R-1 landing): {cert:#?}"
    );
}

// ════════════════════════════════════════════════════════════════════════════════
// O-2 — the FULL refusal matrix: every OUT-of-v1 shape does NOT certify L3-via-lean.
// ════════════════════════════════════════════════════════════════════════════════

#[test]
fn refusal_matrix_no_lean_certification() {
    if !lake_present() {
        eprintln!("SKIP: lake not present — the refusal matrix lean landing is not run.");
        return;
    }
    // Each shape is OUT of the §4.2.1 v1 grammar (the §4.2.5 inventory). Under `--engine
    // lean` each is the honest `Unknown`/degrade skip — NEVER an L3-via-lean verdict. The
    // OUT class is hand-derived from §4.2.5 (R-CHAR-3). Boundary cases pinned in
    // `engine.rs::while_refusal_inventory_is_structured` (the structured `ExportRefusal`
    // variant); here we assert the CERTIFICATE-level consequence (no Lean L3).
    let matrix: &[(&str, &str)] = &[
        (
            "loop_kind",
            "fn f(n: u64) -> u64 req n <= 100 ens result == n fx pure \
             { let mut lo: u64 = 0; loop inv lo <= n dec n - lo { lo = lo + 1; } lo }",
        ),
        (
            "nested_loop",
            "fn f(n: u64) -> u64 req n <= 100 ens result == n fx pure \
             { let mut lo: u64 = 0; while lo < n inv lo <= n dec n - lo \
               { while lo < n inv lo <= n dec n - lo { lo = lo + 1; } } lo }",
        ),
        (
            "while_under_if",
            "fn f(n: u64) -> u64 req n <= 100 ens result == n fx pure \
             { let mut lo: u64 = 0; if n > 0 { while lo < n inv lo <= n dec n - lo \
               { lo = lo + 1; } } lo }",
        ),
        (
            "multi_loop",
            "fn f(n: u64) -> u64 req n <= 100 ens result == n fx pure \
             { let mut lo: u64 = 0; while lo < n inv lo <= n dec n - lo { lo = lo + 1; } \
               while lo < n inv lo <= n dec n - lo { lo = lo + 1; } lo }",
        ),
        (
            "break_body",
            "fn f(n: u64) -> u64 req n <= 100 ens result == n fx pure \
             { let mut lo: u64 = 0; while lo < n inv lo <= n dec n - lo { break; } lo }",
        ),
        (
            "continue_body",
            "fn f(n: u64) -> u64 req n <= 100 ens result == n fx pure \
             { let mut lo: u64 = 0; while lo < n inv lo <= n dec n - lo { continue; } lo }",
        ),
        (
            "mid_return",
            "fn f(n: u64) -> u64 req n <= 100 ens result == n fx pure \
             { let mut lo: u64 = 0; while lo < n inv lo <= n dec n - lo { return lo; } lo }",
        ),
        (
            "empty_inv",
            "fn f(n: u64) -> u64 req n <= 100 ens result == n fx pure \
             { let mut lo: u64 = 0; while lo < n dec n - lo { lo = lo + 1; } lo }",
        ),
        (
            "all_true_inv",
            "fn f(n: u64) -> u64 req n <= 100 ens result == n fx pure \
             { let mut lo: u64 = 0; while lo < n inv true dec n - lo { lo = lo + 1; } lo }",
        ),
        (
            "non_scalar_assign",
            "fn f(xs: &[u32], n: u64) -> u64 req n <= 100 ens result == n fx pure \
             { let mut lo: u64 = 0; while lo < n inv lo <= n dec n - lo \
               { xs[lo] = lo; lo = lo + 1; } lo }",
        ),
    ];
    for (name, src) in matrix {
        let path = write_fixture(name, src);
        let cert = check_lean_opt(&path).unwrap_or_default();
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
        assert!(
            !(level_of(&cert) == "L3" && certified_via_lean(&cert)),
            "the `{name}` shape is OUT of the §4.2.1 v1 grammar — it must NOT certify \
             L3-via-lean (the honest §4.2.5 refusal skip): {cert:#?}"
        );
    }
}

// ════════════════════════════════════════════════════════════════════════════════
// O-3 — the while-true vacuity fixture is NEVER Proven L3-via-lean (§4.2.3 gate).
// ════════════════════════════════════════════════════════════════════════════════

#[test]
fn while_true_no_op_is_not_proven_l3_via_lean() {
    if !lake_present() {
        eprintln!("SKIP: lake not present — the while-true vacuity gate is not run.");
        return;
    }
    // A non-exiting loop: `0 < 1` is constantly true, the measure `acc - acc` never
    // descends — `whileBodyConverges` is FALSE at every fuel, so the HYPOTHESIZE CONTRACT
    // obligation is VACUOUSLY discharge-able, BUT the conjoined `_converges` obligation
    // FAILS (the §4.2.3 termination-vacuity gate; the `PinWhileVacuity` mirror). The item
    // must NOT certify L3 via Lean. Expected from §4.2.3 (R-CHAR-3).
    let src = "fn spin(lo: u64) -> u64 req lo <= 100 ens result == lo fx pure \
               { let mut acc: u64 = lo; while 0 < 1 inv acc <= acc dec acc - acc \
                 { acc = acc; } acc }";
    let path = write_fixture("spin", src);
    let cert = check_lean_required(&path);
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
    assert!(
        !(level_of(&cert) == "L3" && certified_via_lean(&cert)),
        "a non-terminating `while true`-shaped body must NOT certify L3 via Lean — the \
         conjoined `_converges` obligation fails (§4.2.3, the PinWhileVacuity gate): \
         {cert:#?}"
    );
}

// ════════════════════════════════════════════════════════════════════════════════
// O-4 — an in-grammar while-body mutant is ATTEMPTED against Lean (REQ-11.7).
// ════════════════════════════════════════════════════════════════════════════════

#[test]
fn in_grammar_while_mutants_are_attempted_not_untested() {
    if !lake_present() {
        eprintln!("SKIP: lake not present — the REQ-11.7 mutation-attempt check is not run.");
        return;
    }
    let path = write_fixture("count_mut", COUNT_SRC);
    let cert = check_lean_required(&path);
    let _ = std::fs::remove_dir_all(path.parent().unwrap());

    // REQ-11.7 (§4.2.7): an in-grammar while-body mutant is now ADMITTED + ATTEMPTED
    // against the Lean fragment (it no longer hits the `0/0` `UntestedAgainstLean`
    // backstop that EVERY while mutant hit pre-(v-b), when the item itself refused
    // export). The certificate's mutation report must therefore show a NON-ZERO attempted
    // denominator "against lean", NOT a pure-`untested` tally. Expected from §4.2.7
    // (R-CHAR-3): the qualifier names "lean" and the attempted count is positive.
    let report = cert
        .first()
        .and_then(|c| c.get("contract_quality"))
        .and_then(|q| q.get("mutants_killed"))
        .and_then(|m| m.as_str())
        .unwrap_or("");
    assert!(
        report.contains("against lean"),
        "the while item's mutation report must be against the Lean engine (REQ-9 \
         engine-generic): {report:?}"
    );
    // A positive attempted denominator: the report's `k/n killed` has `n >= 1` (the
    // in-grammar mutants are ATTEMPTED, not all `UntestedAgainstLean`). The `0/0` backstop
    // would read `0/0`; a real attempt reads `k/n` with `n >= 1`.
    let attempted_positive = report
        .split_whitespace()
        .find(|tok| tok.contains('/'))
        .and_then(|frac| frac.split('/').nth(1))
        .and_then(|den| den.parse::<u32>().ok())
        .map(|n| n >= 1)
        .unwrap_or(false);
    assert!(
        attempted_positive,
        "in-grammar while-body mutants are ATTEMPTED against Lean — the killed-ratio \
         denominator is >= 1, not the `0/0` UntestedAgainstLean backstop (REQ-11.7): \
         {report:?}"
    );
}
