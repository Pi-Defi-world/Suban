# Phase 4 Quote Preview

Status: **preview only (21 October 2026)**. This does not satisfy the Phase 4
gate, does not expose a deployed `/quote` HTTP service, and has not been matched
against on-chain simulation yet.

## What exists

- `sdk/src/quote.ts` exports `quoteCpmm(...)`.
- `CpmmPoolClient.getQuote(tokenIn, amountIn)` calls the contract's read-only
  `get_quote` view through Soroban simulation.
- `sdk/tests/quote.test.ts` covers the integer formula, price impact, dust
  rounding, validation and the `get_quote` wiring.

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
```

## Explicit limits

- **No route discovery.** The caller supplies the pool, direction and reserves.
- **No HTTP endpoint.** Phase 4.1's `/quote` service is not built.
- **No on-chain parity test.** The pure formula matches the contract source and
  the unit vectors, but it has not been compared with a deployed `get_quote`
  simulation on testnet. That comparison is required before claiming 4.1/4.2.
- **No stableswap quote.** Only the constant-product pool is covered.
- **No execution.** No transaction is built, signed or submitted.
- **No capital.** Nothing here authorizes vaults, liquidation or mainnet funds.

## Verification

Run from `sdk/`:

```bash
npm run typecheck
npm test
```

On 21 October 2026: `tsc --noEmit` passed and 20 Vitest tests passed, including
6 quote/wiring tests.
