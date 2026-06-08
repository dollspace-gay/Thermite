# Thermite example programs

Four runnable Thermite programs that prove the verified-primitive basis (C1–C9)
**composes into real programs that run** — each `forge check`s at **L3** (its
bug-prone logic proven for all inputs by real Verus) *and* `forge build`s into a
native binary you run directly. Only the raw syscalls at the I/O seam are trusted.

The pattern is always the same: **`forge build … --entry <fn> --out <name>`** drops
a standalone binary at `<name>`; then run `./<name>`. (`forge build` lowers the
program to executable Rust + the always-active runtime contract checks and shells
out to `rustc`; `--out` names the artifact so no wrapper script is needed.)

| program | what it proves | `forge check` |
|---|---|---|
| [`editor/`](editor/) | a **multi-line interactive text editor** — editing, line nav, cursor layout, keystroke decode all L3; only `read`/`write`/`ioctl`/`open` trusted | edit+nav+layout core L3, shell L1 |
| [`formatter/`](formatter/) | `u64` → decimal `String`, the digit round-trip `parse_be(to_string(n)) == n` proven | `format_*` L3 |
| [`calculator/`](calculator/) | parse two digit-strings + add; the sum is pinned, parse refuses non-digits (loud `None`) | `add_*` L3 |
| [`parser/`](parser/) | split a string on a separator into a `Vec<String>`; the field-count bound proven | `has_key`/`fields` L3 |

## The editor (the keystone — interactive)

```sh
# build the standalone editor binary (one time):
cargo run -q -p forge -- build examples/editor/editor.th --entry run --out ./nano

# run it directly — it self-sets raw mode (extern-C termios; no stty, no script):
THERMITE_EDITOR_FILE=mydoc.txt ./nano
```

Keys: type to insert · **Enter** newline · **↑↓** move between lines · **←→** within
a line · **Backspace** delete · **Ctrl-S** save · **Ctrl-Q** quit. See
[`editor/README.md`](editor/README.md) for the verified-vs-trusted breakdown.
It runs **under the default seccomp sandbox** — its `raw_mode_on`/`raw_mode_off`
boundaries declare `fx term`, whose `fx`-derived seccomp widening grants exactly the
terminal `ioctl` the termios raw mode needs (crosslink #106), so the binary is
effect-confined, not unsandboxed.

## The formatter / calculator / parser (runnable demos)

These have zero-argument demo entries `forge build` can run directly:

```sh
# formatter: 42 -> "42"
cargo run -q -p forge -- build examples/formatter/format.th --entry format_42 --out ./fmt && ./fmt

# calculator: 2 + 3 -> Some(5)
cargo run -q -p forge -- build examples/calculator/calc.th --entry add_2_3 --out ./calc && ./calc

# parser: "a,b,c" split on ',' -> 3 fields
cargo run -q -p forge -- build examples/parser/parse_lines.th --entry split_abc --out ./parse && ./parse
```

Other demo entries: `format_0` / `format_1000000`; `add_100_200`.

## Check the proof (any program)

```sh
# certify: per-item L3/L2/L1 certificate (real Verus), no binary produced
cargo run -q -p forge -- check examples/calculator/calc.th
```

`forge check` runs the verification ladder + the anti-Goodhart battery; an item is
**L3** only when real Verus discharges every obligation and the contract kills the
generated mutants. The `examples/*/README.md` files give the per-item proof tables.
