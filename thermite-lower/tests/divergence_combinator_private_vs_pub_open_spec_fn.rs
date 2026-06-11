//! Divergence pin (re-audit of 2f327b63/#230 — the visibility class-completion
//! is INCOMPLETE) — #230 promotes every user `spec fn` to `pub open spec fn`,
//! but the combinator definitions woven from the frozen registry
//! (`thermite-spec::combinators` `verus_l3`, emitted by
//! `emit_combinator_defs`) stay PRIVATE `spec fn`. A user spec fn whose body
//! calls a registered combinator — validator-LEGAL per
//! `.design/spec/spectherm-combinators.md` REQ-3 ("accepts registered
//! combinators, declared spec-fn calls" when walking `SpecFnItem.body`) — now
//! emits a `pub open spec fn` body naming a private `spec fn`, which verus
//! rejects: `error: in pub open spec function, cannot refer to private
//! function` → the item dies at L0 though the Thermite source is correct
//! (verus-confirmed live on the emitted unit). This is a REGRESSION introduced
//! by 2f327b63: before #230 the user spec fn was itself private, and a private
//! caller may name a private callee.
//!
//! THE AUTHORITY (R-CHAR-3): thermite-design.md §6 — L3 is a fully-discharged
//! real-verus proof, so the emitted sub-program for a validator-legal item
//! must VERIFY; plus `.design/lower/verus-lowering.md` REQ-8's own grounding
//! finding (cited by 2f327b63 itself): "a `pub open` body may refer only to
//! `pub` items". The assertion is OUTCOME-anchored (real verus on the emitted
//! unit, the conformance-harness pattern), not anchored to any particular fix
//! direction — never copied from lowerer output.
//!
//! Tracking: crosslink #235.

use std::path::{Path, PathBuf};
use std::process::Command;

fn lower(src: &str) -> String {
    let parsed = thermite_syntax::parse(src);
    assert!(parsed.is_clean(), "fixture must parse clean: {parsed:?}");
    thermite_lower::lower(&parsed.program).expect("lowering must succeed")
}

/// Locate `verus`: `VERUS_BIN`, then PATH, then `~/.local/bin/verus` (the
/// `lower_conformance.rs` resolution order). `None` → SKIP LOUDLY.
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

/// A user `spec fn` whose BODY calls the registered combinator `forall_in`
/// (validator-legal, REQ-3), plus a fn contract that ALSO names `forall_in`
/// (so `emit_combinator_defs` weaves the registry definition into the unit —
/// isolating the VISIBILITY divergence from the separate def-emission scan).
const COMBINATOR_IN_SPEC_FN_BODY: &str = "\
spec fn all_small(s: &[u32]) -> bool
  dec s.len()
{
  forall_in(s, |x| x < 10)
}

fn k(xs: &[u32]) -> u32
  req forall_in(xs, |x| x < 100) && all_small(xs)
  ens result == 0
  fx  pure
{
  0
}
";

#[test]
#[ignore = "divergence: #230 made user spec fns `pub open` but woven combinator defs stay private -> verus `cannot refer to private function` -> L0; tracking #235"]
fn pub_open_user_spec_fn_calling_combinator_verifies() {
    let emitted = lower(COMBINATOR_IN_SPEC_FN_BODY);
    // Non-vacuity: the divergent shape is genuinely present in the unit.
    assert!(
        emitted.contains("pub open spec fn all_small"),
        "fixture must exercise the #230 pub-open user spec fn:\n{emitted}"
    );
    assert!(
        emitted.contains("forall_in"),
        "fixture must exercise a combinator call:\n{emitted}"
    );
    let tmp = std::env::temp_dir().join("divergence_combinator_pub_open.rs");
    std::fs::write(&tmp, &emitted).expect("write temp unit");
    match run_verus(&tmp) {
        Some((ok, output)) => {
            assert!(
                ok && output.contains("verified, 0 errors"),
                "thermite-design.md §6: a validator-legal program's emitted \
                 unit must verify; verus rejected it (the #230 `pub open` user \
                 spec fn names the PRIVATE woven combinator def):\n--- verus \
                 ---\n{output}\n--- emitted ---\n{emitted}"
            );
        }
        None => eprintln!(
            "SKIP (LOUD): verus not found (VERUS_BIN/PATH/~/.local/bin) — \
             the L3 assertion did not run; divergence #235 unverified here."
        ),
    }
}
