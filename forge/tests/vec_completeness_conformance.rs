//! Cluster C6 conformance (`.design/basis/04-collections.md` REQ-8..12, issue
//! #98) against the EXTERNAL truth: the real `verus` binary. The emitted L3
//! lowering of each C6 form MUST verify (`0 errors`); the no-OOB negative MUST
//! FAIL (non-vacuity, R-DEFER-9). Expected verus counts are the DESIGN's GROUNDED
//! record (the "C6 grounding record" section), NEVER copied from toolchain output
//! (R-CHAR-3).
//!
//! The forms (all GROUNDED in `.design/basis/04-collections.md` with real
//! `verus 0.2026.05.24`):
//! - `Vec<u64>` `pop_last`/`last`/`insert`/`remove`/`contains` (REQ-8) → L3.
//! - the `insert` WITHOUT the `i <= len` guard at the call site → L0.
//! - `Vec<String>` build + borrow-`get` + len (REQ-9, the make-or-break) → L3.
//! - `Vec<struct>` push + borrow-`get` + field read (REQ-9) → L3.
//! - nested `Vec<Vec<u64>>` push + borrow-`get` (REQ-9) → L3.
//! - a body-local `Vec::new()` with NO `Vec` param/return (REQ-11) → L3.
//!
//! `unwrap`/`expect`/`panic!` are fine here — `tests/` is not anti-pattern-gated.

use std::path::{Path, PathBuf};
use std::process::Command;

// ---- verus driver (shared shape with thermite-lower collections_conformance) --

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

/// Run `verus --no-cheating <file>`; `None` if verus is unavailable (caller SKIPs
/// LOUDLY). `--no-cheating` so a sneaked `assume`/`external_body` is a hard error.
fn run_verus(file: &Path) -> Option<(bool, String)> {
    let bin = verus_bin()?;
    let out = Command::new(bin)
        .arg("--no-cheating")
        .arg(file)
        .current_dir(std::env::temp_dir())
        .output()
        .ok()?;
    let mut combined = String::from_utf8_lossy(&out.stdout).to_string();
    combined.push_str(&String::from_utf8_lossy(&out.stderr));
    Some((out.status.success(), combined))
}

fn verify(crate_name: &str, emitted: &str) -> Option<(bool, String)> {
    let tmp = std::env::temp_dir().join(format!("{crate_name}.rs"));
    std::fs::write(&tmp, emitted).unwrap_or_else(|e| panic!("write temp {crate_name}: {e}"));
    run_verus(&tmp)
}

fn parse_src(src: &str, label: &str) -> thermite_syntax::ast::Program {
    let parsed = thermite_syntax::parse(src);
    assert!(
        parsed.errors.is_empty(),
        "{label} must parse clean: {:?}",
        parsed.errors
    );
    parsed.program
}

fn lower_l3(src: &str, label: &str) -> String {
    let program = parse_src(src, label);
    thermite_lower::lower(&program).unwrap_or_else(|e| panic!("{label} L3 lowering failed: {e}"))
}

fn assert_no_cheats(emitted: &str, name: &str) {
    for forbidden in [
        "assume(false)",
        "assume(",
        "#[verifier::external]",
        "#[verifier::external_body]",
        "admit(",
        "#[slag]",
    ] {
        assert!(
            !emitted.contains(forbidden),
            "{name} emission contains forbidden cheat token `{forbidden}` (R-DEFER-9):\n{emitted}"
        );
    }
}

fn assert_verifies(label: &str, emitted: &str) {
    match verify(label, emitted) {
        Some((ok, output)) => {
            assert!(
                ok && output.contains("verified, 0 errors"),
                "{label}: emitted output did NOT verify L3 (R-CODE-4). exit_success={ok}\n\
                 --- verus output ---\n{output}\n--- emitted ---\n{emitted}"
            );
        }
        None => {
            eprintln!("SKIP: verus unavailable — {label} L3 verification not run (set VERUS_BIN).")
        }
    }
}

// ---- AC-5: the missing ops over Vec<u64> certify L3 (REQ-8/REQ-12) ----------
//
// GROUNDED (the C6 grounding record): pop_last/last/insert/remove/contains over
// Vec<u64> verify together `9 verified, 0 errors`. The ops emit on TVecU64; the
// &mut ops use final(self); contains is the exec linear scan.

const VEC_U64_OPS: &str = r#"
fn build_u64(i: usize, x: u64) -> u64
  req true
  ens true
  fx alloc
{
    let mut v: Vec<u64> = Vec::new();
    v.push(7);
    v.push(9);
    let lst = v.last();
    let c = v.contains(7);
    v.remove(0);
    v.pop_last();
    lst
}
"#;

#[test]
fn vec_u64_ops_certify_l3() {
    let emitted = lower_l3(VEC_U64_OPS, "vec_u64_ops");
    // REQ-8: the five ops are emitted on TVecU64.
    for needle in [
        "pub fn pop_last(&mut self)",
        "pub fn last(&self) -> (result: u64)",
        "pub fn insert(&mut self, i: usize, x: u64)",
        "pub fn remove(&mut self, i: usize)",
        "pub fn contains(&self, x: u64) -> (result: bool)",
    ] {
        assert!(
            emitted.contains(needle),
            "REQ-8 op `{needle}` not emitted on TVecU64:\n{emitted}"
        );
    }
    // REQ-8 the no-OOB insert guard (`i <= len`) is present (load-bearing).
    assert!(
        emitted.contains("i <= old(self).data.len(),"),
        "REQ-8 insert's `i <= len` no-OOB guard absent (would be vacuous):\n{emitted}"
    );
    // REQ-8 the &mut ops use final(self) (the grounding finding).
    assert!(
        emitted.contains("final(self).data@ == old(self).data@.insert(i as int, x),")
            && emitted.contains("final(self).data@ == old(self).data@.remove(i as int),"),
        "REQ-8 insert/remove must use the final(self) Seq postcondition:\n{emitted}"
    );
    assert_no_cheats(&emitted, "vec_u64_ops");
    // GROUNDED `9 verified, 0 errors` (design C6 grounding record).
    assert_verifies("vec_u64_ops_c6", &emitted);
}

// ---- AC-5 negative: insert without the i<=len guard at the call site → L0 ---
//
// GROUNDED: the insert whose index `i` is unconstrained leaves the wrapper's
// `req i <= len` undischarged → verus FAILS (`8 verified, 1 errors`, the L0
// demonstration; R-DEFER-9 non-vacuity).

const VEC_INSERT_OOB: &str = r#"
fn bad_insert(i: usize, x: u64) -> u64
  req true
  ens true
  fx alloc
{
    let mut v: Vec<u64> = Vec::new();
    v.insert(i, x);
    0
}
"#;

#[test]
fn vec_insert_without_oob_guard_fails_verus_l0() {
    let emitted = lower_l3(VEC_INSERT_OOB, "vec_insert_oob");
    // It LOWERS (well-formed); the FAILURE is at verus (the wrapper's `req i <=
    // len` cannot be discharged for an unconstrained `i`), not a lowerer error.
    assert!(
        emitted.contains("v.insert(i, x)"),
        "the reject program lowers to the wrapper insert call:\n{emitted}"
    );
    assert_no_cheats(&emitted, "vec_insert_oob");
    match verify("vec_insert_oob_c6", &emitted) {
        Some((ok, output)) => {
            assert!(
                !ok || !output.contains("0 errors"),
                "the unguarded insert MUST FAIL verus (L0, not laundered to L3):\n{output}\n\
                 --- emitted ---\n{emitted}"
            );
            assert!(
                output.contains("precondition not satisfied") || output.contains("error"),
                "expected a verus precondition error for the unguarded insert:\n{output}"
            );
        }
        None => eprintln!("SKIP: verus unavailable — insert-OOB L0 reject not run."),
    }
}

// ---- AC-6: Vec<String> builds/indexes via borrow-get, certifies L3 ----------
//
// THE MAKE-OR-BREAK. GROUNDED `4 verified, 0 errors`: TVecTString over
// vstd::vec::Vec<TString>, the BORROW-returning get -> &TString, push consuming
// the owned element, a build_and_read fn. The by-value-move form FAILS E0507.

const VEC_STRING: &str = r#"
fn build_str() -> u64
  req true
  ens true
  fx alloc
{
    let mut v: Vec<String> = Vec::new();
    let s = String::from_byte(65);
    v.push(s);
    let e = v.get(0);
    e.len()
}
"#;

#[test]
fn vec_string_borrow_get_certifies_l3() {
    let emitted = lower_l3(VEC_STRING, "vec_string");
    // REQ-9: TVecTString over Vec<TString>.
    assert!(
        emitted.contains("pub struct TVecTString { pub data: Vec<TString> }"),
        "REQ-9 Vec<String> → TVecTString over Vec<TString>:\n{emitted}"
    );
    // REQ-9 the BORROW-returning get -> &TString (NOT by value — the E0507 fix).
    assert!(
        emitted.contains("pub fn get(&self, i: usize) -> (result: &TString)")
            && emitted.contains("ensures *result == self.data@[i as int],")
            && emitted.contains("{ &self.data[i] }"),
        "REQ-9 the borrow-returning get -> &TString (the non-Copy fix):\n{emitted}"
    );
    // REQ-10: the TString element wrapper is WOVEN (present in the same verus!
    // block) so the TVecTString that names it resolves. Verus resolves references
    // within a `verus!` block order-independently (the 17/0 verify confirms), so
    // the requirement is PRESENCE, not literal source order.
    assert!(
        emitted.contains("pub struct TString { pub data: Vec<u8> }"),
        "REQ-10 the TString element wrapper must be woven for TVecTString:\n{emitted}"
    );
    assert_no_cheats(&emitted, "vec_string");
    // GROUNDED L3 (the make-or-break — NO E0507).
    assert_verifies("vec_string_c6", &emitted);
}

// ---- AC-7: Vec<struct> push/borrow-get certifies L3 (REQ-9/REQ-10) ----------
//
// GROUNDED `4 verified, 0 errors`: TVecPoint over a 2-field non-Copy struct, the
// borrow-get -> &Point, push by move, a fn pushing/borrow-getting/reading a field.

const VEC_STRUCT: &str = r#"
struct Point { x: u64, y: u64 }

fn build_struct() -> u64
  req true
  ens true
  fx alloc
{
    let mut v: Vec<Point> = Vec::new();
    v.push(Point { x: 3, y: 4 });
    let e = v.get(0);
    e.x
}
"#;

#[test]
fn vec_struct_borrow_get_certifies_l3() {
    let emitted = lower_l3(VEC_STRUCT, "vec_struct");
    assert!(
        emitted.contains("pub struct TVecPoint { pub data: Vec<Point> }"),
        "REQ-9 Vec<Point> → TVecPoint over Vec<Point>:\n{emitted}"
    );
    assert!(
        emitted.contains("pub fn get(&self, i: usize) -> (result: &Point)"),
        "REQ-9 the borrow-returning get -> &Point:\n{emitted}"
    );
    // REQ-10: the Point struct decl is WOVEN (present) so TVecPoint resolves (the
    // #68 weave). Verus resolves references within a `verus!` block
    // order-independently (the 7/0 verify confirms), so the requirement is
    // PRESENCE, not literal source order.
    assert!(
        emitted.contains("pub struct Point {"),
        "REQ-10 the Point struct decl must be woven for TVecPoint:\n{emitted}"
    );
    assert_no_cheats(&emitted, "vec_struct");
    assert_verifies("vec_struct_c6", &emitted);
}

// ---- AC-7 (nested): Vec<Vec<u64>> push/borrow-get certifies L3 (REQ-9/REQ-10)
//
// GROUNDED `4 verified, 0 errors`: TVecTVecU64 over Vec<TVecU64> (the element
// TVecU64 itself non-Copy), the SAME borrow-get. The inner TVecU64 wrapper is
// declared before the outer (REQ-10). NOTE: the `Vec<Vec<u64> >` close needs a
// SPACE — the lexer tokenizes `>>` as a shift op (a parser-side limitation tracked
// separately; the lowering+verus stack handles nested Vecs end-to-end).

const VEC_NESTED: &str = r#"
fn build_nested() -> u64
  req true
  ens true
  fx alloc
{
    let mut outer: Vec<Vec<u64> > = Vec::new();
    let inner: Vec<u64> = Vec::new();
    outer.push(inner);
    let e = outer.get(0);
    0
}
"#;

#[test]
fn vec_nested_borrow_get_certifies_l3() {
    let emitted = lower_l3(VEC_NESTED, "vec_nested");
    assert!(
        emitted.contains("pub struct TVecTVecU64 { pub data: Vec<TVecU64> }"),
        "REQ-9 nested Vec<Vec<u64>> → TVecTVecU64 over Vec<TVecU64>:\n{emitted}"
    );
    // REQ-10 emission ORDER: the inner TVecU64 wrapper precedes the outer.
    let inner_pos = emitted
        .find("pub struct TVecU64")
        .expect("inner TVecU64 wrapper emitted");
    let outer_pos = emitted
        .find("pub struct TVecTVecU64")
        .expect("outer TVecTVecU64 wrapper emitted");
    assert!(
        inner_pos < outer_pos,
        "REQ-10 the inner TVecU64 must precede the outer TVecTVecU64:\n{emitted}"
    );
    // The outer's borrow-get returns &TVecU64.
    assert!(
        emitted.contains("pub fn get(&self, i: usize) -> (result: &TVecU64)"),
        "REQ-9 the outer borrow-get -> &TVecU64:\n{emitted}"
    );
    assert_no_cheats(&emitted, "vec_nested");
    assert_verifies("vec_nested_c6", &emitted);
}

// ---- AC-8: a local Vec::new() with NO Vec param certifies L3 (REQ-11) -------
//
// The reachability fix. A fn whose ONLY Vec is a body-local `let mut v: Vec<u64>
// = Vec::new();` (no Vec param/return) must emit TVecU64 — NOT E0425. GROUNDED
// feasible (the verus form verifies; the gap was wrapper-emission reachability).

const VEC_LOCAL_NEW: &str = r#"
fn local_only(x: u64) -> u64
  req x < 1000000
  ens true
  fx alloc
{
    let mut v: Vec<u64> = Vec::new();
    v.push(x);
    let r = v.get(0);
    r
}
"#;

#[test]
fn local_vec_new_no_param_certifies_l3() {
    let emitted = lower_l3(VEC_LOCAL_NEW, "vec_local_new");
    // REQ-11: the TVecU64 wrapper IS emitted even though no fn param/return is a
    // Vec — the body-local `let` annotation drove the reachability.
    assert!(
        emitted.contains("pub struct TVecU64 { pub data: Vec<u64> }"),
        "REQ-11 a body-local `Vec::new()` must emit TVecU64 (the reachability fix):\n{emitted}"
    );
    // The init lowered to the wrapper construction (not a bare Vec::new()).
    assert!(
        emitted.contains("TVecU64 { data: Vec::new() }"),
        "REQ-11 the local `Vec::new()` lowers to the wrapper construction:\n{emitted}"
    );
    assert_no_cheats(&emitted, "vec_local_new");
    // GROUNDED L3 — NOT E0425 cannot find type TVecU64.
    assert_verifies("vec_local_new_c6", &emitted);
}

// ---- no regression: a non-Vec program emits no TVec wrapper -----------------

#[test]
fn non_vec_program_emits_no_vec_wrapper() {
    let src = r#"
fn add(a: u64, b: u64) -> u64
  req a < 1000 && b < 1000
  ens result == a + b
  fx pure
{
    a + b
}
"#;
    let emitted = lower_l3(src, "non_vec");
    assert!(
        !emitted.contains("TVec") && !emitted.contains("pub data: Vec<"),
        "a non-Vec program must emit no Vec wrapper (byte-stable, no regression):\n{emitted}"
    );
    assert_verifies("non_vec_c6", &emitted);
}
