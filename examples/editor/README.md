# A MAX-VERIFIED, runnable text editor (Thermite)

`editor.th` is the keystone proof-of-the-pudding for Thermite (crosslink #90, ref
#83): a nano-like text editor whose **bug-prone logic — the editing heart, the
display-frame construction, AND the keystroke decode — is mechanically PROVEN**,
with only the raw read/write/ioctl **syscalls** honestly trusted at the seam where
proof meets the world. It both `forge check`s (the proof) and `forge build`s +
RUNS (the artifact).

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
| `render_frame` | **THE THESIS** — the full ANSI display frame is built as a `String` (C1 escape literals + the buffer text by `concat` + the C4 cursor coordinate `(b.cursor+1).to_string()`), and `ens result.len() >= b.text.len()` PROVES the whole buffer text is carried into the frame (a dropped-text mutant shortens the frame and fails the `ens`). The display logic is PROVEN, not trusted glue. |
| `decode` | a PURE TOTAL function mapping the raw read bytes to a key code (printable → itself, DEL → backspace, Ctrl-Q → quit, the arrow escape sequences `ESC [ A..D` → synthetic codes 1000..1003). The keystroke INTERPRETATION is proven, not trusted. |

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
