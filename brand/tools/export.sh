#!/bin/sh
# Export PNG/ICO assets from the SVG sources and regenerate the Tauri app icons.
# Needs Inkscape (SVG -> PNG), ImageMagick (ICO) and the repo's node_modules (tauri CLI).
set -e
REPO=$(cd "$(dirname "$0")/../.." && pwd)
B=$REPO/brand
png() { inkscape "$1" -o "$2" -w "$3" -h "${4:-$3}" 2>/dev/null; }

# web: favicons and touch icons
png "$B/app-icon-small.svg" "$B/favicon/favicon-16.png" 16
png "$B/app-icon-small.svg" "$B/favicon/favicon-32.png" 32
png "$B/app-icon-small.svg" "$B/favicon/favicon-48.png" 48
magick "$B/favicon/favicon-16.png" "$B/favicon/favicon-32.png" "$B/favicon/favicon-48.png" "$B/favicon/favicon.ico"
rm "$B/favicon/favicon-48.png"
png "$B/app-icon-touch.svg" "$B/favicon/apple-touch-icon.png" 180
png "$B/app-icon-touch.svg" "$B/favicon/icon-maskable-512.png" 512
png "$B/app-icon.svg" "$B/favicon/icon-192.png" 192
png "$B/app-icon.svg" "$B/favicon/icon-512.png" 512

# social card
png "$B/social/og-card.svg" "$B/social/og-card.png" 1200 630

# desktop: Tauri icons from the master, then hand the tiny sizes to the small master
cd "$REPO"
npx tauri icon "$B/app-icon.svg" -o src-tauri/icons >/dev/null 2>&1
rm -rf src-tauri/icons/android src-tauri/icons/ios
T=$(mktemp -d "$REPO/.brand-export.XXXX")
for s in 16 24 32 48; do png "$B/app-icon-small.svg" "$T/$s.png" $s; done
for s in 64 128 256; do png "$B/app-icon.svg" "$T/$s.png" $s; done
cp "$T/32.png" src-tauri/icons/32x32.png
magick "$T/16.png" "$T/24.png" "$T/32.png" "$T/48.png" "$T/64.png" "$T/128.png" "$T/256.png" src-tauri/icons/icon.ico
rm -rf "$T"
