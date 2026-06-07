#!/usr/bin/env bash
# Build the MAX-VERIFIED text editor and run it INTERACTIVELY.
#
# The editor reads one keystroke (raw byte) at a time and self-sets RAW mode via
# its own extern-C termios boundary (`raw_mode_on`/`raw_mode_off`) — no `stty`
# needed; the binary clears ICANON/ECHO on entry and restores the terminal on exit.
#
# Keymap (the L3-proven `decode`) — MULTI-LINE (#125):
#   printable keys (space..~)  -> insert the char at the cursor (the frame is
#                                 redrawn after every keystroke by the L3 render_frame)
#   Enter (CR/LF)              -> insert a newline (the cursor drops to the next line)
#   UP / DOWN arrow            -> move the cursor to the same column on the prev/next
#                                 line (ESC [ A / ESC [ B; the L3 move_up/move_down)
#   LEFT / RIGHT arrow         -> move the cursor one byte (ESC [ D / ESC [ C)
#   Backspace / DEL (0x7f)     -> delete the char before the cursor
#   Ctrl-S (0x13)              -> save the buffer to the file (write_file)
#   Ctrl-Q (0x11)              -> quit (restores the terminal)
#
# Honest note: the editor LOADS the buffer from a fixed file on start (read_file;
# THERMITE_EDITOR_FILE, else /tmp/thermite_editor.txt), clears the screen and
# positions the cursor by ROW/COLUMN each frame (the C1 ANSI escapes + the verified
# cursor_row/cursor_col in render_frame). The DISPLAY logic (render_frame), the INPUT
# logic (decode), the EDIT logic (insert/backspace/cursor bounds), AND the multi-line
# NAVIGATION + LAYOUT logic (cursor_row/cursor_col/move_up/move_down) are all PROVEN
# at L3; only the raw read/write/ioctl/open SYSCALLS are the trusted boundary. For a
# clean, deterministic view, use the piped form (README.md):
#   SAVE=/tmp/thermite_editor.txt
#   printf 'ab\rcd\x1b[A\x13\x11' | THERMITE_EDITOR_FILE="$SAVE" <binary>
#       # "ab", Enter, "cd", UP arrow (row 1), Ctrl-S (save -> ab\ncd), Ctrl-Q
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
