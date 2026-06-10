//! Conformance for the EXEC-BODY (state-refinement) TRANSLATION-VALIDATION phase
//! (`.design/verified/exec-stmt-tv.md` REQ-5 + `.design/verified/loop-tv.md` REQ-5;
//! epic crosslink #169, blocker #162). The state analogue of
//! `exec_tv_conformance.rs`. Four load-bearing properties, all through the REAL
//! `verus` binary (SKIP LOUDLY if absent, mirroring `exec_tv_conformance.rs` /
//! `body_teeth.rs`):
//!
//!   - **a FAITHFUL straight-line body → Faithful** (`forge body-tv` on a `{ let a =
//!     x + 1; let b = a * 2; b }` fixture): the production `lower_exec_body` produces
//!     the reference FINAL STATE → the body obligation VERIFIES → `faithful`.
//!   - **a FAITHFUL v1 `while`-loop body → Faithful** (all THREE per-run obligations
//!     verify): a `while lo < n inv lo <= n dec n - lo { lo = lo + 1 }` loop's entry /
//!     preservation / exit obligations all VERIFY → `faithful`.
//!   - **a MUTATED production → Divergent**: a deliberately-wrong production body
//!     (the `body_teeth.rs` B2 REORDERED-mutation shape — `s = s * 2; s = s + 1` for
//!     source `s = s + 1; s = s * 2`) discharged through the SAME body obligation
//!     fails the `ensures result == <body_ref_state>` postcondition → `Divergent`.
//!     (The corpus path uses the FAITHFUL lowerer so it never diverges; the Divergent
//!     ARM is exercised by injecting a known-wrong production at the obligation layer,
//!     exactly as `exec_tv::divergent_teeth` does for the exec-expr phase.)
//!   - **an OUT-of-subset body → Skipped+reason**: `binary_search.th`'s `loop`-kind
//!     body (multi-exit, mid-body `return`s — OUT of the v1 single-`while` subset) is
//!     `skipped` WITH a reason (the honest 2.2.1-vs-2.2.2 boundary — never `faithful`,
//!     R-HONEST-3).
//!
//! Expected values trace to the design's faithful-lowering invariant + the frozen
//! subset, never to the lowerer's output (R-CHAR-3). `unwrap`/`expect` are fine here
//! (`tests/` is not anti-pattern-gated).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

use thermite_syntax::ast::{BinOp, Block, Expr, Stmt};
use thermite_tv::obligation::{body_equivalence_obligation, BodyObligationFrame, BodyParamDecl};

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("conformance")
}

/// Verus locator (mirrors `body_teeth.rs` / `exec_tv_conformance.rs`): `VERUS_BIN`,
/// then PATH, then `~/.local/bin/verus`. SKIP LOUDLY otherwise.
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

fn verus_present() -> bool {
    verus_bin().is_some()
}

/// `forge body-tv` resolves a bare `verus` via PATH, so the conformance run needs
/// `verus` on PATH (or `~/.local/bin` added). Build a `Command` whose `PATH` includes
/// `~/.local/bin` (where verus lives in this env) so the spawned `forge` finds it.
fn run_body_tv_json(file: &Path) -> Value {
    let mut cmd = Command::new(forge_bin());
    cmd.arg("body-tv").arg(file).arg("--json");
    // Ensure the bare-`verus` spawn inside `forge` resolves: prepend the dir of the
    // located verus binary to PATH (so the test is robust whether verus is on PATH or
    // only in `~/.local/bin`).
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
    serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!(
            "forge body-tv --json stdout must be one JSON document: {e}\nstdout:\n{stdout}\n\
             stderr:\n{}",
            String::from_utf8_lossy(&out.stderr)
        )
    })
}

/// Write a `.th` source to a temp file and return its path.
fn write_th(name: &str, src: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("body_tv_conformance_{name}.th"));
    std::fs::write(&path, src).unwrap_or_else(|e| panic!("write {name}.th: {e}"));
    path
}

// ---- AST helpers + verus discharge (mirrors body_teeth.rs) for the Divergent arm --

fn path(name: &str) -> Expr {
    Expr::Path(vec![name.to_string()])
}

fn int(value: u128) -> Expr {
    Expr::IntLit {
        value,
        raw: value.to_string(),
    }
}

fn bin(op: BinOp, lhs: Expr, rhs: Expr) -> Expr {
    Expr::Binary {
        op,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    }
}

fn let_(mutable: bool, name: &str, init: Expr) -> Stmt {
    Stmt::Let {
        mutable,
        name: name.to_string(),
        ty: None,
        init,
    }
}

fn assign(target: &str, value: Expr) -> Stmt {
    Stmt::Assign {
        target: path(target),
        value,
    }
}

fn run_verus(file: &Path) -> Option<(bool, String)> {
    let bin = verus_bin()?;
    let out = Command::new(bin)
        .arg(file)
        .current_dir(std::env::temp_dir())
        .output()
        .ok()?;
    let mut combined = String::from_utf8_lossy(&out.stdout).to_string();
    combined.push_str(&String::from_utf8_lossy(&out.stderr));
    Some((out.status.success(), combined))
}

fn parse_results(output: &str) -> Option<(u32, u32)> {
    let line = output
        .lines()
        .find(|l| l.contains("verified,") && l.contains("errors"))?;
    let verified = line
        .split("verified,")
        .next()?
        .split_whitespace()
        .last()?
        .parse::<u32>()
        .ok()?;
    let errors = line
        .split("verified,")
        .nth(1)?
        .split_whitespace()
        .next()?
        .parse::<u32>()
        .ok()?;
    Some((verified, errors))
}

// ---- 1. a faithful straight-line body → Faithful ---------------------------

/// REQ-5 (the straight-line arm): a faithful `{ let a = x + 1; let b = a * 2; b }`
/// body — the production `lower_exec_body` produces the reference final state — is
/// reported `faithful` (the body state-refinement obligation VERIFIES), 0 divergent.
#[test]
fn faithful_straight_line_body_is_faithful() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — the faithful straight-line body-TV not discharged.");
        return;
    }
    let file = write_th(
        "sl",
        "fn sl(x: u64) -> u64\n  req x <= 1000\n  ens result == (x + 1) * 2\n  fx  pure\n\
         {\n  let a: u64 = x + 1;\n  let b: u64 = a * 2;\n  b\n}\n",
    );
    let report = run_body_tv_json(&file);
    let counts = &report["counts"];
    assert_eq!(
        counts["divergent"].as_u64().unwrap(),
        0,
        "a faithful straight-line body must NOT diverge. report: {report}"
    );
    assert_eq!(
        counts["faithful"].as_u64().unwrap(),
        1,
        "the faithful straight-line body `sl` must be reported `faithful`. report: {report}"
    );
    let sl_faithful = report["bodies"]
        .as_array()
        .unwrap()
        .iter()
        .any(|b| b["body"].as_str() == Some("sl") && b["verdict"].as_str() == Some("faithful"));
    assert!(
        sl_faithful,
        "the `sl` body must be reported `faithful` (the state transformation MEANS the \
         reference state-denotation). report: {report}"
    );
}

// ---- 2. a faithful v1 while-loop body → Faithful (all three obligations) ----

/// loop-tv REQ-5 (the loop arm): a faithful v1-subset `while lo < n inv lo <= n dec
/// n - lo { lo = lo + 1 }` body — all THREE per-run obligations (entry / preservation
/// / exit) VERIFY — is reported `faithful`, 0 divergent.
#[test]
fn faithful_while_loop_body_is_faithful() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — the faithful while-loop body-TV not discharged.");
        return;
    }
    let src = concat!(
        "fn wl(n: usize) -> usize\n",
        "  req n <= 1000\n",
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
    );
    let file = write_th("wl", src);
    let report = run_body_tv_json(&file);
    let counts = &report["counts"];
    assert_eq!(
        counts["divergent"].as_u64().unwrap(),
        0,
        "a faithful v1 while-loop body must NOT diverge. report: {report}"
    );
    assert_eq!(
        counts["faithful"].as_u64().unwrap(),
        1,
        "the faithful while-loop body `wl.loop` must be `faithful` (all three per-run \
         obligations verify). report: {report}"
    );
    let wl_faithful = report["bodies"].as_array().unwrap().iter().any(|b| {
        b["body"].as_str() == Some("wl.loop") && b["verdict"].as_str() == Some("faithful")
    });
    assert!(
        wl_faithful,
        "the `wl.loop` body must be reported `faithful` (entry + preservation + exit all \
         verify). report: {report}"
    );
}

// ---- 3. a mutated production → Divergent ------------------------------------

/// REQ-5 (the Divergent arm): a DELIBERATELY-WRONG production body — the
/// `body_teeth.rs` B2 REORDERED-mutation shape (`s = s * 2; s = s + 1` for source
/// `s = s + 1; s = s * 2`, final state `(x*2)+1 != (x+1)*2`) — discharged through the
/// SAME body state-refinement obligation `forge body-tv` builds (the obligation TEXT
/// is the contract surface) fails the `ensures result == <body_ref_state>`
/// postcondition. This is the FORGE Divergent classification's trigger: a results line
/// with `errors >= 1` → `BodyVerdict::Divergent`. The corpus path never diverges (the
/// faithful lowerer), so the Divergent arm is exercised by injecting a known-wrong
/// production at the obligation layer (exactly as `exec_tv::divergent_teeth` does).
#[test]
fn mutated_production_diverges() {
    if !verus_present() {
        eprintln!(
            "SKIP: verus not available — the mutated-production Divergent arm not discharged."
        );
        return;
    }
    // The B2 source body `{ let mut s = x; s = s + 1; s = s * 2; s }`, reference
    // `((x + 1) * 2)`.
    let body = Block {
        stmts: vec![
            let_(true, "s", path("x")),
            assign("s", bin(BinOp::Add, path("s"), int(1))),
            assign("s", bin(BinOp::Mul, path("s"), int(2))),
        ],
        tail: Some(Box::new(path("s"))),
    };
    let frame = BodyObligationFrame {
        params: vec![BodyParamDecl::new("x", "u64")],
        ret_type: "u64".to_string(),
        req: Some("x <= 1000".to_string()),
        ..Default::default()
    };
    // The MUTATED (reordered) production — each RHS is value-faithful in isolation, the
    // ORDER is the bug → final state `(x * 2) + 1`, != the reference `((x + 1) * 2)`.
    let program = body_equivalence_obligation(
        &body,
        "    let mut s = x;\n    s = s * 2;\n    s = s + 1;\n    s\n",
        &frame,
    )
    .expect("the reordered-mutation body obligation builds");

    let tmp = std::env::temp_dir().join("body_tv_conformance_divergent.rs");
    std::fs::write(&tmp, &program).unwrap_or_else(|e| panic!("write divergent obligation: {e}"));
    match run_verus(&tmp) {
        Some((_ok, output)) => {
            let (_verified, errors) = parse_results(&output).unwrap_or_else(|| {
                panic!(
                    "mutated production: expected a postcondition counterexample but no verus \
                        results line:\n{output}\n--- program ---\n{program}"
                )
            });
            // errors >= 1 is EXACTLY the signal `body_tv::discharge` maps to
            // `BodyVerdict::Divergent` (the `DischargeOutcome::Errors` arm). The catch
            // shape is the body-state `ensures` postcondition (a final-state difference).
            assert!(
                errors >= 1,
                "the REORDERED-mutation production must DIVERGE (a `postcondition not \
                 satisfied` — the final state `(x*2)+1` != the reference `(x+1)*2`); the \
                 forge Divergent classification maps errors>=1 to Divergent. errors={errors}\n\
                 --- verus output ---\n{output}\n--- program ---\n{program}"
            );
            assert!(
                output.contains("postcondition not satisfied"),
                "the divergence must be at the `ensures result == <ref state>` postcondition \
                 (a final-state difference — the state-sequencing teeth), not an unrelated \
                 failure:\n{output}"
            );
            eprintln!(
                "DIVERGENT (reordered mutation): verus = {errors} errors (postcondition — \
                 final state differs) — the forge Divergent arm fires (PASS)"
            );
        }
        None => eprintln!("SKIP: verus not available — the Divergent arm not discharged."),
    }
}

// ---- 4. an out-of-subset body → Skipped+reason -----------------------------

/// loop-tv REQ-5 / AC-4 (the Skipped arm, the LIVE corpus demo): `binary_search.th`'s
/// `loop`-kind body (a multi-exit form with mid-body `return None`/`return Some(mid)`
/// — OUT of the v1 single-`while` subset) is reported `skipped` WITH a reason, NEVER
/// `faithful` (R-HONEST-3 — a skip never masks an infidelity). This is the honest
/// expected corpus result.
#[test]
fn binary_search_loop_is_skipped_with_reason() {
    // No verus needed — the loop is recognized OUT-of-v1 BEFORE any discharge (the
    // `loop_ref_obligations` recognizer refuses). Robust even without verus.
    let report = run_body_tv_json(&corpus_dir().join("binary_search.th"));
    let counts = &report["counts"];
    assert_eq!(
        counts["faithful"].as_u64().unwrap(),
        0,
        "binary_search's `loop`-kind body must NOT be reported faithful (it is OUT of v1 \
         — R-HONEST-3). report: {report}"
    );
    assert!(
        counts["skipped"].as_u64().unwrap() >= 1,
        "binary_search's loop must be SKIPPED honestly. report: {report}"
    );
    let body = report["bodies"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["body"].as_str() == Some("binary_search.loop"))
        .unwrap_or_else(|| panic!("the `binary_search.loop` body must be reported: {report}"));
    assert_eq!(
        body["verdict"].as_str(),
        Some("skipped"),
        "binary_search.loop must be `skipped` (a `loop`-kind multi-exit form, OUT of v1). \
         report: {report}"
    );
    let reason = body["detail"].as_str().unwrap_or("");
    assert!(
        reason.contains("loop") && reason.to_lowercase().contains("v1"),
        "the skip must carry a REASON naming the OUT-of-v1 cause (the `loop`-kind / \
         multi-exit boundary), so the skip never silently masks. detail: {reason}"
    );
}

/// REQ-5 (the exit-code convention): a clean body-TV run (no Divergent — only Faithful
/// / Skipped) exits 0 (the SAME convention `forge exec-tv` uses). `binary_search.th`
/// is all-Skipped, so the run exits 0.
#[test]
fn skipped_only_run_exits_zero() {
    let mut cmd = Command::new(forge_bin());
    cmd.arg("body-tv")
        .arg(corpus_dir().join("binary_search.th"));
    if let Some(bin) = verus_bin() {
        if let Some(dir) = bin.parent() {
            let path = std::env::var("PATH").unwrap_or_default();
            cmd.env("PATH", format!("{}:{}", dir.display(), path));
        }
    }
    let status = cmd
        .status()
        .unwrap_or_else(|e| panic!("spawn forge body-tv: {e}"));
    assert!(
        status.success(),
        "a body-TV run with NO divergent body (binary_search is all-Skipped) must exit 0 \
         (the Faithful/Skipped/Unverifiable → 0, only Divergent → nonzero convention)"
    );
}
