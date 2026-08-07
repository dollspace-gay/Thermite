//! Divergence pins for the two load-bearing residuals of REQ-KPRIM-2.
//!
//! `.design/build/fixed-collections.md`, "Remaining collection closure", names
//! the residual work verbatim. Items 4 and 5 are pinned here:
//!
//! > 4. quantified framing and equality for aggregate collection states;
//! > 5. quantified aggregate body TV and strict aggregate receipt/runtime
//! >    fixtures;
//!
//! Those two items are the stated gate on the collection package's public
//! surface. `.design/build/kernel-primitives.md`, "Fixed-collection package":
//!
//! > The canonical package builds and replays as a strict freestanding receipt
//! > rooted at the scalar ring-index transition, binding all five roots and
//! > rejecting receipt-source tampering. ... Full collection exports remain
//! > gated by quantified aggregate-state framing and dedicated aggregate
//! > receipt/runtime coverage.
//!
//! `.design/build/fixed-collections.md`, "Fixed bitset", states the missing
//! quantified form for the aggregate collection state directly:
//!
//! > Bulk union, intersection, and difference pin all four result words through
//! > exact fixed-array equality. Generic capacities and a quantified
//! > all-indices public contract remain future work.
//!
//! and "Assurance and adversarial evidence" ties it to the export gate:
//!
//! > Quantified all-index aggregate-state framing remains open, so these
//! > increments do not generalize the focused results into a claim that every
//! > collection lifecycle is already a strict public export.
//!
//! What ships today is one rung below that. `.design/build/aggregate-array-
//! relations.md`, "Validation", scopes the shipped relation family to a single
//! array with at most two excluded indices:
//!
//! > Every relation operand must still be a named array (or direct
//! > reference/deref of them) with exactly the same element type and capacity.
//!
//! So `.array_eq` / `.array_same_except` / `.array_same_except_two` quantify
//! over the 4-word `[u64; 4]` storage of `FixedBitmap256`. The design's
//! quantified aggregate-state form quantifies over the collection state's own
//! 256-bit logical index space, which no shipped relation reaches.
//!
//! The expected assurance level is the design constant, not a Forge reading.
//! `.design/build/kernel-primitives.md`, "Completion rule":
//!
//! > Every Thermite-authored language semantic, model, and reusable algorithm
//! > has an L3-or-L4 assurance floor. ... L2, L1, L0, an unrun proof, or a
//! > skipped translation-validation row is not a completed primitive.
//!
//! R-CHAR-3: every collection declaration used below is sliced verbatim out of
//! `stdlib/kernel-primitives/collections/bitmap.th` and checked against that
//! file before the fixture is assembled; the expected outcomes (`L3`, a
//! successful strict aggregate-rooted build, a replayed receipt) are design
//! constants. No expected value is copied from Forge output.
//!
//! Tracking: the crosslink hub refuses writes on this checkout
//! (`this hub uses the legacy v2 layout`), so both divergences carry their
//! authority inline rather than an issue number.

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

/// The verbatim body of the shipped `fixed_bitmap_union` transition.
const BITMAP_UNION_BODY: &str = "\
  FixedBitmap256 {
    words: [
      left.words[0] | right.words[0],
      left.words[1] | right.words[1],
      left.words[2] | right.words[2],
      left.words[3] | right.words[3],
    ],
    capacity: left.capacity,
  }";

/// The two quantified all-index contracts the design owes for an aggregate
/// collection state, expressed over the shipped membership observer rather
/// than over the 4-word storage array.
///
/// `fixed_bitmap_state_same_except_spec` is the quantified frame: every bit of
/// the collection state other than the one written keeps its membership.
/// `fixed_bitmap_union_all_bits_spec` is the quantified equality: membership in
/// the union agrees with the disjunction at every bit of the state.
const QUANTIFIED_AGGREGATE_STATE_CONTRACTS: &str = "\
spec fn fixed_bitmap_state_same_except_spec(
  result: &FixedBitmap256,
  before: &FixedBitmap256,
  changed: usize,
  count: usize,
) -> bool
  dec count
{
  if count == 0 {
    true
  } else {
    if count <= FIXED_BITMAP_BITS {
      (count - 1 == changed
        || fixed_bitmap_contains_spec(result, count - 1)
          == fixed_bitmap_contains_spec(before, count - 1))
        && fixed_bitmap_state_same_except_spec(result, before, changed, count - 1)
    } else {
      false
    }
  }
}

spec fn fixed_bitmap_union_all_bits_spec(
  result: &FixedBitmap256,
  left: &FixedBitmap256,
  right: &FixedBitmap256,
  count: usize,
) -> bool
  dec count
{
  if count == 0 {
    true
  } else {
    if count <= FIXED_BITMAP_BITS {
      (fixed_bitmap_contains_spec(result, count - 1)
        == (fixed_bitmap_contains_spec(left, count - 1)
          || fixed_bitmap_contains_spec(right, count - 1)))
        && fixed_bitmap_union_all_bits_spec(result, left, right, count - 1)
    } else {
      false
    }
  }
}";

fn shipped_bitmap_source() -> String {
    fs::read_to_string(root().join(SHIPPED_BITMAP)).unwrap()
}

/// Build the quantified aggregate-state fixture out of the shipped collection
/// declarations. Every borrowed block must still occur verbatim in the shipped
/// module, so the fixture cannot drift into a private dialect (R-CHAR-3).
fn quantified_aggregate_state_fixture() -> String {
    let shipped = shipped_bitmap_source();
    for block in [
        BITMAP_STATE_DECLS,
        BITMAP_INDEX_HELPERS,
        BITMAP_INSERT_BODY,
        BITMAP_UNION_BODY,
    ] {
        assert!(
            shipped.contains(block),
            "fixture block is no longer verbatim in `{SHIPPED_BITMAP}`:\n{block}"
        );
    }
    format!(
        "{BITMAP_STATE_DECLS}\n\n{BITMAP_INDEX_HELPERS}\n\n\
         {QUANTIFIED_AGGREGATE_STATE_CONTRACTS}\n\n\
         fn fixed_bitmap_insert_quantified_frame(\n\
         \x20 bitmap: FixedBitmap256,\n\
         \x20 bit: usize,\n\
         ) -> FixedBitmap256\n\
         \x20 req fixed_bitmap_wf_spec(&bitmap) && bit < FIXED_BITMAP_BITS\n\
         \x20 ens fixed_bitmap_wf_spec(&result)\n\
         \x20 ens fixed_bitmap_contains_spec(&result, bit)\n\
         \x20 ens fixed_bitmap_state_same_except_spec(\n\
         \x20   &result,\n\
         \x20   &bitmap,\n\
         \x20   bit,\n\
         \x20   FIXED_BITMAP_BITS,\n\
         \x20 )\n\
         \x20 fx pure\n\
         {{\n{BITMAP_INSERT_BODY}\n}}\n\n\
         fn fixed_bitmap_union_quantified_equality(\n\
         \x20 left: FixedBitmap256,\n\
         \x20 right: &FixedBitmap256,\n\
         ) -> FixedBitmap256\n\
         \x20 req fixed_bitmap_wf_spec(&left) && fixed_bitmap_wf_spec(right)\n\
         \x20 ens fixed_bitmap_wf_spec(&result)\n\
         \x20 ens fixed_bitmap_union_all_bits_spec(&result, &left, right, FIXED_BITMAP_BITS)\n\
         \x20 fx pure\n\
         {{\n{BITMAP_UNION_BODY}\n}}\n"
    )
}

/// Divergence 1 — REQ-KPRIM-2 / `.design/build/fixed-collections.md`
/// "Remaining collection closure" item 4: "quantified framing and equality for
/// aggregate collection states".
///
/// The shipped relation family quantifies over one storage array with at most
/// two excluded indices. The design owes a quantified contract over the
/// aggregate collection state's own index space: "a quantified all-indices
/// public contract" (`.design/build/fixed-collections.md`, "Fixed bitset").
///
/// Expected (design): every row of the fixture certifies at `L3`
/// (`.design/build/kernel-primitives.md`, "Completion rule").
///
/// Actual (today): the two quantified transitions certify at `L0` with
/// `postcondition not satisfied`. The word-level rows around them are `L3`, so
/// the gap is the lift from storage-array framing to collection-state framing,
/// not the fixture.
#[test]
#[ignore = "divergence: quantified aggregate collection-state framing/equality certifies at L0, not the design's L3 floor; .design/build/fixed-collections.md 'Remaining collection closure' item 4"]
fn quantified_aggregate_collection_state_framing_and_equality_reach_l3() {
    let temp = TempDir::new("quantified");
    let fixture = temp.0.join("quantified_aggregate_state.th");
    fs::write(&fixture, quantified_aggregate_state_fixture()).unwrap();
    let fixture_s = fixture.to_string_lossy().to_string();

    let checked = forge(&["check", &fixture_s, "--level", "l3", "--json"]);
    let rows: Vec<serde_json::Value> =
        serde_json::from_slice(&checked.stdout).unwrap_or_else(|_| {
            panic!(
                "forge check emitted no certificate array\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&checked.stdout),
                String::from_utf8_lossy(&checked.stderr)
            )
        });

    // The two quantified aggregate-state rows must exist and must be L3.
    for item in [
        "fixed_bitmap_insert_quantified_frame",
        "fixed_bitmap_union_quantified_equality",
    ] {
        let row = rows
            .iter()
            .find(|row| row["item"] == item)
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
    assert!(
        checked.status.success(),
        "forge check rejected the quantified aggregate collection-state fixture\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&checked.stdout),
        String::from_utf8_lossy(&checked.stderr)
    );
}

/// Divergence 2 — REQ-KPRIM-2 / `.design/build/fixed-collections.md`
/// "Remaining collection closure" item 5: "quantified aggregate body TV and
/// strict aggregate receipt/runtime fixtures".
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
/// Actual (today): Forge refuses at the `exports` stage before any proof runs.
#[test]
#[ignore = "divergence: aggregate-rooted collection exports are refused at the plan stage, so no strict aggregate receipt/runtime fixture exists; .design/build/fixed-collections.md 'Remaining collection closure' item 5"]
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
