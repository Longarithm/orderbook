#!/usr/bin/env bash
set -euo pipefail

CONTRACT_ACC=${CONTRACT_ACC:-gloomyswamp.testnet}
BASE_TOKEN=${BASE_TOKEN:-frog.gloomyswamp.testnet}
QUOTE_TOKEN=${QUOTE_TOKEN:-toad.gloomyswamp.testnet}

# Build and deploy using cargo-near (non-reproducible wasm), then initialize
cargo near deploy build-non-reproducible-wasm "$CONTRACT_ACC"
near call "$CONTRACT_ACC" new '{"base_token_id":"'$BASE_TOKEN'","quote_token_id":"'$QUOTE_TOKEN'"}' --accountId "$CONTRACT_ACC"

echo "Deployed to $CONTRACT_ACC with base=$BASE_TOKEN quote=$QUOTE_TOKEN"
