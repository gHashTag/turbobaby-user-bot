#!/usr/bin/env bash
# Bundle ALL project assets into a single transferable archive (for backup /
# migrating the shop to another server or repo). The archive is written to
# exports/ (gitignored — never bloats the repo) with a manifest + checksum.
#
# Usage:  ./scripts/export-assets.sh
# Output: exports/woody-assets-<UTCdate>.tar.gz  (+ .sha256, + MANIFEST.txt)
#
# What's included:
#   assets/   — logo, game sprites (1..14.png), packs, icons, member-cards,
#               product images, AND assets/uploads/ (admin-uploaded photos/videos)
#   styles/   — all CSS
#
# NOTE: product photos/videos uploaded AFTER S3 was enabled live in the S3
# bucket (S3_PUBLIC_URL), NOT in this repo — back those up from the bucket
# separately. assets/uploads/ here covers the local-stored ones.
set -euo pipefail
cd "$(dirname "$0")/.."

OUT_DIR="exports"
mkdir -p "$OUT_DIR"

# Deterministic UTC stamp (no Math.random / locale surprises).
STAMP="$(date -u +%Y%m%d-%H%M%S)"
ARCHIVE="$OUT_DIR/woody-assets-${STAMP}.tar.gz"
MANIFEST="$OUT_DIR/MANIFEST-${STAMP}.txt"

echo "▶ collecting assets…"
{
  echo "Woody Weed Bot — asset export"
  echo "Created (UTC): $STAMP"
  echo "Git commit:    $(git rev-parse --short HEAD 2>/dev/null || echo n/a)"
  echo ""
  echo "Directories:"
  du -sh assets styles 2>/dev/null || true
  echo ""
  echo "File count: $(find assets styles -type f | wc -l | tr -d ' ')"
  echo ""
  echo "Files:"
  find assets styles -type f | sort
} > "$MANIFEST"

tar -czf "$ARCHIVE" assets styles
( command -v shasum >/dev/null && shasum -a 256 "$ARCHIVE" > "${ARCHIVE}.sha256" ) || \
  ( command -v sha256sum >/dev/null && sha256sum "$ARCHIVE" > "${ARCHIVE}.sha256" ) || true

SIZE="$(du -sh "$ARCHIVE" | cut -f1)"
echo "✅ assets bundled → $ARCHIVE ($SIZE)"
echo "   manifest      → $MANIFEST"
echo "   checksum      → ${ARCHIVE}.sha256"
echo ""
echo "To restore on a new machine: tar -xzf $(basename "$ARCHIVE") -C <repo-root>"
