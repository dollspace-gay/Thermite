#!/usr/bin/env bash
# Build the Stage 4 SAT/LRAT tools from their frozen upstream revisions.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=g4-toolchain.env
source "$ROOT/scripts/g4-toolchain.env"

for tool in git make cc c++; do
  command -v "$tool" >/dev/null 2>&1 || {
    echo "install-g4-tools: required build tool not found: $tool" >&2
    exit 2
  }
done

DEST="$ROOT/target/g4-tools"
if [[ -x "$DEST/bin/cadical" && -x "$DEST/bin/drat-trim" &&
      "$("$DEST/bin/cadical" --version)" == "$CADICAL_VERSION" &&
      "$("$DEST/bin/drat-trim" --thermite-version)" == "drat-trim $DRAT_TRIM_REV" ]]; then
  echo "Stage 4 toolchain already installed in $DEST"
  exit 0
fi

WORK="$(mktemp -d)"
cleanup() {
  rm -rf "$WORK"
}
trap cleanup EXIT

git clone --quiet --no-checkout https://github.com/arminbiere/cadical.git "$WORK/cadical"
git -C "$WORK/cadical" checkout --quiet --detach "$CADICAL_REV"
(
  cd "$WORK/cadical"
  ./configure --quiet
  make -j2
)

git clone --quiet --no-checkout https://github.com/marijnheule/drat-trim.git "$WORK/drat-trim"
git -C "$WORK/drat-trim" checkout --quiet --detach "$DRAT_TRIM_REV"
cc -D_GNU_SOURCE -std=c99 -O2 \
  "$WORK/drat-trim/drat-trim.c" \
  -o "$WORK/drat-trim/drat-trim"

mkdir -p "$DEST/bin" "$DEST/libexec"
install -m 0755 "$WORK/cadical/build/cadical" "$DEST/bin/cadical"
install -m 0755 "$WORK/drat-trim/drat-trim" "$DEST/libexec/drat-trim.real"
install -m 0755 "$ROOT/scripts/g4-tools/drat-trim" "$DEST/bin/drat-trim"

test "$("$DEST/bin/cadical" --version)" = "$CADICAL_VERSION"
test "$("$DEST/bin/drat-trim" --thermite-version)" = "drat-trim $DRAT_TRIM_REV"
echo "Installed pinned Stage 4 toolchain in $DEST"
