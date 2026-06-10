//! Divergence pin (critic audit of #193 increment (iii), commit bf29a050): the
//! open-hole validator guards ONLY `check::check_file_with_options`. Every OTHER
//! path that lowers an exec-fn body bypasses it, and because a `?N` hole is
//! recorded on `FnItem.holes` and NOT threaded into the statement stream (no
//! `Stmt` variant — the deliberate #193 design), those paths lower the holed
//! body with the hole SILENTLY OMITTED: the hole vanishes into a syntactically
//! valid program with no trace.
//!
//! OBSERVED (live, this tree):
//! - `forge build` on `fn main() { ?0  42 }` emits an rlib ARTIFACT and a build
//!   manifest claiming `assurance: L1 (built, runtime-checked)` — exit 0, no
//!   mention of the open hole anywhere (`build::build_file` runs only the
//!   parse/validate/check_effects front, then `thermite_lower::lower_l1`; it
//!   never consults `f.holes`).
//! - `forge body-tv` on the same file reports `main — faithful, 0 skipped`
//!   (exit 0): it lowers the hole-stripped body, ships it to verus, and certifies
//!   the TV verdict "faithful" for an INCOMPLETE body.
//! - (`forge exec-tv` corpus mode likewise reports the holed body's tail expr
//!   `faithful` — same root cause, pinned transitively by the same fix.)
//!
//! AUTHORITY:
//! - `.design/forge/goal-repl.md` REQ-4: "an item with any open hole is
//!   L0-equivalent until every hole is filled"; REQ-5: a holed item gets "no
//!   lowering, no verus"; Architecture: "A holed item NEVER reaches verus; it
//!   can never accidentally certify."
//! - `thermite_syntax::ast` (`FnItem.holes` doc, shipped by bf29a050): "a holed
//!   item never lowers — it short-circuits at `forge check`" — the very
//!   invariant that justified omitting a `Stmt::Hole` variant. If ANY path
//!   lowers a holed body, the omission turns the hole into silent deletion.
//! - `thermite-design.md` §6: "The certificate attached to a build artifact …
//!   This manifest **is** the deliverable's trust statement" — a build manifest
//!   asserting L1 for a fn whose body still carries an open goal is a false
//!   trust statement; §5.1: an open hole is an OPEN GOAL the oracle must
//!   re-present, never silently drop.
//!
//! These tests assert the AUTHORITY's behavior (a holed item never lowers: build
//! must refuse with a structured error, body-TV must never report a holed body
//! `faithful`) and therefore FAIL against the current toolchain.
//! Tracking: crosslink blocker (see issue filed with this commit).

use std::path::{Path, PathBuf};
use std::process::Command;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// A minimal holed exec fn: the body carries the open hole `?0` ahead of a tail
/// expr, so the hole-stripped statement stream is a VALID body (`{ 42 }`) — the
/// exact shape in which the hole vanishes silently. Hand-derived from
/// `thermite-design.md` §5.1 `body = hole ?0` (R-CHAR-3).
const HOLED_MAIN: &str =
    "fn main() -> u64\n  req true\n  ens result == 42\n  fx  pure\n{\n  ?0\n  42\n}\n";

fn temp_th(tag: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "forge_divergence_holed_{tag}_{}.th",
        std::process::id()
    ));
    std::fs::write(&path, HOLED_MAIN).expect("write temp .th");
    path
}

fn run_forge(args: &[&str]) -> (String, String, bool) {
    let out = Command::new(forge_bin())
        .args(args)
        .output()
        .expect("spawn forge");
    (
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
        out.status.success(),
    )
}

/// `true` iff verus is reachable (the `goal_repl_fill.rs` convention).
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

/// `true` iff rustc is reachable (`forge build`'s backend; without it the build
/// fails for the WRONG reason and the divergence assert would pass vacuously).
fn rustc_present() -> bool {
    Command::new("rustc")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Divergence: `forge build` on a holed fn lowers the hole-stripped body and
/// emits an artifact + an `assurance: L1` manifest line — the open hole `?0`
/// silently vanishes into a valid compiled program.
/// Authority: `.design/forge/goal-repl.md` REQ-4/REQ-5 ("no lowering, no verus";
/// "L0-equivalent until every hole is filled") + `thermite-design.md` §6 (the
/// build manifest is the trust statement). Expected: build REFUSES a holed item
/// with a structured error naming the open hole — non-success exit, no artifact.
#[test]
fn divergence_build_emits_artifact_for_holed_item() {
    if !rustc_present() {
        eprintln!("SKIP: rustc not reachable; `forge build` would fail for the wrong reason");
        return;
    }
    let file = temp_th("build");
    let (stdout, stderr, ok) = run_forge(&["build", file.to_str().unwrap()]);
    let _ = std::fs::remove_file(&file);
    assert!(
        !ok,
        "AUTHORITY (goal-repl.md REQ-4/REQ-5; thermite-design.md §6): a holed item is \
         L0-equivalent and never lowers — `forge build` must REFUSE it, not emit an \
         artifact with the hole silently dropped.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        !stdout.contains("artifact:"),
        "no artifact may be emitted for a holed item.\nstdout:\n{stdout}"
    );
}

/// Divergence: `forge body-tv` on a holed fn lowers the hole-stripped body,
/// ships it to verus, and reports the body `faithful` — a TV verdict certifying
/// fidelity of an INCOMPLETE body whose open goal was silently deleted.
/// Authority: `.design/forge/goal-repl.md` REQ-5 + Architecture ("a holed item
/// NEVER reaches verus") + the `FnItem.holes` contract ("a holed item never
/// lowers"). Expected: the holed body is never counted `faithful` (the honest
/// classes are a refusal or an explicit skip naming the open hole).
#[test]
fn divergence_body_tv_reports_holed_body_faithful() {
    if !verus_present() {
        eprintln!("SKIP: verus not reachable; body-tv would report unverifiable, not faithful");
        return;
    }
    let file = temp_th("bodytv");
    let (stdout, stderr, _ok) = run_forge(&["body-tv", file.to_str().unwrap()]);
    let _ = std::fs::remove_file(&file);
    assert!(
        !stdout.contains("main — faithful"),
        "AUTHORITY (goal-repl.md REQ-5; ast.rs FnItem.holes contract): a holed body never \
         lowers and never reaches verus — body-TV must not certify it `faithful`.\n\
         stdout:\n{stdout}\nstderr:\n{stderr}"
    );
}
