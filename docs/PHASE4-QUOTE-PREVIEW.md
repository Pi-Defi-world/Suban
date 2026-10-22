# Phase 4 Quote Preview

Status: **preview only (22 October 2026)**. This does not satisfy the Phase 4
gate and has not been matched against on-chain simulation yet.

## What exists

- `sdk/src/quote.ts` exports `quoteCpmm(...)`.
- `CpmmPoolClient.getQuote(tokenIn, amountIn)` calls the contract's read-only
  `get_quote` view through Soroban simulation.
- `sdk/tests/quote.test.ts` covers the integer formula, price impact, dust
  rounding, validation and the `get_quote` wiring.
- `suban-controller` exposes `POST /api/v1/quote/cpmm` for caller-supplied
  reserves and `GET /api/v1/quote/cpmm/:contractId?token_in=...&amount_in=...`
  for the latest indexed pool state. Both label themselves as previews.
- `suban-controller/src/services/quote.test.ts` covers the HTTP formula inputs.
- `contracts/cpmm-pool` has a unit test that calls the contract's real
  `get_quote` view for the shared 1,000,000/1,000,000, 1,000-in, 30-bps vector
  and asserts 996 out, matching the SDK and HTTP tests.

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
```

## Explicit limits

- **No route discovery.** The caller supplies the pool, direction and reserves,
  or a single indexed pool is named explicitly.
- **Indexed state can lag.** `GET /quote/cpmm/:contractId` returns the source
  ledger and refuses pools without enriched token/reserve/fee events.
- **No deployed parity test.** A contract unit test confirms the real
  `get_quote` view returns the shared preview vector, but the previews have not
  been compared with a deployed `get_quote` simulation on testnet. That
  comparison is required before claiming 4.1/4.2.
- **No stableswap quote.** Only the constant-product pool is covered.
- **No execution.** No transaction is built, signed or submitted.
- **No capital.** Nothing here authorizes vaults, liquidation or mainnet funds.

## Verification

Run from `sdk/`:

```bash
npm run typecheck
npm test
```

On 22 October 2026: SDK `tsc --noEmit` passed and 20 Vitest tests passed,
including 6 quote/wiring tests. Controller `tsc` build passed and 49 Jest tests
passed on Node 22 Docker, including 3 HTTP quote-formula tests.
