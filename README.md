# NEAR FT/FT Limit Orderbook Prototype

- Base token: `frog.gloomyswamp.testnet`
- Quote token: `toad.gloomyswamp.testnet`
- Orderbook contract target: `gloomyswamp.testnet`

## Build

```bash
bash contracts/orderbook/build.sh
```

WASM output: `contracts/orderbook/target/wasm32-unknown-unknown/release/orderbook.wasm`

## Deploy

```bash
bash scripts/deploy_orderbook.sh
# or override
CONTRACT_ACC=gloomyswamp.testnet BASE_TOKEN=frog.gloomyswamp.testnet QUOTE_TOKEN=toad.gloomyswamp.testnet bash scripts/deploy_orderbook.sh
```

## Usage (near-cli examples)

```bash
bash scripts/example_calls.sh
```

Key calls:
- Deposit via FT contracts using `ft_transfer_call` to the orderbook contract
- Place: `place_order(side, amount_base, max_spend_quote?, price_num, price_den)` attached deposit: 1 yocto
- Cancel: `cancel_order(order_id)` attached deposit: 1 yocto
- Execute: `execute(maker_order_id, taker_order_id, base_fill, quote_paid)` attached deposit: 1 yocto
- Withdraw: `withdraw(token_id, amount, receiver_id?, msg?)` attached deposit: 1 yocto

Views:
- `get_config()` -> `(base_token_id, quote_token_id)`
- `get_balance(account_id, token_id)` -> `U128`
- `get_order(order_id)` -> `Order | null`
- `get_orders(from_index, limit)` -> `Order[]`
- `get_orders_by_owner(owner_id)` -> `Order[]`

Price is represented as rational `price_num/price_den` (quote per 1 unit base). Amounts are in smallest token units.

## Off-chain matcher (prototype)

```bash
cd matcher
npm i
ORDERBOOK_CONTRACT_ID=gloomyswamp.testnet MATCHER_ACCOUNT_ID=gloomyswamp.testnet npm run dev
```

- Polls orders via `get_orders`, applies a simple crossing check, and submits `execute`.
- Set `DRY_RUN=1` to log matches without sending transactions.

## Notes

- Event logs are emitted with prefix `EVENT_JSON:` and standard `orderbook@1.0.0` for: `deposit`, `order_place`, `order_cancel`, `order_fill`, `withdraw`.
- For production, switch matcher to consume events via an indexer (Pagoda Indexer, Near Lake) instead of polling.
