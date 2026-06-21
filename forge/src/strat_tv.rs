//! The stratified-cage classifier differential battery — the M2b half of stage-2
//! REQ-4 (`.design/stage2-stratified-cage.md` REQ-4 / AC-4; audit check [8]).
//!
//! The Rust admission classifier (`thermite_spec::classifier`) mirrors the Lean kernel
//! classifier `Thermite.Strat.Cls.admitted` (`lean/Thermite/Strat/Fragment.lean`, REQ-3).
//! This module is the differential that holds the two byte-equal: it draws a
//! deterministic, well-sorted formula stream from the SplitMix64 generator
//! (`thermite_tv::gen::gen_strat_formulas`), classifies each with BOTH the Rust
//! classifier AND the Lean kernel `admitted` (via `lake env lean --run
//! Thermite/Strat/Cls/Wire.lean`, fed the shared S-expression wire format on stdin), and
//! compares verdict-for-verdict.
//!
//! - A **disagreement** (Rust admit ≠ Lean admit, both definite) is a real classifier
//!   infidelity → surfaced as a verification-failure `ExitCode` (the hard CI failure the
//!   audit check [8] gate raises), NOT a `ForgeError`. Mirrors `contract_tv`'s divergent
//!   handling.
//! - The **unknown-on-admitted tripwire**: a formula the Rust classifier could not vouch
//!   for ([`thermite_spec::Verdict::Unknown`], or a Lean `parse-error`) while the kernel
//!   admitted it. This is the `classifier-suspect` signal — counted, logged, and escalated
//!   (it also fails the gate); never silently retried (AC-4). In the healthy state the
//!   Rust classifier is total, so the count is structurally 0.
//! - A harness/environment failure (lake un-spawnable, the Lean driver exits non-zero,
//!   the verdict-line count desyncs) is a [`ForgeError::StratDifferential`]; lake-absent
//!   is an honest `Skipped` (not run), never a false pass.
//!
//! ## REQ status
//!
//! Tracked centrally as **REQ-S2-4** in `.design/reqs/registry.toml` (the stage-2
//! tracking entry, alongside REQ-S2-1/2/3), rendered into `.design/reqs/status.md`;
//! governing design `.design/stage2-stratified-cage.md` REQ-4 / AC-4.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use thermite_spec::classifier::{self, Frm, Verdict};

use crate::cli::ForgeError;

/// The default generated-formula count for the differential battery (the fixed-seed CI
/// gate). Kept modest because the Lean `admitted` runs the exponential Roy–Warshall
/// `reach` per formula (`Strat/Graph.lean`); the generator bounds each formula's edge
/// set so this count classifies in well under a second through `lake env lean --run`.
pub const STRAT_TV_DEFAULT_N: usize = 200;

/// The pinned default seed for the fixed-seed CI gate (the reproducible run). The
/// scheduled job overrides it with a rotating seed (`--seed`), walking the clause space.
pub const STRAT_TV_DEFAULT_SEED: u64 = 0x5354_5241_5430_3034; // "STRAT004"

/// One verdict disagreement between the Rust classifier and the Lean kernel `admitted` —
/// a real classifier infidelity (the hard-failure finding).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disagreement {
    /// The formula's 0-based index in the generated stream (reproducible from the seed).
    pub index: usize,
    /// The wire encoding (the exact line fed to the Lean driver — replayable by hand).
    pub wire: String,
    /// The Rust classifier's boolean verdict (`admitted`).
    pub rust_admitted: bool,
    /// The Lean kernel's raw verdict line (`true`/`false`).
    pub lean: String,
}

/// The differential report over a generated formula stream (REQ-4 / AC-4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StratTvReport {
    /// How many formulas were classified by both sides.
    pub checked: usize,
    /// How many the two sides agreed on (the healthy count).
    pub agreements: usize,
    /// The Rust admit count (telemetry — the admit/reject balance).
    pub rust_admitted: usize,
    /// The verdict disagreements (each a hard-failure finding). Empty = the battery
    /// passed.
    pub disagreements: Vec<Disagreement>,
    /// The unknown-on-admitted tripwire count (`classifier-suspect`, escalated). The Rust
    /// classifier is total, so this is 0 in the healthy state.
    pub tripwire_unknown_on_admitted: usize,
}

impl StratTvReport {
    /// The battery passes iff there is no disagreement AND no tripwire (AC-4). A failing
    /// battery maps to a verification-failure `ExitCode` at the CLI.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.disagreements.is_empty() && self.tripwire_unknown_on_admitted == 0
    }
}

/// The outcome of a differential run: it either ran end-to-end, or was honestly skipped
/// because `lake` is absent (the local-without-Lean case — never a false pass).
#[derive(Debug, Clone)]
pub enum StratTvOutcome {
    Ran(StratTvReport),
    Skipped(String),
}

/// The `lean/` package root (the cwd for `lake env lean`), resolved relative to the
/// `forge` crate dir (the workspace's `lean/` sibling). Deterministic (R-CODE-5);
/// mirrors `check::lean_package_root` / `engine::LeanEngine::lean_root`.
fn lean_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("lean")
}

/// Locate the `lake` binary (mirrors `engine::LeanEngine::lake_binary`): the
/// elan-managed `~/.elan/bin/lake` if present (so a non-login shell still finds it), else
/// the bare `lake` on PATH. `None` if neither is available — the honest `Skipped` case.
fn lake_binary() -> Option<PathBuf> {
    if let Some(home) = std::env::var_os("HOME") {
        let elan = PathBuf::from(home).join(".elan/bin/lake");
        if elan.exists() {
            return Some(elan);
        }
    }
    if Command::new("lake")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        return Some(PathBuf::from("lake"));
    }
    None
}

/// Compare the Rust verdicts against the Lean verdict lines, one formula at a time, into
/// a [`StratTvReport`] (REQ-4). Pure (R-CODE-5) — separated from the lake-spawning
/// [`run_generated`] so the comparison logic (including the tripwire) is unit-testable
/// without a Lean toolchain. The three vectors are index-aligned; the caller guarantees
/// equal lengths (a length mismatch is a harness error caught upstream).
fn compare(formulas: &[Frm], rust: &[Verdict], lean_lines: &[String]) -> StratTvReport {
    let mut report = StratTvReport {
        checked: formulas.len(),
        agreements: 0,
        rust_admitted: 0,
        disagreements: Vec::new(),
        tripwire_unknown_on_admitted: 0,
    };
    for (i, (phi, rv)) in formulas.iter().zip(rust.iter()).enumerate() {
        let lean = lean_lines.get(i).map_or("", |s| s.as_str()).trim();
        let lean_admitted = match lean {
            "true" => Some(true),
            "false" => Some(false),
            _ => None, // "parse-error" or anything unexpected → the kernel is unsure
        };
        match rv {
            // The healthy total path: the Rust classifier gave a definite verdict.
            Verdict::Admitted | Verdict::Rejected(_) => {
                let rust_admitted = rv.is_admitted();
                if rust_admitted {
                    report.rust_admitted += 1;
                }
                match lean_admitted {
                    Some(la) if la == rust_admitted => report.agreements += 1,
                    Some(la) => report.disagreements.push(Disagreement {
                        index: i,
                        wire: classifier::to_wire(phi),
                        rust_admitted,
                        lean: if la { "true" } else { "false" }.to_string(),
                    }),
                    // The kernel could not classify (a `parse-error`) what the Rust side
                    // vouched for as admitted → classifier-suspect (a wire/parse desync).
                    None => {
                        if rust_admitted {
                            report.tripwire_unknown_on_admitted += 1;
                        } else {
                            // Rust rejected AND the kernel could not parse: still a
                            // harness anomaly, but not the dangerous admit case — count it
                            // as a disagreement so it is never silently dropped.
                            report.disagreements.push(Disagreement {
                                index: i,
                                wire: classifier::to_wire(phi),
                                rust_admitted,
                                lean: lean.to_string(),
                            });
                        }
                    }
                }
            }
            // The Rust classifier punted (it is total today, so this never fires for the
            // shipped classifier). If the kernel admitted the formula, this is the
            // unknown-on-admitted tripwire — the classifier is weaker than the kernel.
            Verdict::Unknown(_) => {
                if lean_admitted == Some(true) {
                    report.tripwire_unknown_on_admitted += 1;
                } else {
                    // Unknown on a non-admitted formula is benign (the classifier is
                    // conservative), but still counted as a non-agreement disagreement so
                    // the run is not silently green.
                    report.disagreements.push(Disagreement {
                        index: i,
                        wire: classifier::to_wire(phi),
                        rust_admitted: false,
                        lean: lean.to_string(),
                    });
                }
            }
        }
    }
    report
}

/// Run the differential battery over `n` formulas generated from `seed` (REQ-4 / AC-4).
/// Generates the well-sorted stream, classifies each with the Rust classifier, pipes the
/// wire encodings to `lake env lean --run Thermite/Strat/Cls/Wire.lean` on stdin, reads
/// the kernel verdicts, and compares.
///
/// Returns [`StratTvOutcome::Skipped`] (never an error) if `lake` is absent — the honest
/// not-run case. A [`ForgeError::StratDifferential`] is a harness/environment failure
/// (spawn failure, non-zero Lean exit, verdict-line desync), surfaced not swallowed
/// (R-CODE-4). A verdict DISAGREEMENT is NOT an error — it lands in the returned report's
/// `disagreements` for the CLI to surface as a verification-failure exit.
pub fn run_generated(seed: u64, n: usize) -> Result<StratTvOutcome, ForgeError> {
    let Some(lake) = lake_binary() else {
        return Ok(StratTvOutcome::Skipped(
            "`lake` not found (set up elan / install Lean to run the differential battery)"
                .to_string(),
        ));
    };

    let formulas = thermite_tv::gen::gen_strat_formulas(seed, n);
    let rust: Vec<Verdict> = formulas.iter().map(classifier::classify).collect();

    // One wire line per formula (no blank lines — the Lean driver maps one output line
    // per input line, so blanks would desync the count).
    let mut input = String::with_capacity(formulas.len() * 32);
    for phi in &formulas {
        input.push_str(&classifier::to_wire(phi));
        input.push('\n');
    }

    let mut child = Command::new(&lake)
        .arg("env")
        .arg("lean")
        .arg("--run")
        .arg("Thermite/Strat/Cls/Wire.lean")
        .current_dir(lean_root())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| ForgeError::StratDifferential {
            detail: format!("could not spawn `lake env lean --run`: {e}"),
        })?;

    child
        .stdin
        .take()
        .ok_or_else(|| ForgeError::StratDifferential {
            detail: "could not open the Lean driver's stdin".to_string(),
        })?
        .write_all(input.as_bytes())
        .map_err(|e| ForgeError::StratDifferential {
            detail: format!("failed writing wire formulas to the Lean driver: {e}"),
        })?;

    let out = child
        .wait_with_output()
        .map_err(|e| ForgeError::StratDifferential {
            detail: format!("failed waiting on the Lean driver: {e}"),
        })?;

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let head: String = stderr.chars().take(600).collect();
        return Err(ForgeError::StratDifferential {
            detail: format!(
                "the Lean driver (`lake env lean --run Thermite/Strat/Cls/Wire.lean`) exited \
                 non-zero — the spine may not be built (run `lake build Thermite.Strat.Cls.Wire`): \
                 {head}"
            ),
        });
    }

    let stdout = String::from_utf8_lossy(&out.stdout);
    let lean_lines: Vec<String> = stdout
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    if lean_lines.len() != formulas.len() {
        return Err(ForgeError::StratDifferential {
            detail: format!(
                "verdict-line desync: {} formulas in, {} verdict lines out (the wire format \
                 may have drifted between the Rust serializer and the Lean parser)",
                formulas.len(),
                lean_lines.len()
            ),
        });
    }

    Ok(StratTvOutcome::Ran(compare(&formulas, &rust, &lean_lines)))
}

/// Render a human-readable differential report (REQ-4). Used by `forge strat-tv` and the
/// audit surface; the disagreements are listed verbatim (with the replayable wire) so a
/// finding is auditable by a skeptical third party (`thermite-design.md` §1).
#[must_use]
pub fn render_report(report: &StratTvReport, header: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("=== {header} ===\n"));
    out.push_str(&format!(
        "  {} formulas: {} agree, {} disagree, {} unknown-on-admitted tripwire (admit={}, \
         reject={})\n",
        report.checked,
        report.agreements,
        report.disagreements.len(),
        report.tripwire_unknown_on_admitted,
        report.rust_admitted,
        report.checked - report.rust_admitted,
    ));
    for d in &report.disagreements {
        out.push_str(&format!(
            "  DISAGREEMENT [{}]: rust admitted={}, lean=`{}`\n    wire: {}\n",
            d.index, d.rust_admitted, d.lean, d.wire
        ));
    }
    if report.tripwire_unknown_on_admitted > 0 {
        out.push_str(&format!(
            "  TRIPWIRE: {} formula(s) the Rust classifier could not vouch for while the kernel \
             admitted them — classifier-suspect, escalated (never silently retried)\n",
            report.tripwire_unknown_on_admitted
        ));
    }
    if report.passed() {
        out.push_str(
            "  PASS — the Rust classifier matched the Lean kernel `admitted` on every \
                      generated formula\n",
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use thermite_spec::classifier::RejectReason;

    fn admit_verdict() -> Verdict {
        Verdict::Admitted
    }
    fn reject_verdict() -> Verdict {
        Verdict::Rejected(RejectReason::IndexGrammar)
    }

    // A trivial closed formula (an opaque qfree leaf — always admitted) used as a wire
    // placeholder for the pure-comparison tests (the wire content is irrelevant; only the
    // verdict pairing matters).
    fn dummy() -> Frm {
        Frm::Atom(thermite_spec::classifier::Atom::QFree)
    }

    #[test]
    fn compare_counts_agreements_and_disagreements() {
        let fs = vec![dummy(), dummy(), dummy()];
        let rust = vec![admit_verdict(), reject_verdict(), admit_verdict()];
        // Lean agrees on 0 and 1, DISAGREES on 2 (lean says reject, rust says admit).
        let lean = vec!["true".to_string(), "false".to_string(), "false".to_string()];
        let r = compare(&fs, &rust, &lean);
        assert_eq!(r.checked, 3);
        assert_eq!(r.agreements, 2);
        assert_eq!(r.disagreements.len(), 1);
        assert_eq!(r.disagreements[0].index, 2);
        assert!(r.disagreements[0].rust_admitted);
        assert_eq!(r.disagreements[0].lean, "false");
        assert_eq!(r.tripwire_unknown_on_admitted, 0);
        assert!(!r.passed());
    }

    #[test]
    fn compare_all_agree_passes() {
        let fs = vec![dummy(), dummy()];
        let rust = vec![admit_verdict(), reject_verdict()];
        let lean = vec!["true".to_string(), "false".to_string()];
        let r = compare(&fs, &rust, &lean);
        assert_eq!(r.agreements, 2);
        assert!(r.passed());
        assert_eq!(r.rust_admitted, 1);
    }

    #[test]
    fn unknown_on_admitted_increments_the_tripwire() {
        // The classifier-suspect case: the Rust classifier punted (`Unknown`) on a
        // formula the kernel admitted. The tripwire counts it and the battery fails
        // (escalated, never silently green) — AC-4.
        let fs = vec![dummy()];
        let rust = vec![Verdict::Unknown("synthetic punt".to_string())];
        let lean = vec!["true".to_string()];
        let r = compare(&fs, &rust, &lean);
        assert_eq!(r.tripwire_unknown_on_admitted, 1);
        assert!(
            r.disagreements.is_empty(),
            "an unknown-on-admitted is a tripwire, not a disagreement"
        );
        assert!(!r.passed(), "a tripwire must fail the battery (escalate)");
    }

    #[test]
    fn lean_parse_error_on_rust_admitted_is_a_tripwire() {
        // A kernel `parse-error` (it could not classify) on a Rust-admitted formula is
        // the wire-desync flavour of classifier-suspect — also a tripwire.
        let fs = vec![dummy()];
        let rust = vec![admit_verdict()];
        let lean = vec!["parse-error".to_string()];
        let r = compare(&fs, &rust, &lean);
        assert_eq!(r.tripwire_unknown_on_admitted, 1);
        assert!(!r.passed());
    }

    #[test]
    fn render_lists_disagreements_verbatim() {
        let fs = vec![dummy()];
        let rust = vec![admit_verdict()];
        let lean = vec!["false".to_string()];
        let r = compare(&fs, &rust, &lean);
        let text = render_report(&r, "test");
        assert!(text.contains("DISAGREEMENT [0]"));
        assert!(text.contains("wire:"));
        assert!(!text.contains("PASS —"));
    }
}
