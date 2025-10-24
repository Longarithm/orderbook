import { connect, keyStores, transactions, utils, Near, Account } from 'near-api-js';
import pino from 'pino';

const logger = pino({ level: process.env.LOG_LEVEL || 'info' });

const ORDERBOOK_CONTRACT = process.env.ORDERBOOK_CONTRACT_ID || 'gloomyswamp.testnet';
const NETWORK_ID = process.env.NEAR_ENV || 'testnet';
const NODE_URL = process.env.NEAR_NODE_URL || 'https://rpc.testnet.near.org';
const MATCHER_ACCOUNT_ID = process.env.MATCHER_ACCOUNT_ID || 'gloomyswamp.testnet';

interface OrderEvent {
  standard: string;
  version: string;
  event: string;
  data: any;
}

interface OrderView {
  id: number;
  owner_id: string;
  side: 'Buy' | 'Sell' | 'buy' | 'sell';
  price_num: string | number;
  price_den: string | number;
  remaining_base: string | number;
}

type Price = { num: bigint; den: bigint };

function cmpPriceGE(qPaid: bigint, base: bigint, limit: Price): boolean {
  return qPaid * BigInt(limit.den) >= base * BigInt(limit.num);
}
function cmpPriceLE(qPaid: bigint, base: bigint, limit: Price): boolean {
  return qPaid * BigInt(limit.den) <= base * BigInt(limit.num);
}

class OrderbookLocal {
  buys: Map<number, OrderView> = new Map();
  sells: Map<number, OrderView> = new Map();

  upsert(o: OrderView) {
    const side = (typeof o.side === 'string' ? o.side.toLowerCase() : o.side) as 'buy' | 'sell';
    if (side === 'buy') this.buys.set(o.id, o); else this.sells.set(o.id, o);
  }
  remove(id: number) {
    this.buys.delete(id); this.sells.delete(id);
  }
}

async function initNear(): Promise<{ near: Near; account: Account }>{
  const keystore = new keyStores.InMemoryKeyStore();
  // Allow FS keystore if present
  try {
    const fs = new keyStores.UnencryptedFileSystemKeyStore(`${process.env.HOME}/.near-credentials`);
    (keystore as any).keys = { ...(keystore as any).keys, ...(fs as any).keys };
  } catch {}

  const near = await connect({
    networkId: NETWORK_ID,
    nodeUrl: NODE_URL,
    deps: { keyStore: keystore },
  } as any);
  const account = await near.account(MATCHER_ACCOUNT_ID);
  return { near, account };
}

async function fetchRecentOrders(account: Account): Promise<OrderView[]> {
  const orders: OrderView[] = await account.viewFunction({
    contractId: ORDERBOOK_CONTRACT,
    methodName: 'get_orders',
    args: { from_index: 0, limit: 200 },
  });
  return orders.filter(o => (o as any).status === 'Open' || (o as any).status?.Open === undefined);
}

function pickMatch(ob: OrderbookLocal): { makerId: number; takerId: number; baseFill: bigint; quotePaid: bigint } | null {
  // Naive: iterate sells ascending price vs buys descending price
  const sells = Array.from(ob.sells.values());
  const buys = Array.from(ob.buys.values());
  sells.sort((a,b)=> Number(BigInt(a.price_num as any) * 1_000000000000000000n / BigInt(a.price_den as any) - BigInt(b.price_num as any) * 1_000000000000000000n / BigInt(b.price_den as any)));
  buys.sort((a,b)=> Number(BigInt(b.price_num as any) * 1_000000000000000000n / BigInt(b.price_den as any) - BigInt(a.price_num as any) * 1_000000000000000000n / BigInt(a.price_den as any)));
  for (const s of sells) {
    for (const b of buys) {
      const sP: Price = { num: BigInt(s.price_num as any), den: BigInt(s.price_den as any) };
      const bP: Price = { num: BigInt(b.price_num as any), den: BigInt(b.price_den as any) };
      // Cross if buy price >= sell price
      if (cmpPriceGE(bP.num, bP.den, sP)) {
        const baseFill = BigInt(Math.min(Number(BigInt(s.remaining_base as any)), Number(BigInt(b.remaining_base as any))));
        if (baseFill === 0n) continue;
        const quotePaid = (baseFill * sP.num + sP.den - 1n) / sP.den; // ceil
        return { makerId: s.id, takerId: b.id, baseFill, quotePaid };
      }
    }
  }
  return null;
}

async function submitExecute(account: Account, m: { makerId: number; takerId: number; baseFill: bigint; quotePaid: bigint }) {
  logger.info({ m }, 'Submitting execute');
  await account.functionCall({
    contractId: ORDERBOOK_CONTRACT,
    methodName: 'execute',
    args: {
      maker_order_id: m.makerId,
      taker_order_id: m.takerId,
      base_fill: m.baseFill.toString(),
      quote_paid: m.quotePaid.toString(),
    },
    gas: '150000000000000',
    attachedDeposit: '1',
  });
}

async function main() {
  const { account } = await initNear();
  const ob = new OrderbookLocal();
  const orders = await fetchRecentOrders(account);
  for (const o of orders) ob.upsert(o);
  logger.info({ counts: { buys: ob.buys.size, sells: ob.sells.size } }, 'Seeded orderbook');

  // In a loop, poll and try to match
  for (;;) {
    try {
      const updated = await fetchRecentOrders(account);
      ob.buys.clear(); ob.sells.clear();
      for (const o of updated) ob.upsert(o);
      const match = pickMatch(ob);
      if (match) {
        if (process.env.DRY_RUN === '1') {
          logger.info({ match }, 'DRY_RUN match');
        } else {
          await submitExecute(account, match);
        }
      }
    } catch (e) {
      logger.error({ err: e }, 'loop error');
    }
    await new Promise(r => setTimeout(r, 3000));
  }
}

main().catch(e => { logger.error(e); process.exit(1); });
