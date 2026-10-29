# Suban Architecture

Status: canonical platform architecture (Phase 2 complete; Phase 3 technical/testnet scope complete; Phase 4 testnet prototypes, October 2026). For the current build and deployment inventory, see [PROJECT-STATUS.md](./PROJECT-STATUS.md).

This document explains how the Suban repositories and services fit together. It
is the entry point for new contributors and integrators; deep-dive documents are
linked at the end. Where a capability is testnet-only or not yet deployed, it is
marked explicitly.

## 1. What Suban is

Suban is blockchain and financial infrastructure for Pi Network. It gives
applications four reusable capabilities:

1. **Read the chain** — RPC and Horizon-compatible access to Pi network state.
2. **Understand the chain** — indexed events, account/transaction views and
   normalized data APIs.
3. **Understand markets** — multi-source price aggregation, provenance,
   confidence and explicit oracle trust states.
4. **Move value safely** — contracts with tests and explicit authorization
   boundaries, escrow primitives and a gated cross-chain bridge.

The platform is intentionally layered. Data access is useful on its own; market
data and oracle publication build on it; execution and capital products are only
built after demand is demonstrated.

```text
Pi applications / wallets / explorers / DeFi protocols
                          │
        ┌─────────────────┼──────────────────┐
        ▼                 ▼                  ▼
  Suban-api          suban-controller     suban-rpc
  (Horizon/data)     (market data/oracle) (JSON-RPC/execution)
        │                 │                  │
        └────────┬────────┴────────┬─────────┘
                 ▼                 ▼
          Pi Horizon / Pi node   Soroban contracts
                 │                 │
                 └──── event-indexer / bridge-relayer ────┘
```

## 2. Repository map

| Repository | Runtime | Responsibility | Canonical entry point |
|---|---|---|---|
| `Suban` | Rust + TypeScript | Contracts, shared crates, SDK, event indexer, bridge relayer, escrow viewer, deployment assets | `Cargo.toml`, `sdk/`, `services/` |
| `suban-rpc` | Go + Rust (preflight) | Pi JSON-RPC: network queries, transaction submission, status/history, simulation, Soroban interaction | `cmd/stellar-rpc` |
| `Suban-api` | Node.js/TypeScript | Horizon-compatible REST wrapper with caching, rate limiting, enhanced endpoints and raw Horizon proxy | `src/index.ts` |
| `suban-controller` | Node.js/TypeScript | Multi-asset market-data aggregation, source health, trust state, chain analytics, Horizon proxy, optional on-chain oracle publication | `src/index.ts` |

Repository consolidation rule: do not create a new repository unless the
deployment lifecycle, security boundary, runtime or operational ownership
genuinely requires it.

## 3. Service responsibilities

### 3.1 `suban-rpc` — chain access and execution

- Public JSON-RPC on `:8000`; internal admin API on `:8001` (must never be
  exposed publicly).
- Network state, transaction submission/status/history, preflight simulation and
  Soroban contract interaction.
- Owns the Pi node/captive-core integration; it does **not** own price
  aggregation or HTTP data-product semantics.
- Known limitation: the preflight simulation test fixture targets a protocol-25
  config that the pinned `soroban-env-host` cannot execute yet. The
  `getLedgerEntries` endpoint itself is served by the Go path and is covered by
  [`suban-rpc/COMPATIBILITY.md`](../../suban-rpc/COMPATIBILITY.md); preflight
  tests are excluded from the CI gate while still compiling under `go vet`.

### 3.2 `Suban-api` — developer-facing data access

- Horizon-compatible REST on `:4000` with `/api/v1` convenience endpoints and a
  raw `/horizon/*` proxy.
- In-memory caching and per-IP rate limiting; Prometheus metrics on `/metrics`.
- Read-only. It does not sign transactions, publish prices or hold privileged
  keys.

### 3.3 `suban-controller` — market data and oracle control plane

- Aggregates PI/USD, USDC/USD, USDT/USD and (when configured) oUSD/USD.
- Each asset has its own source set, cache key, circuit breaker and trust state
  machine. Assets cannot contaminate each other's provenance.
- Persists fresh observations in SQLite (`PRICE_DB_PATH`, 30-day default
  retention) and exposes history, versioned TWAP/volatility and a deterministic
  lending-risk calculator. Liquidity-aware pricing is not implemented and is
  marked `false` in `/api/v1/methodology`.
- Publishes healthy observations to a Soroban oracle contract only when
  `ORACLE_PUSH_ENABLED=true` and the asset has an oracle address.
- Exposes Prometheus metrics on `/metrics`.
- Control routes (`/api/v1/price/commit`, `/api/v1/circuit-breaker/reset`) are
  authenticated and rate-limited; they return `503` when no admin key is
  configured.

**Price pipeline**

```text
MEXC / CoinGecko / OKX / Bitget / Arc (PI)
CoinGecko / OKX / MEXC (USDC)
CoinGecko / OKX (USDT)
HTTP feed (oUSD, opt-in)
        │
        ▼
staleness + outlier filtering + weighted aggregation
        │
        ▼
confidence + explicit trust state (HEALTHY/DEGRADED/BREAKER/RECOVERY)
        │
        ├── SQLite price_observations (audit, history, stats)
        ├── /api/v1/price, /history, /statistics, /twap, /volatility
        ├── /api/v1/risk/lending (deterministic calculator, live prices)
        ├── /api/v1/trust, /assets, /methodology, /metrics
        └── optional Soroban set_price (HEALTHY only)
```

Design rules:

- **No fabricated prices.** oUSD is registered but unavailable until
  `OUSD_PRICE_URL` is configured; requests return `503`.
- **No untrusted publication.** Only `HEALTHY` observations can be pushed
  on-chain. A failed push does not move the circuit-breaker reference.
- **No cached duplicates.** Cache hits and breaker fallbacks are not written as
  new observations.
- **Per-asset independence.** Trust and breaker state never leak between PI,
  USDC, USDT and oUSD.

### 3.4 `event-indexer` — data platform

- Ingests two checkpointed streams into one normalized SQLite schema:
  Soroban contract events via RPC, and Pi ledgers/transactions/operations/account
  activity via Horizon.
- Serves `/events`, `/transactions`, `/operations`, `/accounts`, `/ledgers`,
  `/contracts`, `/financial/*`, `/status`, `/stats`, `/health` and `/metrics` on
  `:3002`.
- Answers historical queries from SQLite only; no query handler calls Horizon or
  RPC. All data responses carry `schema_version` and `freshness`.
- Financial projections use enriched swap/liquidity/lending events. USD volume and LP
  positions are derived from enriched events; legacy events stay explicitly
  unenriched. USD valuation is opt-in, live-only and refuses stale/degraded
  prices. LP positions are netted internal share events, not transferable LP
  tokens.
- Owns the contract metadata registry (`contracts`), public API-key quotas,
  webhook outbox/retries and replay checkpoints.
- Not a source of truth: it is a replayable derived view. A replay from a
  checkpoint must reproduce the same rows; writes are idempotent.
- Testnet/experimental: public DNS/TLS and a production-sized backfill are
  deployment actions, not completed claims. See
  [`DATA-PLATFORM.md`](./DATA-PLATFORM.md).

### 3.5 `bridge-relayer` — PUSD cross-chain worker

- Watches Stellar burn events and Arc EVM `PusdBurned` events.
- Builds the destination-chain mint through `TransactionBuilder` and
  `EventProcessor`.
- Exposes `/health` and `/metrics` on `:9464`; no public JSON API.
- Arc→Stellar minting is **disabled** until signer authorization entries are
  implemented. Stellar→Arc remains the supported testnet direction.
- Relayer keys must be isolated; this process should never run on a public API
  host.

### 3.6 Contracts, SDK and frontend

- `contracts/`: Soroban contracts (AMM, stableswap, router, lending, backstop,
  oracle, escrow, bridge, pause registry, fee router, staking, identity) plus the
  Arc Solidity bridge.
- `crates/`: shared Rust primitives (`hub-types`, `hub-errors`, `hub-events`,
  `hub-oracle`, `hub-token`).
- `sdk/`: typed TypeScript client for AMM, bridge, escrow, lending and oracle
  (`@suban/sdk`).
- `frontend/escrow-viewer/`: public escrow auditability dashboard.

## 4. Data and storage

| Store | Owner | Contents | Recovery |
|---|---|---|---|
| Pi node / Horizon | `deploy/` | Chain state, Horizon API | Re-sync from network |
| `suban-rpc` SQLite | `suban-rpc` | Tx status/history | Rebuild from node/config |
| `prices.db` | `suban-controller` | Price observations, schema/method versions, trust state at observation | Restore backup; history is append-only |
| `events.db` | `event-indexer` | Normalized ledgers, transactions, operations, accounts, contract events, registry, checkpoints, API-key usage and webhook outbox | Restore backup and replay from last checkpoint |
| In-memory caches | API/controller | Short-lived read cache | Cold start |

Canonical data contracts (target state):

- `Asset` — id, symbol, name, oracle address.
- `PriceObservation` — symbol, price, confidence, sources used, trust state,
  observation timestamp, schema/method version.
- `AggregatedPrice` — observation plus provenance, freshness, aggregation method
  and trust snapshot.
- `Event` — contract, event type, tx hash, ledger, timestamp, payload.
- `Ledger` / `Transaction` / `Operation` / `Account` — normalized indexed rows
  with schema version `2.0` and append-only checkpoints.
- `Quote` / `ExecutionRequest` — later execution phase.

Schemas are versioned independently from implementation languages. Breaking
changes bump the response `schema_version`.

## 5. Deployment topology

For the verified local container state and portable setup, see
[LOCAL-SYSTEM.md](./LOCAL-SYSTEM.md). The table below describes the intended
public surfaces; it is not a claim that every surface is healthy at every
revision.

Known/target hostnames (owner-managed, not all verified at every revision):

| Surface | Target | Notes |
|---|---|---|
| Oracle/controller | `oracle.suban.org` | Public read routes; control routes private |
| Horizon/data API | `horizon.suban.org` | `Suban-api` + Pi mainnet Horizon |
| Testnet Horizon API | `testnet.suban.org` | `pi-testnet` Horizon, not a frontend |
| RPC | `rpc.suban.org` | Public JSON-RPC; see LOCAL-SYSTEM.md for current health |
| Test RPC | `testrpc.suban.org` | Testnet Soroban JSON-RPC (`stellar-rpc-testnet`) |
| Data platform | `data.suban.org` (target; not yet provisioned) | Indexed historical API; `/metrics` private |
| Bridge relayer | internal worker | Metrics-only listener configured at `:9464` |

Default ports: controller `3000`, Suban-api `4000`, event indexer `3002`, bridge
relayer metrics `9464`, RPC `8000`/`8001`, Horizon `42401`/`41401`. The Pi
Network desktop app also runs a separate `testnet2` node on `31401`-`31403`; it
is not the Suban Compose testnet node. The official Pi-node supervisor `rpc`
service on `41403`/`42403` is currently STOPPED; testnet Soroban RPC is served by
`stellar-rpc-testnet`.

Public read traffic and privileged control traffic must be separated by network
policy. Oracle signing keys, relayer keys and the controller admin key are never
shipped to frontends or public API processes.

## 6. Trust boundaries

| Boundary | What crosses it | Control |
|---|---|---|
| Internet → public read APIs | Anonymous reads | Rate limits, caching, no write routes |
| Public API → control plane | Admin operations | Shared secret, constant-time compare, per-IP rate limit, `503` if unset |
| Controller → Soroban oracle | Signed `set_price` | Trust gating, opt-in, isolated admin secret |
| Relayer → chains | Signed mint/burn transactions | Per-signer contract auth, duplicate rejection, disabled Arc→Stellar path |
| Services → Horizon/RPC | Chain reads | Timeouts, error handling, no secret exposure |
| CI → repository | Source and test fixtures | Gitleaks working-tree gate, history report, pinned deps |

## 7. Failure behavior

- A failing price source is marked stale/error and excluded; the asset becomes
  `DEGRADED` when coverage or confidence falls below thresholds.
- A large deviation trips the per-asset circuit breaker; the last committed price
  is served with `aggregation_method: circuit_breaker_fallback` and no push
  occurs. If no committed reference exists, the price route returns `503` rather
  than a synthetic zero.
- After the breaker clears, `RECOVERY` requires a healthy streak before
  `HEALTHY`.
- If SQLite is unavailable, the controller fails at startup rather than silently
  running without history.
- If the oracle push fails, the observation remains in history and the breaker
  reference does not move.
- If the indexer misses a range, its checkpoint makes replay deterministic.
- If the data platform is stale, `/health` returns `503` and every data response
  carries `freshness.stale=true`; consumers must not present stale rows as live.
- If a webhook receiver fails, deliveries retry with backoff and remain visible
  in the outbox; indexed data is not rolled back.

## 8. Related documents

- [PRODUCTS.md](./PRODUCTS.md) — capability catalog and status.
- [DATA-PLATFORM.md](./DATA-PLATFORM.md) — indexed schema, API, freshness and replay.
- [ROADMAP.md](./ROADMAP.md) — phases, gates and exit criteria.
- [SECURITY.md](./SECURITY.md) — threat model and controls.
- [OWNERSHIP.md](./OWNERSHIP.md) — module owners and deployment targets.
- [API.md](./API.md) — canonical API index.
- [OBSERVABILITY.md](./OBSERVABILITY.md) — metrics, dashboards and SLOs.
- [DEVELOPER.md](./DEVELOPER.md) — contract addresses and developer quick start.
- [PHASE2-BRIDGE-DESIGN.md](./PHASE2-BRIDGE-DESIGN.md) — bridge design.
- [INCIDENT-RUNBOOK.md](./INCIDENT-RUNBOOK.md) — incident procedures.
- [SECRET-ROTATION.md](./SECRET-ROTATION.md) — secret rotation and history purge.
- [PLAN-EXECUTION.md](../PLAN-EXECUTION.md) — current execution plan and progress.
