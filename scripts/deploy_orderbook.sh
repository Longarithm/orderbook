#!/usr/bin/env bash
set -euo pipefail

CONTRACT_ACC=${CONTRACT_ACC:-gloomyswamp.testnet}
BASE_TOKEN=${BASE_TOKEN:-frog.gloomyswamp.testnet}
QUOTE_TOKEN=${QUOTE_TOKEN:-toad.gloomyswamp.testnet}
WASM_PATH=${WASM_PATH:-./target/wasm32-unknown-unknown/release/orderbook.wasm}

if [ ! -f "$WASM_PATH" ]; then
  echo "WASM not found at $WASM_PATH. Build first: bash contracts/orderbook/build.sh" >&2
  exit 1
fi

near deploy $CONTRACT_ACC $WASM_PATH
near call $CONTRACT_ACC new '{"base_token_id":"'$BASE_TOKEN'","quote_token_id":"'$QUOTE_TOKEN'"}' --accountId $CONTRACT_ACC --gas 100000000000000

echo "Deployed $WASM_PATH to $CONTRACT_ACC with base=$BASE_TOKEN quote=$QUOTE_TOKEN"
