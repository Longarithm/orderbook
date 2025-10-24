#!/usr/bin/env bash
set -euo pipefail

CONTRACT=${CONTRACT:-gloomyswamp.testnet}
BASE=${BASE:-frog.gloomyswamp.testnet}
QUOTE=${QUOTE:-toad.gloomyswamp.testnet}
TRADER=${TRADER:-gloomyswamp.testnet}

# Deposit 10 base and 100 quote (adjust decimals to your tokens)
near call $BASE ft_transfer_call '{"receiver_id":"'$CONTRACT'","amount":"10000000000000000000000000","msg":""}' --accountId $TRADER --depositYocto 1 --gas 100000000000000
near call $QUOTE ft_transfer_call '{"receiver_id":"'$CONTRACT'","amount":"100000000000000000000000000","msg":""}' --accountId $TRADER --depositYocto 1 --gas 100000000000000

# View balances
near view $CONTRACT get_balance '{"account_id":"'$TRADER'","token_id":"'$BASE'"}'
near view $CONTRACT get_balance '{"account_id":"'$TRADER'","token_id":"'$QUOTE'"}'

# Place a sell order: sell 5 base at price 10 quote per 1 base
near call $CONTRACT place_order '{"side":"Sell","amount_base":"5000000000000000000000000","max_spend_quote":null,"price_num":"10","price_den":"1"}' --accountId $TRADER --depositYocto 1 --gas 100000000000000

# Place a buy order: buy up to 5 base paying at most 50 quote @ 10/1
near call $CONTRACT place_order '{"side":"Buy","amount_base":"5000000000000000000000000","max_spend_quote":"50000000000000000000000000","price_num":"10","price_den":"1"}' --accountId $TRADER --depositYocto 1 --gas 100000000000000

# View orders
near view $CONTRACT get_orders '{"from_index":0,"limit":20}'

# Execute a trade (example ids 0 and 1). Fill 2 base for 20 quote
near call $CONTRACT execute '{"maker_order_id":0,"taker_order_id":1,"base_fill":"2000000000000000000000000","quote_paid":"20000000000000000000000000"}' --accountId $TRADER --depositYocto 1 --gas 150000000000000

# Cancel order id 0
near call $CONTRACT cancel_order '{"order_id":0}' --accountId $TRADER --depositYocto 1 --gas 40000000000000

# Withdraw 1 base to self
near call $CONTRACT withdraw '{"token_id":"'$BASE'","amount":"1000000000000000000000000","receiver_id":null,"msg":null}' --accountId $TRADER --depositYocto 1 --gas 30000000000000
