//! Pinned divergence (#148): the #146 cast-`<` paren fix is INCOMPLETE on the
//! struct type-invariant lowering path.
//!
//! Commit 167b9f4 fixed `lower_binary_operand` (the fn-contract / loop-`inv`
//! path, used by `forge tv` + `forge check`'s `requires`/`ensures`/loop
//! `invariant`): a `Cast` left-operand of a `<`-leading op (`<`/`<=`/`<<`) is
//! parenthesized so `x as u32 < 33` does not mis-parse as a generic-argument
//! list (`u32<33, …>` → "expected `,`"). See `divergence_cast_paren.rs` for the
//! #122/#146 family.
//!
//! But the STRUCT type-invariant path (`lower_inv_expr` → `lower_inv_operand`,
//! the REQ-8 `well_formed()` predicate, `.design/lower/...` struct invariants)
//! has its OWN operand-parenthesizer (`lower_inv_operand`) that was NOT given the
//! cast-`<` fix. So a struct invariant carrying a cast left of `<`
//! (`} inv (x as u32) < cap`) emits the BARE `x as u32 < cap` and `forge check`
//! reports L0 with `error: expected ,` — the exact #146 mis-parse, on a path the
//! fix missed. The whole cast-`<` class must be fixed (R-DEFER-8: a convention
//! starts somewhere; the fix is incomplete until every cast-`<` site is covered).
//!
//! Authority: blocker #146 / #148 (the cast-`<` paren discipline — the dual of
//! #122). Expected: the lowering parenthesizes the cast (`(x as u32) < cap`), the
//! SAME form `lower_binary_operand` now emits and `thermite_tv::ref_encode`
//! always emits. R-CHAR-3: the paren'd form is the design's parse-correct form,
//! not copied from the lowerer's output (the lowerer currently emits the WRONG
//! unparenthesized form).
//!
//! `#[ignore]`d (blocker #148 tracks the fix; un-ignore when `lower_inv_operand`
//! gets the cast-`<` paren — R-DEFER-3).

/// A struct type-invariant with a cast left of `<` must lower with the cast
/// parenthesized (`(x as u32) < cap`), never the mis-parsing bare form
/// `x as u32 < cap` (which Verus/Rust reads as `u32<cap, …>` — "expected `,`").
/// This is the #146 cast-`<` fix on the struct type-invariant path it missed.
#[test]
#[ignore = "blocker #148: lower_inv_operand misses the #146 cast-< paren fix (struct type-invariant)"]
fn struct_invariant_cast_lt_is_parenthesized() {
    // `Gauge`'s invariant casts `x` then compares with `<` — the cast-`<`
    // ambiguity. (The source parens are stripped + re-emitted through
    // `lower_inv_operand`, which does not re-add them.)
    let src = "struct Gauge { x: u64, cap: u64, } inv (x as u32) < cap";
    let parsed = thermite_syntax::parse(src);
    assert!(parsed.is_clean(), "fixture must parse clean: {parsed:?}");

    let l3 = thermite_lower::lower(&parsed.program).expect("L3 lowering");

    // The mis-parsing bare form MUST NOT appear — this is the divergence the bug
    // produces (and what makes `forge check` emit `error: expected ,`).
    assert!(
        !l3.contains("self.x as u32 < self.cap"),
        "struct-invariant cast-`<` must NOT emit the unparenthesized \
         `self.x as u32 < self.cap` (= the `u32<...>` generic mis-parse, #146/#148):\n{l3}"
    );
    // The parse-correct form is the parenthesized cast — the SAME discipline
    // `lower_binary_operand` applies on the fn-contract / loop-inv path.
    assert!(
        l3.contains("(self.x as u32) < self.cap"),
        "struct-invariant cast-`<` must parenthesize the cast \
         (`(self.x as u32) < self.cap`), the #146 fix on the struct-inv path:\n{l3}"
    );
}
