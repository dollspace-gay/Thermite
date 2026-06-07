# Verified `u64` decimal formatter — acceptance program 1 of 3 (#103)

`format.th` is a verified `u64` → decimal-`String` formatter, built ENTIRELY from
shipped verified primitives: cluster **C4**'s `n.to_string()`
(`.design/basis/07-strings.md` REQ-8) with its gold-standard round-trip contract.
No new toolchain feature — this is C4 composed into a real program.

## What is proven

```thermite
fn format(n: u64) -> String
  req true
  ens parse_be(result) == n        // THE ROUND-TRIP: the decimal bytes parse back to n
  ens result.len() >= 1            // and the string is non-empty
  fx  alloc
{ n.to_string() }
```

`forge check format.th` certifies `format` at **L3**: for ALL `n`, the produced
decimal byte sequence parses back to exactly `n` (`parse_be` is the MSB-first
read-order parse). This is the gold standard, not a floor — a formatter that
emitted a wrong digit would produce bytes that do not parse back to `n`, so the
`ens` is real teeth (non-vacuity is pinned in
`forge/tests/string_format_conformance.rs`).

## Run it

```bash
cargo run -p forge -- build examples/formatter/format.th --entry format_42
# → format_42() = TString { data: [52, 50] }          == "42"
cargo run -p forge -- build examples/formatter/format.th --entry format_0
# → format_0() = TString { data: [48] }               == "0"
cargo run -p forge -- build examples/formatter/format.th --entry format_1000000
# → format_1000000() = TString { data: [49, 48, 48, 48, 48, 48, 48] }  == "1000000"
```

`forge build --entry <fn>` lowers to runtime-checked Rust, compiles with `rustc`,
and runs the binary, printing the result's bytes. v1's `to_string` builds the
digits LSB-first then reverses to the human-readable MSB-first display order
(REQ-8 / blocker #96), so the bytes read left-to-right as the decimal: `52`='4',
`50`='2', `48`='0', `49`='1'.

The entry points (`format_42` etc.) are zero-argument wrappers that fix `n`
internally — the v0.1 deterministic runner synthesizes no `String`/`u64` argument
for an entry (the `editor.th` `run` precedent), so an entry takes no parameters and
builds its inputs in the body.

## Verification

Grounded by `forge/tests/acceptance_programs.rs`:
`formatter_round_trip_certifies_l3` (forge check → L3) and
`formatter_builds_and_runs_each_value` (build + run 42/0/1000000). The formatter
composes CLEANLY end-to-end — both `forge check` and `forge build` succeed.
