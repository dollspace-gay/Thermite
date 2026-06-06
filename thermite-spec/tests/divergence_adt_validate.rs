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
//! These tests assert the AUTHORITY's required behavior (REQ-5/REQ-12 reject)
//! and FAIL against `5f5a4b7`. They are release-blockers (the handled-or-loud
//! guarantee), so they are NOT `#[ignore]`d — the failing test IS the block.

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

fn has_non_exhaustive(errors: &[SpecError]) -> bool {
    errors
        .iter()
        .any(|e| matches!(e, SpecError::NonExhaustiveMatch { .. }))
}

/// DIVERGENCE 1 (CRITICAL — false ACCEPT of a non-exhaustive declared-enum
/// match). `enum E { foo, bar }` is a well-formed declared enum with two unit
/// variants (`parse_enum` admits any ident as a variant name — no uppercase
/// rule). `match e { foo => 0 }` handles ONLY `foo` and has no `Wildcard`, so
/// by REQ-5/REQ-12 the validator MUST reject it with `NonExhaustiveMatch {
/// missing: ["bar"] }` (declaration order). Authority:
/// `.design/basis/01-adts.md` REQ-5 ("rejects a `match` over an enum value
/// whose arms do not cover every variant (and is not closed by a `Wildcard`
/// arm), with … `NonExhaustiveMatch { missing }`") + REQ-12 ("a non-exhaustive
/// `match` does not compile").
///
/// Toolchain (commit 5f5a4b7): `parse_path_pattern` turns the lowercase
/// single-segment `foo` into `Pattern::Binding("foo")` (a catch-all);
/// `variant_pattern_name` returns `None` for a `Binding`; so
/// `check_match_exhaustiveness` finds no declared variant in any arm, infers
/// NO matched enum, and returns early — `validate` yields `Ok(())`. The
/// non-exhaustive match SLIPS THROUGH.
///
/// Expected: `Err` containing `NonExhaustiveMatch { missing: ["bar"] }`.
/// Actual (5f5a4b7): `Ok(())`.
#[test]
fn divergence_lowercase_variant_bypasses_exhaustiveness() {
    let program = parse_clean(
        "enum E { foo, bar } \
         fn f(e: E) -> u64 req true ens result == result fx pure { match e { foo => 0 } }",
    );
    let result = validate(&program);
    let errors = match result {
        Ok(()) => panic!(
            "REQ-5/REQ-12 DIVERGENCE: a non-exhaustive `match e {{ foo => 0 }}` over declared \
             `enum E {{ foo, bar }}` (missing `bar`, no wildcard) was ACCEPTED — the \
             handled-or-loud compile-time tooth is bypassed because the lowercase variant \
             `foo` parsed as a binding catch-all. Expected NonExhaustiveMatch{{missing:[bar]}}."
        ),
        Err(errors) => errors,
    };
    let found_missing_bar = errors.iter().any(|e| {
        matches!(e, SpecError::NonExhaustiveMatch { missing, .. } if missing == &vec!["bar".to_string()])
    });
    assert!(
        found_missing_bar,
        "expected NonExhaustiveMatch {{ missing: [\"bar\"] }} (REQ-5, declaration order); got {errors:?}"
    );
}

/// DIVERGENCE 2 (CRITICAL — the worst shape: a real declared variant left
/// UNHANDLED while another real variant is mistaken for a catch-all).
/// `enum Shape { Circle(u64), Rect { .. }, tri }` declares three variants; the
/// lowercase `tri` is a legal unit variant. `match s { Circle(r) => r, tri =>
/// 0 }` NEVER handles `Rect` and has no `Wildcard` — so by REQ-5 it MUST be
/// rejected with `NonExhaustiveMatch` whose `missing` contains `Rect`.
/// Authority: `.design/basis/01-adts.md` REQ-5 + REQ-12.
///
/// Toolchain (5f5a4b7): the lowercase `tri` arm parses as
/// `Pattern::Binding("tri")` → a catch-all → `wildcard_seen` is set, the missing
/// check is suppressed, and `Rect` is treated as covered. The match validates
/// `Ok(())` despite `Rect` being modeled-but-unhandled.
///
/// Expected: `Err` containing a `NonExhaustiveMatch` listing `Rect` missing.
/// Actual (5f5a4b7): `Ok(())`.
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
            "REQ-5/REQ-12 DIVERGENCE: `match s {{ Circle(r) => r, tri => 0 }}` over \
             `enum Shape {{ Circle, Rect, tri }}` leaves `Rect` modeled-but-unhandled, yet was \
             ACCEPTED — the declared lowercase variant `tri` was mistaken for a binding \
             catch-all, masking the missing `Rect`. Expected NonExhaustiveMatch containing Rect."
        ),
        Err(errors) => errors,
    };
    assert!(
        has_non_exhaustive(&errors)
            && errors.iter().any(|e| matches!(
                e,
                SpecError::NonExhaustiveMatch { missing, .. } if missing.iter().any(|m| m == "Rect")
            )),
        "expected NonExhaustiveMatch with `Rect` missing (REQ-5); got {errors:?}"
    );
}
