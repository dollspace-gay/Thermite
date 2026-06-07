# A MAX-VERIFIED, runnable MULTI-LINE text editor (Thermite)

`editor.th` is the keystone proof-of-the-pudding for Thermite (crosslink #125,
builds on #90, ref #83): a nano-like **multi-line** text editor whose **bug-prone
logic — the editing heart, the line NAVIGATION + cursor LAYOUT math, the
display-frame construction, AND the keystroke decode — is mechanically PROVEN**,
with only the raw read/write/ioctl/open **syscalls** honestly trusted at the seam
where proof meets the world. It both `forge check`s (the proof) and `forge build`s +
RUNS (the artifact).

It is genuinely nano-like: **newlines (Enter), up/down line navigation, a
full-screen render that positions the cursor by ROW/COLUMN, and file load/save**.
The buffer is ONE `String` with `\n` bytes; the cursor is a byte offset, so the
shipped edit core (`insert_str`/`backspace`/`move_left`/`move_right`) works
unchanged over the multi-line text (a `\n` is just a byte). The ROW/COLUMN cursor
math and the UP/DOWN navigation are VERIFIED Thermite (L3) — the navigation +
layout logic is proven, not trusted glue.

## The thesis: verified vs trusted, pushed as far as the language allows

Thermite does not pretend the kernel can be proven — but it pushes the proof
boundary all the way down to the syscalls. The display logic and the input
interpretation, the parts that are actually bug-prone, are VERIFIED.

### Verified logic — `forge check` certifies **L3** (total + mutation-proven)

| item | what is proven |
|---|---|
| `Buffer` | the type invariant `cursor <= text.len() && text.len() <= 1_000_000` — the cursor NEVER points past the text, the text stays within the bounded-`String` cage (§4.2) |
| `insert_str` | the text grows by exactly `ins.len()` and the cursor advances by exactly `ins.len()` (so it still points within the new text) |
| `backspace` | the text shrinks by exactly one and the cursor steps back exactly one (`req cursor > 0` guarantees a byte to delete) |
| `move_left` | the cursor steps back exactly one; the text is unchanged |
| `move_right` | the cursor advances exactly one (`req cursor < text.len()` keeps it in bounds); the text is unchanged |
| `count_nl` | **NAV (#125)** — a verified recursive FORWARD scan counting the newline (byte 10) occurrences in `text[i..end]`; `ens result <= end - i` plus a boundary teeth clause (a newline at `i` forces `result >= 1`) make it non-vacuous, `dec end - i` proves termination. The row math. |
| `line_start` / `line_end` | **NAV (#125)** — verified forward scans for the start/end of the line a position sits on (`line_start` carries the running "index after the last newline" accumulator). The line-boundary math up/down navigation stands on. |
| `cursor_row` / `cursor_col` | **LAYOUT (#125)** — the cursor's 0-based ROW (newlines in `text[0..cursor]`, via `count_nl`) and COLUMN (`cursor − line_start`). `ens result <= b.cursor`; they feed the ANSI cursor positioning. Proven cursor layout, not trusted. |
| `move_up` / `move_down` | **NAV (#125)** — verified up/down line navigation: find the prev/next line boundaries, clamp the target column to that line's length (`min2`), set the new cursor. PROVEN: the text is unchanged AND the new cursor stays in bounds (`cursor <= text.len()` — the Buffer type invariant). |
| `to_1based` | **LAYOUT (#125)** — the 0→1-based ANSI coordinate conversion, `ens result == x + 1`. PROVEN exactly, so an off-by-one mutant is killed — the `+1` is verified, not an unverified literal in `render_frame`. |
| `render_frame` | **THE THESIS (multi-line, #125)** — the full ANSI display frame is built as a `String` (C1 escape literals + the buffer text [`\n` bytes render as terminal line breaks] by `concat` + the cursor coordinate `\x1b[<row+1>;<col+1>H` from the VERIFIED `cursor_row`/`cursor_col` via the C4 `.to_string()` and the proven `to_1based`), and `ens result.len() >= b.text.len()` PROVES the whole buffer text is carried into the frame (a dropped-text mutant shortens the frame and fails the `ens`). The display logic is PROVEN, not trusted glue. |
| `decode` | a PURE TOTAL function mapping the raw read bytes to a key code (printable → itself, DEL → backspace, **Enter (CR/LF) → insert-newline 1004**, **Ctrl-S → save 19**, Ctrl-Q → quit, the arrow escape sequences `ESC [ A..D` → synthetic codes 1000..1003). The keystroke INTERPRETATION is proven, not trusted. |

Each is a TOTAL function: its math holds for ALL inputs, an SMT proof discharges
every obligation, and the §7 mutation battery confirms the contract is strong (not
a tautology a weak body could satisfy). **L3 = total correctness.**

`render_frame` L3 rests on a real C4 strengthening: `u64_to_string`'s `ens` now
bounds the formatted decimal length `result.data.len() <= 20` (a u64 is < 10^20, so
at most 20 digits — PROVED, not assumed), so the bounded `concat`'s §4.2 cage
precondition discharges for any formatted-number concat.

### The minimal trusted syscall boundary — **L1** (the honest seam to the world)

| item | level | why it is trusted, not proven |
|---|---|---|
| `raw_mode_on` | L1 boundary | `#[boundary("os::raw_mode_on")]` — put the terminal in RAW mode (clear `ICANON`/`ECHO` so each keystroke reaches the editor live) via extern-C `tcgetattr`/`tcsetattr` (libc linked through std — self-contained, no `stty`). `ens result <= 1` (0 = ok, 1 = not-a-TTY/error). The binary self-sets raw mode; a non-TTY (piped) stdin is handled gracefully — `tcgetattr` returns ENOTTY, the wrapper returns 1, **no crash**. |
| `raw_mode_off` | L1 boundary | `#[boundary("os::raw_mode_off")]` — restore the saved original termios; runs on the quit path so the terminal is never left in raw mode. A clean no-op when raw mode was never entered. |
| `read_key_raw` | L1 boundary | `#[boundary("os::read_key_raw")]` — read one keystroke, returning the raw bytes PACKED into a u64 for `decode` (`b0` bits 0..9, `b1` 9..18, `b2` 18..27; an ESC reads the 2-byte arrow tail). `ens result <= 134_217_727` (the 27-bit packing width — an honest boundary bound). |
| `write_frame` | L1 boundary | `#[boundary("os::write_frame")]` — write the rendered frame `String` to stdout and flush. `ens result <= 1`. |
| `read_file` | L1 boundary | `#[boundary("os::read_file")]` — LOAD the initial buffer from the fixed demo file (`THERMITE_EDITOR_FILE` if set, else `/tmp/thermite_editor.txt`) via extern-C `std::fs::read`; the multi-line `\n` bytes are preserved. A missing file yields the EMPTY string (a fresh buffer) — the honest arm, no crash. `ens result.len() <= 1_000_000`. (#125) |
| `write_file` | L1 boundary | `#[boundary("os::write_file")]` — SAVE the buffer `String`'s bytes (incl. the `\n` line breaks) to the same fixed file on Ctrl-S, via `std::fs::write`. `ens result <= 1` (0 = ok, 1 = I/O error). (#125) |
| `run` | L1 (partial correctness) | the `fx diverge` event loop. An event loop is **non-terminating by design**, so it cannot honestly claim L3 = TOTAL correctness. It caps at **L1 = partial correctness**: the loop runs under its always-active runtime contract checks, and the logic it drives (`decode`, `render_frame`, `insert_str`/`backspace`/`move_left`/`move_right`) is the L3-proven core its correctness rests on. The §7 mutation gate is exempt for a diverge fn — `run`'s shape is honestly weak, NOT a gamed one (R-DEFER-9). |

The whole-project assurance is the **min over functions = L1** — capped by the
trusted syscall seam, exactly as it should be. The verified guarantee is
*to-the-boundary*: the display + input logic is PROVEN; only the raw syscalls are
trusted.

## How to check it

```sh
forge check examples/editor/editor.th
```

Expect the verified logic (`Buffer`/`insert_str`/`backspace`/`move_left`/
`move_right`/`render_frame`/`decode`) at **L3**, the syscall boundary
(`raw_mode_on`/`raw_mode_off`/`read_key_raw`/`write_frame`) at **L1 boundary**, and
`run` at **L1** (diverge / partial correctness — NOT an L0 `WeakContract` reject).
Project assurance: **L1**.

## How to run it

```sh
forge build examples/editor/editor.th --entry run --no-sandbox
# then run the produced binary, feeding keystrokes on stdin:
printf 'ab\x1b[DX\x7f\x11' | <the-built-binary>
```

The keystrokes are `a`, `b`, then a **LEFT arrow** (`ESC [ D` = `\x1b[D`, decode →
1003: cursor steps left between `a` and `b`), then `X` (the L3 `insert_str` SPLICES
mid-text → `aXb`), then **Backspace** (`0x7f`, deletes `X` → `ab`), then **Ctrl-Q**
(`0x11` = 17 = quit). The editor self-sets raw mode, decodes each keystroke with the
L3-proven `decode`, dispatches to the L3-proven edit ops, builds each frame with the
L3-proven `render_frame`, writes it, and exits clean on Ctrl-Q — restoring the
terminal on the way out.

### The MULTI-LINE keymap (#125)

```sh
SAVE=/tmp/thermite_editor.txt
# type "ab", ENTER (newline), "cd", UP arrow, Ctrl-S (save), Ctrl-Q (quit):
printf 'ab\rcd\x1b[A\x13\x11' | THERMITE_EDITOR_FILE="$SAVE" <the-built-binary>
cat "$SAVE"   # -> ab\ncd   (the multi-line buffer, saved with its newline)
```

| key | byte(s) | decode | action |
|---|---|---|---|
| printable (space..~) | 32..126 | itself | insert the char at the cursor |
| **Enter** | CR `\r` (13) or LF (10) | 1004 | insert `"\n"` (a new line — the cursor drops to row+1, col 1) |
| **UP** / **DOWN** arrow | `ESC [ A` / `ESC [ B` | 1000 / 1001 | move the cursor to the same column on the previous / next line (`move_up`/`move_down`) |
| **LEFT** / **RIGHT** arrow | `ESC [ D` / `ESC [ C` | 1003 / 1002 | move the cursor one byte (`move_left`/`move_right`) |
| Backspace / DEL | `0x7f` | 127 | delete the char before the cursor |
| **Ctrl-S** | `0x13` | 19 | **save** the buffer to the file (`write_file`) |
| Ctrl-Q | `0x11` | 17 | quit (restores the terminal) |

The buffer is LOADED from the file on start (`read_file`; an empty/missing file is a
fresh buffer). Each frame is the full-screen render: clear+home, the multi-line
buffer text (`\n` → line breaks), then the cursor positioned at
`\x1b[<row+1>;<col+1>H` from the VERIFIED `cursor_row`/`cursor_col`. A piped session
shows TWO lines and the cursor moving between them — e.g. after Enter the cursor is
at `\x1b[2;1H` (row 2), and after the UP arrow it is back at `\x1b[1;3H` (row 1).

**`--no-sandbox` note:** the termios boundary (`raw_mode_on`/`raw_mode_off`) issues
the `ioctl` syscall (16) via `tcgetattr`/`tcsetattr`. The v0.1 `write(output)`
seccomp set does not yet grant `ioctl`, so the sandboxed binary is SIGSYS-killed
before the wrapper's graceful non-TTY handling can run. Build with `--no-sandbox`
until the sandbox table grows a terminal-control entry (a separate `sandbox.rs` /
`runtime-sandbox.md` item). The compile path and the proof are identical either way.

The runnable session is grounded as a test: `forge/tests/editor_runs.rs` builds the
editor with `rustc`, runs it with the piped keystrokes above, and asserts the frames
show the mid-text splice (`aXb`) then the backspace undo (`ab`) and a clean exit —
alongside the cert-level checks (logic L3, boundary/`run` L1) and the diverge-only
honesty regressions.
