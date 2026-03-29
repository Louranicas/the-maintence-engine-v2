#!/usr/bin/env bash
# habitat-nexus build + deploy script
# Run from the repo root

set -euo pipefail

PLUGIN_DIR="$HOME/.config/zellij/plugins"
WASM_NAME="habitat-nexus.wasm"
CARGO_TARGET="wasm32-wasip1"

echo "=== Building habitat-nexus ==="
cd habitat-nexus
rustup target add "$CARGO_TARGET" 2>/dev/null || true
cargo build --release --target "$CARGO_TARGET"

echo "=== Deploying ==="
mkdir -p "$PLUGIN_DIR"
cp "target/$CARGO_TARGET/release/habitat_nexus.wasm" "$PLUGIN_DIR/$WASM_NAME"
echo "Deployed to $PLUGIN_DIR/$WASM_NAME"

echo "=== Reloading in active Zellij session ==="
zellij action start-or-reload-plugin \
  "file:$PLUGIN_DIR/$WASM_NAME" 2>/dev/null \
  && echo "Plugin reloaded" \
  || echo "(No active session — plugin will load on next session start)"
