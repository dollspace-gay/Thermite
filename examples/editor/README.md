# A verified, runnable text editor (Thermite)

`editor.th` is the proof-of-the-pudding for Thermite (crosslink #83): a nano-like
text editor whose **bug-prone editing heart is mechanically PROVEN** and whose
terminal I/O is **honestly trusted** at the seam where proof meets the world. It
both `forge check`s (the proof) and `forge build`s + RUNS (the artifact).

## The honest split: verified vs trusted

Thermite does not pretend the terminal can be proven. The editor is split into two
clearly-labelled halves, and the certificate says exactly which is which.

### Verified edit core — `forge check` certifies **L3** (total + mutation-proven)

| item | what is proven |
|---|---|
| `Buffer` | the type invariant `cursor <= text.len() && text.len() <= 1_000_000` — the cursor NEVER points past the text, the text stays within the bounded-`String` cage (§4.2) |
| `insert_str` | the text grows by exactly `ins.len()` and the cursor advances by exactly `ins.len()` (so it still points within the new text) |
| `backspace` | the text shrinks by exactly one and the cursor steps back exactly one (`req cursor > 0` guarantees a byte to delete) |
| `move_left` | the cursor steps back exactly one; the text is unchanged |
| `move_right` | the cursor advances exactly one (`req cursor < text.len()` keeps it in bounds); the text is unchanged |

Each is a TOTAL function: its cursor math and buffer edits hold for ALL inputs, an
SMT proof discharges every obligation, and the §7 mutation battery confirms the
contract is strong (not a tautology a weak body could satisfy). **L3 = total
correctness.**

### Trusted runnable shell — **L1** (the honest seam to the world)

| item | level | why it is trusted, not proven |
|---|---|---|
| `read_key` | L1 boundary | `#[boundary("os::read_key")]` — a raw keystroke byte (or the EOF sentinel 256) from stdin; you cannot prove the terminal. The contract `ens result <= 256` is enforced at the crossing; the foreign body is trusted-by-fiat, seccomp-confined to the `read` syscall set. |
| `key_str` | L1 boundary | `#[boundary("os::key_str")]` — the host glue mapping a keystroke byte to a one-byte `String` (a capability the surface language lacks). `ens result.len() <= 1`. |
| `render` | L1 boundary | `#[boundary("os::print")]` — hands the buffer text to stdout. |
| `run` | L1 (partial correctness) | the `fx diverge` event loop. An event loop is **non-terminating by design**, so it cannot honestly claim L3 = TOTAL correctness. It caps at **L1 = partial correctness**: the loop runs under its always-active runtime contract checks, and the edit ops it calls (`insert_str` / `backspace`) are the L3-proven total functions its correctness rests on. The §7 mutation gate (which validates a strong-functional `ens`) is the wrong instrument for a partial-correctness loop, so it is exempt — `run`'s `ens result <= 256` is an honestly weak shape, NOT a gamed one. The cap claims LESS than L3, never more (the honesty gate, R-DEFER-9). |

The whole-project assurance is the **min over functions = L1** — capped by the
trusted terminal seam, exactly as it should be. The verified guarantee is
*to-the-boundary*: the editing logic is proven; the world it talks to is trusted.

## How to check it

```sh
forge check examples/editor/editor.th
```

Expect the edit core (`Buffer`/`insert_str`/`backspace`/`move_left`/`move_right`)
at **L3**, the boundary primitives (`read_key`/`key_str`/`render`) at **L1
boundary**, and `run` at **L1** (diverge / partial correctness — NOT an L0
`WeakContract` reject). Project assurance: **L1**.

## How to run it

```sh
forge build examples/editor/editor.th --entry run
# then run the produced binary, feeding keystrokes on stdin:
printf 'hi\x11' | <the-built-binary>
```

The keystrokes are `h`, `i`, then **Ctrl-Q** (byte `0x11` = 17 = quit). The editor
inserts `h` and `i` via the L3-proven `insert_str`, renders the buffer after each
keystroke, sees Ctrl-Q, and exits clean. The build emits L1 runtime checks
(every `req`/`ens`/`inv` is an always-active check that fires loudly on violation)
and installs a seccomp filter confining the process to the syscalls its effect row
declares (`read` / `write` / `alloc`).

The runnable session is grounded as a test: `forge/tests/editor_runs.rs` builds
the editor with `rustc`, runs it with the piped keystrokes above, and asserts it
exits clean and renders the edited buffer — alongside the cert-level checks
(edit core L3, `run` L1) and the diverge-only honesty regressions.
