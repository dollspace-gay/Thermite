//! Quantified aggregate collection-state framing and equality, and the two
//! residuals of REQ-KPRIM-2 that remain open behind it.
//!
//! `.design/build/fixed-collections.md`, "Remaining collection closure", names
//! the residual work verbatim. Items 4 and 5 are the subject of this file:
//!
//! > 4. quantified framing and equality for aggregate collection states, whose
//! >    surface, iff semantics, fail-closed boundary, and lowering obligation are
//! >    fixed in `.design/build/aggregate-array-relations.md` (REQ-AGGREL-2 through
//! >    REQ-AGGREL-5); the slot views of the ring, vector, slab, freelist,
//! >    intrusive metadata, and both maps are index-transparent and land first;
//! > 5. quantified aggregate body TV and strict aggregate receipt/runtime fixtures;
//!
//! The shipped storage relations quantify over one fixed array with at most two
//! excluded indices. `.design/build/aggregate-array-relations.md`, "Validation":
//!
//! > Every relation operand must still be a named array (or direct
//! > reference/deref of them) with exactly the same element type and capacity.
//!
//! The declared-index family quantifies over the collection's own index space
//! instead. `.design/build/aggregate-array-relations.md`, "Meaning":
//!
//! > `left.logical_same_except(right, except)` is true exactly when, for every
//! > `i: usize` with `i < C` and `i != except`, `obs(&left, i) == obs(&right, i)`.
//!
//! Its dischargeability splits three ways, and this file is organized on that
//! split. `.design/build/aggregate-array-relations.md`, "What makes the form
//! dischargeable":
//!
//! > The body is congruence. For a skolem `i`, both sides unfold to the observer
//! > applied to arguments that are equal by the premise, so the two terms are
//! > equal whatever the observer does with `i`. No arithmetic, no bit-vector
//! > reasoning, and no index-transparency requirement. … which is why
//! > `logical_eq` is admitted for every declared view.
//!
//! and, for a frame:
//!
//! > Index-transparency is what discharges that body. Every field the observer
//! > reads is indexed by `i` directly, so each storage relation instantiated at
//! > `j = i` gives the field equality, and the observer's two applications then
//! > differ only in arguments that are equal.
//!
//! The expected assurance level is the design constant, not a Forge reading.
//! `.design/build/kernel-primitives.md`, "Completion rule":
//!
//! > Every Thermite-authored language semantic, model, and reusable algorithm
//! > has an L3-or-L4 assurance floor. … L2, L1, L0, an unrun proof, or a
//! > skipped translation-validation row is not a completed primitive.
//!
//! R-CHAR-3: every collection declaration used below is sliced verbatim out of
//! `stdlib/kernel-primitives/collections/bitmap.th` or
//! `stdlib/kernel-primitives/collections/ring.th` and checked against those
//! files before the fixture is assembled; the `#[logical(bound = …, observe =
//! …)]` declaration line comes from `.design/build/aggregate-array-relations.md`,
//! "Declaring a logical view"; and the expected outcomes (`L3`, a successful
//! strict aggregate-rooted build, a replayed receipt) are design constants. No
//! expected value is copied from Forge output.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn forge(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_forge"))
        .args(args)
        .current_dir(root())
        .output()
        .unwrap()
}

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "thermite_divergence_aggregate_state_{name}_{}_{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// The shipped collection module that owns the packed 256-bit state.
const SHIPPED_BITMAP: &str = "stdlib/kernel-primitives/collections/bitmap.th";

/// The shipped collection module that owns the 64-slot FIFO ring.
const SHIPPED_RING: &str = "stdlib/kernel-primitives/collections/ring.th";

/// Verbatim capacity, state, and membership declarations from
/// `stdlib/kernel-primitives/collections/bitmap.th`.
const BITMAP_STATE_DECLS: &str = "\
const FIXED_BITMAP_BITS: usize = 256;
const FIXED_BITMAP_WORD_BITS: usize = 64;
const FIXED_BITMAP_WORDS: usize = 4;

struct FixedBitmap256 {
  words: [u64; FIXED_BITMAP_WORDS],
  capacity: usize,
}

spec fn fixed_bitmap_wf_spec(bitmap: &FixedBitmap256) -> bool
  dec bitmap.capacity
{
  bitmap.capacity == FIXED_BITMAP_BITS
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
}";

/// Verbatim executable word/offset selectors from the same shipped module.
const BITMAP_INDEX_HELPERS: &str = "\
fn fixed_bitmap_word(bit: usize) -> usize
  req bit < FIXED_BITMAP_BITS
  ens result == fixed_bitmap_word_spec(bit)
  ens result < FIXED_BITMAP_WORDS
  fx pure
{
  bit / FIXED_BITMAP_WORD_BITS
}

fn fixed_bitmap_offset(bit: usize) -> usize
  req bit < FIXED_BITMAP_BITS
  ens result == fixed_bitmap_offset_spec(bit)
  ens result < FIXED_BITMAP_WORD_BITS
  fx pure
{
  bit % FIXED_BITMAP_WORD_BITS
}";

/// The verbatim body of the shipped `fixed_bitmap_insert` transition.
const BITMAP_INSERT_BODY: &str = "\
  let word: usize = fixed_bitmap_word(bit);
  let offset: usize = fixed_bitmap_offset(bit);
  let mut words: [u64; FIXED_BITMAP_WORDS] = bitmap.words;
  words[word] = words[word].bit_set(offset);
  FixedBitmap256 {
    words: words,
    capacity: bitmap.capacity,
  }";

/// Verbatim capacity and state declarations from
/// `stdlib/kernel-primitives/collections/ring.th`.
const RING_STATE_DECLS: &str = "\
const FIXED_RING_CAPACITY: usize = 64;

struct FixedRing64 {
  slots: [u64; FIXED_RING_CAPACITY],
  head: usize,
  len: usize,
}";

/// The verbatim well-formedness predicate of the shipped ring.
const RING_WF_SPEC: &str = "\
spec fn fixed_ring_wf_spec(ring: &FixedRing64) -> bool
  dec ring.len
{
  ring.head < FIXED_RING_CAPACITY && ring.len <= FIXED_RING_CAPACITY
}";

/// The verbatim storage-copy step of the shipped `fixed_ring_push` transition.
const RING_SLOT_COPY: &str = "    let mut slots: [u64; FIXED_RING_CAPACITY] = ring.slots;";

/// The declaration line `.design/build/aggregate-array-relations.md`,
/// "Declaring a logical view", fixes: `bound` names the size of the index space
/// and `observe` names the `spec fn` that reads one index. The bitmap's 256
/// logical indices share four storage words, so its bound is
/// `FIXED_BITMAP_BITS`, not `FIXED_BITMAP_WORDS` — "the two numbers are
/// unrelated by construction".
const BITMAP_LOGICAL_DECL: &str =
    "#[logical(bound = \"FIXED_BITMAP_BITS\", observe = \"fixed_bitmap_contains_spec\")]";

/// The same declaration for the ring's index-transparent slot view. The design
/// lists `FixedRing64` among the views that "store one element per logical
/// index, so their slot observers are index-transparent".
const RING_LOGICAL_DECL: &str =
    "#[logical(bound = \"FIXED_RING_CAPACITY\", observe = \"fixed_ring_slot_spec\")]";

/// The ring's one-index observer, written to the shape the design fixes:
/// `spec fn obs(value: &L, index: usize) -> V`.
const RING_SLOT_OBSERVER: &str = "\
spec fn fixed_ring_slot_spec(ring: &FixedRing64, slot: usize) -> u64
  dec slot
{
  ring.slots[slot]
}";

fn shipped(module: &str) -> String {
    fs::read_to_string(root().join(module)).unwrap()
}

/// Every borrowed block must still occur verbatim in its shipped module, so a
/// fixture cannot drift into a private dialect (R-CHAR-3).
fn assert_verbatim(module: &str, blocks: &[&str]) {
    let source = shipped(module);
    for block in blocks {
        assert!(
            source.contains(block),
            "fixture block is no longer verbatim in `{module}`:\n{block}"
        );
    }
}

/// Declare the packed 256-bit view over the verbatim shipped state. The
/// attribute line is inserted immediately above the shipped `struct`, leaving
/// every borrowed declaration byte-identical to the module.
fn bitmap_declarations_with_logical_view() -> String {
    assert_verbatim(SHIPPED_BITMAP, &[BITMAP_STATE_DECLS, BITMAP_INDEX_HELPERS]);
    BITMAP_STATE_DECLS.replace(
        "struct FixedBitmap256 {",
        &format!("{BITMAP_LOGICAL_DECL}\nstruct FixedBitmap256 {{"),
    )
}

/// Declare the index-transparent 64-slot view over the verbatim shipped ring
/// state.
fn ring_declarations_with_logical_view() -> String {
    assert_verbatim(
        SHIPPED_RING,
        &[RING_STATE_DECLS, RING_WF_SPEC, RING_SLOT_COPY],
    );
    format!(
        "{}\n\n{RING_WF_SPEC}\n\n{RING_SLOT_OBSERVER}\n",
        RING_STATE_DECLS.replace(
            "struct FixedRing64 {",
            &format!("{RING_LOGICAL_DECL}\nstruct FixedRing64 {{"),
        )
    )
}

/// Run `forge check --level l3 --json` over a fixture and return its output.
fn check_fixture(name: &str, fixture_name: &str, source: &str) -> Output {
    let temp = TempDir::new(name);
    let fixture = temp.0.join(fixture_name);
    fs::write(&fixture, source).unwrap();
    let fixture_s = fixture.to_string_lossy().to_string();
    forge(&["check", &fixture_s, "--level", "l3", "--json"])
}

/// Require every certificate row of a checked fixture to reach the design's L3
/// assurance floor, naming the rows the acceptance criterion calls out.
fn assert_rows_reach_l3(checked: &Output, items: &[&str]) {
    let rows: Vec<serde_json::Value> = match serde_json::from_slice(&checked.stdout) {
        Ok(rows) => rows,
        Err(_) => panic!(
            "the fixture produced no certificate at all, so the required rows {items:?} \
             cannot reach L3\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&checked.stdout),
            String::from_utf8_lossy(&checked.stderr)
        ),
    };
    for item in items {
        let row = rows
            .iter()
            .find(|row| row["item"] == *item)
            .unwrap_or_else(|| panic!("missing certificate row for `{item}`"));
        assert_eq!(
            row["level"], "L3",
            "`{item}` must reach the design's L3 assurance floor for a quantified \
             aggregate collection-state contract; certificate was: {row}"
        );
    }
    assert!(
        rows.iter().all(|row| row["level"] == "L3"),
        "a quantified aggregate collection-state row fell below L3: {rows:?}"
    );
}

/// REQ-KPRIM-2 / `.design/build/fixed-collections.md` "Remaining collection
/// closure" item 4, the whole-state half:
/// `.design/build/aggregate-array-relations.md`, "What makes the form
/// dischargeable", admits `logical_eq` "for every declared view" because its
/// bridge "is congruence … whatever the observer does with `i`".
///
/// The packed bitmap is the hardest case for that claim: its observer reads
/// `bitmap.words[bit / 64].bit_test(bit % 64)`, so the relation quantifies over
/// 256 logical indices while the storage array holds four words. A transition
/// that adopts another bitmap's state must therefore export the complete
/// 256-index membership agreement in the collection's own vocabulary, without
/// naming `words`.
///
/// Expected (design): every row of the fixture certifies at `L3`
/// (`.design/build/kernel-primitives.md`, "Completion rule").
#[test]
fn packed_collection_state_equality_reaches_l3() {
    let source = format!(
        "{}\n\n{BITMAP_INDEX_HELPERS}\n\n\
         fn fixed_bitmap_adopt_state(\n\
         \x20 target: FixedBitmap256,\n\
         \x20 source: &FixedBitmap256,\n\
         ) -> FixedBitmap256\n\
         \x20 req fixed_bitmap_wf_spec(&target) && fixed_bitmap_wf_spec(source)\n\
         \x20 ens fixed_bitmap_wf_spec(&result)\n\
         \x20 ens result.logical_eq(source)\n\
         \x20 fx pure\n\
         {{\n\
         \x20 FixedBitmap256 {{\n\
         \x20   words: source.words,\n\
         \x20   capacity: target.capacity,\n\
         \x20 }}\n\
         }}\n",
        bitmap_declarations_with_logical_view()
    );
    let checked = check_fixture("packed_equality", "packed_state_equality.th", &source);
    assert_rows_reach_l3(&checked, &["fixed_bitmap_adopt_state"]);
}

/// REQ-KPRIM-2 / `.design/build/fixed-collections.md` "Remaining collection
/// closure" item 4, the frame half. The design fixes which collections land
/// first: "the slot views of the ring, vector, slab, freelist, intrusive
/// metadata, and both maps are index-transparent and land first".
///
/// `FixedRing64` stores one `u64` per logical index, so its slot observer reads
/// storage at the index the logical space uses and both frame relations close
/// "by congruence plus one instantiation per read field, with no arithmetic".
///
/// Expected (design): every row certifies at `L3`, including the one-index and
/// two-index frames and whole-state equality over the same declared view.
#[test]
fn index_transparent_collection_state_frames_reach_l3() {
    let source = format!(
        "{}\n\
         fn fixed_ring_write_slot(\n\
         \x20 ring: FixedRing64,\n\
         \x20 slot: usize,\n\
         \x20 value: u64,\n\
         ) -> FixedRing64\n\
         \x20 req fixed_ring_wf_spec(&ring) && slot < FIXED_RING_CAPACITY\n\
         \x20 ens fixed_ring_wf_spec(&result)\n\
         \x20 ens fixed_ring_slot_spec(&result, slot) == value\n\
         \x20 ens result.logical_same_except(&ring, slot)\n\
         \x20 fx pure\n\
         {{\n{RING_SLOT_COPY}\n\
         \x20 slots[slot] = value;\n\
         \x20 FixedRing64 {{\n\
         \x20   slots: slots,\n\
         \x20   head: ring.head,\n\
         \x20   len: ring.len,\n\
         \x20 }}\n\
         }}\n\n\
         fn fixed_ring_write_two_slots(\n\
         \x20 ring: FixedRing64,\n\
         \x20 first: usize,\n\
         \x20 second: usize,\n\
         \x20 value: u64,\n\
         ) -> FixedRing64\n\
         \x20 req fixed_ring_wf_spec(&ring)\n\
         \x20   && first < FIXED_RING_CAPACITY\n\
         \x20   && second < FIXED_RING_CAPACITY\n\
         \x20   && first != second\n\
         \x20 ens fixed_ring_wf_spec(&result)\n\
         \x20 ens fixed_ring_slot_spec(&result, first) == value\n\
         \x20 ens fixed_ring_slot_spec(&result, second) == value\n\
         \x20 ens result.logical_same_except_two(&ring, first, second)\n\
         \x20 fx pure\n\
         {{\n{RING_SLOT_COPY}\n\
         \x20 slots[first] = value;\n\
         \x20 slots[second] = value;\n\
         \x20 FixedRing64 {{\n\
         \x20   slots: slots,\n\
         \x20   head: ring.head,\n\
         \x20   len: ring.len,\n\
         \x20 }}\n\
         }}\n\n\
         fn fixed_ring_adopt_state(\n\
         \x20 target: FixedRing64,\n\
         \x20 source: &FixedRing64,\n\
         ) -> FixedRing64\n\
         \x20 req fixed_ring_wf_spec(&target) && fixed_ring_wf_spec(source)\n\
         \x20 ens fixed_ring_wf_spec(&result)\n\
         \x20 ens result.logical_eq(source)\n\
         \x20 fx pure\n\
         {{\n\
         \x20 FixedRing64 {{\n\
         \x20   slots: source.slots,\n\
         \x20   head: source.head,\n\
         \x20   len: source.len,\n\
         \x20 }}\n\
         }}\n",
        ring_declarations_with_logical_view()
    );
    let checked = check_fixture("transparent_frames", "index_transparent_frames.th", &source);
    assert_rows_reach_l3(
        &checked,
        &[
            "fixed_ring_write_slot",
            "fixed_ring_write_two_slots",
            "fixed_ring_adopt_state",
        ],
    );
}

/// Divergence — `.design/build/aggregate-array-relations.md` AC-5, tracked as
/// GitHub issue #132 (REQ-AGGREL-5, "Derived-index logical frames").
///
/// The design refuses the two frame relations over a derived-index observer
/// until that requirement lands, and states exactly what is missing
/// (`.design/build/aggregate-array-relations.md`, "The packed frame and what it
/// needs"):
///
/// > `fixed_bitmap_contains_spec` reads
/// > `bitmap.words[bit / 64].bit_test(bit % 64)`. Its 256 logical indices share
/// > four storage words, so the frame bridge above does not apply and the two
/// > frame relations are refused on that receiver. Establishing
/// > `result.logical_same_except(&bitmap, bit)` from the postconditions
/// > `fixed_bitmap_insert` already proves needs three facts the toolchain does
/// > not supply:
/// >
/// > 1. `i < 256 ==> i / 64 < 4`, to instantiate the storage frame at the
/// >    derived word.
/// > 2. `i / 64 == bit / 64 && i != bit ==> i % 64 != bit % 64`, to reach the
/// >    case where the observed bit shares the written word.
/// > 3. A proof-position form of the bit-preservation witness.
///
/// AC-5 is the acceptance criterion this pins: "`FixedBitmap256`'s
/// `fixed_bitmap_insert`, `fixed_bitmap_remove`, and `fixed_bitmap_set_to`
/// export `logical_same_except` against the requested bit at strict L3".
///
/// Expected (design): the insert transition's row certifies at `L3`.
///
/// Actual (today): the relation is refused before lowering, because no rung can
/// discharge it — `pub fn u64_bit_defs in thermite-lower/src/lower.rs` emits the
/// bit-preservation witnesses as executable functions, and a generated bridge
/// proof cannot call an executable function. A named red pin, not a hidden one
/// (goal.md R-DEFER-3): it goes green when issue #132 lands.
#[test]
fn packed_collection_state_frame_reaches_l3() {
    let source = format!(
        "{}\n\n{BITMAP_INDEX_HELPERS}\n\n\
         fn fixed_bitmap_insert_logical_frame(\n\
         \x20 bitmap: FixedBitmap256,\n\
         \x20 bit: usize,\n\
         ) -> FixedBitmap256\n\
         \x20 req fixed_bitmap_wf_spec(&bitmap) && bit < FIXED_BITMAP_BITS\n\
         \x20 ens fixed_bitmap_wf_spec(&result)\n\
         \x20 ens fixed_bitmap_contains_spec(&result, bit)\n\
         \x20 ens result.logical_same_except(&bitmap, bit)\n\
         \x20 fx pure\n\
         {{\n{BITMAP_INSERT_BODY}\n}}\n",
        bitmap_declarations_with_logical_view()
    );
    assert_verbatim(SHIPPED_BITMAP, &[BITMAP_INSERT_BODY]);
    let checked = check_fixture("packed_frame", "packed_state_frame.th", &source);
    assert_rows_reach_l3(&checked, &["fixed_bitmap_insert_logical_frame"]);
}

/// Divergence — REQ-KPRIM-2 / `.design/build/fixed-collections.md` "Remaining
/// collection closure" item 5: "quantified aggregate body TV and strict
/// aggregate receipt/runtime fixtures".
///
/// `.design/build/kernel-primitives.md`, "Fixed-collection package", records
/// that the canonical package receipt is "rooted at the scalar ring-index
/// transition" and that "Full collection exports remain gated by quantified
/// aggregate-state framing and dedicated aggregate receipt/runtime coverage".
///
/// `fixed_ring_push` is the canonical aggregate collection transition of that
/// package (`.design/build/fixed-collections.md`, "Fixed FIFO ring": "push
/// either returns `Pushed64 { ring }` or `RingFull64 { ring, value }`"). The
/// design requires it to build and replay as a strict freestanding L3 receipt
/// exactly as the scalar `fixed_ring_advance` root already does.
///
/// Expected (design): the aggregate-rooted strict build succeeds, the receipt
/// replays, the plan names the aggregate root, and every translation-validation
/// row is `faithful`.
///
/// Actual (today): `plan_exports` refuses at the `exports` stage before any
/// proof runs, on the second of the two gates
/// `.design/build/kernel-primitives.md`, "Why an enum-returning collection
/// transition does not export", names. The first gate is closed: `fn
/// supported_public_return_type in forge/src/verified_build.rs` now admits the
/// closed result enum `FixedRingPush64` under REQ-L3BUILD-15, whose payloads are
/// the finite plain record `FixedRing64` and a `u64`. The second gate holds: `fn
/// executable_precondition in forge/src/verified_build.rs` refuses `Expr::Call`,
/// and `fixed_ring_push` states `req fixed_ring_wf_spec(&ring)`, so the build
/// stops with "has a non-executable precondition and cannot receive a total
/// wrapper". REQ-L3BUILD-16 in `.design/build/l3-verified-artifact.md` governs
/// that gate: the executable form of a `spec fn` guard must be derived, emitted
/// from `fn lower_l3_export_wrapper in thermite-lower/src/lower.rs`, and
/// reproduced independently in `fn exec_tv_export_guard in
/// forge/src/exec_tv.rs`, because widening the predicate alone yields `error:
/// cannot call function fixed_ring_wf_spec with mode spec`. A named red pin for
/// REQ-KPRIM-2 item 5 (goal.md R-DEFER-3), outside the declared-index relation
/// family this file's first three rows cover.
#[test]
fn aggregate_rooted_collection_export_is_a_strict_l3_receipt() {
    let temp = TempDir::new("receipt");
    let bundle = temp.0.join("collections_aggregate.verified");
    let bundle_s = bundle.to_string_lossy().to_string();

    let built = forge(&[
        "build",
        "stdlib/kernel-primitives/collections.thpkg.json",
        "--level",
        "l3",
        "--export",
        "fixed_ring_push",
        "--target",
        "kernel",
        "--out",
        &bundle_s,
        "--json",
    ]);
    assert!(
        built.status.success(),
        "the aggregate ring transition must build as a strict freestanding L3 export\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&built.stdout),
        String::from_utf8_lossy(&built.stderr)
    );

    let replayed = forge(&["verify-build", &bundle_s, "--replay", "--json"]);
    assert!(
        replayed.status.success(),
        "the aggregate-rooted collection receipt must replay\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&replayed.stdout),
        String::from_utf8_lossy(&replayed.stderr)
    );

    let plan: serde_json::Value =
        serde_json::from_slice(&fs::read(bundle.join("evidence/artifact-plan.v1")).unwrap())
            .unwrap();
    assert_eq!(plan["package"]["name"], "thermite_fixed_collections");
    assert_eq!(plan["exports"][0]["thermite_name"], "fixed_ring_push");

    let tv: serde_json::Value = serde_json::from_slice(
        &fs::read(bundle.join("evidence/translation-validation.json")).unwrap(),
    )
    .unwrap();
    assert!(
        tv["rows"]
            .as_array()
            .unwrap()
            .iter()
            .all(|row| row["verdict"] == "faithful"),
        "aggregate collection-state body TV is not faithful: {tv}"
    );
}
