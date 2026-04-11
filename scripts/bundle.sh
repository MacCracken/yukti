#!/usr/bin/env bash
# Bundle yukti into a single dist/yukti.cyr for stdlib distribution.
# Strips include statements — consumers provide their own stdlib.
set -euo pipefail

REPO="$(cd "$(dirname "$0")/.." && pwd)"
VERSION=$(cat "$REPO/VERSION" | tr -d '[:space:]')
OUT="$REPO/dist/yukti.cyr"

mkdir -p "$REPO/dist"

echo "Bundling yukti v${VERSION} -> dist/yukti.cyr"

cat > "$OUT" << HEADER
# yukti.cyr — device abstraction for AGNOS
# Bundled distribution of yukti v${VERSION}
# Source: https://github.com/MacCracken/yukti
# License: GPL-3.0-only
#
# Usage: include "lib/yukti.cyr"
# Init:  alloc_init();
#
# Requires stdlib: syscalls, string, alloc, str, fmt, vec, hashmap, io, fs,
#                  tagged, process, fnptr, sakshi, chrono

HEADER

# Append each module in dependency order (matching src/lib.cyr)
for mod in error device event storage optical udev linux udev_rules partition device_db network; do
    echo "" >> "$OUT"
    echo "# --- ${mod}.cyr ---" >> "$OUT"
    # Strip any include lines from individual modules
    grep -v "^include " "$REPO/src/${mod}.cyr" >> "$OUT"
done

# Strip consecutive blank lines (lint requires single blanks only)
awk 'NF{blank=0} !NF{blank++} blank<=1' "$OUT" > "${OUT}.tmp"
mv "${OUT}.tmp" "$OUT"

LINES=$(wc -l < "$OUT")
BYTES=$(wc -c < "$OUT")
echo "Done: ${LINES} lines, ${BYTES} bytes"
