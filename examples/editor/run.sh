#!/usr/bin/env bash
# Build the verified text editor and run it INTERACTIVELY.
#
# The editor reads one keystroke (raw byte) at a time. To feel interactive we
# put the terminal in raw mode (each key reaches the editor live, no line
# buffering, no echo), run the editor, then restore the terminal on exit.
#
# Keymap:
#   printable keys (space..~)  -> insert the char at the cursor (the buffer is
#                                 re-printed after every keystroke)
#   Backspace / DEL (0x7f)     -> delete the char before the cursor
#   Ctrl-Q (0x11)              -> quit
#
# Honest note: this is a single-line buffer that prints itself after each key
# (no screen clear), so the output accumulates down the screen. The EDIT LOGIC
# (insert / backspace / cursor bounds) is what is proven at L3; the terminal
# loop is the trusted boundary. For a clean, deterministic view of the buffer
# evolving, use the piped form instead (see README.md):
#   printf 'cat\x7fb\x11' | <binary>
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

echo "Building the verified editor (forge build --entry run)..." >&2
bin="$(cargo run -q -p forge -- build examples/editor/editor.th --entry run 2>&1 \
        | grep -oE '/tmp/[^ ]+editor_build' | head -1)"
if [ -z "${bin:-}" ] || [ ! -x "$bin" ]; then
  echo "build failed (no binary produced)" >&2
  exit 1
fi

echo "Editor ready. Type to insert; Backspace deletes; Ctrl-Q quits." >&2
echo "----------------------------------------------------------------" >&2

if [ -t 0 ]; then
  saved="$(stty -g)"
  stty raw -echo
  "$bin" || true
  stty "$saved"
  echo
else
  # not a TTY (piped) -> run directly, raw mode not applicable
  "$bin"
fi
