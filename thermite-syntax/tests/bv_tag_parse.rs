//! Tests for the Stage-3 `@bv` machine-semantics clause tag
//! (`.design/stage3-bv-reconstruction.md` REQ-1, AC-1): the first clause-level
//! annotation in `thermite-syntax`, parse-gated behind the shadow-flag plumbing.
//!
//! The tag (`ens@bvN` / `inv@bvN` / `@bvN(nowrap)`, N ∈ {8, 16, 32, 64}) parses
//! only when the crate is built with the `bv` cargo feature — the
//! structural lock R-BV-1: a build without the plumbing cannot parse
//! the tag (the parser code path is `#[cfg]`-removed), so the tag is a structured
//! syntax error there. This file pins both halves of AC-1, each behind the
//! matching `cfg`:
//!
//! - `#[cfg(not(feature = "bv"))]` — the negative half: `ens@bv64` fails to
//!   parse with `SyntaxError::BvTagWithoutShadowPlumbing`.
//! - `#[cfg(feature = "bv")]` — the positive half: all four widths +
//!   `nowrap` parse with the AST tag recovered and the clause `text` round-trips
//!   (the tag sits outside the addressing oracle string).
//!
//! CI runs `cargo test -p thermite-syntax` (negative half) and
//! `cargo test -p thermite-syntax --features bv` (positive half) so both
//! configurations are exercised in one run (AC-1's "build-flag test in CI").
//! R-CHAR-3: shapes hand-derived from the grammar; `tests/` is ungated.

use thermite_syntax::{parse, SyntaxError};

// ===========================================================================
// Negative half (AC-1): without the shadow-flag plumbing, the tag is a parse
// error — the feature cannot exist in the build.
// ===========================================================================

#[cfg(not(feature = "bv"))]
mod plumbing_absent {
    use super::*;

    /// `ens@bv64` in a build without `bv` is a structured syntax error
    /// pointing at the `@` (the structural lock R-BV-1 / AC-1 negative half).
    #[test]
    fn ens_bv_tag_is_a_parse_error_without_plumbing() {
        let src = "fn f(a: u64) -> u64 req true ens@bv64 result == 0 fx pure { 0 }";
        let result = parse(src);
        assert!(
            !result.is_clean(),
            "the `@bv` tag must NOT parse without the shadow-flag plumbing"
        );
        assert!(
            result
                .errors
                .iter()
                .any(|e| matches!(e, SyntaxError::BvTagWithoutShadowPlumbing { .. })),
            "expected a BvTagWithoutShadowPlumbing error, got: {:?}",
            result.errors
        );
    }

    /// The same gate applies to a `@bvN(nowrap)` spelling and to an `inv` clause —
    /// nothing carrying the tag parses without the plumbing.
    #[test]
    fn nowrap_and_inv_tags_are_parse_errors_without_plumbing() {
        for src in [
            "fn f(a: u64) -> u64 req true ens@bv64(nowrap) result == 0 fx pure { 0 }",
            "struct S { x: u64 } inv@bv32 x == x",
        ] {
            let result = parse(src);
            assert!(
                result
                    .errors
                    .iter()
                    .any(|e| matches!(e, SyntaxError::BvTagWithoutShadowPlumbing { .. })),
                "expected BvTagWithoutShadowPlumbing for {src:?}, got: {:?}",
                result.errors
            );
        }
    }

    /// The error message names the `bv` feature so the gate is discoverable.
    #[test]
    fn the_error_message_points_at_the_feature() {
        let src = "fn f(a: u64) -> u64 req true ens@bv64 result == 0 fx pure { 0 }";
        let err = parse(src)
            .errors
            .into_iter()
            .find(|e| matches!(e, SyntaxError::BvTagWithoutShadowPlumbing { .. }))
            .expect("a BvTagWithoutShadowPlumbing error");
        assert!(
            err.to_string().contains("bv"),
            "the diagnostic should name the `bv` feature: {err}"
        );
    }
}

// ===========================================================================
// Positive half (AC-1): with the shadow-flag plumbing, all four widths +
// `nowrap` parse, the AST tag is recovered, and the clause text round-trips.
// ===========================================================================

#[cfg(feature = "bv")]
mod plumbing_present {
    use super::*;
    use thermite_syntax::{BvWidth, Clause, ForgeItem, Item};

    /// Parse `src` and return the single `fn`'s `ens` clauses.
    fn fn_ens(src: &str) -> Vec<Clause> {
        let result = parse(src);
        assert!(
            result.is_clean(),
            "expected a clean parse of {src:?}, got: {:?}",
            result.errors
        );
        match result.program.items.into_iter().next().unwrap() {
            Item::Fn(f) => f.contract.ens,
            other => panic!("expected a fn, got {other:?}"),
        }
    }

    /// Scaffold a minimal exec `fn` whose single `ens` clause is `<ens> result == 0`.
    fn fn_with_ens(ens: &str) -> String {
        format!("fn f(a: u64) -> u64 req true ens{ens} result == 0 fx pure {{ 0 }}")
    }

    #[test]
    fn all_four_widths_parse() {
        for (tag, width, bits) in [
            ("@bv8", BvWidth::W8, 8),
            ("@bv16", BvWidth::W16, 16),
            ("@bv32", BvWidth::W32, 32),
            ("@bv64", BvWidth::W64, 64),
        ] {
            let ens = fn_ens(&fn_with_ens(tag));
            let bv = ens[0].bv.expect("the tagged ens clause carries a BvTag");
            assert_eq!(bv.width, width, "width for {tag}");
            assert_eq!(bv.width.bits(), bits, "bits for {tag}");
            assert!(!bv.nowrap, "a bare {tag} is not nowrap");
        }
    }

    #[test]
    fn nowrap_modifier_parses() {
        let ens = fn_ens(&fn_with_ens("@bv64(nowrap)"));
        let bv = ens[0].bv.expect("a BvTag");
        assert_eq!(bv.width, BvWidth::W64);
        assert!(bv.nowrap, "`@bv64(nowrap)` sets nowrap");
    }

    #[test]
    fn the_tag_sits_outside_the_clause_text_round_trip() {
        // The clause `text` is the addressing oracle string (semantic-addressing
        // AC-1): the `@bv64` tag must not bleed into it — only the expression.
        let ens = fn_ens(&fn_with_ens("@bv64"));
        assert_eq!(ens[0].text, "result == 0");
        assert!(ens[0].bv.is_some());
    }

    #[test]
    fn tagged_and_untagged_clauses_coexist_on_one_item() {
        // The RFC mix64 shape: wraparound and unbounded clauses side by side, each
        // labeled. Here a `@bv64` ens next to an untagged ens.
        let src = "fn f(a: u64) -> u64 req true \
                   ens@bv64 result == 0 \
                   ens result == a \
                   fx pure { 0 }";
        let ens = fn_ens(src);
        assert_eq!(ens.len(), 2);
        assert_eq!(ens[0].bv.expect("first ens is tagged").width, BvWidth::W64);
        assert!(ens[1].bv.is_none(), "second ens is untagged");
    }

    #[test]
    fn inv_clause_accepts_the_tag() {
        // A struct type-invariant `inv@bvN` parses under the same gate.
        let result = parse("struct S { x: u64 } inv@bv32 x == x");
        assert!(
            result.is_clean(),
            "expected a clean parse, got: {:?}",
            result.errors
        );
        match result.program.items.into_iter().next().unwrap() {
            Item::Struct(s) => {
                let inv = s.inv.expect("the struct has an inv clause");
                assert_eq!(inv.bv.expect("the inv is tagged").width, BvWidth::W32);
                assert_eq!(inv.text, "x == x");
            }
            other => panic!("expected a struct, got {other:?}"),
        }
    }

    #[test]
    fn lemma_items_accept_the_tag() {
        // REQ-1: `lemma` items accept the tag under the same gate (its `ens`
        // conclusion flows through the shared `parse_clause` seam).
        let result = parse("lemma l(a: u64) req true ens@bv64 a == a proof { omega }");
        assert!(
            result.is_clean(),
            "expected a clean parse, got: {:?}",
            result.errors
        );
        match result.program.items.into_iter().next().unwrap() {
            Item::Forge(ForgeItem::Lemma(l)) => {
                assert_eq!(
                    l.ens[0].bv.expect("the lemma ens is tagged").width,
                    BvWidth::W64
                );
            }
            other => panic!("expected a lemma, got {other:?}"),
        }
    }

    #[test]
    fn an_invalid_width_is_a_structured_error() {
        // `@bv7` is not one of the four committed widths -> BvWidthInvalid.
        let result = parse(&fn_with_ens("@bv7"));
        assert!(
            result
                .errors
                .iter()
                .any(|e| matches!(e, SyntaxError::BvWidthInvalid { found, .. } if found == "bv7")),
            "expected BvWidthInvalid(bv7), got: {:?}",
            result.errors
        );
    }

    #[test]
    fn a_bare_at_with_no_width_is_a_structured_error() {
        // `@` with no `bvN` following -> BvWidthInvalid (the width token is absent).
        let result = parse("fn f(a: u64) -> u64 req true ens@ result == 0 fx pure { 0 }");
        assert!(
            result
                .errors
                .iter()
                .any(|e| matches!(e, SyntaxError::BvWidthInvalid { .. })),
            "expected BvWidthInvalid for a bare `@`, got: {:?}",
            result.errors
        );
    }

    #[test]
    fn a_malformed_nowrap_modifier_is_a_structured_error() {
        // `@bv64(foo)` -> the generic unexpected-token error for the `nowrap` slot.
        let result = parse(&fn_with_ens("@bv64(foo)"));
        assert!(
            !result.is_clean(),
            "`@bv64(foo)` must not parse cleanly: {:?}",
            result.errors
        );
        assert!(
            result
                .errors
                .iter()
                .any(|e| matches!(e, SyntaxError::Unexpected { .. })),
            "expected an Unexpected error for `(foo)`, got: {:?}",
            result.errors
        );
    }
}
