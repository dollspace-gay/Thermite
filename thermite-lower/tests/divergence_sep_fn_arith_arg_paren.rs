//! Divergence pin (re-audit of caa38848/#231 probe 2 — the surviving TEXTUAL
//! suffix site) — the C5 `count_sep`/`sep_free` separator-arg coercion
//! (`lower_expr` Call arm, `sep_fn && i == 1`) still uses the
//! `lowered.ends_with("as u8")` heuristic #231 removed from the
//! `spec_call_param_cast` consumers, AND its cast emission drops the #122
//! inner paren: a `Binary` separator arg emits `{lowered} as u8` unwrapped.
//! `req count_sep(s, sep + 1) == 0` emits
//! `__thermite_count_sep(s.data@, sep + 1 as u8)` — `as` binds tighter than
//! `+`, so this is `sep + (1 as u8)`, the whole arg is the unbounded spec
//! `int`, and the `sep: u8` param rejects it: E0308 → the item dies at L0
//! though the Thermite source is correct (verus-confirmed live on the
//! emitted unit).
//!
//! THE AUTHORITY (R-CHAR-3): `.design/basis/07-strings.md` REQ-15 — the
//! generated `count_sep(s: Seq<u8>, sep: u8)` takes a `u8` separator while the
//! surface separator is a `u64`, so the SECOND arg is cast `as u8`; plus the
//! #122 paren discipline (`.design/lower/verus-lowering.md` REQ-5 precedent,
//! applied at the NEIGHBORING `coerce_u8` exec site and at every #231 cast
//! site): `as` binds tighter than `+`/`-`, so a compound arg parenthesizes the
//! inner — `(sep + 1) as u8`, never `sep + 1 as u8`. The expected substring is
//! HAND-DERIVED from REQ-15 + #122, never copied from lowerer output.
//!
//! Tracking: crosslink #234.

fn lower(src: &str) -> String {
    let parsed = thermite_syntax::parse(src);
    assert!(parsed.is_clean(), "fixture must parse clean: {parsed:?}");
    thermite_lower::lower(&parsed.program).expect("lowering must succeed")
}

/// A contract naming the generated C5 `count_sep` with an ARITHMETIC
/// separator arg (`sep + 1`, a `Binary`) at the `u8` position (index 1).
const SEP_ARITH_PROGRAM: &str = "\
fn g(s: String, sep: u64) -> u64
  req count_sep(s, sep + 1) == 0
  ens result == 0
  fx  pure
{
  0
}
";

#[test]
fn count_sep_arith_separator_arg_parenthesizes_before_u8_cast() {
    let out = lower(SEP_ARITH_PROGRAM);
    // REQ-15 (`sep: u8`) + #122 (`as` binds tighter than `+`): the compound
    // separator must be `(sep + 1) as u8` — inner-parenthesized.
    assert!(
        out.contains("(sep + 1) as u8"),
        "the C5 separator coercion must parenthesize a Binary arg before \
         `as u8` (verus rejects `sep + 1 as u8` with E0308: expected u8, \
         found int):\n{out}"
    );
}
