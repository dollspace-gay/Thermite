# Verified line / CSV parser — acceptance program 3 of 3 (#103)

`parse_lines.th` composes shipped verified primitives — cluster **C5**'s `split`
(`String` → `Vec<String>` on a separator byte, `.design/basis/07-strings.md`
REQ-15) over cluster **C6**'s non-`Copy` `Vec<String>` wrapper (`TVecTString`),
plus C5's `contains` substring predicate (REQ-13). No new toolchain feature is
needed to CERTIFY.

## What is proven

```thermite
fn fields(s: String, sep: u64) -> Vec<String>
  req true
  ens result.len() == 1 + count_sep(s, sep)    // the EXACT piece count
  fx  alloc
{ s.split(sep) }

fn has_sep(s: &String, sep: &String) -> bool
  req true
  ens result == contains_sub(s, sep)            // the substring relation
  fx  pure
{ s.contains(sep) }
```

- `has_sep` certifies **L3** through the FULL `forge check` §7-mutation-scored
  ladder (the substring predicate is real teeth — a broken `contains` fails).
- `fields` (the split count-bound) certifies **L3 under real verus** on the
  lowering (GROUNDED `7 verified, 0 errors`, REQ-15). Its thin `{ s.split(sep) }`
  body delegates entirely to the proven `split` method, so `forge check`'s §7 gate
  cannot mutation-score it (no scoreable body mutant — the documented split-caller
  precedent in `forge/tests/string_search_conformance.rs`); its L3 is established by
  running verus on the emitted Verus source.

## Run it (the split core)

```bash
cargo run -p forge -- build examples/parser/parse_lines.th --entry split_abc  # see the gap below first
```

The split core RUNS — "a,b,c" split on ',' (byte 44) → 3 pieces:

```
split_abc() = TVecTString { data: [TString { data: [97] }, TString { data: [98] }, TString { data: [99] }] }
#                                              "a"=97              "b"=98              "c"=99   → 3 pieces
```

The `split_abc` entry's `ens result.len() >= 1` is a non-vacuous floor (every split
yields at least one piece) that lowers to an L1 runtime check WITHOUT naming a C5
spec fn, so the runnable binary compiles.

## FORCING-FUNCTION FINDING — the count-bound contracts cannot `forge build`

`forge build parse_lines.th` (the full program) **fails to compile**:

```
error[E0425]: cannot find function `count_sep` in this scope
error[E0425]: cannot find function `contains_sub` in this scope
```

Same class as the calculator's gap: `forge build` lowers every function's contract
to a runtime `thermite_check!`, and `fields`/`has_sep` name the **C5 spec fns**
`count_sep` / `contains_sub`, which `thermite-lower`'s `emit_string_runtime_l1`
does **not** emit an L1 runnable form for (only C4's numfmt spec fns got one). This
belongs to the **C5 / #102 build-side cluster** (the L1 mirror of the C5 contract
spec fns `count_sep` / `sep_free` / `occurs_at` / `contains_sub`). When that L1
lowering lands, `parse_lines.th` builds + runs end-to-end with the count-bound
contract enforced at runtime. The C5 split/contains METHODS already have L1 forms
(the split core above runs); only the contract SPEC fns lack one.

## Verification

`forge/tests/acceptance_programs.rs`: `parser_contains_predicate_certifies_l3`
(forge check → L3 for `has_sep`), `parser_split_count_bound_verifies_under_real_verus`
(verus → L3 for `fields`), `parser_split_core_builds_and_runs_three_pieces` (the
split core RUNS → 3 pieces), and `parser_build_is_blocked_by_missing_l1_count_sep`
(PINS the gap; flips to assert end-to-end build when the C5 L1 lowering lands).
