# Liquidation bot prototype

Testnet-only liquidation bot for the Suban lending pool. It is a **prototype**,
not a production keeper, and it never runs against mainnet capital.

## Safety model

- **Dry-run is the default.** `LIQUIDATION_MODE=execute` is required to sign.
- Keys are read only from the process environment (`LIQUIDATOR_SECRET`). No
  tracked file in this directory contains a secret, and `.env.example` contains
  no values. A local `.env.local` may hold the testnet key; it is gitignored.
- Candidate discovery from the data platform is treated as an address list only.
  Health factors and positions are re-read from the deployed contract before a
  decision. Stale indexer data cannot trigger a transaction by itself.
- The bot never reads or prints the secret. Logs contain addresses, health
  factors, planned repayment and transaction hashes.
- The contract enforces the health check, bonus, and collateral cap. The bot's
  `LIQUIDATION_SLIPPAGE_BPS` only lowers `min_collateral`.
- `LIQUIDATION_MAX_REPAY_RAW` bounds exposure per liquidation.
- This prototype has no flash-loan or capital source. The liquidator account
  must already hold the debt asset.

## Configuration

Copy `.env.example` to a local ignored file and fill it in. The process reads
ordinary environment variables; it does not load dotfiles implicitly.

| Variable | Meaning |
|---|---|
| `LENDING_POOL_CONTRACT_ID` | Deployed lending pool contract (`C...`) |
| `STELLAR_RPC_URL` | Soroban RPC (default Pi testnet) |
| `STELLAR_NETWORK_PASSPHRASE` | Network passphrase (default `Pi Testnet`) |
| `STELLAR_TX_FEE` | Inclusion fee in stroops (Pi testnet needs > default) |
| `LIQUIDATION_MODE` | `dry-run` (default) or `execute` |
| `LIQUIDATOR_SECRET` | Required only for `execute` |
| `LIQUIDATOR_PUBLIC_KEY` | Required for dry-run simulation if no secret |
| `LIQUIDATION_ACCOUNTS` | Comma-separated accounts, or empty to discover |
| `DATA_PLATFORM_URL` | Financial API base URL for discovery |
| `LIQUIDATION_MAX_REPAY_RAW` | Per-liquidation repayment bound |
| `LIQUIDATION_MIN_HEALTH_BPS` | Decision threshold, default `10000` |
| `LIQUIDATION_BONUS_BPS` | Expected contract bonus, default `500` |
| `LIQUIDATION_SLIPPAGE_BPS` | `min_collateral` buffer, default `100` |
| `LIQUIDATION_POLL_MS` | Loop interval |
| `LIQUIDATION_MAX_CANDIDATES` | Discovery bound |

## Commands

```sh
npm ci
npm test
npm run typecheck
node --env-file=.env.local dist/index.js --once
```

`npm start` loops. `npm run scan` performs one dry-run pass. Node 22 can load
the gitignored `.env.local` with `--env-file`; the application itself never
loads dotfiles implicitly.

## Evidence and limits

- Unit tests cover the health boundary, repayment cap, bonus, collateral cap,
  slippage floor, config validation and data-platform discovery.
- The deployed testnet lending contract has a successful liquidation event
  recorded in `docs/PHASE4-LIQUIDATION.md`.
- This is not audited and must not be pointed at mainnet or funded beyond a
  testnet liquidator balance.
