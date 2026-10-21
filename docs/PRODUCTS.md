# Suban Products and Capabilities

Status: canonical product catalog (Phase 2, October 2026).

This document describes what Suban offers today and what is intentionally not
yet offered. Status values are:

- **Testnet** — implemented and exercised on Pi/Arc testnet; not a mainnet
  financial commitment.
- **Implemented** — code and tests exist; deployment is owner-managed.
- **Experimental** — usable for integration work, but interfaces or economics may
  change.
- **Planned** — design only; do not depend on it.

## 1. Infrastructure products

### 1.1 Suban RPC

| | |
|---|---|
| What | JSON-RPC access to Pi network state, transaction submission/status/history, preflight simulation and Soroban interaction |
| For | Wallets, DApps, SDKs, backend services |
| Interface | JSON-RPC `:8000`; internal admin `:8001` |
| Repository | `suban-rpc` |
| Status | Implemented; operator-managed |
| Limits | Admin API is internal-only. The Rust preflight *simulation* tests are gated while the host/fixture protocol mismatch is resolved; `getLedgerEntries` itself is served by the Go path and is supported. See `suban-rpc/COMPATIBILITY.md`. |

### 1.2 Suban API (Horizon-compatible data access)

| | |
|---|---|
| What | Cached, rate-limited REST wrapper around Pi Horizon with convenience endpoints and a raw proxy |
| For | Explorers, analytics, applications that need account/ledger/transaction/asset data |
| Interface | `/api/v1/*`, `/horizon/*`, `/arc/*`, `/health`, `/metrics` on `:4000` |
| Repository | `Suban-api` |
| Status | Implemented |
| Limits | Read-only; no signing. Upstream Horizon availability determines freshness. |

### 1.3 Data platform (`event-indexer`)

| | |
|---|---|
| What | Versioned index of Pi ledgers, transactions, operations, accounts and Suban contract events with historical query API |
| For | Explorers, analytics, applications that should not run a Pi node or their own ingestion pipeline |
| Interface | `/events`, `/transactions`, `/operations`, `/accounts/:id`, `/ledgers`, `/contracts`, `/status`, `/stats`, `/health`, `/metrics` on `:3002` |
| Repository | `Suban/services/event-indexer` |
| Status | Implemented and tested; testnet/experimental deployment |
| Limits | Derived, replayable view. Account balances are snapshots or activity stubs. Public hostname, TLS and production backfill are not yet provisioned. Public keys/quotas and webhooks are implemented. |

## 2. Market data and oracle products

### 2.1 Multi-asset price controller

| | |
|---|---|
| What | Independent PI/USD, USDC/USD, USDT/USD and oUSD/USD aggregation with provenance, freshness, confidence and trust states |
| For | Applications, risk systems and contracts that need a defensible price rather than a single exchange ticker |
| Interface | `/api/v1/assets`, `/api/v1/price?asset=...`, `/api/v1/history`, `/api/v1/trust`, `/api/v1/sources`, `/api/v1/health`, `/api/v1/methodology`, `/metrics` on `:3000` |
| Repository | `suban-controller` |
| Status | Implemented |
| Limits | oUSD requires an explicit HTTP feed (`OUSD_PRICE_URL`); without it the asset returns `503` and is never assigned a fabricated peg. PI source weights and freshness policy are configuration, not universal truth. |

### 2.2 On-chain oracle publication

| | |
|---|---|
| What | Opt-in signing of healthy controller observations into the Soroban oracle contract (`set_price`) |
| For | On-chain consumers that need a per-asset price with confidence |
| Interface | Soroban `oracle` contract; configured by `ORACLE_PUSH_ENABLED`, `ORACLE_CONTRACT_ID`, `ORACLE_ADMIN_SECRET` and per-asset `ORACLE_ASSET*` |
| Repository | `suban-controller` + `Suban/contracts/oracle` |
| Status | Implemented; disabled by default |
| Limits | Only `HEALTHY` observations are pushed. Failed pushes do not advance the breaker reference. Arc→Stellar bridge work does not change this path. |

### 2.3 Price history and statistics

| | |
|---|---|
| What | Append-only SQLite observations plus window high, low, open, close and count; versioned TWAP and volatility |
| For | Charts, dashboards, audits and downstream analytics |
| Interface | `/api/v1/history?asset=...&hours=...&limit=...`, `/api/v1/statistics`, `/api/v1/twap`, `/api/v1/volatility` |
| Repository | `suban-controller` |
| Status | Implemented |
| Limits | 30-day default retention; breaker observations are stored for audit but excluded from public stats. TWAP/volatility return `503` when no observations exist and label partial coverage. |

### 2.4 Deterministic risk calculator

| | |
|---|---|
| What | Versioned lending-risk formulas: collateral value, debt value, health factor, max borrow and single-collateral liquidation price |
| For | Integrators that want consistent risk math before on-chain lending-state indexing exists |
| Interface | `GET /api/v1/risk/methods`, `POST /api/v1/risk/lending` |
| Repository | `suban-controller` |
| Status | Implemented as a calculator |
| Limits | Does not read on-chain lending positions. Prices come from live aggregators; missing or stale prices return `503`, fresh non-HEALTHY prices are marked degraded. Liquidation price is `null` for multi-collateral baskets. |

### 2.5 Financial event projections

| | |
|---|---|
| What | Pool, trade, liquidity and bounded account-activity views derived from indexed contract events |
| For | Explorers and analytics that need to inspect real swap/liquidity activity |
| Interface | `/financial/pools`, `/financial/pools/:id`, `/trades`, `/liquidity`, `/financial/accounts/:id/activity`, `/financial/summary` on the data platform (`:3002`) |
| Repository | `Suban/services/event-indexer` |
| Status | Partial |
| Limits | Raw amounts only; USD volume and LP positions are explicitly unavailable. Current events lack token direction/decimals and LP balances. Lending events do not exist. No consumer integration yet. |

## 3. Value-movement products

### 3.1 Escrow

| | |
|---|---|
| What | On-chain escrow contracts, factory and a public auditability viewer |
| For | Counterparties that need conditional settlement without a custodian |
| Interface | Soroban escrow/escrow-factory contracts; escrow viewer frontend |
| Repository | `Suban/contracts/escrow*`, `Suban/frontend/escrow-viewer`, SDK `escrow.ts` |
| Status | Testnet / experimental |
| Limits | Viewer is read-only. Not a promise of fiat custody or legal enforceability. |

### 3.2 PUSD bridge

| | |
|---|---|
| What | Burn-and-mint PUSD transfer between Stellar/Pi and Arc |
| For | Users and applications moving stable value across chains |
| Interface | `bridge-burn-mint` (Soroban), Arc bridge (Solidity), relayer worker |
| Repository | `Suban/contracts/bridge-burn-mint`, `contracts/arc-bridge`, `services/bridge-relayer` |
| Status | Testnet; Stellar→Arc supported, Arc→Stellar mint disabled |
| Limits | Signer authorization entries are required before Arc→Stellar is enabled. Relayer has no public API; metrics on `:9464`. |

### 3.3 Core DeFi contracts

| | |
|---|---|
| What | AMM (`cpmm-pool`), stableswap, swap router, pool factory, LP token, liquidity mining, lending pool, backstop, fee router, pause registry, node staking, identity |
| For | Protocol developers building on Suban primitives |
| Interface | Soroban contracts + `@suban/sdk` |
| Repository | `Suban/contracts/*`, `Suban/crates/*` |
| Status | Testnet / experimental |
| Limits | Not audited for mainnet capital. See `SECURITY.md` and `AUDIT-PROGRAM.md`. |

### 3.4 SDK

| | |
|---|---|
| What | Typed TypeScript client for AMM, bridge, escrow, lending and oracle |
| For | Application developers |
| Interface | `@suban/sdk` |
| Repository | `Suban/sdk` |
| Status | Implemented; pre-1.0 interface |
| Limits | Contract addresses are network-specific; see `DEVELOPER.md`. |

## 4. Operations products

### 4.1 Observability pack

| | |
|---|---|
| What | Prometheus scrape config, Grafana dashboard and per-service metrics |
| For | Operators and on-call |
| Interface | `Suban/ops/observability/`; endpoints `/metrics` and `/health` |
| Status | Implemented |
| Limits | Targets must be adjusted for the deployment network; see `OBSERVABILITY.md`. |

### 4.2 Secret rotation and incident response

| | |
|---|---|
| What | Owner runbooks for key rotation, history purge and incident severity/procedures |
| For | Operators and security responders |
| Interface | `docs/SECRET-ROTATION.md`, `docs/INCIDENT-RUNBOOK.md` |
| Status | Implemented; rotation actions require owner execution |
| Limits | Git-history purge is intentionally pending destructive approval. |

## 5. Planned financial products (gated)

These are **hypotheses**, not commitments. They are built only after the gates in
`ROADMAP.md` are met.

| Product | Trigger |
|---|---|
| Liquidity vault / optimization | Measurable LP demand and a sustainable fee source |
| Lending market expansion | Borrow demand, collateral liquidity and liquidation capacity |
| Liquidation network | Multiple lending markets or clear external demand |
| Router / execution service | Multiple meaningful liquidity sources |
| Settlement layer | Multiple applications need shared financial-state reconciliation |

Suban does not pre-commit to Aquarius, Blend or any single external protocol as
the winning product.

## 6. What Suban is not

- Not a bank, custodian or fiat on/off-ramp.
- Not a promise of yield.
- Not a mainnet capital deployment until audits and demand gates pass.
- Not a single monolithic DeFi app; the value is the reusable infrastructure
  beneath many applications.
