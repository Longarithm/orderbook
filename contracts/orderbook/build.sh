#!/usr/bin/env bash
set -euo pipefail

rustup target add wasm32-unknown-unknown || true
cargo build -p orderbook --target wasm32-unknown-unknown --release

OUT=./target/wasm32-unknown-unknown/release/orderbook.wasm
echo "Built $OUT"
