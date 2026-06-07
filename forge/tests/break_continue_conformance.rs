//! L3/L0/L1-grounding conformance for the `break` / `continue` LOOP-CONTROL
//! layer — crosslink #93 (cluster 3 of the primitive-completeness buildout).
//!
//! This test certifies, against the REAL `verus` binary, the GROUNDED
//! verification semantics pinned in `.design/lower/verus-lowering.md` REQ-12 /
//! AC-7 (and mirrored in `.design/syntax/{lexer.md,ast.md,parser.md}` #93):
//!
//!   - a TERMINATING `while`+`dec` whose `continue` preserves the invariant AND
//!     decreases the measure certifies L3 (the continue is a loop back-edge that
//!     re-establishes every `inv` and the `decreases` — REQ-12(a)(b));
//!   - a `continue` that BREAKS the invariant is L0 ("loop invariant not
//!     satisfied … at this continue" — REQ-12(a));
//!   - a `continue` that does NOT decrease the measure is L0 ("decreases not
//!     satisfied at continue" — REQ-12(b)) — the back-edge owes the same
//!     termination obligation as the implicit loop-end edge;
//!   - a `break` early-exit certifies L3 when its post-loop fact follows from the
//!     loop invariants that hold AT the break point (REQ-12(c) / OQ-5 policy
//!     (ii): a v0.1 `inv` lowers to a PLAIN Verus `invariant`, checked at break);
//!   - a `fx diverge` loop with BOTH `break` and `continue` (no `decreases`,
//!     `#[verifier::exec_allows_no_decreases_clause]`) verifies its invariants
//!     and is STRUCTURALLY capped at L1 by the #88 diverge gate (REQ-12(d)) —
//!     the editor's event loop now works WITHOUT the quit-flag hack;
//!   - the in-loop STRUCTURAL rule (`parser.md` REQ-10): a `break;`/`continue;`
//!     OUTSIDE any loop body is a structured `SyntaxError`, never a panic; a
//!     `break;` nested inside an `if` WITHIN a loop is accepted.
//!
//! NON-VACUITY (R-DEFER-9 / `thermite-design.md` §7): the terminating L3 probes
//! observe the loop through a tight `ens result == <value>` pinned by a loop
//! invariant, so the §7 mutation battery bites (a wrong body is killed); the §7
//! vacuity gate (which rejects `ens true`) is respected. The L0 probes are L0
//! because the obligation BITES at the continue (not because the contract is
//! vacuous).
//!
//! R-CHAR-3: the expected LEVELS trace to the design (`verus-lowering.md`
//! Verification §`break`/`continue` — GROUNDED with real `verus 0.2026.05.24`:
//! continue-ok → L3, bad-inv/bad-dec continue → L0, break-exit → L3, diverge
//! loop → L1) and the expected VALUES are the program's own arithmetic
//! constants; NEITHER is copied from forge's own output. Runs the BUILT `forge`
//! binary; if verus is absent the verus-dependent probes SKIP LOUDLY (never
//! panic on a missing solver), mirroring `operators_conformance.rs`. The
//! PARSER-level structural-rule probes run unconditionally (no verus needed).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// `true` iff verus is reachable (mirrors `operators_conformance.rs`).
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

/// Write `program` to a temp `.th`, `forge check --json` it, return the cert
/// array. The temp file is removed before returning (scratch hygiene, #53).
fn check_program(tag: &str, program: &str) -> Vec<Value> {
    let fixture = std::env::temp_dir().join(format!(
        "forge_bc_{tag}_{}_{}.th",
        std::process::id(),
        tag.len()
    ));
    std::fs::write(&fixture, program).expect("write fixture");
    let out = Command::new(forge_bin())
        .arg("check")
        .arg(&fixture)
        .arg("--json")
        .output()
        .expect("spawn forge");
    let _ = std::fs::remove_file(&fixture);
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

fn level(certs: &[Value], item: &str) -> String {
    cert_for(certs, item)["level"]
        .as_str()
        .unwrap_or("<none>")
        .to_string()
}

// ---------------------------------------------------------------------------
// (1) terminating `while`+`dec`, `continue` preserves invariant + decreases → L3
//     (verus-lowering.md REQ-12(a)(b), AC-7 probe 1).
// ---------------------------------------------------------------------------

/// A loop that adds 2 to `c` per iteration, with the index advancing BEFORE the
/// `continue` so the invariant `c == i * 2` AND the measure `n - i` both hold at
/// the continue. The `continue` is load-bearing (it carries the +2 increment and
/// the post-continue statement is dead). Tight `ens result == n * 2` (pinned by
/// the invariant) → the §7 mutation battery bites → L3.
#[test]
fn continue_preserving_invariant_and_decreases_certifies_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — continue-ok L3 grounding not exercised.");
        return;
    }
    let certs = check_program(
        "cont_ok",
        "fn sum_to(n: u64) -> u64\n  \
           req n <= 100000\n  \
           ens result == n * 2\n  \
           fx pure\n{\n  \
             let mut i: u64 = 0;\n  \
             let mut c: u64 = 0;\n  \
             while i < n\n    \
               inv i <= n\n    \
               inv c == i * 2\n    \
               dec n - i\n  \
             {\n    \
               c = c + 1;\n    \
               i = i + 1;\n    \
               if i <= n {\n      c = c + 1;\n      continue;\n    }\n    \
               c = c + 1;\n  }\n  \
             c\n}\n",
    );
    assert_eq!(
        level(&certs, "sum_to"),
        "L3",
        "DESIGN verus-lowering.md REQ-12(a)(b): a `continue` that re-establishes every \
         `inv` AND decreases the measure certifies L3 (GROUNDED: continue-ok → L3). \
         forge: {certs:?}"
    );
}

// ---------------------------------------------------------------------------
// (2) `continue` that BREAKS the invariant → L0
//     (verus-lowering.md REQ-12(a), AC-7 probe 2).
// ---------------------------------------------------------------------------

/// The same accumulator loop, but the `continue` fires BEFORE the matching index
/// advance, so the invariant `c == i * 2` is FALSE at the continue. The
/// invariant obligation BITES at the continue point → L0 (not an L3, not a
/// vacuity pass). GROUNDED: "loop invariant not satisfied … at this continue".
#[test]
fn continue_breaking_invariant_is_l0() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — continue-bad-invariant L0 grounding not exercised.");
        return;
    }
    let certs = check_program(
        "cont_bad_inv",
        "fn bad_inv(n: u64) -> u64\n  \
           req n <= 100000\n  \
           ens result == n * 2\n  \
           fx pure\n{\n  \
             let mut i: u64 = 0;\n  \
             let mut c: u64 = 0;\n  \
             while i < n\n    \
               inv i <= n\n    \
               inv c == i * 2\n    \
               dec n - i\n  \
             {\n    \
               c = c + 1;\n    \
               if i < n {\n      continue;\n    }\n    \
               i = i + 1;\n  }\n  \
             c\n}\n",
    );
    assert_eq!(
        level(&certs, "bad_inv"),
        "L0",
        "DESIGN verus-lowering.md REQ-12(a): a `continue` that leaves the loop with an \
         invariant BROKEN is L0 (the invariant obligation BITES at the continue — \
         break/continue cannot launder the invariant, R-DEFER-9). forge: {certs:?}"
    );
}

// ---------------------------------------------------------------------------
// (3) `continue` that does NOT decrease the measure → L0
//     (verus-lowering.md REQ-12(b), AC-7 probe 3).
// ---------------------------------------------------------------------------

/// A loop whose `continue;` re-enters WITHOUT advancing the loop variable `i`, so
/// the measure `n - i` does NOT strictly decrease at the continue back-edge,
/// while the invariant `i <= n` still holds (isolating the decreases obligation).
/// GROUNDED: "decreases not satisfied at continue" → L0.
#[test]
fn continue_not_decreasing_measure_is_l0() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — continue-bad-decreases L0 grounding not exercised.");
        return;
    }
    let certs = check_program(
        "cont_bad_dec",
        "fn bad_dec(n: u64) -> u64\n  \
           req n <= 100000\n  \
           ens result <= n\n  \
           fx pure\n{\n  \
             let mut i: u64 = 0;\n  \
             while i < n\n    \
               inv i <= n\n    \
               dec n - i\n  \
             {\n    \
               if i < n {\n      continue;\n    }\n    \
               i = i + 1;\n  }\n  \
             i\n}\n",
    );
    assert_eq!(
        level(&certs, "bad_dec"),
        "L0",
        "DESIGN verus-lowering.md REQ-12(b): a `continue` back-edge that does NOT \
         decrease the measure is L0 (the decreases obligation BITES at the continue — \
         break/continue are NOT a termination escape hatch, R-DEFER-9). forge: {certs:?}"
    );
}

// ---------------------------------------------------------------------------
// (4) `break` early-exit → L3
//     (verus-lowering.md REQ-12(c), AC-7 probe 4 / OQ-5 policy (ii)).
// ---------------------------------------------------------------------------

/// A loop whose body may `break` early; the post-break fact `result == 5` follows
/// from the invariant `c == 5` that holds AT the break point (a v0.1 Thermite
/// `inv` lowers to a PLAIN Verus `invariant`, which Verus checks at break too —
/// REQ-12(c) / OQ-5 policy (ii)). The break exits cleanly; the `Stmt::Break`
/// lowers to a native Verus `break;`. Tight `ens result == 5` → §7 bites → L3.
#[test]
fn break_early_exit_certifies_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — break-exit L3 grounding not exercised.");
        return;
    }
    let certs = check_program(
        "break_ok",
        "fn const_break(n: u64) -> u64\n  \
           req n <= 100000\n  \
           ens result == 5\n  \
           fx pure\n{\n  \
             let mut i: u64 = 0;\n  \
             let mut c: u64 = 5;\n  \
             while i < n\n    \
               inv i <= n\n    \
               inv c == 5\n    \
               dec n - i\n  \
             {\n    \
               if i == 3 {\n      break;\n    }\n    \
               i = i + 1;\n  }\n  \
             c\n}\n",
    );
    assert_eq!(
        level(&certs, "const_break"),
        "L3",
        "DESIGN verus-lowering.md REQ-12(c): a `break` early-exit certifies L3 when its \
         post-loop fact follows from the loop invariants that hold AT the break point \
         (the plain-`invariant` lowering, GROUNDED: break-exit → L3). forge: {certs:?}"
    );
}

// ---------------------------------------------------------------------------
// (5) `fx diverge` loop with BOTH break and continue → L1 (the #88 cap)
//     (verus-lowering.md REQ-12(d), AC-7 probe 5 — the editor pattern payoff).
// ---------------------------------------------------------------------------

/// The editor's event-loop shape: an `fx diverge` fn whose loop has NO real
/// termination (the lowering suppresses the `decreases` and emits
/// `#[verifier::exec_allows_no_decreases_clause]`). Inside, a `continue` skips an
/// iteration and a `break` exits cleanly on the quit key — NEITHER carries a
/// decreases obligation (no measure). Verus verifies the loop INVARIANTS
/// (partial correctness); forge STRUCTURALLY caps the fn at L1 (#88 diverge cap),
/// NOT L0 — break/continue do not change the cap. This is the payoff: the editor
/// event loop now works WITHOUT the quit-flag + `dec 1` hack.
#[test]
fn diverge_loop_with_break_and_continue_caps_at_l1() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — diverge break/continue L1 grounding not exercised.");
        return;
    }
    let certs = check_program(
        "diverge_bc",
        "fn event_loop(limit: u64) -> u64\n  \
           req limit <= 100000\n  \
           ens result <= 1\n  \
           fx  diverge\n{\n  \
             let mut seen: u64 = 0;\n  \
             loop\n    \
               inv seen <= 1\n    \
               dec 1\n  \
             {\n    \
               let k: u64 = next_key(seen);\n    \
               if k == 0 {\n      continue;\n    }\n    \
               if k == 17 {\n      break;\n    }\n    \
               seen = 1;\n  }\n  \
             seen\n}\n\n\
         fn next_key(s: u64) -> u64\n  \
           req true\n  \
           ens result == s\n  \
           fx pure\n{\n  s\n}\n",
    );
    assert_eq!(
        level(&certs, "event_loop"),
        "L1",
        "DESIGN verus-lowering.md REQ-12(d): a `fx diverge` loop with break/continue \
         (no decreases) verifies its invariants and is STRUCTURALLY capped at L1 by the \
         #88 diverge gate — break/continue exit/skip cleanly, no termination claim \
         (GROUNDED: diverge loop → L1). forge: {certs:?}"
    );
    assert_eq!(
        cert_for(&certs, "event_loop")["boundary"],
        Value::from(false),
        "the diverge cap is keyed on `fx diverge`, NOT a `#[boundary]`: {certs:?}"
    );
    // The diverge cap is NOT a reject (the §7 mutation/strengthen gate is SKIPPED
    // for a diverge fn — `editor_runs.rs` precedent).
    assert!(
        cert_for(&certs, "event_loop")
            .get("reject")
            .map(|r| r.is_null())
            .unwrap_or(true),
        "the diverge cap is L1 partial correctness, NOT an L0 reject: {certs:?}"
    );
    // The helper is fully proved (a sanity check that the program is otherwise sound).
    assert_eq!(
        level(&certs, "next_key"),
        "L3",
        "the pure helper `next_key` certifies L3: {certs:?}"
    );
}

// ---------------------------------------------------------------------------
// (6) PARSER-level in-loop structural rule (parser.md REQ-10, AC-8). No verus.
// ---------------------------------------------------------------------------

/// A `break;` / `continue;` parsed OUTSIDE any loop body is a structured
/// `SyntaxError::BreakContinueOutsideLoop`, never a panic (parser.md REQ-10,
/// AC-8). The check is the parser's loop-depth counter (a structural rule,
/// analogous to the mandatory-clause rule), not a verification rule.
#[test]
fn break_or_continue_outside_a_loop_is_a_structured_error_not_a_panic() {
    use thermite_syntax::parser::SyntaxError;

    let break_top =
        "fn f(n: u64) -> u64\n  req true\n  ens result == 0\n  fx pure\n{\n  break;\n  0\n}\n";
    let r = thermite_syntax::parse(break_top);
    assert!(
        !r.is_clean(),
        "a top-level `break;` (no enclosing loop) must be a parse error: {:?}",
        r.errors
    );
    assert!(
        r.errors
            .iter()
            .any(|e| matches!(e, SyntaxError::BreakContinueOutsideLoop { keyword, .. } if keyword == "break")),
        "DESIGN parser.md REQ-10: a `break;` outside a loop is a structured \
         `BreakContinueOutsideLoop` diagnostic. errors: {:?}",
        r.errors
    );

    let continue_top =
        "fn g(n: u64) -> u64\n  req true\n  ens result == 0\n  fx pure\n{\n  continue;\n  0\n}\n";
    let r2 = thermite_syntax::parse(continue_top);
    assert!(
        r2.errors
            .iter()
            .any(|e| matches!(e, SyntaxError::BreakContinueOutsideLoop { keyword, .. } if keyword == "continue")),
        "DESIGN parser.md REQ-10: a `continue;` outside a loop is a structured \
         `BreakContinueOutsideLoop` diagnostic. errors: {:?}",
        r2.errors
    );
}

/// A `break;` nested inside an `if` block that is itself INSIDE a loop body is
/// accepted (loop-depth > 0 — the counter is per-loop, not per-block), and the
/// statement parses to a `Stmt::Break` in the loop body (parser.md REQ-10,
/// AC-8). A `continue;` likewise.
#[test]
fn break_and_continue_inside_a_loop_parse_cleanly_as_stmt_nodes() {
    use thermite_syntax::ast::{Item, Stmt};

    let prog = "fn f(n: u64) -> u64\n  \
                  req true\n  ens result == 0\n  fx pure\n{\n  \
                    let mut i: u64 = 0;\n  \
                    while i < n\n    inv i <= n\n    dec n - i\n  \
                    {\n    \
                      if i == 3 {\n      break;\n    }\n    \
                      if i == 4 {\n      continue;\n    }\n    \
                      i = i + 1;\n  }\n  \
                    0\n}\n";
    let r = thermite_syntax::parse(prog);
    assert!(
        r.is_clean(),
        "a `break;`/`continue;` nested in an `if` inside a loop body parses cleanly \
         (depth > 0): {:?}",
        r.errors
    );
    // Structurally confirm the loop body holds a `Stmt::Break` and a `Stmt::Continue`
    // (each inside its own `if` statement's then-block).
    let Item::Fn(f) = &r.program.items[0] else {
        panic!("first item is the fn `f`: {:?}", r.program.items);
    };
    let body = f.body.as_ref().expect("fn `f` has a body");
    let loop_stmt = body
        .stmts
        .iter()
        .find_map(|s| match s {
            Stmt::Loop(l) => Some(l),
            _ => None,
        })
        .expect("the body contains a loop");
    let mut saw_break = false;
    let mut saw_continue = false;
    for s in &loop_stmt.body.stmts {
        if let Stmt::If { then, .. } = s {
            for inner in &then.stmts {
                match inner {
                    Stmt::Break => saw_break = true,
                    Stmt::Continue => saw_continue = true,
                    _ => {}
                }
            }
        }
    }
    assert!(
        saw_break,
        "DESIGN ast.md REQ-12 / parser.md REQ-10: a `break;` in a loop-nested `if` \
         parses to a `Stmt::Break`. loop body: {:?}",
        loop_stmt.body.stmts
    );
    assert!(
        saw_continue,
        "DESIGN ast.md REQ-12 / parser.md REQ-10: a `continue;` in a loop-nested `if` \
         parses to a `Stmt::Continue`. loop body: {:?}",
        loop_stmt.body.stmts
    );
}

// ---------------------------------------------------------------------------
// (7) NO REGRESSION: the corpus loops (no break/continue) still certify L3.
//     (verus-lowering.md AC-7 "NO regression": sum/binary_search UNCHANGED.)
// ---------------------------------------------------------------------------

/// `conformance/sum.th` and `conformance/binary_search.th` — whose loops use
/// NEITHER break nor continue — still certify L3 after the `Stmt` ripple. The
/// `Stmt::Break`/`Continue` arms are layer-neutral leaves (no effect, no mutant,
/// no spec-fn call), so a loop WITHOUT them lowers byte-identically.
#[test]
fn corpus_loops_without_break_or_continue_still_certify_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — corpus no-regression L3 not exercised.");
        return;
    }
    for (file, item) in [
        ("conformance/sum.th", "sum"),
        ("conformance/binary_search.th", "binary_search"),
    ] {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace root")
            .join(file);
        let out = Command::new(forge_bin())
            .arg("check")
            .arg(&path)
            .arg("--json")
            .output()
            .expect("spawn forge");
        let stdout = String::from_utf8_lossy(&out.stdout);
        let certs: Vec<Value> = serde_json::from_str::<Value>(stdout.trim())
            .unwrap_or_else(|e| panic!("forge --json for {file}: {e}\nstdout:\n{stdout}"))
            .as_array()
            .expect("array of certs")
            .clone();
        assert_eq!(
            level(&certs, item),
            "L3",
            "NO REGRESSION (verus-lowering.md AC-7): the corpus loop `{item}` (no \
             break/continue) STILL certifies L3 after the #93 Stmt ripple. forge: {certs:?}"
        );
    }
}
