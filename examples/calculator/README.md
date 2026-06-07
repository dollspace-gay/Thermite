# Verified calculator core — acceptance program 2 of 3 (#103)

`calc.th` composes shipped verified primitives — cluster **C7**'s `parse_u64`
(`String` → `Option<u64>`, `.design/basis/09-option-result.md` / `07-strings.md`
REQ-9) with its big-endian round-trip, the built-in `Option<u64>` + the
spec-`match`-in-`ens` payload projection, and the C2 partial-`+` (overflow is a
proof obligation). No new toolchain feature is needed to CERTIFY.

## What is proven (forge check → L3)

```thermite
fn add(a: String, b: String) -> Option<u64>
  req all_digits(a) && a.len() >= 1 && parse_be(a) <= 9223372036854775807
   && all_digits(b) && b.len() >= 1 && parse_be(b) <= 9223372036854775807
  ens result is Some
  ens match result { Some(v) => v == parse_be(a) + parse_be(b), None => true }
  fx  pure
{
  match parse_u64(a) {
    Some(x) => match parse_u64(b) { Some(y) => Some(x + y), None => None },
    None => None,
  }
}
```

`forge check calc.th` certifies `add` at **L3**: a valid, in-range pair of digit
strings parses to `Some(parse_be(a) + parse_be(b))` — the sum is PINNED (a `None`
or a wrong-sum mutant violates the `ens`). The nested-`match` composition over the
two `parse_u64` calls certifies cleanly. The arithmetic core `add_vals`/`add_2_3`/
`add_100_200` also certify L3.

## Run it (the arithmetic core)

```bash
cargo run -p forge -- build examples/calculator/calc.th --entry add_2_3   # see the gap below first
```

The arithmetic core RUNS:

```
add_2_3()     = Some(5)      # 2 + 3
add_100_200() = Some(300)    # 100 + 200
```

## FORCING-FUNCTION FINDING — the string-parse front-end cannot `forge build`

`forge build calc.th` (the full program, including `add`) **fails to compile**:

```
error[E0425]: cannot find function `all_digits` in this scope
error[E0425]: cannot find function `parse_be` in this scope
error[E0425]: cannot find function `parse_u64` in this scope
```

This is a real gap, NOT a defect in the program (it certifies L3 correctly). The
cause: `forge build` lowers EVERY function to its always-active runtime
`thermite_check!`, and `add`'s contract names the **C7 spec fns** `all_digits` /
`parse_be` and its body calls the free `parse_u64` — but `thermite-lower`'s
`emit_string_runtime_l1` emits an **L1 (runtime / build) runnable form ONLY for
cluster C4**'s `parse_be` / `parse_le` / `pow10` / `u64_to_string` (the formatter).
The C7 parse spec fns have **no L1 emission**, so the runtime check cannot resolve
them. This belongs to the **C7 / #95 build-side cluster** (the L1 mirror of the C7
spec fns), not the calculator. When that L1 lowering lands, `calc.th` builds + runs
the full string-parse path end-to-end ("2"+"3" → `Some(5)`).

Until then, the calculator's add-and-return-`Some(sum)` core composes and runs (the
arithmetic core above); the digit-STRING parse at RUNTIME awaits the C7 L1 form.

## Verification

`forge/tests/acceptance_programs.rs`: `calculator_sum_contract_certifies_l3`
(forge check → L3 for `add` + the core), `calculator_arithmetic_core_builds_and_runs`
(the core RUNS → `Some(5)`, `Some(300)`), and
`calculator_string_parse_build_is_blocked_by_missing_l1_parse_u64` (PINS the gap;
flips to assert end-to-end build when the C7 L1 lowering lands).
