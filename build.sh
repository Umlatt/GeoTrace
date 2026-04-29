#!/usr/bin/env bash
set -e

cd "$(dirname "$0")"

VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
TARGETS=(
    "aarch64-apple-darwin"
    "x86_64-apple-darwin"
)

echo "GeoTrace v${VERSION} — installing targets..."
for TARGET in "${TARGETS[@]}"; do
    rustup target add "$TARGET" 2>/dev/null
done
echo ""

for TARGET in "${TARGETS[@]}"; do
    echo "Building ${TARGET}..."
    cargo build --release --target "$TARGET"
    OUT="geotrace-${VERSION}-${TARGET}"
    cp "target/${TARGET}/release/geotrace" "$OUT"
    echo "  -> ${OUT}"
done

echo ""
echo "Done."
