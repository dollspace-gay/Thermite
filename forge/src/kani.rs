//! `forge/src/kani.rs` — the L2 driver (`.design/lower/l2-kani.md`;
//! `thermite-design.md` §6 the L2 rung, §5.1 counterexamples, §13 v0.2). It is
//! the bounded-model-check parallel of `check.rs`'s verus path: it takes a Kani
//! proof harness (`thermite_lower::lower_l2`), writes it into a TEMP CARGO CRATE
//! (kani needs a crate context), spawns the REAL `cargo kani` / `cargo-kani`
//! binary, CHECKS THE EXIT STATUS (R-CODE-4 — never swallow a subprocess
//! failure), parses Kani 0.67.0's `--output-format terse` summary into either a
//! verified-up-to-bound `Level::L2` certificate or a concrete counterexample, and
//! cleans up the temp crate.
//!
//! Governing design: `.design/lower/l2-kani.md`.
//!
//! ## Parsing the REAL Kani output (REQ-5; the external truth, R-CHAR-3)
//!
//! Grounded against `cargo kani 0.67.0`'s terse format:
//!
//! - **success →** `Level::L2`: the line `VERIFICATION:- SUCCESSFUL` (and
//!   `** 0 of N failed`). The discharged obligation RECORDS THE BOUND
//!   (`slice <= N, unwind K`) so a reader sees the L2 caveat (REQ-6).
//! - **counterexample →** a non-L2 reported cert: `VERIFICATION:- FAILED` plus
//!   `Failed Checks: <description>` and (where present) `File: "<src>", line <n>`.
//!   Each failed check becomes an `ObligationResult::failed(description, location,
//!   raw)` — the §5.1 counterexample witness. The `unwinding assertion loop 0`
//!   under-bound failure parses the same way (a reported non-L2 result, never a
//!   false pass — AC-5).
//! - **no summary line / kani internal failure →** `ForgeError::KaniOutput`
//!   (never a swallowed success, R-CODE-4), parallel to `parse_verus_output`'s
//!   VIR-error branch.
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-4 (`run_kani` invocation — real binary, temp crate, exit status) | SHIPPED | `pub fn run_kani` writes a temp cargo crate (`write_kani_crate`, no-`.` stem via `crate_stem`), spawns `cargo-kani`, captures exit status; ENOENT → `ForgeError::KaniAbsent`, other spawn failure → `KaniSpawn`; temp crate removed best-effort. Consumer: `check::check_l2_file` (`check.rs`). |
//! | REQ-5 (Kani output → L2-or-counterexample) | SHIPPED | `parse_kani_output` keys on `VERIFICATION:- SUCCESSFUL`/`FAILED` + `Failed Checks:`/`File:`; success → `Level::L2` w/ bound obligation, failure → per-`ObligationResult::failed` witnesses, no summary → `KaniOutput`. Pure tests `success_terse_is_l2`/`failure_terse_is_counterexample`/`under_bound_is_reported_failure`/`no_summary_is_kani_output_error`. |
//! | REQ-6 (the L2 "up to bound" caveat) | SHIPPED | `run_kani` takes the `bound` string (`thermite_lower::bound_string`) and records it on the discharged obligation + the cert (`Level::L2`, programmatically distinct from L3). `bound_recorded_on_l2_cert` (AC-6). |
//! | REQ-8 (Kani-absent = structured error) | SHIPPED | spawn `ErrorKind::NotFound` → `ForgeError::KaniAbsent`, never a silent success; `run_kani_with_absent_binary_is_kani_absent` (AC-7, points `KANI_BIN` at a non-existent binary). |
//! | REQ-9 (determinism) | SHIPPED | the bound is the fixed input (`l2.rs` `SLICE_BOUND`); the temp crate path uses pid + a monotonic counter (not wall-clock); `solver_time_ms` is the only wall-clock field, excluded from the cert oracle (`manifest::Certificate::oracle_subset`). |
//!
//! ## #10 gate (the L2 under-bound-vs-counterexample split for the degrade ladder, OQ-2)
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | degrade-ladder OQ-2 (split the L2 `FAILED` bucket) | SHIPPED | `pub fn classify_l2_outcome` maps an `L2Result` to `enum L2Verdict { Verified, UnderBound, Counterexample }`: a `Level::L2` result is `Verified`; a `Level::L0` failure whose EVERY failed obligation is an `is_under_bound_failure` (`unwinding assertion` / `unwind`) is `UnderBound` (the L2 analog of a timeout → degrade to L1); ANY real property failure (`assertion failed: <ens>`) OR an ambiguous/empty-witness failure is a `Counterexample` (hard fail, CONSERVATIVE per R-DEFER-9). Consumer: `degrade::run_ladder` (`degrade.rs`). Tests `classify_l2_{successful_is_verified, unwinding_assertion_is_under_bound, real_assertion_is_counterexample, mixed_failure_is_counterexample, ambiguous_failure_is_counterexample}`. |

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use crate::cli::ForgeError;
use crate::manifest::{Certificate, Level, ObligationResult, ObligationStatus};

/// The parsed result of one Kani run: the assurance level (L2 on success, L0 on a
/// reported counterexample) plus the per-obligation results (REQ-5) and the
/// wall-clock solver time (excluded from the cert oracle, REQ-9).
#[derive(Debug, Clone)]
pub struct L2Result {
    /// `Level::L2` on `VERIFICATION:- SUCCESSFUL`; `Level::L0` on a reported
    /// counterexample (a non-L2 result, never a false pass — REQ-5/§6).
    pub level: Level,
    /// The per-obligation witnesses: one discharged obligation recording the
    /// bound on success, or the failed `Failed Checks:` lines on a counterexample.
    pub obligations: Vec<ObligationResult>,
    /// Wall-clock kani time in ms — NON-DETERMINISTIC, excluded from the oracle.
    pub solver_time_ms: u64,
}

/// Run the real `cargo kani` binary on a Kani proof `harness`, returning the
/// parsed [`L2Result`] (REQ-4). `bound` is the L2 caveat string
/// (`thermite_lower::bound_string`, e.g. `"slice <= 4, unwind 5"`) recorded on the
/// success obligation so the certificate states the bound (REQ-6).
///
/// A reachable contract `assert!` failure is NOT an `Err`: it is a valid
/// [`L2Result`] at `Level::L0` carrying the counterexample (REQ-5). Only an
/// ENVIRONMENT / internal failure (kani absent, unparseable output) is an `Err`
/// (R-CODE-4 — never swallow a subprocess failure, never a false pass).
pub fn run_kani(harness: &str, label: &str, bound: &str) -> Result<L2Result, ForgeError> {
    let stem = crate_stem(label);
    let crate_dir = unique_crate_dir(&stem);
    write_kani_crate(&crate_dir, &stem, harness)?;

    let result = invoke_kani(&crate_dir, bound);

    // Best-effort cleanup of the whole temp crate; never mask the real result on
    // a cleanup failure (exactly `run_verus`'s temp-file discipline).
    let _ = std::fs::remove_dir_all(&crate_dir);

    result
}

/// Spawn `cargo kani` in the temp crate and parse its output. Split from
/// [`run_kani`] so the temp crate is cleaned up regardless of outcome. The kani
/// binary is `KANI_BIN` (the test seam — a non-existent path exercises the
/// ENOENT/`KaniAbsent` branch) else `cargo-kani` (what `cargo kani` invokes; a
/// missing plugin is the ENOENT signal for kani-absent, REQ-8).
fn invoke_kani(crate_dir: &Path, bound: &str) -> Result<L2Result, ForgeError> {
    let binary = std::env::var("KANI_BIN").unwrap_or_else(|_| "cargo-kani".to_string());
    let started = Instant::now();
    let output = Command::new(&binary)
        .arg("--output-format")
        .arg("terse")
        .current_dir(crate_dir)
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ForgeError::KaniAbsent { binary }
            } else {
                ForgeError::KaniSpawn { source: e }
            }
        })?;
    let solver_time_ms = started.elapsed().as_millis() as u64;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let exit_code = output.status.code();

    parse_kani_output(&stdout, &stderr, exit_code, bound, solver_time_ms)
}

/// Compute a valid Rust crate stem from a label (REQ-4): every non-alphanumeric
/// char replaced by `_`, prefixed if it starts with a digit, suffixed `_l2`.
/// Mirrors `check.rs::crate_stem` (kani, like verus, derives the crate name from
/// the package name and rejects a `.`).
fn crate_stem(label: &str) -> String {
    let mut stem = String::with_capacity(label.len() + 4);
    for ch in label.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            stem.push(ch);
        } else {
            stem.push('_');
        }
    }
    if stem
        .chars()
        .next()
        .map(|c| c.is_ascii_digit())
        .unwrap_or(true)
    {
        stem.insert(0, 'c');
    }
    stem.push_str("_l2");
    stem
}

/// Build a unique temp crate directory (REQ-4/REQ-9). Uniqueness uses the process
/// id + a monotonic counter — NOT wall-clock — so concurrent runs do not collide
/// without violating R-CODE-5 (determinism is a property of the CERTIFICATE, not
/// the scratch path; mirrors `check.rs::unique_temp_path`).
fn unique_crate_dir(stem: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    std::env::temp_dir().join(format!("forge_kani_{stem}_{pid}_{n}"))
}

/// Write the throwaway cargo crate kani checks: a `Cargo.toml` (package =
/// `stem`, a `[lib]` pointing at `src/lib.rs`) + `src/lib.rs` carrying the
/// harness (REQ-4 / OQ-4 — the per-run tiny crate the grounding used). IO
/// failures map to `ForgeError::Io` (no panic, R-CODE-2).
fn write_kani_crate(crate_dir: &Path, stem: &str, harness: &str) -> Result<(), ForgeError> {
    let src_dir = crate_dir.join("src");
    std::fs::create_dir_all(&src_dir).map_err(|e| ForgeError::Io {
        path: src_dir.display().to_string(),
        source: e,
    })?;
    let manifest = format!(
        "[package]\nname = \"{stem}\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n\
         [lib]\npath = \"src/lib.rs\"\n\n[workspace]\n"
    );
    std::fs::write(crate_dir.join("Cargo.toml"), manifest).map_err(|e| ForgeError::Io {
        path: crate_dir.join("Cargo.toml").display().to_string(),
        source: e,
    })?;
    let lib = src_dir.join("lib.rs");
    std::fs::write(&lib, harness).map_err(|e| ForgeError::Io {
        path: lib.display().to_string(),
        source: e,
    })?;
    Ok(())
}

/// Parse Kani's `--output-format terse` output into an [`L2Result`] (REQ-5). The
/// summary line `VERIFICATION:- SUCCESSFUL`/`FAILED` drives the level; the
/// `Failed Checks:` lines (+ the following `File: "<src>", line <n>`) become the
/// per-obligation counterexample witnesses. No recognizable summary line → a
/// `ForgeError::KaniOutput` (never swallowed, never a false pass — R-CODE-4).
fn parse_kani_output(
    stdout: &str,
    stderr: &str,
    exit_code: Option<i32>,
    bound: &str,
    solver_time_ms: u64,
) -> Result<L2Result, ForgeError> {
    let combined = format!("{stdout}\n{stderr}");
    let succeeded = combined.contains("VERIFICATION:- SUCCESSFUL");
    let failed = combined.contains("VERIFICATION:- FAILED");

    // Neither marker present → kani did not run a verification (compile error,
    // reachable unsupported construct, internal failure). NEVER a silent success.
    if !succeeded && !failed {
        return Err(ForgeError::KaniOutput {
            detail: format!(
                "no `VERIFICATION:- SUCCESSFUL/FAILED` summary in kani output (exit {exit:?}); \
                 head: {head}",
                exit = exit_code,
                head = first_lines(&combined, 12),
            ),
        });
    }

    // Defensive: both markers present is contradictory output — surface it, do
    // not guess (R-CODE-4).
    if succeeded && failed {
        return Err(ForgeError::KaniOutput {
            detail: "kani output contains BOTH `VERIFICATION:- SUCCESSFUL` and `FAILED` \
                     (contradictory); refusing to guess the verdict"
                .to_string(),
        });
    }

    if succeeded {
        // Verified up to bound → L2, with one discharged obligation recording the
        // bound caveat (REQ-6 / AC-6).
        return Ok(L2Result {
            level: Level::L2,
            obligations: vec![ObligationResult::discharged(format!(
                "bounded model check passed ({bound})"
            ))],
            solver_time_ms,
        });
    }

    // Failed: parse the `Failed Checks:` lines into counterexample witnesses
    // (REQ-5 / §5.1). A failure with no parseable witness still yields a single
    // failed obligation (never a bare boolean, never swallowed).
    let failures = parse_failed_checks(&combined);
    let obligations = if failures.is_empty() {
        vec![ObligationResult::failed(
            "kani reported VERIFICATION:- FAILED",
            None,
            Some(first_lines(&combined, 12)),
        )]
    } else {
        failures
    };
    Ok(L2Result {
        level: Level::L0,
        obligations,
        solver_time_ms,
    })
}

/// Parse the `Failed Checks: <description>` lines (+ the following
/// `File: "<src>", line <n>` location) into per-obligation failure witnesses
/// (REQ-5). Grounded format (Kani 0.67.0 terse):
///
/// ```text
/// Failed Checks: assertion failed: result == spec_sum(xs)
///  File: "src/lib.rs", line 22, in check_sum
/// ```
///
/// The `unwinding assertion loop 0` under-bound failure is the same shape (AC-5),
/// deduped so the repeated unwinding lines collapse to one witness.
fn parse_failed_checks(text: &str) -> Vec<ObligationResult> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<ObligationResult> = Vec::new();
    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        let Some(desc) = trimmed.strip_prefix("Failed Checks: ") else {
            continue;
        };
        let desc = desc.trim().to_string();
        // Dedup identical descriptions (the under-bound case emits the same
        // `unwinding assertion loop 0` for every undetermined unwinding).
        if out.iter().any(|o| o.name == desc) {
            continue;
        }
        // The location is on the following `File: "<src>", line <n>` line.
        let location = lines
            .iter()
            .skip(idx + 1)
            .take(2)
            .find_map(|l| parse_file_line(l.trim_start()));
        out.push(ObligationResult::failed(
            desc.clone(),
            location,
            Some(format!("Failed Checks: {desc}")),
        ));
    }
    out
}

/// Parse a Kani `File: "<src>", line <n>[, in <fn>]` line into `<src>:<n>`
/// (REQ-5). Returns `None` if the line is not a kani file-location line.
fn parse_file_line(line: &str) -> Option<String> {
    let rest = line.strip_prefix("File: ")?;
    // rest is `"<src>", line <n>, in <fn>` — pull the quoted src and the line no.
    let (quoted, after) = rest.split_once(',')?;
    let src = quoted.trim().trim_matches('"');
    let after = after.trim();
    let n = after.strip_prefix("line ")?;
    let line_no: String = n.chars().take_while(|c| c.is_ascii_digit()).collect();
    if src.is_empty() || line_no.is_empty() {
        return None;
    }
    Some(format!("{src}:{line_no}"))
}

/// Take the first `n` non-empty lines of a diagnostic blob (bounded — never echo
/// unbounded solver output; mirrors `check.rs::first_lines`).
fn first_lines(text: &str, n: usize) -> String {
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .take(n)
        .collect::<Vec<_>>()
        .join("\n")
}

/// The DEGRADE-relevant three-way classification of an L2 (kani) run for the
/// auto-degrade ladder (issue #10, `.design/forge/degrade-ladder.md` OQ-2). At L3
/// the timeout-vs-counterexample split is #11's `SolverProfile`-presence
/// discriminator; at L2 the discriminator is the SHAPE of the kani failure:
///
/// - [`L2Verdict::Verified`]: `VERIFICATION:- SUCCESSFUL` (`L2Result` is
///   `Level::L2`) → the ladder certifies L2.
/// - [`L2Verdict::UnderBound`]: a `VERIFICATION:- FAILED` whose ONLY failed
///   obligations are `unwinding assertion` (kani ran out of unwind / could not
///   bound the loop — the L2 ANALOG of a timeout, INCONCLUSIVE). The ladder
///   degrades to L1 (REQ-3).
/// - [`L2Verdict::Counterexample`]: a `VERIFICATION:- FAILED` carrying a real
///   property `assertion failed: <ens clause>` witness (kani DISPROVED the
///   contract — a real bug). A HARD FAIL, NEVER a degrade (REQ-2 anti-cheat).
///
/// CONSERVATIVE (R-DEFER-9, the doc's OQ-2 ratified resolution): an AMBIGUOUS
/// `FAILED` shape (any failed obligation that is NOT an unwinding assertion, or a
/// failure with no parseable witness) is treated as a `Counterexample` (hard
/// fail), NEVER an under-bound degrade — hiding a bug behind a lowered stamp is
/// strictly worse than over-reporting a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum L2Verdict {
    /// Verified up to bound → certify L2.
    Verified,
    /// Bound exhausted (only `unwinding assertion` failures) → inconclusive,
    /// degrade to L1.
    UnderBound,
    /// A real contract property was disproved (or the failure is ambiguous) →
    /// hard fail, never a degrade.
    Counterexample,
}

/// Classify an [`L2Result`] for the degrade ladder (issue #10 OQ-2). Splits the
/// `Level::L0` kani-failure bucket into an INCONCLUSIVE under-bound (degrade to
/// L1) and a real COUNTEREXAMPLE (hard fail), per [`L2Verdict`]. The under-bound
/// discriminator is the grounded kani text `unwinding assertion` (the
/// `under_bound_is_reported_failure` shape): a failure whose EVERY failed
/// obligation is an unwinding assertion is the bound running out; ANY other failed
/// obligation (a real `assertion failed: <ens>` witness) — or a failure with no
/// parseable failed obligation at all — is a counterexample (CONSERVATIVE,
/// R-DEFER-9). Consumer: `degrade::run_ladder`.
pub fn classify_l2_outcome(result: &L2Result) -> L2Verdict {
    if result.level == Level::L2 {
        return L2Verdict::Verified;
    }
    let failed: Vec<&ObligationResult> = result
        .obligations
        .iter()
        .filter(|o| o.status == ObligationStatus::Failed)
        .collect();
    // A FAILED run with no parseable failed obligation is ambiguous → conservative
    // counterexample (never silently an under-bound degrade — R-DEFER-9).
    if failed.is_empty() {
        return L2Verdict::Counterexample;
    }
    // Under-bound iff EVERY failed obligation is an unwinding-assertion / resource
    // failure (the bound ran out). A single real property failure → counterexample.
    if failed.iter().all(|o| is_under_bound_failure(&o.name)) {
        L2Verdict::UnderBound
    } else {
        L2Verdict::Counterexample
    }
}

/// `true` iff a kani failed-check description is an UNDER-BOUND / resource
/// exhaustion (the L2 analog of a timeout, issue #10 OQ-2), as opposed to a real
/// property counterexample. Grounded kani shapes: `unwinding assertion loop 0`
/// (the loop unwind ran out) and the `unwind` resource markers. A real property
/// failure (`assertion failed: result == spec_sum(xs)`) is NOT under-bound. Match
/// is on the kani text (R-CHAR-3 — kani's wording, not forge's).
fn is_under_bound_failure(description: &str) -> bool {
    let d = description.to_ascii_lowercase();
    d.contains("unwinding assertion") || d.contains("unwind")
}

/// Assemble the L2 [`Certificate`] for one item from its [`L2Result`]
/// (REQ-6). `Level::L2` (programmatically distinct from L3, §12) with the bound
/// recorded in the obligations. Consumer: `check::check_l2_file`.
pub fn assemble_l2_certificate(item: &str, effects: Vec<String>, result: &L2Result) -> Certificate {
    Certificate::new(
        item,
        result.level,
        effects,
        result.solver_time_ms,
        result.obligations.clone(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::ObligationStatus;

    const BOUND: &str = "slice <= 4, unwind 5";

    // REQ-5 / AC-1: a terse SUCCESSFUL summary → Level::L2 with the bound recorded
    // (the grounded `sum` output, R-CHAR-3 — Kani's real format, not forge's).
    #[test]
    fn success_terse_is_l2() {
        let stdout = "Checking harness check_sum...\n\nVERIFICATION RESULT:\n ** 0 of 38 failed\n\nVERIFICATION:- SUCCESSFUL\n";
        let r = parse_kani_output(stdout, "", Some(0), BOUND, 7).expect("parse");
        assert_eq!(r.level, Level::L2);
        assert_eq!(r.obligations.len(), 1);
        assert_eq!(r.obligations[0].status, ObligationStatus::Discharged);
        assert!(
            r.obligations[0].name.contains("slice <= 4, unwind 5"),
            "the L2 obligation records the bound caveat: {}",
            r.obligations[0].name
        );
    }

    // REQ-5 / AC-3: a terse FAILED summary + `Failed Checks:` + `File:` → a
    // non-L2 reported result carrying the counterexample with its location (the
    // grounded broken-`sum` output, R-CHAR-3).
    #[test]
    fn failure_terse_is_counterexample() {
        let stdout = " ** 1 of 38 failed\nFailed Checks: assertion failed: result == spec_sum(xs)\n File: \"src/lib.rs\", line 22, in check_sum\n\nVERIFICATION:- FAILED\n";
        let r = parse_kani_output(stdout, "", Some(1), BOUND, 3).expect("parse");
        assert_eq!(r.level, Level::L0);
        let failed: Vec<_> = r
            .obligations
            .iter()
            .filter(|o| o.status == ObligationStatus::Failed)
            .collect();
        assert_eq!(failed.len(), 1, "exactly the one failed check");
        assert_eq!(failed[0].name, "assertion failed: result == spec_sum(xs)");
        assert_eq!(
            failed[0].location.as_deref(),
            Some("src/lib.rs:22"),
            "the counterexample carries the kani source location"
        );
    }

    // REQ-5 / AC-5: an under-bound run (`unwinding assertion loop 0`, repeated) is
    // a REPORTED non-L2 failure, never a false pass; the repeated lines dedup to
    // one witness (the grounded binary_search unwind(2) output, R-CHAR-3).
    #[test]
    fn under_bound_is_reported_failure() {
        let stdout = " ** 3 of 82 failed (79 undetermined)\nFailed Checks: unwinding assertion loop 0\nFailed Checks: unwinding assertion loop 0\nFailed Checks: unwinding assertion loop 0\nVERIFICATION:- FAILED\n";
        let r = parse_kani_output(stdout, "", Some(1), BOUND, 1).expect("parse");
        assert_eq!(r.level, Level::L0, "under-bound is NOT a false L2 pass");
        let failed: Vec<_> = r
            .obligations
            .iter()
            .filter(|o| o.status == ObligationStatus::Failed)
            .collect();
        assert_eq!(failed.len(), 1, "the repeated unwinding lines dedup to one");
        assert_eq!(failed[0].name, "unwinding assertion loop 0");
    }

    // REQ-5 / R-CODE-4: no recognizable summary line → KaniOutput error, never a
    // silent success and never a false pass.
    #[test]
    fn no_summary_is_kani_output_error() {
        let r = parse_kani_output("error: could not compile", "boom", Some(101), BOUND, 1);
        assert!(matches!(r, Err(ForgeError::KaniOutput { .. })));
    }

    // R-CODE-4: contradictory output (both markers) is an error, not a guess.
    #[test]
    fn contradictory_summary_is_kani_output_error() {
        let r = parse_kani_output(
            "VERIFICATION:- SUCCESSFUL\nVERIFICATION:- FAILED\n",
            "",
            Some(0),
            BOUND,
            1,
        );
        assert!(matches!(r, Err(ForgeError::KaniOutput { .. })));
    }

    // REQ-8 / AC-7: run_kani with the kani binary absent (KANI_BIN points at a
    // non-existent path) → ForgeError::KaniAbsent, no panic, no silent success.
    #[test]
    fn run_kani_with_absent_binary_is_kani_absent() {
        // SAFETY of test isolation: this sets a process-global env var; the test
        // restores it. It targets the ENOENT spawn branch deterministically.
        let prev = std::env::var("KANI_BIN").ok();
        std::env::set_var(
            "KANI_BIN",
            "/nonexistent/forge-kani-absent-probe-binary-xyz",
        );
        let r = run_kani(
            "#[cfg(kani)]\n#[kani::proof]\nfn check_f() { assert!(true); }\n",
            "absent_probe",
            BOUND,
        );
        match prev {
            Some(v) => std::env::set_var("KANI_BIN", v),
            None => std::env::remove_var("KANI_BIN"),
        }
        assert!(
            matches!(r, Err(ForgeError::KaniAbsent { .. })),
            "absent kani binary → KaniAbsent (not a panic, not a false pass): {r:?}"
        );
    }

    // REQ-4: crate_stem is a valid Rust crate name (no `.`, not digit-leading).
    #[test]
    fn crate_stem_is_valid() {
        for input in ["sum", "binary_search", "9bad", "a.b.c"] {
            let stem = crate_stem(input);
            assert!(!stem.contains('.'), "stem `{stem}` must have no dot");
            let first = stem.chars().next().expect("non-empty");
            assert!(
                first.is_ascii_alphabetic() || first == '_',
                "stem `{stem}` not digit-leading"
            );
            assert!(stem.ends_with("_l2"));
        }
    }

    // REQ-5: parse_file_line pulls the kani `<src>:<line>` location.
    #[test]
    fn parse_file_line_extracts_location() {
        assert_eq!(
            parse_file_line("File: \"src/lib.rs\", line 22, in check_sum"),
            Some("src/lib.rs:22".to_string())
        );
        assert_eq!(parse_file_line("not a file line"), None);
    }

    // #10 OQ-2: classify_l2_outcome splits the L0 failure bucket. A Level::L2
    // result is Verified. Levels/markers trace to kani's terse output (R-CHAR-3).
    #[test]
    fn classify_l2_successful_is_verified() {
        let r = L2Result {
            level: Level::L2,
            obligations: vec![ObligationResult::discharged("bounded model check passed")],
            solver_time_ms: 5,
        };
        assert_eq!(classify_l2_outcome(&r), L2Verdict::Verified);
    }

    // #10 OQ-2: an under-bound FAILED (only `unwinding assertion loop` failures, the
    // grounded binary_search unwind(2) shape) is the L2 analog of a timeout →
    // UnderBound (the ladder degrades to L1, REQ-3). R-CHAR-3 — kani's wording.
    #[test]
    fn classify_l2_unwinding_assertion_is_under_bound() {
        let r = L2Result {
            level: Level::L0,
            obligations: vec![ObligationResult::failed(
                "unwinding assertion loop 0",
                None,
                None,
            )],
            solver_time_ms: 1,
        };
        assert_eq!(
            classify_l2_outcome(&r),
            L2Verdict::UnderBound,
            "an only-unwinding-assertion failure is an inconclusive bound exhaustion"
        );
    }

    // #10 OQ-2 / REQ-2 anti-cheat: a real property `assertion failed: <ens>` FAILED
    // is a COUNTEREXAMPLE (hard fail), NEVER an under-bound degrade — kani DISPROVED
    // the contract (the grounded broken-`sum` shape). R-CHAR-3.
    #[test]
    fn classify_l2_real_assertion_is_counterexample() {
        let r = L2Result {
            level: Level::L0,
            obligations: vec![ObligationResult::failed(
                "assertion failed: result == spec_sum(xs)",
                Some("src/lib.rs:22".to_string()),
                None,
            )],
            solver_time_ms: 3,
        };
        assert_eq!(
            classify_l2_outcome(&r),
            L2Verdict::Counterexample,
            "a real ens-property failure must NEVER degrade to L1 (REQ-2)"
        );
    }

    // #10 OQ-2 (CONSERVATIVE, R-DEFER-9): a MIXED failure (a real assertion AND an
    // unwinding assertion) is a COUNTEREXAMPLE — an under-bound classification
    // requires EVERY failure be an unwinding assertion.
    #[test]
    fn classify_l2_mixed_failure_is_counterexample() {
        let r = L2Result {
            level: Level::L0,
            obligations: vec![
                ObligationResult::failed("assertion failed: result == spec_sum(xs)", None, None),
                ObligationResult::failed("unwinding assertion loop 0", None, None),
            ],
            solver_time_ms: 3,
        };
        assert_eq!(classify_l2_outcome(&r), L2Verdict::Counterexample);
    }

    // #10 OQ-2 (CONSERVATIVE, R-DEFER-9): a FAILED L2Result with NO parseable failed
    // obligation is AMBIGUOUS → Counterexample (never silently an under-bound
    // degrade — hiding a bug is worse than over-reporting).
    #[test]
    fn classify_l2_ambiguous_failure_is_counterexample() {
        let r = L2Result {
            level: Level::L0,
            obligations: vec![],
            solver_time_ms: 0,
        };
        assert_eq!(classify_l2_outcome(&r), L2Verdict::Counterexample);
    }

    // REQ-6 / AC-6: assemble_l2_certificate yields an L2 cert distinct from L3,
    // carrying the bound on its obligation.
    #[test]
    fn bound_recorded_on_l2_cert() {
        let res = L2Result {
            level: Level::L2,
            obligations: vec![ObligationResult::discharged(format!(
                "bounded model check passed ({BOUND})"
            ))],
            solver_time_ms: 9,
        };
        let cert = assemble_l2_certificate("sum", vec!["pure".to_string()], &res);
        assert_eq!(cert.level, Level::L2);
        assert_ne!(cert.level, Level::L3);
        assert!(cert.obligations[0].name.contains("slice <= 4, unwind 5"));
    }
}
