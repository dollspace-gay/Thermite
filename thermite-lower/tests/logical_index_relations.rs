//! Emission and proof coverage for the quantified declared-index relations
//! (`.design/build/aggregate-array-relations.md`, "Lowering the relation").
//!
//! Each relation is one first-order `forall` over `usize` whose triggers are the
//! declared observer applied to each operand, written as alternatives. The
//! depth-`C` recursive encoding the design rejects certifies at L0 because Verus
//! spends its unfolding budget before the postcondition is reachable; the
//! `forall` moves the work from unfolding to instantiation, so an
//! index-transparent frame closes from the body with no author-written hint.

use std::path::{Path, PathBuf};
use std::process::Command;

use thermite_lower::lower;

/// An index-transparent 64-slot view: one storage element per logical index.
const RING_SOURCE: &str = r#"
const FIXED_RING_CAPACITY: usize = 64;

#[logical(bound = "FIXED_RING_CAPACITY", observe = "fixed_ring_slot_spec")]
struct FixedRing64 {
  slots: [u64; FIXED_RING_CAPACITY],
  head: usize,
  len: usize,
}

spec fn fixed_ring_slot_spec(ring: &FixedRing64, slot: usize) -> u64
  dec slot
{
  ring.slots[slot]
}

fn ring_write(ring: FixedRing64, at: usize, value: u64) -> FixedRing64
  req at < FIXED_RING_CAPACITY
  ens result.slots[at] == value
  ens result.logical_same_except(&ring, at)
  fx pure
{
  let mut slots: [u64; FIXED_RING_CAPACITY] = ring.slots;
  slots[at] = value;
  FixedRing64 {
    slots: slots,
    head: ring.head,
    len: ring.len,
  }
}

fn ring_write_two(
  ring: FixedRing64,
  first: usize,
  second: usize,
  value: u64,
) -> FixedRing64
  req first < FIXED_RING_CAPACITY
    && second < FIXED_RING_CAPACITY
    && first != second
  ens result.slots[first] == value
  ens result.slots[second] == value
  ens result.logical_same_except_two(&ring, first, second)
  fx pure
{
  let mut slots: [u64; FIXED_RING_CAPACITY] = ring.slots;
  slots[first] = value;
  slots[second] = value;
  FixedRing64 {
    slots: slots,
    head: ring.head,
    len: ring.len,
  }
}

fn ring_adopt(target: FixedRing64, source: &FixedRing64) -> FixedRing64
  req true
  ens result.logical_eq(source)
  fx pure
{
  FixedRing64 {
    slots: source.slots,
    head: target.head,
    len: target.len,
  }
}
"#;

fn program(source: &str) -> thermite_syntax::Program {
    let parsed = thermite_syntax::parse(source);
    assert!(parsed.is_clean(), "fixture must parse: {:?}", parsed.errors);
    thermite_spec::validate(&parsed.program).expect("declared-index relations must validate");
    parsed.program
}

fn verus_bin() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("VERUS_BIN") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Some(path);
        }
    }
    let output = Command::new("which").arg("verus").output().ok()?;
    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path.is_empty() {
            return Some(PathBuf::from(path));
        }
    }
    None
}

fn run_verus(path: &Path) -> Option<(bool, String)> {
    let output = Command::new(verus_bin()?)
        .args(["-Z", "no-codegen"])
        .arg(path)
        .current_dir(std::env::temp_dir())
        .output()
        .ok()?;
    let mut combined = String::from_utf8_lossy(&output.stdout).to_string();
    combined.push_str(&String::from_utf8_lossy(&output.stderr));
    Some((output.status.success(), combined))
}

#[test]
fn emits_one_first_order_quantifier_per_relation_with_alternative_observer_triggers() {
    let emitted = lower(&program(RING_SOURCE)).expect("declared-index relation lowering");
    for (name, guard) in [
        ("__thermite_logical_eq_FixedRing64", ""),
        (
            "__thermite_logical_same_except_FixedRing64",
            " && i != except",
        ),
        (
            "__thermite_logical_same_except_two_FixedRing64",
            " && i != first && i != second",
        ),
    ] {
        assert!(
            emitted.contains(&format!("pub open spec fn {name}(left: &FixedRing64")),
            "the per-view relation `{name}` must be emitted:\n{emitted}"
        );
        assert!(
            emitted.contains(&format!(
                "        i < FIXED_RING_CAPACITY{guard} ==> fixed_ring_slot_spec(left, i) == fixed_ring_slot_spec(right, i)\n"
            )),
            "`{name}` must quantify over the declared bound with the observer applied to each operand:\n{emitted}"
        );
    }
    assert_eq!(
        emitted
            .matches("        #![trigger fixed_ring_slot_spec(left, i)]\n        #![trigger fixed_ring_slot_spec(right, i)]\n")
            .count(),
        3,
        "each of the three relations carries the observer applied to each operand as alternative triggers:\n{emitted}"
    );
    assert!(
        emitted.contains("    forall|i: usize|\n"),
        "the index space is quantified over `usize`, so no `as int` coercion enters a trigger term:\n{emitted}"
    );
    assert!(
        emitted.contains("impl __thermite_LogicalRelations for FixedRing64 {"),
        "the relation call site dispatches through the generated trait:\n{emitted}"
    );
    // Each relation applies the receiver's view to both operands, with the
    // `usize` exception indices after the second operand. The clause is checked
    // by the names it must mention rather than by its borrow punctuation, which
    // the design does not pin.
    for (method, operands) in [
        (
            "__thermite_logical_same_except_spec",
            vec!["result", "ring", "at"],
        ),
        (
            "__thermite_logical_same_except_two_spec",
            vec!["result", "ring", "first", "second"],
        ),
        ("__thermite_logical_eq_spec", vec!["result", "source"]),
    ] {
        let clause = emitted
            .lines()
            .find(|line| line.contains(method) && line.contains("result"))
            .unwrap_or_else(|| panic!("no `ens` clause calls `{method}`:\n{emitted}"));
        for operand in operands {
            assert!(
                clause.contains(operand),
                "`{method}` must name the operand `{operand}` in `{clause}`"
            );
        }
    }
}

#[test]
fn emits_nothing_for_a_program_that_declares_a_view_but_names_no_relation() {
    let declaration_only = r#"
const SLOTS: usize = 8;

#[logical(bound = "SLOTS", observe = "state_slot_spec")]
struct State {
  slots: [u64; SLOTS],
}

spec fn state_slot_spec(state: &State, slot: usize) -> u64
  dec slot
{
  state.slots[slot]
}

fn read(state: State, at: usize) -> u64
  req at < SLOTS
  ens result == state_slot_spec(&state, at)
  fx pure
{
  state.slots[at]
}
"#;
    let emitted = lower(&program(declaration_only)).expect("declaration-only lowering");
    assert!(
        !emitted.contains("__thermite_LogicalRelations"),
        "the relation family is emitted only for a program that names it:\n{emitted}"
    );
}

#[test]
fn index_transparent_frames_and_whole_state_equality_verify_with_real_verus() {
    let source = lower(&program(RING_SOURCE)).expect("declared-index relation lowering");
    let path = std::env::temp_dir().join("thermite_logical_index_relations.rs");
    std::fs::write(&path, &source).expect("write declared-index Verus unit");
    match run_verus(&path) {
        Some((success, output)) => assert!(
            success && output.contains("verified, 0 errors"),
            "the declared-index relations did not verify:\n{output}\n--- emitted ---\n{source}"
        ),
        None => eprintln!("SKIP: verus unavailable; emission assertions still ran"),
    }
}

#[test]
fn a_bound_shifted_by_one_fails_its_direct_verus_obligation() {
    // `.design/build/aggregate-array-relations.md` AC-4: "a mutant that … shifts
    // `bound` by one fails its direct obligation". Widening the frame's index
    // space past the storage capacity leaves the observer reading outside the
    // array, so the postcondition is no longer discharged by the body.
    let source = lower(&program(RING_SOURCE)).expect("declared-index relation lowering");
    let needle = "i < FIXED_RING_CAPACITY && i != except ==>";
    assert!(source.contains(needle), "frame guard missing:\n{source}");
    let mutant = source.replacen(needle, "i < FIXED_RING_CAPACITY + 1 && i != except ==>", 1);
    let path = std::env::temp_dir().join("thermite_logical_index_relations_bound_mutant.rs");
    std::fs::write(&path, &mutant).expect("write shifted-bound Verus mutant");
    match run_verus(&path) {
        Some((success, output)) => assert!(
            !success && !output.contains("verified, 0 errors"),
            "shifting the declared bound by one must break the frame obligation:\n\
             {output}\n--- mutant ---\n{mutant}"
        ),
        None => eprintln!("SKIP: verus unavailable; shifted-bound mutation still formed"),
    }
}

#[test]
fn reusing_the_first_exception_index_fails_its_direct_verus_obligation() {
    // AC-4: "a mutant that … reuses the first exception index … fails its direct
    // obligation" — the two-index relation must exclude both writes.
    let source = lower(&program(RING_SOURCE)).expect("declared-index relation lowering");
    let needle = "i < FIXED_RING_CAPACITY && i != first && i != second ==>";
    assert!(
        source.contains(needle),
        "two-index frame guard missing:\n{source}"
    );
    let mutant = source.replacen(
        needle,
        "i < FIXED_RING_CAPACITY && i != first && i != first ==>",
        1,
    );
    let path = std::env::temp_dir().join("thermite_logical_index_relations_exception_mutant.rs");
    std::fs::write(&path, &mutant).expect("write reused-exception Verus mutant");
    match run_verus(&path) {
        Some((success, output)) => assert!(
            !success && !output.contains("verified, 0 errors"),
            "dropping the second exception index must break the two-index obligation:\n\
             {output}\n--- mutant ---\n{mutant}"
        ),
        None => eprintln!("SKIP: verus unavailable; reused-exception mutation still formed"),
    }
}

#[test]
fn a_packed_observer_still_reaches_whole_state_equality() {
    // "whole-state equality closes by congruence for any observer" — the packed
    // 256-over-4 view is admitted for `logical_eq` while its frames wait on
    // REQ-AGGREL-5.
    let packed = r#"
const FIXED_BITMAP_BITS: usize = 256;
const FIXED_BITMAP_WORD_BITS: usize = 64;
const FIXED_BITMAP_WORDS: usize = 4;

#[logical(bound = "FIXED_BITMAP_BITS", observe = "fixed_bitmap_contains_spec")]
struct FixedBitmap256 {
  words: [u64; FIXED_BITMAP_WORDS],
  capacity: usize,
}

spec fn fixed_bitmap_word_spec(bit: usize) -> usize
  dec bit
{
  bit / FIXED_BITMAP_WORD_BITS
}

spec fn fixed_bitmap_offset_spec(bit: usize) -> usize
  dec bit
{
  bit % FIXED_BITMAP_WORD_BITS
}

spec fn fixed_bitmap_contains_spec(
  bitmap: &FixedBitmap256,
  bit: usize,
) -> bool
  dec bit
{
  bitmap.words[fixed_bitmap_word_spec(bit)]
    .bit_test(fixed_bitmap_offset_spec(bit))
}

fn fixed_bitmap_adopt(
  target: FixedBitmap256,
  source: &FixedBitmap256,
) -> FixedBitmap256
  req true
  ens result.logical_eq(source)
  fx pure
{
  FixedBitmap256 {
    words: source.words,
    capacity: target.capacity,
  }
}
"#;
    let source = lower(&program(packed)).expect("packed declared-index lowering");
    assert!(
        source.contains(
            "        i < FIXED_BITMAP_BITS ==> fixed_bitmap_contains_spec(left, i) == fixed_bitmap_contains_spec(right, i)\n"
        ),
        "the packed view quantifies over its 256 logical indices, not its 4 storage words:\n{source}"
    );
    let path = std::env::temp_dir().join("thermite_logical_index_relations_packed.rs");
    std::fs::write(&path, &source).expect("write packed declared-index Verus unit");
    match run_verus(&path) {
        Some((success, output)) => assert!(
            success && output.contains("verified, 0 errors"),
            "packed whole-state equality did not verify:\n{output}\n--- emitted ---\n{source}"
        ),
        None => eprintln!("SKIP: verus unavailable; packed emission assertions still ran"),
    }
}
