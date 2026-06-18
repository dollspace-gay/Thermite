//! Regression: the cast-`<` paren discipline on the struct type-invariant
//! lowering path (blocker #148, CLOSED — this test pins the fix).
//!
//! Commit 167b9f4 first fixed `lower_binary_operand` (the fn-contract / loop-`inv`
//! path, used by `forge tv` + `forge check`'s `requires`/`ensures`/loop
//! `invariant`): a `Cast` left-operand of a `<`-leading op (`<`/`<=`/`<<`) is
//! parenthesized so `x as u32 < 33` does not mis-parse as a generic-argument
//! list (`u32<33, …>` → "expected `,`"). See `divergence_cast_paren.rs` for the
//! #122/#146 family.
//!
//! The struct type-invariant path (`lower_inv_expr` → `lower_inv_operand`, the
//! REQ-8 `well_formed()` predicate, `.design/lower/...` struct invariants) has its
//! own operand-parenthesizer, which #148 extended with the SAME cast-`<` guard
//! (`lower_inv_operand` in `thermite-lower/src/lower.rs`: a `Cast` left operand of
//! an `is_lt_leading` parent is parenthesized — R-DEFER-8, the convention is now
//! uniform across every emission site). So `} inv (x as u32) < cap` lowers to the
//! parse-correct `(self.x as u32) < self.cap`, not the bare `self.x as u32 <
//! self.cap` that Verus/Rust would read as `u32<…>`.
//!
//! Authority: blocker #146 / #148 (the cast-`<` paren discipline — the dual of
//! #122). R-CHAR-3: the paren'd form below is the design's parse-correct form,
//! asserted directly, not copied from the lowerer's output. This test runs in CI
//! (no `#[ignore]`) and fails if the struct-inv parenthesizer ever regresses.

/// A struct type-invariant with a cast left of `<` must lower with the cast
/// parenthesized (`(x as u32) < cap`), never the mis-parsing bare form
/// `x as u32 < cap` (which Verus/Rust reads as `u32<cap, …>` — "expected `,`").
/// This is the #146 cast-`<` fix on the struct type-invariant path it missed.
#[test]
fn struct_invariant_cast_lt_is_parenthesized() {
    // `Gauge`'s invariant casts `x` then compares with `<` — the cast-`<`
    // ambiguity. (The source parens are stripped + re-emitted through
    // `lower_inv_operand`, which does not re-add them.)
    let src = "struct Gauge { x: u64, cap: u64, } inv (x as u32) < cap";
    let parsed = thermite_syntax::parse(src);
    assert!(parsed.is_clean(), "fixture must parse clean: {parsed:?}");

    let l3 = thermite_lower::lower(&parsed.program).expect("L3 lowering");

    // The mis-parsing bare form must not appear — this is the divergence the bug
    // produces (and what makes `forge check` emit `error: expected ,`).
    assert!(
        !l3.contains("self.x as u32 < self.cap"),
        "struct-invariant cast-`<` must NOT emit the unparenthesized \
         `self.x as u32 < self.cap` (= the `u32<...>` generic mis-parse, #146/#148):\n{l3}"
    );
    // The parse-correct form is the parenthesized cast — the same discipline
    // `lower_binary_operand` applies on the fn-contract / loop-inv path.
    assert!(
        l3.contains("(self.x as u32) < self.cap"),
        "struct-invariant cast-`<` must parenthesize the cast \
         (`(self.x as u32) < self.cap`), the #146 fix on the struct-inv path:\n{l3}"
    );
}
