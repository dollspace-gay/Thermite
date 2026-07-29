# Verified terminal editor

[`editor.th`](editor.th) is a small multi-line terminal editor. Thermite proves
the buffer operations, navigation, cursor layout, frame construction, and key
decoding. File and terminal operations remain at the L1 boundary.

## Assurance boundary

The project-level assurance is L1 because a project's level is the minimum of
its functions. Most editor logic reaches L3; syscall wrappers and the
non-terminating event loop remain L1.

### L3 logic

| Item | Guarantee |
|---|---|
| `Buffer` | `cursor <= text.len()` and `text.len() <= 1_000_000` |
| `insert_str` | Inserts at the cursor and advances it by `ins.len()` |
| `backspace` | Deletes the preceding byte and moves the cursor back one |
| `move_left` / `move_right` | Moves one byte while preserving the text and cursor bound |
| `count_nl` | Counts newlines in a bounded range and terminates by `end - i` |
| `line_start` / `line_end` | Finds the bounds of the line containing a position |
| `cursor_row` / `cursor_col` | Computes zero-based display coordinates |
| `move_up` / `move_down` | Preserves the text and clamps the cursor to the target line |
| `to_1based` | Converts terminal coordinates with `result == x + 1` |
| `render_frame` | Includes the buffer text and positions the terminal cursor |
| `decode` | Maps printable bytes, control keys, and arrow sequences to editor commands |

These functions are total and pass the mutation battery. The decimal formatter's
20-digit upper bound is used to prove that cursor-coordinate concatenation stays
within the bounded string capacity.

### L1 boundary

| Item | Responsibility |
|---|---|
| `raw_mode_on` / `raw_mode_off` | Enter and restore terminal raw mode through `tcgetattr` and `tcsetattr` |
| `read_key_raw` | Read and pack a key sequence for `decode` |
| `write_frame` | Write and flush a rendered frame |
| `read_file` / `write_file` | Load and save the configured file |
| `run` | Drive the `fx diverge` event loop under runtime contract checks |

The terminal wrappers return status values instead of panicking when stdin is
not a TTY. A missing input file produces an empty buffer. `run` is partial
correctness at L1 because the event loop is intentionally non-terminating.

## Check it

```sh
cargo run -q -p forge -- check examples/editor/editor.th
```

Expect the pure editor functions at L3, boundary functions at L1, and `run` at
L1. The resulting project level is L1.

## Build and run

```sh
cargo run -q -p forge -- build examples/editor/editor.th --entry run --out ./nano
THERMITE_EDITOR_FILE=mydoc.txt ./nano
```

The editor sets and restores raw mode itself. No `stty` wrapper is required.

| Key | Action |
|---|---|
| Printable byte | Insert at the cursor |
| Enter | Insert `"\n"` |
| Up / Down | Move to the previous or next line, preserving the column when possible |
| Left / Right | Move one byte |
| Backspace | Delete the byte before the cursor |
| Ctrl-S | Save |
| Ctrl-Q | Quit and restore the terminal |

You can also run a deterministic piped session:

```sh
printf 'ab\x1b[DX\x7f\x11' | ./nano
```

That sequence inserts `ab`, moves left, inserts and deletes `X`, then quits.

For a multi-line save:

```sh
SAVE=/tmp/thermite_editor.txt
printf 'ab\rcd\x1b[A\x13\x11' |
  THERMITE_EDITOR_FILE="$SAVE" ./nano
cat "$SAVE"
```

The saved content is `ab\ncd`.

## Sandbox

The binary runs under the default seccomp filter. The terminal boundaries
declare `fx term`, which adds `ioctl`; file effects add the required `read`,
`write`, and `openat` calls. Programs without `fx term` do not receive `ioctl`.
Classic seccomp-BPF cannot filter the `ioctl` command argument, so the grant is
syscall-wide.

`forge/tests/editor_runs.rs` checks certification, builds the editor, drives the
piped session, verifies the rendered splice and backspace behavior, and checks a
clean exit.
