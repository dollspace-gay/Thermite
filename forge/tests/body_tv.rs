//! Conformance for the exec-body (state-refinement) translation-validation phase
//! (`.design/verified/exec-stmt-tv.md` REQ-5 + `.design/verified/loop-tv.md` REQ-5;
//! epic crosslink #169, blocker #162). The state analogue of
//! `exec_tv_conformance.rs`. Four required properties, all through the real
//! `verus` binary (skip with a logged reason if absent, mirroring
//! `exec_tv_conformance.rs` / `body_teeth.rs`):
//!
//!   - a faithful straight-line body → Faithful (`forge body-tv` on a `{ let a =
//!     x + 1; let b = a * 2; b }` fixture): the production `lower_exec_body` produces
//!     the reference final state → the body obligation verifies → `faithful`.
//!   - a faithful v1 `while`-loop body → Faithful (all three per-run obligations
//!     verify): a `while lo < n inv lo <= n dec n - lo { lo = lo + 1 }` loop's entry /
//!     preservation / exit obligations all verify → `faithful`.
//!   - a mutated production → Divergent: a wrong production body (the
//!     `body_teeth.rs` B2 reordered-mutation shape — `s = s * 2; s = s + 1` for
//!     source `s = s + 1; s = s * 2`) discharged through the same body obligation
//!     fails the `ensures result == <body_ref_state>` postcondition → `Divergent`.
//!     (The corpus path uses the faithful lowerer so it never diverges; the Divergent
//!     arm is exercised by injecting a known-wrong production at the obligation layer,
//!     as `exec_tv::divergent_teeth` does for the exec-expr phase.)
//!   - an out-of-subset body → Skipped+reason: `binary_search.th`'s `loop`-kind
//!     body (multi-exit, mid-body `return`s — out of the v1 single-`while` subset) is
//!     `skipped` with a reason (the 2.2.1-vs-2.2.2 boundary — never `faithful`,
//!     R-HONEST-3).
//!
//! Expected values trace to the design's faithful-lowering invariant + the frozen
//! subset, rather than to the lowerer's output (R-CHAR-3). `unwrap`/`expect` are
//! fine here (`tests/` is not anti-pattern-gated).

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
/// then PATH, then `~/.local/bin/verus`. Skip with a logged reason otherwise.
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
/// reported `faithful` (the body state-refinement obligation verifies), 0 divergent.
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

#[test]
fn exclusive_aggregate_storage_writes_are_faithful() {
    if !verus_present() {
        eprintln!(
            "SKIP: verus not available — exclusive aggregate-storage body-TV not discharged."
        );
        return;
    }
    let src = concat!(
        "fn write_slice(data: &mut [[u64; 2]], at: usize, value: u64) -> u64\n",
        "  req at < data.len()\n",
        "  ens result == value\n",
        "  ens final(data)[at][0] == value\n",
        "  fx platform(memory)\n",
        "{\n",
        "  data[at] = [value, value];\n",
        "  value\n",
        "}\n",
        "fn write_array(data: &mut [u64; 4], at: usize, value: u64) -> u64\n",
        "  req at < 4\n",
        "  ens result == value\n",
        "  ens final(data)[at] == value\n",
        "  fx platform(memory)\n",
        "{\n",
        "  data[at] = value;\n",
        "  value\n",
        "}\n",
    );
    let file = write_th("exclusive_aggregate_storage", src);
    let report = run_body_tv_json(&file);
    let counts = &report["counts"];
    assert_eq!(counts["faithful"].as_u64(), Some(2), "{report}");
    assert_eq!(counts["divergent"].as_u64(), Some(0), "{report}");
    assert_eq!(counts["unverifiable"].as_u64(), Some(0), "{report}");
    assert_eq!(counts["skipped"].as_u64(), Some(0), "{report}");
}

// ---- 2. a faithful v1 while-loop body → Faithful (all three obligations) ----

/// loop-tv REQ-5 (the loop arm): a faithful v1-subset `while lo < n inv lo <= n dec
/// n - lo { lo = lo + 1 }` body — all three per-run obligations (entry / preservation
/// / exit) verify — is reported `faithful`, 0 divergent.
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

/// Fixed-array indexed mutation is inside the exact state-refinement subset: the
/// independent reference is the finite-view update at `at`, and production is the
/// native array assignment. This exercises the real Forge file walk, capacity
/// declaration preamble, production lowering, Verus discharge, and verdict mapping.
#[test]
fn faithful_fixed_array_update_is_faithful() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — fixed-array body-TV not discharged.");
        return;
    }
    let src = concat!(
        "const SLOTS: usize = 4;\n",
        "fn replace(slots: [u64; SLOTS], at: usize, value: u64) -> [u64; SLOTS]\n",
        "  req at < SLOTS\n",
        "  ens result[at] == value\n",
        "  fx pure\n",
        "{\n",
        "  let mut updated: [u64; SLOTS] = slots;\n",
        "  updated[at] = value;\n",
        "  updated\n",
        "}\n",
        "fn array_len(slots: [u64; SLOTS]) -> usize\n",
        "  req true\n",
        "  ens result == slots.len()\n",
        "  fx pure\n",
        "{ slots.len() }\n",
        "fn arrays_equal(left: [u64; SLOTS], right: [u64; SLOTS]) -> bool\n",
        "  req true\n",
        "  ens result == left.array_eq(right)\n",
        "  fx pure\n",
        "{ left.array_eq(right) }\n",
        "fn arrays_same_except(left: [u64; SLOTS], right: [u64; SLOTS], at: usize) -> bool\n",
        "  req true\n",
        "  ens result == left.array_same_except(right, at)\n",
        "  fx pure\n",
        "{ left.array_same_except(right, at) }\n",
    );
    let file = write_th("fixed_array_update", src);
    let report = run_body_tv_json(&file);
    assert_eq!(report["counts"]["divergent"].as_u64(), Some(0), "{report}");
    assert_eq!(report["counts"]["faithful"].as_u64(), Some(4), "{report}");
    assert!(report["bodies"].as_array().unwrap().iter().any(|body| {
        body["body"].as_str() == Some("replace") && body["verdict"].as_str() == Some("faithful")
    }));
    assert!(report["bodies"].as_array().unwrap().iter().any(|body| {
        body["body"].as_str() == Some("array_len") && body["verdict"].as_str() == Some("faithful")
    }));
    assert!(report["bodies"].as_array().unwrap().iter().any(|body| {
        body["body"].as_str() == Some("arrays_equal")
            && body["verdict"].as_str() == Some("faithful")
    }));
    assert!(report["bodies"].as_array().unwrap().iter().any(|body| {
        body["body"].as_str() == Some("arrays_same_except")
            && body["verdict"].as_str() == Some("faithful")
    }));
}

#[test]
fn named_record_lifecycle_bodies_are_faithful() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — named-record lifecycle body-TV not discharged.");
        return;
    }
    let src = r#"
#[opaque] struct State { generation: u64, occupied: bool }

fn state_new(generation: u64, occupied: bool) -> State
  req true
  ens result.generation == generation
  ens result.occupied == occupied
  fx pure
{
  State { generation: generation, occupied: occupied }
}

fn observe(state: &State) -> bool
  req true
  ens result == state.occupied
  fx pure
{
  state.occupied
}

fn advance(state: &mut State, next: u64) -> bool
  req next > old(state).generation
  ens result == old(state).occupied
  ens final(state).generation == next
  ens final(state).occupied == old(state).occupied
  fx pure
{
  let previous: bool = state.occupied;
  state.generation = next;
  previous
}

fn set_generation(state: &mut State, next: u64) -> ()
  req true
  ens final(state).generation == next
  ens final(state).occupied == old(state).occupied
  fx pure
{
  state.generation = next;
}

fn choose_generation(state: &mut State, choose_next: bool, next: u64) -> u64
  req true
  ens true
  fx pure
{
  if choose_next {
    state.generation = next;
  } else {
    state.generation = 0;
  }
  0
}
"#;
    let file = write_th("named_record_lifecycle", src);
    let report = run_body_tv_json(&file);
    assert_eq!(report["counts"]["checked"].as_u64(), Some(5), "{report}");
    assert_eq!(report["counts"]["faithful"].as_u64(), Some(5), "{report}");
    assert_eq!(report["counts"]["divergent"].as_u64(), Some(0), "{report}");
    assert_eq!(
        report["counts"]["unverifiable"].as_u64(),
        Some(0),
        "{report}"
    );
    assert_eq!(report["counts"]["skipped"].as_u64(), Some(0), "{report}");
    for name in [
        "state_new",
        "observe",
        "advance",
        "set_generation",
        "choose_generation",
    ] {
        assert!(
            report["bodies"].as_array().unwrap().iter().any(|body| {
                body["body"].as_str() == Some(name) && body["verdict"].as_str() == Some("faithful")
            }),
            "{name}: {report}"
        );
    }
}

#[test]
fn nested_record_and_terminal_array_writes_are_faithful() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — nested aggregate body-TV not discharged.");
        return;
    }
    let src = r#"
const SLOTS: usize = 2;
struct Inner { value: u64, guard: u64 }
struct Nested { inner: Inner, slots: [u64; SLOTS], tag: u64 }

fn nested_owned(state: Nested, index: usize, next: u64) -> Nested
  req index < SLOTS && next < 1000
  ens result.inner.value == next
  ens result.inner.guard == state.inner.guard
  ens result.slots[index] == next + 1
  ens result.tag == state.tag
  fx pure
{
  let mut updated: Nested = state;
  updated.inner.value = next;
  updated.slots[index] = updated.inner.value + 1;
  updated
}

fn nested_borrowed(state: &mut Nested, index: usize, next: u64) -> u64
  req index < SLOTS && next < 1000
  ens result == next
  ens final(state).inner.value == next
  ens final(state).inner.guard == old(state).inner.guard
  ens final(state).slots[index] == next + 1
  ens final(state).tag == old(state).tag
  fx pure
{
  state.inner.value = next;
  state.slots[index] = state.inner.value + 1;
  state.inner.value
}
"#;
    let file = write_th("nested_aggregate_lifecycle", src);
    let report = run_body_tv_json(&file);
    assert_eq!(report["counts"]["checked"].as_u64(), Some(2), "{report}");
    assert_eq!(report["counts"]["faithful"].as_u64(), Some(2), "{report}");
    assert_eq!(report["counts"]["divergent"].as_u64(), Some(0), "{report}");
    assert_eq!(
        report["counts"]["unverifiable"].as_u64(),
        Some(0),
        "{report}"
    );
    assert_eq!(report["counts"]["skipped"].as_u64(), Some(0), "{report}");
    for name in ["nested_owned", "nested_borrowed"] {
        assert!(
            report["bodies"].as_array().unwrap().iter().any(|body| {
                body["body"].as_str() == Some(name) && body["verdict"].as_str() == Some("faithful")
            }),
            "{name}: {report}"
        );
    }
}

#[test]
fn owned_aggregate_fixture_is_entirely_l3_body_faithful() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — owned-aggregate body-TV not discharged.");
        return;
    }
    let file = corpus_dir().join("verified-build/owned_aggregate_lifecycle.th");
    let report = run_body_tv_json(&file);
    assert_eq!(report["counts"]["checked"].as_u64(), Some(7), "{report}");
    assert_eq!(report["counts"]["faithful"].as_u64(), Some(7), "{report}");
    assert_eq!(report["counts"]["divergent"].as_u64(), Some(0), "{report}");
    assert_eq!(
        report["counts"]["unverifiable"].as_u64(),
        Some(0),
        "{report}"
    );
    assert_eq!(report["counts"]["skipped"].as_u64(), Some(0), "{report}");
    for name in [
        "owned_state_generation",
        "owned_state_occupied",
        "owned_state_first",
        "owned_state_second",
        "owned_state_mix_generation",
        "owned_state_mix_second",
        "owned_state_pipeline",
    ] {
        assert!(
            report["bodies"].as_array().unwrap().iter().any(|body| {
                body["body"].as_str() == Some(name) && body["verdict"].as_str() == Some("faithful")
            }),
            "{name}: {report}"
        );
    }
}

#[test]
fn nested_aggregate_fixture_is_entirely_l3_body_faithful() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — nested-aggregate body-TV not discharged.");
        return;
    }
    let file = corpus_dir().join("verified-build/nested_aggregate_lifecycle.th");
    let report = run_body_tv_json(&file);
    assert_eq!(report["counts"]["checked"].as_u64(), Some(5), "{report}");
    assert_eq!(report["counts"]["faithful"].as_u64(), Some(5), "{report}");
    assert_eq!(report["counts"]["divergent"].as_u64(), Some(0), "{report}");
    assert_eq!(
        report["counts"]["unverifiable"].as_u64(),
        Some(0),
        "{report}"
    );
    assert_eq!(report["counts"]["skipped"].as_u64(), Some(0), "{report}");
    for name in [
        "nested_state_value",
        "nested_state_guard",
        "nested_state_tag",
        "nested_state_update",
        "nested_state_pipeline",
    ] {
        assert!(
            report["bodies"].as_array().unwrap().iter().any(|body| {
                body["body"].as_str() == Some(name) && body["verdict"].as_str() == Some("faithful")
            }),
            "{name}: {report}"
        );
    }
}

#[test]
fn mutable_reference_callee_effects_remain_fail_closed() {
    let source = r#"
struct State { value: u64 }
fn mutate(state: &mut State, value: u64) -> ()
  req true
  ens final(state).value == value
  fx pure
{
  state.value = value;
}
fn call_mutate(state: &mut State, value: u64) -> ()
  req true
  ens final(state).value == value
  fx pure
{
  mutate(state, value);
}
"#;
    let file = write_th("mutable_reference_callee", source);
    let report = run_body_tv_json(&file);
    let caller = report["bodies"]
        .as_array()
        .unwrap()
        .iter()
        .find(|body| body["body"].as_str() == Some("call_mutate"))
        .unwrap_or_else(|| panic!("missing caller body: {report}"));
    assert_eq!(caller["verdict"].as_str(), Some("skipped"), "{report}");
    let detail = caller["detail"].as_str().unwrap_or_default();
    assert!(
        detail.contains("mutable-reference") && detail.contains("call-effect"),
        "{detail}"
    );
}

#[test]
fn aggregate_array_relations_are_faithful() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — aggregate-array body-TV not discharged.");
        return;
    }
    let src = concat!(
        "const WORDS: usize = 2;\n",
        "const SLOTS: usize = 4;\n",
        "struct Stamp { words: [u64; WORDS], flags: (bool, u8) }\n",
        "struct Slot { stamp: Stamp, owner: usize }\n",
        "fn records_equal(left: [Slot; SLOTS], right: [Slot; SLOTS]) -> bool\n",
        "  req true\n",
        "  ens result == left.array_eq(right)\n",
        "  fx pure\n",
        "{ left.array_eq(right) }\n",
        "fn records_same_except(left: [Slot; SLOTS], right: [Slot; SLOTS], at: usize) -> bool\n",
        "  req true\n",
        "  ens result == left.array_same_except(right, at)\n",
        "  fx pure\n",
        "{ left.array_same_except(right, at) }\n",
    );
    let file = write_th("aggregate_array_relations", src);
    let report = run_body_tv_json(&file);
    assert_eq!(report["counts"]["checked"].as_u64(), Some(2), "{report}");
    assert_eq!(report["counts"]["faithful"].as_u64(), Some(2), "{report}");
    assert_eq!(report["counts"]["divergent"].as_u64(), Some(0), "{report}");
    for name in ["records_equal", "records_same_except"] {
        assert!(
            report["bodies"].as_array().unwrap().iter().any(|body| {
                body["body"].as_str() == Some(name) && body["verdict"].as_str() == Some("faithful")
            }),
            "{name}: {report}"
        );
    }
}

#[test]
fn faithful_u64_bit_method_body_is_faithful() {
    if !verus_present() {
        eprintln!("SKIP: verus not available — u64-bit body-TV not discharged.");
        return;
    }
    let src = concat!(
        "fn set_through_local(word: u64, bit: usize) -> u64\n",
        "  req true\n",
        "  ens result == word.bit_set(bit)\n",
        "  fx pure\n",
        "{\n",
        "  let updated: u64 = word.bit_set(bit);\n",
        "  updated\n",
        "}\n",
        "fn preserve_through_local(word: u64, changed: usize, observed: usize) -> bool\n",
        "  req true\n",
        "  ens result == word.bit_clear_preserves_other(changed, observed)\n",
        "  fx pure\n",
        "{\n",
        "  let preserved: bool = word.bit_clear_preserves_other(changed, observed);\n",
        "  preserved\n",
        "}\n",
    );
    let file = write_th("u64_bit_methods", src);
    let report = run_body_tv_json(&file);
    assert_eq!(report["counts"]["checked"].as_u64(), Some(2), "{report}");
    assert_eq!(report["counts"]["faithful"].as_u64(), Some(2), "{report}");
    assert_eq!(report["counts"]["divergent"].as_u64(), Some(0), "{report}");
    assert!(report["bodies"].as_array().unwrap().iter().any(|body| {
        body["body"].as_str() == Some("set_through_local")
            && body["verdict"].as_str() == Some("faithful")
    }));
    assert!(report["bodies"].as_array().unwrap().iter().any(|body| {
        body["body"].as_str() == Some("preserve_through_local")
            && body["verdict"].as_str() == Some("faithful")
    }));
}

// ---- 3. a mutated production → Divergent ------------------------------------

/// REQ-5 (the Divergent arm): a wrong production body — the
/// `body_teeth.rs` B2 reordered-mutation shape (`s = s * 2; s = s + 1` for source
/// `s = s + 1; s = s * 2`, final state `(x*2)+1 != (x+1)*2`) — discharged through the
/// same body state-refinement obligation `forge body-tv` builds (the obligation text
/// is the contract surface) fails the `ensures result == <body_ref_state>`
/// postcondition. This is the forge Divergent classification's trigger: a results line
/// with `errors >= 1` → `BodyVerdict::Divergent`. The corpus path never diverges (the
/// faithful lowerer), so the Divergent arm is exercised by injecting a known-wrong
/// production at the obligation layer (as `exec_tv::divergent_teeth` does).
#[test]
fn mutated_production_diverges() {
    if !verus_present() {
        eprintln!(
            "SKIP: verus not available — the mutated-production Divergent arm not discharged."
        );
        return;
    }
    // The B2 source body `{ let mut s = x; s = s + 1; s = s * 2; s }`, with reference
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
    // The mutated (reordered) production — each RHS is value-faithful in isolation, the
    // order is the bug → final state `(x * 2) + 1`, != the reference `((x + 1) * 2)`.
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
            // errors >= 1 is the signal `body_tv::discharge` maps to
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

/// loop-tv REQ-5 / AC-4 (the Skipped arm, the live corpus demo): `binary_search.th`'s
/// `loop`-kind body (a multi-exit form with mid-body `return None`/`return Some(mid)`
/// — out of the v1 single-`while` subset) is reported `skipped` with a reason, never
/// `faithful` (R-HONEST-3 — a skip never masks an infidelity). This is the expected
/// corpus result.
#[test]
fn binary_search_loop_is_skipped_with_reason() {
    // No verus needed — the loop is recognized out-of-v1 before any discharge (the
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

/// REQ-5 (the exit-code convention): a clean body-TV run (no Divergent, only Faithful
/// / Skipped) exits 0 (the convention `forge exec-tv` uses). `binary_search.th`
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
