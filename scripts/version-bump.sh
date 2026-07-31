#!/usr/bin/env bash
set -euo pipefail
[ $# -ne 1 ] && echo "Usage: $0 <version>" && exit 1
NEW_VERSION="$1"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# cyrius.cyml carries version = "${file:VERSION}" — VERSION is the only file to write.
echo "$NEW_VERSION" > "$REPO_ROOT/VERSION"
grep -q "^## \[${NEW_VERSION}\]" "$REPO_ROOT/CHANGELOG.md" \
  || echo "WARN: no '## [${NEW_VERSION}]' heading in CHANGELOG.md — release notes will be empty."
echo "Bumped to ${NEW_VERSION}. Tag and push."
