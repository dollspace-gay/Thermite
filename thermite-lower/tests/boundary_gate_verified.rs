//! OBSERVABLE-dispatch equivalence test anchoring `lower::lower_fn`'s boundary
//! HONESTY gate to the Verus-verified `should_emit_external_body` predicate (epic
//! #60, `.design/verified/self-verification.md` REQ-9 / Target C / AC-9c, mechanism
//! (c)).
//!
//! The verus core proves the 2-bool DECISION (`r == has_boundary || has_slag`,
//! plus the §9 soundness corollary `(!has_boundary && !has_slag) ==> !r`). Per
//! OQ-C, the production anchor must assert the OBSERVABLE dispatch — that
//! `thermite_lower::lower` on a `(false, false)` REGULAR fn emits a fully-proved
//! body (NO `#[verifier::external_body]` substring) and on any flagged
//! (`#[boundary]`/`#[slag]`) fn emits the `#[verifier::external_body]` signature —
//! NOT merely that a mirror predicate returns the same bool. This test enumerates
//! the 4 `(has_boundary, has_slag)` combinations, lowers a fn for each, and asserts
//! the emitted source carries `#[verifier::external_body]` IFF the proved predicate
//! is `true`. The risk (parallel to OQ-5) is a gap between "the predicate is
//! proved" and "`lower_fn` is wired to honor it"; this test inspects the emitted
//! source, closing that gap.
//!
//! R-CHAR-3: the expected value is the verus-verified predicate
//! `thermite_verified::should_emit_external_body` (an EXTERNAL truth proved by
//! `verus --no-cheating`), NEVER the lowerer's own output. `unwrap`/`expect` are
//! fine here — `tests/` is not anti-pattern-gated.

use thermite_syntax::ast::{Item, SlagAttr};
use thermite_syntax::lexer::Span;
use thermite_verified::should_emit_external_body;

/// The verus `#[verifier::external_body]` substring the boundary/slag arm emits.
const EXTERNAL_BODY: &str = "#[verifier::external_body]";

/// Build the surface source for a fn over the `(has_boundary, has_slag)` axis,
/// with AT MOST one surface attribute (the parser accepts one). The contract is
/// held FIXED (`req x < 100 ens result == x fx pure`) so the ONLY varying input is
/// the gate's flags (the proved predicate's exact 2-bool domain). The `(true,
/// true)` case is built by parsing the boundary form and INJECTING the slag flag
/// onto the AST (the parser does not stack two attributes, but a `FnItem` CAN
/// carry both flags — the production gate sees both).
///
/// - `(false, false)` — a REGULAR in-language fn with a real body.
/// - `(true, *)` — a `#[boundary("ext::ext_id")]` fn (foreign body, `;`).
/// - `(false, true)` — a `#[slag(...)]` fn with a fiat body.
fn fn_source(has_boundary: bool, has_slag: bool) -> String {
    let mut attrs = String::new();
    if has_boundary {
        attrs.push_str("#[boundary(\"ext::ext_id\")] ");
    } else if has_slag {
        attrs.push_str(
            "#[slag(reason = \"vendored\", owner = \"agent:forge-7\", review = \"required\")] ",
        );
    }
    let sig = "fn f(x: u32) -> u32 req x < 100 ens result == x fx pure";
    if has_boundary {
        // A boundary fn has a FOREIGN body (`body: None`) — terminated with `;`.
        format!("{attrs}{sig} ;")
    } else {
        // A regular / slag fn carries a real (in-language / fiat) body.
        format!("{attrs}{sig} {{ x }}")
    }
}

/// Lower the single-fn program and return the emitted verus source. A boundary
/// fn at top level is lowered by `lower_fn` directly (the same dispatch
/// `item_subprogram` exercises when it weaves a boundary/slag dependency). For the
/// `(true, true)` combination the parsed boundary fn's `slag` flag is set on the
/// AST so the production gate observes BOTH flags (the parser stacks no two
/// attributes, but the gate's domain is the 2-bool square).
fn lower_fn_source(has_boundary: bool, has_slag: bool) -> String {
    let src = fn_source(has_boundary, has_slag);
    let mut parsed = thermite_syntax::parse(&src);
    assert!(
        parsed.is_clean(),
        "fixture must parse ({has_boundary}, {has_slag}): {:?}\n{src}",
        parsed.errors
    );
    if has_boundary && has_slag {
        for item in &mut parsed.program.items {
            if let Item::Fn(f) = item {
                f.slag = Some(SlagAttr {
                    reason: Some("vendored".to_string()),
                    owner: Some("agent:forge-7".to_string()),
                    review: Some("required".to_string()),
                    span: Span::new(0, 1),
                });
            }
        }
    }
    thermite_lower::lower(&parsed.program)
        .unwrap_or_else(|e| panic!("lowering ({has_boundary}, {has_slag}) failed: {e:?}\n{src}"))
}

/// AC-9c (OBSERVABLE dispatch, REQ-9 / Target C): over the 4 `(has_boundary,
/// has_slag)` combinations, the emitted verus source carries
/// `#[verifier::external_body]` IFF the Verus-verified predicate
/// `should_emit_external_body(has_boundary, has_slag)` is `true`. In particular
/// the `(false, false)` REGULAR case emits NO `external_body` (the fully-proved-
/// body arm — a lying regular body can never be laundered to an assumed-L3
/// signature, §9). 0 mismatches over the full 2×2 domain.
#[test]
fn lower_fn_emits_external_body_iff_proved_predicate() {
    let mut checked = 0;
    for &has_boundary in &[false, true] {
        for &has_slag in &[false, true] {
            // R-CHAR-3: the EXPECTED gate verdict is the verus-proved predicate.
            let expected = should_emit_external_body(has_boundary, has_slag);
            let emitted = lower_fn_source(has_boundary, has_slag);
            let observed = emitted.contains(EXTERNAL_BODY);
            assert_eq!(
                observed, expected,
                "OBSERVABLE dispatch mismatch at (boundary={has_boundary}, \
                 slag={has_slag}): emitted external_body={observed}, proved \
                 predicate={expected}\n--- emitted ---\n{emitted}"
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 4, "all 4 (boundary, slag) combinations enumerated");
}

/// The §9 soundness corollary made OBSERVABLE (REQ-9): the REGULAR fn (neither
/// flag) takes the fully-proved-body arm — its emitted source contains NO
/// `#[verifier::external_body]` and DOES carry the fn's real body (`{`), so the
/// lying-regular-body laundering R-DEFER-9 forbids is structurally impossible.
#[test]
fn regular_fn_is_fully_proved_never_external_body() {
    let emitted = lower_fn_source(false, false);
    assert!(
        !emitted.contains(EXTERNAL_BODY),
        "a REGULAR fn must NEVER be emitted external_body (§9 / R-DEFER-9):\n{emitted}"
    );
    assert!(
        emitted.contains("fn f(x: u32) -> (result: u32)"),
        "a regular fn carries its fully-proved signature + body:\n{emitted}"
    );
}
