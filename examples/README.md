# Thermite examples

This directory contains four programs built from Thermite's verified primitive
library.

| Program | Verified behavior | Runnable entry |
|---|---|---|
| [Editor](editor/) | Buffer edits, line navigation, cursor layout, frame rendering, and key decoding at L3 | `run` |
| [Formatter](formatter/) | `u64` decimal formatting with `parse_be(to_string(n)) == n` at L3 | `format_42` |
| [Calculator](calculator/) | Parsing two decimal strings and returning their sum at L3 | `add_2_3` |
| [Parser](parser/) | String splitting and separator detection at L3 | `split_abc` |

The calculator and parser have a current build limitation: their complete
contracts refer to specification helpers that do not yet have L1 runtime forms.
Their zero-argument demonstration entries still build and run. The individual
READMEs explain the boundary.

## Check a program

```sh
cargo run -q -p forge -- check examples/calculator/calc.th
```

`forge check` runs the verification ladder and the contract-quality battery. An
L3 result means Verus discharged every obligation and the contract met the
mutation-score requirement.

## Build the editor

```sh
cargo run -q -p forge -- build examples/editor/editor.th --entry run --out ./nano
THERMITE_EDITOR_FILE=mydoc.txt ./nano
```

The editor sets terminal raw mode itself and runs under the default seccomp
filter. Its declared `fx` effects permit the required file and terminal calls.

Keys: type to insert, Enter for a newline, arrows to move, Backspace to delete,
Ctrl-S to save, and Ctrl-Q to quit.

## Build the smaller demos

```sh
# 42 -> "42"
cargo run -q -p forge -- build examples/formatter/format.th --entry format_42 --out ./fmt
./fmt

# 2 + 3 -> Some(5)
cargo run -q -p forge -- build examples/calculator/calc.th --entry add_2_3 --out ./calc
./calc

# "a,b,c" -> three fields
cargo run -q -p forge -- build examples/parser/parse_lines.th --entry split_abc --out ./parse
./parse
```

Other demo entries include `format_0`, `format_1000000`, and `add_100_200`.
