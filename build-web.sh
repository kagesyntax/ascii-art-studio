#!/usr/bin/env bash
set -euo pipefail

TARGET="wasm32-unknown-unknown"
PROFILE="${1:-release}"
OUTDIR="web/pkg"

if [ "$PROFILE" = "release" ]; then
  PROFILE_DIR="release"
  FLAGS="--release"
else
  PROFILE_DIR="debug"
  FLAGS=""
fi

echo "Building WASM binary ($PROFILE)..."
cargo build --target "$TARGET" --lib $FLAGS

echo "Generating JS bindings with wasm-bindgen..."
wasm-bindgen --target web --out-dir "$OUTDIR" "target/$TARGET/$PROFILE_DIR/xdcipher.wasm"

echo "Done! Build artifacts in $OUTDIR/"
ls -lh "$OUTDIR/"
