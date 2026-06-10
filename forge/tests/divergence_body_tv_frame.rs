//! Divergence pin (critic audit of #162, commit 540cea0d): `forge body-tv`
//! classifies a FAITHFUL, in-subset body as **Divergent** ("a real body-lowering
//! infidelity", nonzero exit) when the obligation FRAME fails to compile — i.e.
//! when the source `req` references a `spec fn` helper (the design's central
//! contract idiom, `thermite-design.md` §3/§4: `req sorted(haystack)`), because
//! `body_tv::corpus_req` threads the `req` text VERBATIM into a
//! `BodyObligationFrame`/`LoopObligationFrame` whose `spec_defs` is empty
//! (`spec_defs: Vec::new()` in `straight_line_body_tv` / `build_loop_frame`),
//! so verus aborts with an undefined-function compile error and `run_obligation`
//! maps the no-results non-success exit to `DischargeOutcome::CompileAbort` →
//! `BodyVerdict::Divergent`.
//!
//! AUTHORITY:
//! - `.design/verified/exec-stmt-tv.md` REQ-5: the four-way verdict — `Divergent`
//!   ⟺ the lowering and the reference DISAGREE (a counterexample / a
//!   non-compiling PRODUCTION); a body whose FRAME is non-derivable is the
//!   `Skipped` class, "never masking an infidelity" — and symmetrically never
//!   FABRICATING one (R-HONEST-3).
//! - `.design/verified/loop-tv.md` § "The four-way reporting": `Divergent` —
//!   "any obligation's PRODUCTION side fails `postcondition not satisfied` (a
//!   counterexample)". The loop ENTRY obligation (`loop-tv.md` REQ-2: ENTRY =
//!   `proof fn { assert(inv[cells:=entry]); }`) contains NO production text at
//!   all, so its compile abort can NEVER be "the production loop text did not
//!   compile" — yet that is exactly the emitted Divergent detail.
//! - The sibling `forge/src/exec_tv.rs` (`check_corpus_expr`, the req gate):
//!   the `req` is included ONLY when every ident it references is env-declared,
//!   explicitly because otherwise "the obligation would not compile — a FRAMING
//!   failure, not an infidelity". `body_tv::corpus_req` has no such gate.
//!
//! OBSERVED (live, verus 0.2026.05.24): both fixtures below report
//! `1 DIVERGENT` ("verus ABORTED (compile/parse) … a real body-lowering /
//! loop-lowering infidelity") and exit 1, although the production lowering of
//! each body is exactly faithful.
//!
//! These tests assert the AUTHORITY's behavior (a faithful in-subset body is
//! NEVER Divergent; the honest classes for an uncompilable frame are
//! Skipped/Unverifiable) and therefore FAIL against the current toolchain.
//! Tracking: crosslink blocker (see issue filed with this commit).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// Verus locator (mirrors `forge/tests/body_tv.rs`): `VERUS_BIN`, then PATH, then
/// `~/.local/bin/verus`. SKIP LOUDLY otherwise (verus absent → the discharge is
/// `Unverifiable`, and the false-Divergent path under pin cannot be reached).
fn verus_bin() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("VERUS_BIN") {
        let pb = PathBuf::from(p);
        if pb.exists() {
            return Some(pb);
        }
    }
    if let Ok(out) = Command::new("which").arg("verus").output() {
        if out.status.success() {
            let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !p.is_empty() {
                return Some(PathBuf::from(p));
            }
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        let cand = PathBuf::from(home).join(".local/bin/verus");
        if cand.exists() {
            return Some(cand);
        }
    }
    None
}

fn write_th(name: &str, src: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("divergence_body_tv_frame_{name}.th"));
    std::fs::write(&path, src).unwrap_or_else(|e| panic!("write {name}.th: {e}"));
    path
}

/// Spawn `forge body-tv <file> --json` with verus's dir prepended to PATH
/// (mirrors `forge/tests/body_tv.rs::run_body_tv_json`), returning the parsed
/// JSON report plus the process exit success flag.
fn run_body_tv_json(file: &Path) -> (Value, bool) {
    let mut cmd = Command::new(forge_bin());
    cmd.arg("body-tv").arg(file).arg("--json");
    if let Some(bin) = verus_bin() {
        if let Some(dir) = bin.parent() {
            let path = std::env::var("PATH").unwrap_or_default();
            cmd.env("PATH", format!("{}:{}", dir.display(), path));
        }
    }
    let out = cmd
        .output()
        .unwrap_or_else(|e| panic!("spawn forge body-tv: {e}"));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let doc = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!(
            "forge body-tv --json stdout must be one JSON document: {e}\nstdout:\n{stdout}\n\
             stderr:\n{}",
            String::from_utf8_lossy(&out.stderr)
        )
    });
    (doc, out.status.success())
}

/// Divergence (straight-line arm): a FAITHFUL `{ let v: u32 = xs[0]; v }` body —
/// squarely inside the frozen 2.2.1 subset, production lowering exactly faithful —
/// whose `req` references a `spec fn` helper (`all_small(xs)`, the
/// `req sorted(haystack)` corpus idiom) must NEVER be `divergent`
/// (exec-stmt-tv.md REQ-5: Divergent ⟺ the lowering and the reference DISAGREE;
/// an uncompilable FRAME is "a FRAMING failure, not an infidelity" — the
/// exec_tv req-gate authority). The current toolchain reports `1 divergent`
/// ("a real body-lowering infidelity") + exit 1: a FABRICATED infidelity.
#[test]
fn spec_helper_req_straight_line_body_is_not_divergent() {
    if verus_bin().is_none() {
        eprintln!("SKIP: verus not available — the frame-failure classification not reachable.");
        return;
    }
    let file = write_th(
        "sl",
        concat!(
            "spec fn all_small(xs: &[u32]) -> bool\n",
            "  dec xs.len()\n",
            "{\n",
            "  match xs {\n",
            "    []          => true,\n",
            "    [head, ..t] => head < 1000 && all_small(t),\n",
            "  }\n",
            "}\n",
            "\n",
            "fn first_or_zero(xs: &[u32]) -> u32\n",
            "  req all_small(xs) && xs.len() >= 1\n",
            "  ens result == xs[0]\n",
            "  fx  pure\n",
            "{\n",
            "  let v: u32 = xs[0];\n",
            "  v\n",
            "}\n",
        ),
    );
    let (report, exit_ok) = run_body_tv_json(&file);
    let divergent = report["counts"]["divergent"].as_u64().unwrap();
    assert_eq!(
        divergent, 0,
        "a FAITHFUL in-subset straight-line body must never be Divergent: the obligation's \
         compile abort is a FRAME failure (the verbatim `req all_small(xs) && ..` with empty \
         `spec_defs`), not a production-lowering infidelity (exec-stmt-tv.md REQ-5; the \
         exec_tv req-gate). The honest classes are skipped/unverifiable. report: {report}"
    );
    assert!(
        exit_ok,
        "no real divergence → exit 0 (the Divergent-only-nonzero convention). report: {report}"
    );
}

/// Divergence (loop arm): a FAITHFUL v1 `while lo < n inv lo <= n dec n - lo`
/// loop whose fn `req` references a `spec fn` helper must NEVER be `divergent`.
/// The ENTRY obligation (`loop-tv.md` REQ-2: `proof fn { assert(inv[cells:=entry]) }`)
/// contains NO production text, so its compile abort cannot be "the production
/// loop text did not compile" — yet the current toolchain emits exactly that
/// Divergent detail + exit 1 (loop-tv.md § four-way: Divergent ⟺ a PRODUCTION-side
/// counterexample; a non-discharge is Unverifiable/Skipped, never a fabricated
/// infidelity — R-HONEST-3).
#[test]
fn spec_helper_req_v1_while_loop_is_not_divergent() {
    if verus_bin().is_none() {
        eprintln!("SKIP: verus not available — the frame-failure classification not reachable.");
        return;
    }
    let file = write_th(
        "loop",
        concat!(
            "spec fn small(n: u64) -> bool\n",
            "  dec n\n",
            "{\n",
            "  n <= 1000\n",
            "}\n",
            "\n",
            "fn count_up(n: usize) -> usize\n",
            "  req small(n as u64)\n",
            "  ens result == n\n",
            "  fx  pure\n",
            "{\n",
            "  let mut lo: usize = 0;\n",
            "  while lo < n\n",
            "    inv lo <= n\n",
            "    dec n - lo\n",
            "  {\n",
            "    lo = lo + 1;\n",
            "  }\n",
            "  lo\n",
            "}\n",
        ),
    );
    let (report, exit_ok) = run_body_tv_json(&file);
    let divergent = report["counts"]["divergent"].as_u64().unwrap();
    assert_eq!(
        divergent, 0,
        "a FAITHFUL v1 while-loop body must never be Divergent on a FRAME compile abort: \
         the ENTRY obligation contains no production text at all (loop-tv.md REQ-2), so \
         'the production loop text did not compile' is definitionally false — the honest \
         classes are skipped/unverifiable (loop-tv.md four-way, R-HONEST-3). report: {report}"
    );
    assert!(
        exit_ok,
        "no real divergence → exit 0 (the Divergent-only-nonzero convention). report: {report}"
    );
}
