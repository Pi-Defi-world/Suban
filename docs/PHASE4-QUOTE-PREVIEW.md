# Phase 4 Quote Preview

Status: **preview (23 October 2026)**. This does not satisfy the Phase 4 gate
and does not authorize capital. 4.1/4.2 now have a deployed `get_quote` parity
check and a read-only simulation endpoint; route discovery (4.3) is implemented
for CPMM pools only. Execution, vaults, liquidation and capital are absent.

## What exists

- `sdk/src/quote.ts` exports `quoteCpmm(...)`.
- `CpmmPoolClient.getQuote(tokenIn, amountIn)` calls the contract's read-only
  `get_quote` view through Soroban simulation.
- `sdk/tests/quote.test.ts` covers the integer formula, price impact, dust
  rounding, validation and the `get_quote` wiring.
- `sdk/scripts/quote-parity.mjs` (run with `npm run build && npm run
  parity:testnet`) reads a deployed pool's tokens/reserves/fee, simulates
  `get_quote` in both directions and fails on any mismatch with the SDK formula.
- `suban-controller` exposes `POST /api/v1/quote/cpmm` for caller-supplied
  reserves, `GET /api/v1/quote/cpmm/:contractId?token_in=...&amount_in=...` for
  the latest indexed pool state, and
  `GET /api/v1/quote/cpmm/:contractId/onchain?...` for a read-only Soroban
  simulation of the deployed `get_quote` view (requires
  `QUOTE_SIMULATION_SOURCE_ACCOUNT`).
- `GET /api/v1/route/cpmm?token_in=...&token_out=...&amount_in=...[&max_hops=2]`
  discovers the best CPMM route over enriched indexed pools only (1–3 hops,
  bounded to the first 1,000 pool summaries).
- `suban-controller/src/services/route.test.ts` covers direct/2-hop selection,
  no-liquidity `null`, stableswap exclusion, hop limits and invalid input.
- `contracts/cpmm-pool` has a unit test that calls the contract's real
  `get_quote` view for the shared 1,000,000/1,000,000, 1,000-in, 30-bps vector
  and asserts 996 out, matching the SDK and HTTP tests.
- The data platform's `GET /financial/pools` now returns enriched `state` per
  pool (`null` when unenriched) so route discovery does not invent reserves.

## Formula

The calculator mirrors `contracts/cpmm-pool/src/lib.rs` exactly for integer
token units:

```text
amount_in_after_fee = amount_in * (10_000 - fee_bps) / 10_000
amount_out          = reserve_out * amount_in_after_fee
                      / (reserve_in + amount_in_after_fee)
fee_amount          = amount_in - amount_in_after_fee
price_impact_bps    = 10_000 * (amount_in * reserve_out - amount_out * reserve_in)
                      / (amount_in * reserve_out)
```

All divisions are integer divisions with the same truncation as the contract.
Amounts are raw integer token units; callers must apply token decimals.

## Usage

```ts
import { CpmmPoolClient, quoteCpmm } from '@suban/sdk';

const preview = quoteCpmm({
  reserveIn: '1000000',
  reserveOut: '1000000',
  amountIn: '1000',
  feeBps: 30,
});
// preview.amountOut === '996'
// preview.priceImpactBps === 40

// Prefer the on-chain view for execution-time state:
const pool = new CpmmPoolClient(client, poolContractId);
const amountOut = await pool.getQuote(tokenAddress, '1000');

// HTTP preview over caller-supplied reserves:
await fetch('http://localhost:3000/api/v1/quote/cpmm', {
  method: 'POST',
  headers: { 'content-type': 'application/json' },
  body: JSON.stringify({
    reserve_in: '1000000',
    reserve_out: '1000000',
    amount_in: '1000',
    fee_bps: 30,
  }),
});

// HTTP preview over indexed pool state. Requires DATA_PLATFORM_URL on the
// controller and an enriched pool event with token/reserve metadata.
await fetch(
  'http://localhost:3000/api/v1/quote/cpmm/CPOOL?token_in=CTOKEN&amount_in=1000'
);

// Read-only on-chain simulation + parity check. Requires
// QUOTE_SIMULATION_SOURCE_ACCOUNT on the controller.
await fetch(
  'http://localhost:3000/api/v1/quote/cpmm/CPOOL/onchain?token_in=CTOKEN&amount_in=1000'
);

// Best route over enriched indexed pools (1-3 hops).
await fetch(
  'http://localhost:3000/api/v1/route/cpmm?token_in=CTOKEN&token_out=COTHER&amount_in=1000&max_hops=2'
);
```

## Deployed parity evidence

On 23 October 2026 the parity script was run against the live Pi testnet CPMM
pool `CBE7DHJDS6HZHVY7AWU42XDUA3WAG7GRFAEYEBRM5J3XG2KUYDBAO2NZ`:

```text
reserves = 999003993018961 / 1001000000000000, fee = 30 bps, amount_in = 1000000
wPi  -> USDC: preview 998992, on-chain 998992 (match, ledger 27305287)
USDC -> wPi : preview 995011, on-chain 995011 (match, ledger 27305287)
```

The controller's `simulateCpmmQuote` service reproduced the same pair at ledger
27305295. This is a point-in-time read of the currently deployed pool; it is
evidence for formula parity, not a claim that the enriched contracts in this
repository are deployed or that execution is safe.

## Explicit limits

- **CPMM only.** Route discovery considers CPMM pools with enriched indexed
  state; stableswap pools are excluded, and there is no stableswap quote.
- **Indexed state can lag.** `GET /quote/cpmm/:contractId` and `/route/cpmm`
  return the source ledger(s) and refuse pools without enriched
  token/reserve/fee events.
- **Deployed parity is point-in-time.** The script proves the shared formula
  matches `get_quote` on the named deployed pool at the reported ledger. It must
  be rerun after a contract deployment or pool change.
- **No execution.** No transaction is built, signed or submitted by the quote
  or route endpoints. The simulation endpoint only builds a read-only envelope.
- **No capital.** Nothing here authorizes vaults, liquidation or mainnet funds.

## Verification

Run from `sdk/`:

```bash
npm run typecheck
npm test
npm run build
QUOTE_PARITY_POOL_ID=... QUOTE_PARITY_SOURCE_ACCOUNT=... npm run parity:testnet
```

On 23 October 2026:

- SDK `tsc --noEmit` passed and 20 Vitest tests passed on Node 22.
- Controller `tsc` build passed and 56 Jest tests passed on Node 22 Docker,
  including 5 route-discovery tests and 5 data-platform client tests.
- Indexer `tsc` build passed and 17 Vitest tests passed on Node 22 Docker,
  including the pool-list state assertion.
- The parity script and the controller simulation service both matched the
  deployed pool in both directions (see above).
