//! #16 boundary-fn parse oracle (`.design/boundary/ffi-boundary.md` REQ-1/REQ-3,
//! AC-1). The boundary form is `#[boundary("crate::path")] fn NAME(..) -> ret req
//! .. ens .. fx .. ;` — a bodyless `fn` carrying a foreign-target attribute. These
//! tests assert the EXACT AST shape the design pins (`FnItem { boundary: Some(_),
//! body: None, .. }`) and the OQ-2 gate (a bodyless fn WITHOUT `#[boundary]` is a
//! PARSE ERROR, never silently a boundary fn). Expected shapes are hand-derived
//! from the design (R-CHAR-3), never copied from the parser's output.

use thermite_syntax::{parse, FnItem, Item};

/// The design's exact example program (ffi-boundary.md AC-1 / conformance cases).
const FOREIGN_ID: &str =
    "#[boundary(\"ext::foreign_id\")] fn foreign_id(x: u32) -> u32 req x < 100 ens result == x fx pure ;";

fn first_fn(src: &str) -> (thermite_syntax::ParseResult, Option<FnItem>) {
    let r = parse(src);
    let f = r.program.items.first().and_then(|i| match i {
        Item::Fn(f) => Some(f.clone()),
        _ => None,
    });
    (r, f)
}

// AC-1: the boundary fn PARSES clean, and the first item is a `FnItem` with
// `boundary: Some(BoundaryAttr { target: "ext::foreign_id" })` and `body: None`.
#[test]
fn boundary_fn_parses_with_target_and_no_body() {
    let (r, f) = first_fn(FOREIGN_ID);
    assert!(
        r.is_clean(),
        "boundary fn must parse clean, got {:?}",
        r.errors
    );
    let f = f.expect("first item is a `fn`");
    assert_eq!(f.name, "foreign_id");
    let boundary = f
        .boundary
        .as_ref()
        .expect("a `#[boundary]` fn carries `boundary: Some`");
    assert_eq!(
        boundary.target, "ext::foreign_id",
        "the foreign target is the positional string"
    );
    assert!(
        f.body.is_none(),
        "a boundary fn is bodyless: `body: None`, got {:?}",
        f.body
    );
    // The contract is still mandatory and parsed.
    assert_eq!(f.contract.ens.len(), 1, "the `ens` clause is parsed");
}

// OQ-2 (the load-bearing recovery interaction): a bodyless fn WITHOUT `#[boundary]`
// is a PARSE ERROR — a normal fn missing its body must NOT silently become a
// boundary fn.
#[test]
fn bodyless_fn_without_boundary_is_a_parse_error() {
    let src = "fn nope(x: u32) -> u32 req x < 100 ens result == x fx pure ;";
    let r = parse(src);
    assert!(
        !r.is_clean(),
        "a bodyless fn WITHOUT #[boundary] must be a parse error (OQ-2), got a clean parse"
    );
    // It must NOT have parsed as a boundary fn.
    let parsed_as_boundary = matches!(
        r.program.items.first(),
        Some(Item::Fn(f)) if f.boundary.is_some()
    );
    assert!(
        !parsed_as_boundary,
        "a bodyless non-#[boundary] fn must not silently become a boundary fn"
    );
}

// A `#[boundary]` fn WITH a `{ }` body is an error — there is no Thermite body to
// prove (ffi-boundary.md REQ-3: `#[boundary]` REQUIRES the `;` form).
#[test]
fn boundary_fn_with_brace_body_is_a_parse_error() {
    let src = "#[boundary(\"ext::g\")] fn g(x: u32) -> u32 req true ens result == x fx pure { x }";
    let r = parse(src);
    assert!(
        !r.is_clean(),
        "a #[boundary] fn with a {{ }} body must be a parse error (its body is foreign)"
    );
}

// `#[boundary]` is NOT valid on a `spec fn` (ffi-boundary.md surface form).
#[test]
fn boundary_on_spec_fn_is_a_parse_error() {
    let src = "#[boundary(\"ext::s\")] spec fn s(x: u32) -> u32 dec 0 { x }";
    let r = parse(src);
    assert!(
        !r.is_clean(),
        "#[boundary] on a `spec fn` must be a parse error"
    );
}

// A NORMAL bodied fn still parses (the control): `boundary: None`, `body: Some`.
#[test]
fn normal_bodied_fn_still_parses() {
    let src = "fn id(x: u32) -> u32 req true ens result == x fx pure { x }";
    let (r, f) = first_fn(src);
    assert!(
        r.is_clean(),
        "a normal fn must parse clean, got {:?}",
        r.errors
    );
    let f = f.expect("first item is a `fn`");
    assert!(f.boundary.is_none(), "a normal fn has `boundary: None`");
    assert!(f.body.is_some(), "a normal fn has `body: Some`");
}

// The corpus programs still parse clean (no regression from the Option<Block> +
// boundary additions).
#[test]
fn corpus_programs_still_parse_clean() {
    let corpus = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    for name in ["sum", "binary_search"] {
        let path = corpus.join("conformance").join(format!("{name}.th"));
        let src = std::fs::read_to_string(&path).expect("read corpus program");
        let r = parse(&src);
        assert!(
            r.is_clean(),
            "corpus `{name}.th` must still parse clean, got {:?}",
            r.errors
        );
        // None of the corpus fns is a boundary fn.
        for item in &r.program.items {
            if let Item::Fn(f) = item {
                assert!(
                    f.boundary.is_none(),
                    "corpus `{name}` fn is not a boundary fn"
                );
                assert!(f.body.is_some(), "corpus `{name}` fn has a body");
            }
        }
    }
}
