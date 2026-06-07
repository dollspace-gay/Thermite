//! Divergence pin (re-audit of the #96 fix, commit `1b57d60`): `u64_to_string`'s
//! L3 verus emission and its L1 runnable emission DISAGREE on the input `0`.
//!
//! The fix made the decimal MSB-first (correct) and proved the `parse_be(reverse)
//! == parse_le` bridge (a genuine proof — no cheat). But the construction loop is
//! `while m > 0`, which never runs for `n == 0`:
//!
//!   * L1 (`thermite-lower::lower_l1`, `emit_string_runtime_l1`) emits an explicit
//!     zero-guard `if m == 0 { data.push(48u8); }` BEFORE the loop, so the BUILT
//!     binary prints `0` -> byte `[48]` (the ASCII digit '0').
//!   * L3 (`thermite-lower::lower`, `emit_numfmt_defs`) emits NO such guard: for
//!     `n == 0` the loop body never runs, the reverse of `[]` is `[]`, and the
//!     verified `result.data@` is the EMPTY sequence. `parse_be([]) == 0 == n`
//!     verifies vacuously-for-zero, so verus is satisfied either way — it does NOT
//!     detect that the emitted bytes are empty rather than "0".
//!
//! Consequence — TWO authority misses, both un-detected by the round-trip ens:
//!
//!   1. L3 != L1 (the commit message asserts "L3 == L1 ... agree byte-for-byte").
//!      For `n == 0` they differ: L3 -> `[]`, L1 -> `[48]`. The assurance artifact
//!      (the verified L3 lowering) does NOT produce the bytes the runnable binary
//!      produces, breaking the meaning of the L3 cert w.r.t. the run output.
//!   2. `.design/basis/07-strings.md` REQ-8: "The surface emits the human-readable
//!      MSB-first decimal". The human-readable decimal of 0 is "0" (one byte, 48),
//!      NOT the empty string. The L3 form emits "" for 0 — not the decimal of 0.
//!
//! AUTHORITY (R-CHAR-3 — expected value is the design ASCII constant, never forge
//! output): `.design/basis/07-strings.md` REQ-8 ("the surface emits the
//! human-readable MSB-first decimal"); the digit '0' is ASCII 48 (the design `+ 48u8`
//! digit constant). `goal.md` R-DEFER-9 / the commit's own "L3 == L1 byte-for-byte"
//! claim. The L1 zero-guard `if m == 0 { data.push(48u8); }` is the design-correct
//! behavior; the L3 emission must produce the SAME single `48` byte for 0.
//!
//! Tracking: blocker #97.

use thermite_syntax::ast::Program;

fn parse_src(src: &str) -> Program {
    let parsed = thermite_syntax::parse(src);
    assert!(
        parsed.errors.is_empty(),
        "must parse clean: {:?}",
        parsed.errors
    );
    parsed.program
}

/// The `to_string` program both lowerings materialize the generated `u64_to_string`
/// from (it names `parse_be` + uses `n.to_string()`, so `program_uses_numfmt` fires).
const TOSTRING_SRC: &str =
    "fn show(n: u64) -> String\n  req true\n  ens parse_be(result) == n\n  fx alloc\n{ n.to_string() }\n";

#[test]
#[ignore = "divergence: L3 u64_to_string emits the empty seq for input 0 (no zero-guard) while L1 emits [48]=\"0\"; REQ-8 mandates the human-readable decimal of 0 (\"0\"); blocker #97 — un-ignore when fixed"]
fn divergence_numfmt_l3_zero_handling_matches_l1() {
    let program = parse_src(TOSTRING_SRC);

    let l3 = thermite_lower::lower(&program).expect("L3 lower");
    let l1 = thermite_lower::lower_l1(&program).expect("L1 lower");

    // L1 is the design-correct reference: it explicitly pushes the digit '0' (ASCII
    // 48) when the value is zero, so the built binary renders "0". This must be
    // present (it is the behavior REQ-8 mandates for 0).
    assert!(
        l1.contains("if m == 0 { data.push(48u8); }"),
        "L1 `u64_to_string` must zero-guard (push the '0' digit, ASCII 48) so `0` \
         renders as the human-readable \"0\" (07-strings.md REQ-8)"
    );

    // The DIVERGENCE: the L3 verus emission of `u64_to_string` must ALSO produce the
    // single byte 48 ('0') for `n == 0` — otherwise the verified lowering yields the
    // empty string for 0 (parse_be([]) == 0 verifies, masking the gap), and L3 != L1
    // (the commit claims byte-for-byte equality). Pin: the L3 exec body must carry
    // the same zero-guard. It currently does NOT (the `while m > 0` loop is the only
    // digit source), so this assertion FAILS — the divergence.
    let exec = l3
        .find("pub fn u64_to_string")
        .map(|i| &l3[i..])
        .expect("L3 must emit u64_to_string");
    assert!(
        exec.contains("m == 0") && exec.contains("48"),
        "07-strings.md REQ-8 + R-DEFER-9: the L3 `u64_to_string` must emit the digit \
         '0' (ASCII 48) for input 0 so its verified output is the human-readable \
         decimal \"0\" (one byte) and matches the L1 runnable form byte-for-byte. It \
         currently emits NO zero-guard: for n==0 `result.data@` is the EMPTY seq, \
         `parse_be([]) == 0` verifies vacuously-for-zero, and L3 (\"\") != L1 (\"0\"). \
         L3 u64_to_string body:\n{exec}"
    );
}
