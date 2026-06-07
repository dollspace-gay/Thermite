#!/usr/bin/env bash
# Build the MAX-VERIFIED text editor and run it INTERACTIVELY.
#
# The editor reads one keystroke (raw byte) at a time and self-sets RAW mode via
# its own extern-C termios boundary (`raw_mode_on`/`raw_mode_off`) — no `stty`
# needed; the binary clears ICANON/ECHO on entry and restores the terminal on exit.
#
# Keymap (the L3-proven `decode`):
#   printable keys (space..~)  -> insert the char at the cursor (the frame is
#                                 redrawn after every keystroke by the L3 render_frame)
#   LEFT / RIGHT arrow         -> move the cursor (ESC [ D / ESC [ C)
#   Backspace / DEL (0x7f)     -> delete the char before the cursor
#   Ctrl-Q (0x11)              -> quit (restores the terminal)
#
# Honest note: the editor clears the screen and positions the cursor each frame
# (the C1 ANSI escapes in render_frame). The DISPLAY logic (render_frame), the
# INPUT logic (decode), and the EDIT logic (insert/backspace/cursor bounds) are all
# PROVEN at L3; only the raw read/write/ioctl SYSCALLS are the trusted boundary. For
# a clean, deterministic view of the buffer evolving, use the piped form (README.md):
#   printf 'ab\x1b[DX\x7f\x11' | <binary>   # a,b, LEFT, X (splice -> aXb), bksp, Ctrl-Q
#
# --no-sandbox: the termios boundary issues `ioctl` (16), which the v0.1
# `write(output)` seccomp set does not yet grant, so we build WITHOUT the sandbox
# (see README.md). The compile path and the proof are identical either way.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

echo "Building the verified editor (forge build --entry run --no-sandbox)..." >&2
bin="$(cargo run -q -p forge -- build examples/editor/editor.th --entry run --no-sandbox 2>&1 \
        | grep -oE '/tmp/[^ ]+editor_build' | head -1)"
if [ -z "${bin:-}" ] || [ ! -x "$bin" ]; then
  echo "build failed (no binary produced)" >&2
  exit 1
fi

echo "Editor ready. Type to insert; arrows move; Backspace deletes; Ctrl-Q quits." >&2
echo "The binary self-sets raw mode (extern-C termios) — no stty needed." >&2
echo "----------------------------------------------------------------" >&2

# The binary self-sets raw mode via its own termios boundary; just run it. On a
# non-TTY (piped) stdin the editor's raw_mode_on returns gracefully (no crash) and
# reads the piped bytes directly.
"$bin" || true
echo
