//! Divergence pins for the Basis Stage 1b ADT validator (`thermite-spec`,
//! crosslink #65, commit `5f5a4b7`).
//!
//! Authority: `.design/basis/01-adts.md` REQ-5 (exhaustiveness —
//! `NonExhaustiveMatch`/`UnreachableArm`), REQ-12 (handled-or-loud: a
//! non-exhaustive `match` over a declared `enum` MUST be rejected BEFORE the
//! program ships — "every modeled outcome (variant) is handled, or an explicit
//! `Wildcard` catch screams … silently dropping an unhandled variant is
//! structurally impossible"). `goal.md` R-DEFER-9 (no proof cheats / no
//! degenerate pass), R-CHAR-3 (expected values hand-derived from the design,
//! never read back from the validator's own output).
//!
//! THE CRUX (highest value): the validator infers the matched enum from the arm
//! PATTERNS (the AST is untyped) — "a `match` is a declared-enum match iff some
//! arm names a variant of a declared `enum`" (`check_match_exhaustiveness` +
//! `variant_pattern_name` in `validator.rs`). The disambiguation of a
//! single-segment pattern into `Pattern::Enum` vs `Pattern::Binding` is done by
//! the PARSER on FIRST-LETTER CASE alone (`parse_path_pattern` in
//! `thermite-syntax/src/parser.rs`: "an uppercase-initial single segment
//! (`None`) is a zero-field enum pattern", else a binding). But `parse_enum`
//! places NO casing constraint on a variant DECLARATION (`take_ident("a variant
//! name")`), so an `enum` may declare a LOWERCASE variant. A lowercase variant
//! NAMED in a `match` arm is then parsed as a `Pattern::Binding` (a catch-all),
//! so `variant_pattern_name` returns `None` for it, the matched-enum inference
//! treats the arm as a catch-all, and a genuinely NON-EXHAUSTIVE match over a
//! declared enum is ACCEPTED (`Ok(())`). That is a false ACCEPT of the
//! compile-time tooth — the exact R-DEFER-9 / handled-or-loud hole.
//!
//! THE RESOLUTION (`.design/basis/01-adts.md` REQ-2, design-ruled): variant
//! names MUST be UpperCamelCase (uppercase-initial); the validator rejects a
//! lowercase-initial variant DECLARATION with `SpecError::InvalidVariantCasing
//! { name, span }`. This makes the parser's case-based pattern disambiguation
//! SOUND — a lowercase pattern ident is unambiguously a binding, because no
//! lowercase variant can EXIST. So the #66 bypass closes at the ENUM
//! DECLARATION: `enum E { foo, bar }` is rejected with `InvalidVariantCasing`
//! BEFORE any match is considered; the offending program no longer validates
//! clean (the false ACCEPT is gone), just with an earlier, more precise error.
//! The CORE pin holds: the program must NOT validate clean (no silent accept of
//! an unhandled variant). The two bypass tests below assert the
//! `InvalidVariantCasing` reject (hand-derived from REQ-2, R-CHAR-3); the
//! positive companion (`exhaustiveness_intact_uppercase_nonexhaustive`) proves
//! the NORMAL exhaustiveness path is unweakened — an UPPERCASE-variant
//! non-exhaustive match still yields `NonExhaustiveMatch`.

use thermite_spec::{validate, SpecError};

/// Parse a program, asserting it parsed with zero syntax errors (a parse
/// failure would mean `thermite-syntax` broke, not the validator under test).
fn parse_clean(src: &str) -> thermite_syntax::Program {
    let r = thermite_syntax::parse(src);
    assert!(
        r.errors.is_empty(),
        "program failed to PARSE (thermite-syntax errors, not the validator under test): {:?}",
        r.errors
    );
    r.program
}

/// DIVERGENCE 1 — closed at the DECLARATION by REQ-2's casing rule. The bypass
/// program was `enum E { foo, bar }` + `match e { foo => 0 }`: a lowercase
/// variant `foo` parsed as a `Pattern::Binding` catch-all and the non-exhaustive
/// match (missing `bar`) slipped through `Ok(())` (commit 5f5a4b7). REQ-2 now
/// MANDATES variant names be UpperCamelCase: the validator rejects the lowercase
/// `foo`/`bar` declaration with `SpecError::InvalidVariantCasing { name }` in
/// the declaration pre-pass, BEFORE any match is considered. So the program no
/// longer validates clean — the false ACCEPT is gone, with an earlier, more
/// precise error. Authority: `.design/basis/01-adts.md` REQ-2 ("variant names
/// MUST be UpperCamelCase … the validator rejects a lowercase-initial variant
/// declaration with `SpecError::InvalidVariantCasing { name, span }`"). The
/// CORE pin holds: the program must NOT validate clean.
///
/// Expected: `Err` containing `InvalidVariantCasing { name: "foo" }` (the first
/// offending variant; `bar` is also rejected). Hand-derived from REQ-2
/// (R-CHAR-3).
#[test]
fn divergence_lowercase_variant_bypasses_exhaustiveness() {
    let program = parse_clean(
        "enum E { foo, bar } \
         fn f(e: E) -> u64 req true ens result == result fx pure { match e { foo => 0 } }",
    );
    let result = validate(&program);
    let errors = match result {
        Ok(()) => panic!(
            "REQ-2 DIVERGENCE: `enum E {{ foo, bar }}` declares lowercase-initial variants and \
             must be REJECTED with InvalidVariantCasing at the declaration (closing the #66 \
             bypass at its root — a lowercase variant can no longer exist to be mistaken for a \
             catch-all binding). Expected InvalidVariantCasing{{name:foo}}, got Ok(())."
        ),
        Err(errors) => errors,
    };
    let found_casing_foo = errors
        .iter()
        .any(|e| matches!(e, SpecError::InvalidVariantCasing { name, .. } if name == "foo"));
    assert!(
        found_casing_foo,
        "expected InvalidVariantCasing {{ name: \"foo\" }} (REQ-2); got {errors:?}"
    );
}

/// DIVERGENCE 2 — the worst-shape bypass, also closed at the DECLARATION. The
/// program was `enum Shape { Circle(u64), Rect { .. }, tri }` + `match s {
/// Circle(r) => r, tri => 0 }`: the lowercase `tri` arm parsed as a catch-all,
/// masking the modeled-but-unhandled `Rect`, and the match validated `Ok(())`
/// (commit 5f5a4b7). Under REQ-2 the lowercase variant `tri` is rejected at the
/// `enum Shape` declaration with `InvalidVariantCasing { name: "tri" }` before
/// the match is considered — `tri` can no longer exist as a catch-all-masking
/// variant. Authority: `.design/basis/01-adts.md` REQ-2. The CORE pin holds:
/// the program must NOT validate clean.
///
/// Expected: `Err` containing `InvalidVariantCasing { name: "tri" }`.
/// Hand-derived from REQ-2 (R-CHAR-3).
#[test]
fn divergence_lowercase_arm_masks_unhandled_variant() {
    let program = parse_clean(
        "enum Shape { Circle(u64), Rect { w: u64, h: u64 }, tri } \
         fn f(s: Shape) -> u64 req true ens result == result fx pure \
         { match s { Circle(r) => r, tri => 0 } }",
    );
    let result = validate(&program);
    let errors = match result {
        Ok(()) => panic!(
            "REQ-2 DIVERGENCE: `enum Shape {{ Circle, Rect, tri }}` declares the lowercase \
             variant `tri` and must be REJECTED with InvalidVariantCasing at the declaration, \
             closing the #66 catch-all-masking bypass at its root. Expected \
             InvalidVariantCasing{{name:tri}}, got Ok(())."
        ),
        Err(errors) => errors,
    };
    assert!(
        errors.iter().any(|e| matches!(
            e,
            SpecError::InvalidVariantCasing { name, .. } if name == "tri"
        )),
        "expected InvalidVariantCasing {{ name: \"tri\" }} (REQ-2); got {errors:?}"
    );
}

/// POSITIVE COMPANION (REQ-5 unweakened). The casing rule (REQ-2) closed the
/// bypass WITHOUT weakening real exhaustiveness checking: an UPPERCASE-variant
/// enum with a genuinely non-exhaustive match still yields `NonExhaustiveMatch`.
/// `enum Shape { Circle(u64), Rect { w: u64, h: u64 } }` + `match s { Circle(r)
/// => r }` handles only `Circle` and has no `Wildcard`, so by REQ-5 it MUST be
/// rejected with `NonExhaustiveMatch { missing: ["Rect"] }` (declaration order).
/// Both variants are uppercase-initial, so REQ-2 accepts the declaration and the
/// (now-sound) exhaustiveness walk runs. Authority: `.design/basis/01-adts.md`
/// REQ-5. Hand-derived (R-CHAR-3).
#[test]
fn exhaustiveness_intact_uppercase_nonexhaustive() {
    let program = parse_clean(
        "enum Shape { Circle(u64), Rect { w: u64, h: u64 } } \
         fn f(s: Shape) -> u64 req true ens result == result fx pure { match s { Circle(r) => r } }",
    );
    let result = validate(&program);
    let errors = match result {
        Ok(()) => panic!(
            "REQ-5: `match s {{ Circle(r) => r }}` over `enum Shape {{ Circle, Rect }}` leaves \
             `Rect` unhandled with no wildcard and must be rejected with NonExhaustiveMatch — \
             the casing rule (REQ-2) must NOT weaken real exhaustiveness checking. Got Ok(())."
        ),
        Err(errors) => errors,
    };
    let found_missing_rect = errors.iter().any(|e| {
        matches!(e, SpecError::NonExhaustiveMatch { missing, .. } if missing == &vec!["Rect".to_string()])
    });
    // The casing rule must not have spuriously rejected the (uppercase) variants.
    assert!(
        !errors
            .iter()
            .any(|e| matches!(e, SpecError::InvalidVariantCasing { .. })),
        "uppercase variants must NOT trip InvalidVariantCasing; got {errors:?}"
    );
    assert!(
        found_missing_rect,
        "expected NonExhaustiveMatch {{ missing: [\"Rect\"] }} (REQ-5, declaration order); got {errors:?}"
    );
}
