//! Admission and relation-gate coverage for the declared logical index space
//! (`.design/build/aggregate-array-relations.md`, "Admitted shapes and
//! fail-closed boundary" and "Surface").
//!
//! Typing selects the relation family, so a `[T; N]` receiver keeps the storage
//! relations and a `#[logical]` struct receiver takes the declared-index
//! relations. Everything else is refused before lowering, with the diagnostic
//! naming the rule.

use thermite_spec::{logical_views, validate, SpecError};
use thermite_syntax::parse;

/// A ring-shaped declaration whose observer reads one slot per logical index.
/// The shape is the index-transparent case the design names: "`FixedRing64`,
/// `FixedVec64`, `FixedSlab64`, … store one element per logical index, so their
/// slot observers are index-transparent."
const RING_DECLARATION: &str = r#"
const FIXED_RING_CAPACITY: usize = 64;

#[logical(bound = "FIXED_RING_CAPACITY", observe = "fixed_ring_slot_spec")]
struct FixedRing64 {
  slots: [u64; FIXED_RING_CAPACITY],
  head: usize,
}

spec fn fixed_ring_slot_spec(ring: &FixedRing64, slot: usize) -> u64
  dec slot
{
  ring.slots[slot]
}
"#;

/// A packed declaration whose observer applies `bit / 64` and `bit % 64` to its
/// index: "256 logical indices share four storage words".
const BITMAP_DECLARATION: &str = r#"
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
"#;

fn validate_src(src: &str) -> Result<(), Vec<SpecError>> {
    let parsed = parse(src);
    assert!(
        parsed.is_clean(),
        "fixture parse errors: {:?}",
        parsed.errors
    );
    validate(&parsed.program)
}

fn views(src: &str) -> std::collections::BTreeMap<String, thermite_spec::LogicalView> {
    let parsed = parse(src);
    assert!(
        parsed.is_clean(),
        "fixture parse errors: {:?}",
        parsed.errors
    );
    logical_views(&parsed.program)
}

fn rejection_detail(errors: &[SpecError]) -> String {
    errors
        .iter()
        .map(std::string::ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

fn assert_refused(src: &str, needle: &str) {
    let errors = validate_src(src).expect_err("the declaration or relation must be refused");
    let rendered = rejection_detail(&errors);
    assert!(
        rendered.contains(needle),
        "expected a diagnostic naming `{needle}`, got:\n{rendered}"
    );
}

#[test]
fn resolves_the_declared_bound_and_observer() {
    let resolved = views(RING_DECLARATION);
    let view = resolved
        .get("FixedRing64")
        .expect("the ring declaration must be admitted");
    assert_eq!(view.bound, "FIXED_RING_CAPACITY");
    assert_eq!(view.bound_value, 64);
    assert_eq!(view.observer, "fixed_ring_slot_spec");
    assert!(view.index_transparent);
}

#[test]
fn classifies_a_packed_observer_as_derived_index() {
    let resolved = views(BITMAP_DECLARATION);
    let view = resolved
        .get("FixedBitmap256")
        .expect("the packed declaration is admitted; only its frames are refused");
    // The declared space is 256 while the storage array holds 4 words: "the two
    // numbers are unrelated by construction".
    assert_eq!(view.bound_value, 256);
    assert!(!view.index_transparent);
}

#[test]
fn admits_the_three_relations_over_an_index_transparent_view() {
    let source = format!(
        "{RING_DECLARATION}
fn write_one(ring: FixedRing64, at: usize, value: u64) -> FixedRing64
  req at < FIXED_RING_CAPACITY
  ens result.logical_same_except(&ring, at)
  fx pure
{{
  ring
}}

fn write_two(ring: FixedRing64, first: usize, second: usize) -> FixedRing64
  req first < FIXED_RING_CAPACITY && second < FIXED_RING_CAPACITY
  ens result.logical_same_except_two(&ring, first, second)
  fx pure
{{
  ring
}}

fn adopt(target: FixedRing64, source: &FixedRing64) -> FixedRing64
  req true
  ens result.logical_eq(source)
  fx pure
{{
  target
}}
"
    );
    assert!(validate_src(&source).is_ok());
}

#[test]
fn refuses_a_frame_relation_over_a_derived_index_observer() {
    let source = format!(
        "{BITMAP_DECLARATION}
fn insert(bitmap: FixedBitmap256, bit: usize) -> FixedBitmap256
  req bit < FIXED_BITMAP_BITS
  ens result.logical_same_except(&bitmap, bit)
  fx pure
{{
  bitmap
}}
"
    );
    assert_refused(&source, "derived-index");
}

#[test]
fn admits_whole_state_equality_over_a_derived_index_observer() {
    // "whole-state equality closes by congruence for any observer, while a frame
    // has to relate two different index spaces."
    let source = format!(
        "{BITMAP_DECLARATION}
fn adopt(target: FixedBitmap256, source: &FixedBitmap256) -> FixedBitmap256
  req true
  ens result.logical_eq(source)
  fx pure
{{
  target
}}
"
    );
    assert!(validate_src(&source).is_ok());
}

#[test]
fn refuses_a_receiver_with_no_declared_index_space() {
    let source = r#"
struct Plain { value: u64 }

fn compare(left: Plain, right: &Plain) -> bool
  req true
  ens result == left.logical_eq(right)
  fx pure
{
  true
}
"#;
    assert_refused(source, "declares no admitted");
}

#[test]
fn refuses_a_fixed_array_receiver_and_keeps_the_storage_family_disjoint() {
    let source = r#"
const SLOTS: usize = 4;

fn compare(left: [u64; SLOTS], right: [u64; SLOTS]) -> bool
  req true
  ens result == left.logical_eq(right)
  fx pure
{
  left.array_eq(right)
}
"#;
    assert_refused(source, "must be a bare name");
}

#[test]
fn refuses_mismatched_nominal_operands() {
    let source = format!(
        "{RING_DECLARATION}
struct Other {{ value: u64 }}

fn compare(ring: FixedRing64, other: &Other) -> bool
  req true
  ens result == ring.logical_eq(other)
  fx pure
{{
  true
}}
"
    );
    assert_refused(&source, "different types");
}

#[test]
fn refuses_a_computed_operand_and_a_wrong_arity() {
    let computed = format!(
        "{RING_DECLARATION}
spec fn rotate_spec(ring: &FixedRing64) -> FixedRing64
  dec ring.head
{{
  FixedRing64 {{ slots: ring.slots, head: 0 }}
}}

fn compare(ring: FixedRing64) -> bool
  req true
  ens result == ring.logical_eq(rotate_spec(&ring))
  fx pure
{{
  true
}}
"
    );
    assert_refused(&computed, "must be a bare name");

    let arity = format!(
        "{RING_DECLARATION}
fn compare(ring: FixedRing64, other: &FixedRing64, at: usize) -> bool
  req true
  ens result == ring.logical_eq(other, at)
  fx pure
{{
  true
}}
"
    );
    assert_refused(&arity, "expects 1 argument(s), found 2");
}

#[test]
fn refuses_the_family_in_executable_position() {
    let source = format!(
        "{RING_DECLARATION}
fn compare(ring: FixedRing64, other: &FixedRing64) -> bool
  req true
  ens true
  fx pure
{{
  ring.logical_eq(other)
}}
"
    );
    assert_refused(&source, "specification relation");
}

#[test]
fn refuses_a_sealed_receiver() {
    let source = r#"
#[logical(bound = "8", observe = "token_slot_spec")]
#[sealed] struct Token {
  slots: [u64; 8],
}

spec fn token_slot_spec(token: &Token, slot: usize) -> u64
  dec slot
{
  token.slots[slot]
}
"#;
    assert_refused(source, "`#[sealed]`");
}

#[test]
fn refuses_an_unresolvable_or_oversized_bound() {
    let unknown = r#"
#[logical(bound = "MISSING_CAPACITY", observe = "state_slot_spec")]
struct State {
  slots: [u64; 8],
}

spec fn state_slot_spec(state: &State, slot: usize) -> u64
  dec slot
{
  state.slots[slot]
}
"#;
    assert_refused(unknown, "neither a non-negative integer literal");

    let oversized = r#"
#[logical(bound = "1048577", observe = "state_slot_spec")]
struct State {
  slots: [u64; 8],
}

spec fn state_slot_spec(state: &State, slot: usize) -> u64
  dec slot
{
  state.slots[slot]
}
"#;
    assert_refused(oversized, "above the 1048576-element bound");
}

#[test]
fn refuses_an_observer_with_the_wrong_shape() {
    let missing = r#"
#[logical(bound = "8", observe = "absent_spec")]
struct State {
  slots: [u64; 8],
}
"#;
    assert_refused(missing, "names no `spec fn`");

    let wrong_arity = r#"
#[logical(bound = "8", observe = "state_slot_spec")]
struct State {
  slots: [u64; 8],
}

spec fn state_slot_spec(state: &State) -> u64
  dec state.slots[0]
{
  state.slots[0]
}
"#;
    assert_refused(wrong_arity, "must take exactly `(&State, usize)`");

    let wrong_result = r#"
#[logical(bound = "8", observe = "state_slot_spec")]
struct State {
  slots: [u64; 8],
}

spec fn state_slot_spec(state: &State, slot: usize) -> String
  dec slot
{
  "slot"
}
"#;
    assert_refused(wrong_result, "outside the finite structural closure");
}

#[test]
fn a_forwarded_index_inherits_transparency_from_its_callee() {
    // "An observer that passes its index to another `spec fn` is
    // index-transparent when that callee is, decided as a monotone fixed point
    // over the acyclic declaration closure."
    let source = r#"
const SLOTS: usize = 8;

#[logical(bound = "SLOTS", observe = "state_view_spec")]
struct State {
  slots: [u64; SLOTS],
  guards: [bool; SLOTS],
}

spec fn state_slot_spec(state: &State, slot: usize) -> u64
  dec slot
{
  state.slots[slot]
}

spec fn state_view_spec(state: &State, slot: usize) -> (u64, bool)
  dec slot
{
  (state_slot_spec(state, slot), state.guards[slot])
}
"#;
    let resolved = views(source);
    let view = resolved.get("State").expect("the declaration is admitted");
    assert!(
        view.index_transparent,
        "an observer forwarding its index to a transparent callee stays transparent"
    );
}

#[test]
fn a_rotated_view_is_derived_index() {
    // "A rotated view such as a ring's FIFO position, which would read
    // `ring.slots[(ring.head + pos) % 64]`, is derived-index for the same reason."
    let source = r#"
const SLOTS: usize = 64;

#[logical(bound = "SLOTS", observe = "fifo_slot_spec")]
struct Rotated {
  slots: [u64; SLOTS],
  head: usize,
}

spec fn fifo_slot_spec(ring: &Rotated, pos: usize) -> u64
  dec pos
{
  ring.slots[(ring.head + pos) % 64]
}
"#;
    let resolved = views(source);
    let view = resolved
        .get("Rotated")
        .expect("the declaration is admitted");
    assert!(!view.index_transparent);
}
