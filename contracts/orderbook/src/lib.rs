use near_sdk::{
    assert_one_yocto, env, near_bindgen, AccountId, BorshStorageKey, PanicOnDefault,
    Promise, PromiseOrValue, Gas, NearToken,
};
use near_contract_standards::fungible_token::Balance;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LookupMap, UnorderedMap, UnorderedSet};
use near_sdk::serde::{Deserialize, Serialize};
use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;
use near_sdk::json_types::U128;

pub type TokenId = AccountId;

#[derive(BorshSerialize, BorshStorageKey)]
#[borsh(crate = "near_sdk::borsh")]
enum StorageKey {
    Balances,
    Orders,
    OrdersByOwner,
    OrdersByOwnerSet { account_hash: Vec<u8> },
}

#[derive(BorshSerialize, BorshDeserialize)]
#[borsh(crate = "near_sdk::borsh")]
struct BalanceKey {
    account_id: AccountId,
    token_id: TokenId,
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Clone, PartialEq)]
#[serde(crate = "near_sdk::serde")]
pub enum Side {
    Buy,
    Sell,
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(crate = "near_sdk::serde")]
pub enum OrderStatus {
    Open,
    Filled,
    Cancelled,
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct Order {
    pub id: u64,
    pub owner_id: AccountId,
    pub side: Side,
    pub price_num: U128, // quote per unit base (numerator)
    pub price_den: U128, // denominator
    pub amount_base: U128, // original desired base amount
    pub remaining_base: U128, // open base quantity remaining
    pub locked_quote_remaining: U128, // for Buy orders
    pub locked_base_remaining: U128,  // for Sell orders
    pub status: OrderStatus,
    pub created_at: u64,
}

#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
struct OBEvent<T> {
    standard: &'static str,
    version: &'static str,
    event: &'static str,
    data: T,
}

const EVENT_STANDARD: &str = "orderbook";
const EVENT_VERSION: &str = "1.0.0";

fn emit_event<T: Serialize>(event: &'static str, data: T) {
    let e = OBEvent { standard: EVENT_STANDARD, version: EVENT_VERSION, event, data };
    near_sdk::log!("EVENT_JSON:{}", near_sdk::serde_json::to_string(&e).unwrap());
}

fn parse_side(s: &str) -> Side {
    match s.to_ascii_lowercase().as_str() {
        "buy" => Side::Buy,
        "sell" => Side::Sell,
        _ => env::panic_str("invalid side"),
    }
}

fn side_str(side: &Side) -> &'static str {
    match side { Side::Buy => "buy", Side::Sell => "sell" }
}

fn status_str(st: &OrderStatus) -> &'static str {
    match st { OrderStatus::Open => "open", OrderStatus::Filled => "filled", OrderStatus::Cancelled => "cancelled" }
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct Contract {
    pub base_token_id: TokenId,
    pub quote_token_id: TokenId,

    balances: LookupMap<Vec<u8>, Balance>, // key = borsh(BalanceKey)

    orders: UnorderedMap<u64, Order>,
    orders_by_owner: LookupMap<AccountId, UnorderedSet<u64>>,

    next_order_id: u64,
}

#[near_bindgen]
impl Contract {
    #[init]
    pub fn new(base_token_id: TokenId, quote_token_id: TokenId) -> Self {
        assert!(!env::state_exists(), "Already initialized");
        Self {
            base_token_id,
            quote_token_id,
            balances: LookupMap::new(StorageKey::Balances),
            orders: UnorderedMap::new(StorageKey::Orders),
            orders_by_owner: LookupMap::new(StorageKey::OrdersByOwner),
            next_order_id: 0,
        }
    }

    #[payable]
    pub fn place_order(
        &mut self,
        side: String,
        amount_base: U128,
        max_spend_quote: Option<U128>,
        price_num: U128,
        price_den: U128,
    ) -> u64 {
        assert_one_yocto();
        let caller = env::predecessor_account_id();
        let amount_base_u128: u128 = amount_base.0;
        assert!(amount_base_u128 > 0, "amount_base must be > 0");
        assert!(price_num.0 > 0 && price_den.0 > 0, "price must be positive");
        let side_enum = parse_side(&side);

        match side_enum {
            Side::Buy => {
                let spend = max_spend_quote.expect("max_spend_quote required for Buy").0;
                assert!(spend > 0, "max_spend_quote must be > 0");
                let quote_id = self.quote_token_id.clone();
                let bal = self.internal_get_balance(&caller, &quote_id);
                assert!(bal >= spend, "Insufficient quote balance");
                self.internal_sub_balance(&caller, &quote_id, spend);
                let order_id = self.internal_create_order(
                    caller.clone(),
                    Side::Buy,
                    amount_base_u128,
                    spend,
                    0,
                    price_num.0,
                    price_den.0,
                );
                emit_event(
                    "order_place",
                    near_sdk::serde_json::json!({
                        "order_id": order_id,
                        "owner_id": caller,
                        "side": "buy",
                        "amount_base": amount_base_u128.to_string(),
                        "max_spend_quote": spend.to_string(),
                        "price_num": price_num.0.to_string(),
                        "price_den": price_den.0.to_string(),
                    }),
                );
                order_id
            }
            Side::Sell => {
                let base_id = self.base_token_id.clone();
                let bal = self.internal_get_balance(&caller, &base_id);
                assert!(bal >= amount_base_u128, "Insufficient base balance");
                self.internal_sub_balance(&caller, &base_id, amount_base_u128);
                let order_id = self.internal_create_order(
                    caller.clone(),
                    Side::Sell,
                    amount_base_u128,
                    0,
                    amount_base_u128,
                    price_num.0,
                    price_den.0,
                );
                emit_event(
                    "order_place",
                    near_sdk::serde_json::json!({
                        "order_id": order_id,
                        "owner_id": caller,
                        "side": "sell",
                        "amount_base": amount_base_u128.to_string(),
                        "price_num": price_num.0.to_string(),
                        "price_den": price_den.0.to_string(),
                    }),
                );
                order_id
            }
        }
    }

    #[payable]
    pub fn cancel_order(&mut self, order_id: u64) {
        assert_one_yocto();
        let caller = env::predecessor_account_id();
        let mut order = self.orders.get(&order_id).expect("Order not found");
        assert_eq!(order.owner_id, caller, "Only owner can cancel");
        assert_eq!(order.status, OrderStatus::Open, "Order not open");

        match order.side {
            Side::Buy => {
                let refund = order.locked_quote_remaining.0;
                if refund > 0 {
                    let quote_id = self.quote_token_id.clone();
                    self.internal_add_balance(&caller, &quote_id, refund);
                }
            }
            Side::Sell => {
                let refund = order.locked_base_remaining.0;
                if refund > 0 {
                    let base_id = self.base_token_id.clone();
                    self.internal_add_balance(&caller, &base_id, refund);
                }
            }
        }

        order.status = OrderStatus::Cancelled;
        order.locked_base_remaining = U128(0);
        order.locked_quote_remaining = U128(0);
        order.remaining_base = U128(0);
        self.orders.insert(&order_id, &order);

        emit_event(
            "order_cancel",
            near_sdk::serde_json::json!({
                "order_id": order_id,
                "owner_id": caller,
            }),
        );
    }

    #[payable]
    pub fn execute(
        &mut self,
        maker_order_id: u64,
        taker_order_id: u64,
        base_fill: U128,
        quote_paid: U128,
    ) {
        assert_one_yocto();
        assert!(maker_order_id != taker_order_id, "distinct orders required");
        let base_fill_u = base_fill.0;
        let quote_paid_u = quote_paid.0;
        assert!(base_fill_u > 0 && quote_paid_u > 0, "fill must be positive");

        let mut maker = self.orders.get(&maker_order_id).expect("maker not found");
        let mut taker = self.orders.get(&taker_order_id).expect("taker not found");
        assert_eq!(maker.status, OrderStatus::Open, "maker not open");
        assert_eq!(taker.status, OrderStatus::Open, "taker not open");

        // Determine direction: they must be opposite sides
        assert!(maker.side != taker.side, "sides must be opposite");

        // Enforce price limits for both maker and taker
        let (maker_num, maker_den) = (maker.price_num.0, maker.price_den.0);
        let (taker_num, taker_den) = (taker.price_num.0, taker.price_den.0);

        match maker.side {
            Side::Sell => {
                assert!(
                    quote_paid_u.saturating_mul(maker_den) >= base_fill_u.saturating_mul(maker_num),
                    "price below maker's minimum"
                );
                assert!(maker.locked_base_remaining.0 >= base_fill_u, "maker base too small");
            }
            Side::Buy => {
                assert!(
                    quote_paid_u.saturating_mul(maker_den) <= base_fill_u.saturating_mul(maker_num),
                    "price above maker's maximum"
                );
                assert!(maker.locked_quote_remaining.0 >= quote_paid_u, "maker quote too small");
            }
        }
        match taker.side {
            Side::Sell => {
                assert!(
                    quote_paid_u.saturating_mul(taker_den) >= base_fill_u.saturating_mul(taker_num),
                    "price below taker's minimum"
                );
                assert!(taker.locked_base_remaining.0 >= base_fill_u, "taker base too small");
            }
            Side::Buy => {
                assert!(
                    quote_paid_u.saturating_mul(taker_den) <= base_fill_u.saturating_mul(taker_num),
                    "price above taker's maximum"
                );
                assert!(taker.locked_quote_remaining.0 >= quote_paid_u, "taker quote too small");
            }
        }

        // Update maker and taker states and balances
        // Seller gives base, receives quote. Buyer gives quote, receives base.
        {
            let (seller, buyer, seller_id, buyer_id);
            let base_id = self.base_token_id.clone();
            let quote_id = self.quote_token_id.clone();
            if maker.side == Side::Sell {
                seller = &mut maker;
                buyer = &mut taker;
            } else {
                seller = &mut taker;
                buyer = &mut maker;
            }
            // Deduct from locks
            seller.locked_base_remaining = U128(seller.locked_base_remaining.0 - base_fill_u);
            seller.remaining_base = U128(seller.remaining_base.0 - base_fill_u);
            buyer.locked_quote_remaining = U128(buyer.locked_quote_remaining.0 - quote_paid_u);
            buyer.remaining_base = U128(buyer.remaining_base.0 - base_fill_u);
            seller_id = seller.owner_id.clone();
            buyer_id = buyer.owner_id.clone();
            // Credit balances
            self.internal_add_balance(&seller_id, &quote_id, quote_paid_u);
            self.internal_add_balance(&buyer_id, &base_id, base_fill_u);
        }

        // Save orders
        if maker.remaining_base.0 == 0 { maker.status = OrderStatus::Filled; }
        if taker.remaining_base.0 == 0 { taker.status = OrderStatus::Filled; }
        self.orders.insert(&maker_order_id, &maker);
        self.orders.insert(&taker_order_id, &taker);

        emit_event(
            "order_fill",
            near_sdk::serde_json::json!({
                "maker_order_id": maker_order_id,
                "taker_order_id": taker_order_id,
                "base_fill": base_fill_u.to_string(),
                "quote_paid": quote_paid_u.to_string(),
                "maker_remaining": maker.remaining_base.0.to_string(),
                "taker_remaining": taker.remaining_base.0.to_string(),
            }),
        );
    }

    #[payable]
    pub fn withdraw(
        &mut self,
        token_id: TokenId,
        amount: U128,
        receiver_id: Option<AccountId>,
        msg: Option<String>,
    ) -> Promise {
        assert_one_yocto();
        let caller = env::predecessor_account_id();
        let amount_u = amount.0;
        assert!(amount_u > 0, "amount must be > 0");
        self.internal_sub_balance(&caller, &token_id, amount_u);

        let to = receiver_id.unwrap_or_else(|| caller.clone());
        let memo: Option<String> = None;

        let promise = if msg.is_some() {
            // ft_transfer_call
            Promise::new(token_id.clone()).function_call(
                "ft_transfer_call".to_string(),
                near_sdk::serde_json::to_vec(&near_sdk::serde_json::json!({
                    "receiver_id": to,
                    "amount": U128(amount_u),
                    "memo": memo,
                    "msg": msg.clone().unwrap(),
                })).unwrap(),
                NearToken::from_yoctonear(1),
                Gas::from_tgas(25),
            )
        } else {
            // ft_transfer
            Promise::new(token_id.clone()).function_call(
                "ft_transfer".to_string(),
                near_sdk::serde_json::to_vec(&near_sdk::serde_json::json!({
                    "receiver_id": to,
                    "amount": U128(amount_u),
                    "memo": memo,
                })).unwrap(),
                NearToken::from_yoctonear(1),
                Gas::from_tgas(10),
            )
        };

        emit_event(
            "withdraw",
            near_sdk::serde_json::json!({
                "account_id": caller,
                "token_id": token_id,
                "amount": amount_u.to_string(),
                "receiver_id": to,
                "has_msg": msg.is_some(),
            }),
        );

        promise
    }

    // Views
    pub fn get_config(&self) -> (TokenId, TokenId) { (self.base_token_id.clone(), self.quote_token_id.clone()) }

    pub fn get_balance(&self, account_id: AccountId, token_id: TokenId) -> U128 {
        U128(self.internal_get_balance(&account_id, &token_id))
    }

    pub fn get_order(&self, order_id: u64) -> Option<(u64, String, String, U128, U128, U128, U128, U128, U128, String, u64)> {
        self.orders.get(&order_id).map(|o| (
            o.id,
            o.owner_id.to_string(),
            side_str(&o.side).to_string(),
            o.price_num,
            o.price_den,
            o.amount_base,
            o.remaining_base,
            o.locked_quote_remaining,
            o.locked_base_remaining,
            status_str(&o.status).to_string(),
            o.created_at,
        ))
    }

    pub fn get_orders(&self, from_index: u64, limit: u64) -> Vec<(u64, String, String, U128, U128, U128, U128, U128, U128, String, u64)> {
        let keys: Vec<u64> = self.orders.keys_as_vector().to_vec();
        let start = from_index as usize;
        let end = usize::min(start + (limit as usize), keys.len());
        keys[start..end].iter().map(|k| {
            let o = self.orders.get(k).unwrap();
            (
                o.id,
                o.owner_id.to_string(),
                side_str(&o.side).to_string(),
                o.price_num,
                o.price_den,
                o.amount_base,
                o.remaining_base,
                o.locked_quote_remaining,
                o.locked_base_remaining,
                status_str(&o.status).to_string(),
                o.created_at,
            )
        }).collect()
    }

    pub fn get_orders_by_owner(&self, owner_id: AccountId) -> Vec<(u64, String, String, U128, U128, U128, U128, U128, U128, String, u64)> {
        if let Some(set) = self.orders_by_owner.get(&owner_id) {
            set.iter().map(|id| {
                let o = self.orders.get(&id).unwrap();
                (
                    o.id,
                    o.owner_id.to_string(),
                    side_str(&o.side).to_string(),
                    o.price_num,
                    o.price_den,
                    o.amount_base,
                    o.remaining_base,
                    o.locked_quote_remaining,
                    o.locked_base_remaining,
                    status_str(&o.status).to_string(),
                    o.created_at,
                )
            }).collect()
        } else { vec![] }
    }
}

#[near_bindgen]
impl FungibleTokenReceiver for Contract {
    fn ft_on_transfer(&mut self, sender_id: AccountId, amount: U128, _msg: String) -> PromiseOrValue<U128> {
        // Only accept deposits from the two market tokens
        let token_contract = env::predecessor_account_id();
        if token_contract != self.base_token_id && token_contract != self.quote_token_id {
            return PromiseOrValue::Value(amount);
        }
        let amt = amount.0;
        self.internal_add_balance(&sender_id, &token_contract, amt);
        emit_event(
            "deposit",
            near_sdk::serde_json::json!({
                "account_id": sender_id,
                "token_id": token_contract,
                "amount": amt.to_string(),
            }),
        );
        PromiseOrValue::Value(U128(0))
    }
}

impl Contract {
    fn orders_set_for(&mut self, owner_id: &AccountId) -> UnorderedSet<u64> {
        if let Some(set) = self.orders_by_owner.get(owner_id) { return set; }
        let mut prefix = vec![];
        prefix.extend(b"ob:");
        prefix.extend(env::sha256(owner_id.as_bytes()));
        let bytes = StorageKey::OrdersByOwnerSet { account_hash: prefix };
        UnorderedSet::new(near_sdk::borsh::to_vec(&bytes).unwrap())
    }

    fn internal_create_order(
        &mut self,
        owner_id: AccountId,
        side: Side,
        amount_base: u128,
        locked_quote: u128,
        locked_base: u128,
        price_num: u128,
        price_den: u128,
    ) -> u64 {
        let id = self.next_order_id;
        self.next_order_id += 1;
        let order = Order {
            id,
            owner_id: owner_id.clone(),
            side: side.clone(),
            price_num: U128(price_num),
            price_den: U128(price_den),
            amount_base: U128(amount_base),
            remaining_base: U128(amount_base),
            locked_quote_remaining: U128(locked_quote),
            locked_base_remaining: U128(locked_base),
            status: OrderStatus::Open,
            created_at: env::block_timestamp() / 1_000_000,
        };
        self.orders.insert(&id, &order);
        let mut set = self.orders_set_for(&owner_id);
        set.insert(&id);
        self.orders_by_owner.insert(&owner_id, &set);
        id
    }

    fn internal_get_balance(&self, account_id: &AccountId, token_id: &TokenId) -> u128 {
        let key = BalanceKey { account_id: account_id.clone(), token_id: token_id.clone() };
        self.balances.get(&near_sdk::borsh::to_vec(&key).unwrap()).unwrap_or(0)
    }

    fn internal_add_balance(&mut self, account_id: &AccountId, token_id: &TokenId, amount: u128) {
        let key = BalanceKey { account_id: account_id.clone(), token_id: token_id.clone() };
        let k = near_sdk::borsh::to_vec(&key).unwrap();
        let cur = self.balances.get(&k).unwrap_or(0);
        self.balances.insert(&k, &(cur + amount));
    }

    fn internal_sub_balance(&mut self, account_id: &AccountId, token_id: &TokenId, amount: u128) {
        let key = BalanceKey { account_id: account_id.clone(), token_id: token_id.clone() };
        let k = near_sdk::borsh::to_vec(&key).unwrap();
        let cur = self.balances.get(&k).unwrap_or(0);
        assert!(cur >= amount, "insufficient balance");
        let new_bal = cur - amount;
        if new_bal == 0 { self.balances.remove(&k); } else { self.balances.insert(&k, &new_bal); }
    }
}
