#!/usr/bin/env bash
# Regenerate every raster asset in static/ from the two SVG sources here.
#
# Run after editing mark.svg or og.svg (og.svg has the domain baked into the
# image, so it needs a re-run whenever SITE.domain changes):
#
#   ./design/generate.sh
#
# Requires: rsvg-convert (brew install librsvg) and python3 with Pillow.
set -euo pipefail

cd "$(dirname "$0")/.."
SRC=design
OUT=static
mkdir -p "$OUT/icons"

# favicons + PWA icons
for s in 16 32 192 512; do
  rsvg-convert -w "$s" -h "$s" "$SRC/mark.svg" -o "$OUT/icons/icon-$s.png"
done

rsvg-convert -w 180 -h 180 "$SRC/mark.svg" -o "$OUT/apple-touch-icon.png"
rsvg-convert -w 512 -h 512 "$SRC/mark.svg" -o "$OUT/logo.png"          # JSON-LD Organization logo
rsvg-convert -w 1200 -h 630 "$SRC/og.svg" -o "$OUT/og.png"             # social card

# multi-resolution favicon.ico
python3 -c "
from PIL import Image
Image.open('$OUT/icons/icon-512.png').save('$OUT/favicon.ico', sizes=[(16,16),(32,32),(48,48)])
"

# keep the inline SVG favicon in sync with the mark
cp "$SRC/mark.svg" src/lib/assets/favicon.svg

echo "regenerated: $OUT/{favicon.ico,apple-touch-icon.png,logo.png,og.png,icons/*} + src/lib/assets/favicon.svg"
