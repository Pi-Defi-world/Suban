# Phase 4 Quote Preview

Status: **preview (23 October 2026)**. This does not satisfy the Phase 4 gate
and does not authorize capital. 4.1/4.2 now have deployed `get_quote` parity
checks and a read-only simulation endpoint; route discovery (4.3) is implemented
for CPMM pools only. Mainnet execution and capital are absent. Testnet vault
and liquidation prototypes are documented separately in
[`PHASE4-LIQUIDATION.md`](./PHASE4-LIQUIDATION.md); they do not authorize
capital or mainnet use.

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

### Enriched CPMM deployment (current)

On 23 October 2026 the parity script was run against the enriched Pi testnet
CPMM pool `CBN77D4T6EPPVHLCNU2XCAEWMRPGTDH67RSGVKKJSIPHT3W23KB6QNCM`:

```text
reserves = 10100000000 / 9901284197, fee = 30 bps, amount_in = 100000000
wPi  -> PUSD: preview 96783046, on-chain 96783046 (match, ledger 27319628)
PUSD -> wPi : preview 100687090, on-chain 100687090 (match, ledger 27319628)
```

The controller, configured with `DATA_PLATFORM_URL` and
`QUOTE_SIMULATION_SOURCE_ACCOUNT`, returned:

```text
GET /api/v1/route/cpmm          amount_out_raw 96783046, source ledger 27319581
GET /api/v1/quote/cpmm/:id      amount_out_raw 96783046, indexed ledger 27319581
GET /api/v1/quote/cpmm/:id/onchain
                                amount_out_raw 96783046, ledger 27319714,
                                parity.matches = true
```

### Earlier pool (historical)

An earlier parity run against pool
`CBE7DHJDS6HZHVY7AWU42XDUA3WAG7GRFAEYEBRM5J3XG2KUYDBAO2NZ` matched at ledger
27305287 (`998992` and `995011` raw units) before the enriched deployment. That
run remains evidence for the shared formula, but it is superseded by the
enriched-pool run above. Both are point-in-time reads, not execution guarantees.

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
- **No capital.** Testnet vault and liquidation prototypes exist, but nothing
  here authorizes capital, a production keeper or mainnet funds.

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
- Indexer `tsc` build passed and 22 Vitest tests passed on Node 22 Docker,
  including the pool-list state assertion, `scval` decoding and the
  vault-event exclusion test.
- Liquidation bot `tsc` typecheck/build passed and 18 Vitest tests passed on
  Node 22 Docker.
- The parity script and the controller simulation service both matched the
  enriched deployed pool in both directions (see above).
