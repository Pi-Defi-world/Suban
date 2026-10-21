# Suban API

Status: canonical API index (Phase 2, October 2026).

This is the entry point for API consumers. Detailed per-service documents are
linked below; when they disagree with this index, the service document wins for
its own routes and this index is updated in the same change.

## 1. Services and base URLs

| Service | Default local base URL | Public target | Detailed docs |
|---|---|---|---|
| Controller (market data/oracle) | `http://localhost:3000` | `https://oracle.suban.org` | [`suban-controller/docs/API.md`](../../suban-controller/docs/API.md) |
| Data API (Horizon-compatible) | `http://localhost:4000` | `https://horizon.suban.org` | [`../Suban-api/README.md`](../../Suban-api/README.md) |
| Data platform (indexed history) | `http://localhost:3002` | `https://data.suban.org` (target; not yet provisioned) | [`../services/event-indexer/README.md`](../services/event-indexer/README.md) |
| Bridge relayer (metrics/health only) | `http://localhost:9464` | internal worker | [`../docs/OBSERVABILITY.md`](./OBSERVABILITY.md) |
| RPC | `http://localhost:8000` | `https://rpc.suban.org` | [`../../suban-rpc/README.md`](../../suban-rpc/README.md) |

All HTTP APIs return JSON unless noted. Timestamps are ISO-8601 UTC strings.
Endpoints marked **authenticated** require a shared secret and are not part of
the public developer surface.

## 2. Versioning and compatibility

- Public HTTP routes are versioned under `/api/v1` (data API and controller).
  The data platform uses unversioned resource paths (`/events`, `/transactions`,
  `/operations`, `/accounts`, `/ledgers`) and carries `schema_version: "2.0"`
  in every data response.
- The controller price payload carries `schema_version` (`1.0` today) and
  `method_version` (`weighted_average_v1`). Breaking shape changes bump
  `schema_version`; methodology changes bump `method_version`.
- Adding a field is backward-compatible. Removing/renaming a field is not.
- The data platform (`/events`, `/transactions`, `/operations`, `/accounts`,
  `/ledgers`) returns `schema_version: "2.0"` and a `freshness` object on every
  data response. Breaking shape changes bump `schema_version`; additions do not.
- Error responses use `{ "error": "message" }`; controller price errors may add
  `asset` and `available_assets`.
- Unknown assets return `404`; known but unconfigured assets return `503`. Do
  not treat `503` as a zero price.

## 3. Authentication and rate limits

| Surface | Auth | Limits |
|---|---|---|
| Data API public read routes | None | Per-IP rate limit at the edge and in the service |
| Data platform public read routes | `x-api-key` or `Authorization: Bearer <key>` when `INDEXER_REQUIRE_API_KEY=true` | Per-key minute/day quotas; `429` + `Retry-After` |
| Data platform admin routes | `INDEXER_ADMIN_API_KEY` | Private network; `503` if unset |
| Controller control routes | `x-api-key: <key>` or `Authorization: Bearer <key>` | Per-IP limit, default 30/min |
| Controller oracle publisher | `ORACLE_ADMIN_SECRET` (server-side only) | Opt-in; not an HTTP route |
| RPC admin API (`:8001`) | Private network only | Do not expose |
| RPC public JSON-RPC (`:8000`) | None | See RPC docs |
| Metrics `/metrics` | None by default | Should be scraped from a private network |

If `CONTROLLER_ADMIN_API_KEY` is unset, control routes return `503` rather than
falling open. If the key is wrong, they return `401`/`403`; repeated failures are
rate-limited.

## 4. Controller (market data and oracle)

Base: `http://localhost:3000`. Every price route accepts
`?asset=PI|USDC|USDT|OUSD` and defaults to `PI`.

### 4.1 `GET /api/v1/assets`

Returns the asset registry with per-asset source counts, trust state, oracle
address and last committed price.

```json
{
  "schema_version": "1.0",
  "assets": [
    {
      "id": "PI",
      "symbol": "PI",
      "name": "Pi Network",
      "enabled": true,
      "oracle_asset": null,
      "min_sources_required": 1,
      "sources": [{ "name": "mexc", "status": "active" }],
      "trust": {
        "state": "HEALTHY",
        "since": "2026-10-20T12:00:00.000Z",
        "reason": "sources and confidence within bounds",
        "healthy_streak": 3
      },
      "last_committed_price": 0.3125
    }
  ]
}
```

### 4.2 `GET /api/v1/price?asset=PI`

Returns the aggregated price with provenance, freshness, confidence and trust.

```json
{
  "schema_version": "1.0",
  "method_version": "weighted_average_v1",
  "symbol": "PI",
  "price_usd": 0.3125,
  "timestamp": "2026-10-20T12:00:00.000Z",
  "age_ms": 0,
  "is_fresh": true,
  "sources_used": 4,
  "total_sources": 4,
  "aggregation_method": "weighted_average",
  "confidence_score": 0.94,
  "cache_hit": false,
  "source_prices": {
    "mexc": {
      "price": 0.312,
      "weight": 3,
      "timestamp": "2026-10-20T11:59:58.000Z",
      "age_ms": 2000,
      "is_fresh": true
    }
  },
  "trust": {
    "state": "HEALTHY",
    "since": "2026-10-20T11:00:00.000Z",
    "reason": "sources and confidence within bounds",
    "healthy_streak": 12
  }
}
```

Failure behavior:

| Case | Status | Body |
|---|---|---|
| Unknown asset | `404` | `{ "error": "Unknown asset", "asset": "DOGE", "available_assets": ["PI","USDC","USDT","OUSD"] }` |
| Registered but no sources (e.g. oUSD without feed) | `503` | `{ "error": "Asset is not configured", "asset": "OUSD" }` |
| All sources fail / no usable observation | `503` | `{ "error": "No price available", "asset": "PI" }` |
| Breaker active | `200` | last committed price with `aggregation_method: "circuit_breaker_fallback"` and `is_fresh: false` |
| Breaker active with no committed reference | `503` | `{ "error": "No price available", "asset": "PI" }` |

Never infer a price from a `503` or from a missing `source_prices` entry. A breaker
fallback is explicitly stale (`is_fresh: false`) and must not be treated as a
fresh observation.

### 4.3 `GET /api/v1/history?asset=PI`

Query: `hours` (1–720, default 24), `limit` (1–10000, default 1000). Returns
observations newest first plus window stats. Breaker observations are stored for
audit but excluded from public stats.

### 4.3.1 `GET /api/v1/statistics`, `/twap`, `/volatility`

All three paths return the same `twap_volatility_v1` calculation from persisted
observations. Query: `asset`, `window` (`60s`-`30d`; `5m`, `24h`, `30d` or
seconds), and optional `as_of` for replay. `coverage_ratio` and `warnings`
report partial history; an empty window returns `503`, never a fabricated
value. Volatility is `null` with fewer than two returns.

### 4.3.2 `GET /api/v1/methodology`

Returns versioned price/TWAP/risk methods and explicit gaps. In particular,
`price_quality.liquidity_aware` is `false`; no pool-depth input exists yet.

### 4.3.3 Risk API

`GET /api/v1/risk/methods` publishes formulas, defaults and limitations for
`lending_risk_v1`.

`POST /api/v1/risk/lending` accepts:

```json
{
  "collateral": [{ "asset": "PI", "amount": 100 }],
  "debt": [{ "asset": "USDC", "amount": 5 }],
  "liquidation_threshold": 0.8,
  "target_health_factor": 1.5
}
```

Prices come from the live asset aggregators. Unknown assets return `404`,
unconfigured assets and missing prices return `503` without partial results.
Stale/non-`HEALTHY` prices are returned with `data_quality.status: "degraded"`.
The response contains `health_factor`, `collateral_value_usd`, `debt_value_usd`,
`max_borrow_usd` and `liquidation_price_usd_per_collateral_unit` (single
collateral only). The calculator does not read on-chain lending positions.

### 4.4 `GET /api/v1/trust?asset=PI`

Returns the current trust state and transition history. `?limit=` bounds history
(1–100, default 50).

States: `HEALTHY`, `DEGRADED`, `BREAKER`, `RECOVERY`.

### 4.5 `GET /api/v1/sources?asset=PI`

Without `asset`, returns PI sources plus an asset summary for backward
compatibility. With `asset`, returns only that asset's sources.

### 4.6 `GET /api/v1/health`

Top-level fields describe PI (backward compatible) and `assets` lists every
asset's trust, source count and last committed price. `status` is `unhealthy`
when the PI breaker is active, `degraded` when PI is not healthy or fewer than
half of its sources are active, otherwise `healthy`.

### 4.7 Control routes (authenticated)

| Route | Method | Body | Purpose |
|---|---|---|---|
| `/api/v1/price/commit` | POST | `{ "price": number, "asset"?: string }` | Set the breaker reference price |
| `/api/v1/circuit-breaker/reset` | POST | `{ "asset"?: string }` | Clear the breaker for one asset |

### 4.8 `GET /metrics`

Prometheus text exposition. Key series:

- `suban_price_usd{symbol}`, `suban_price_confidence{symbol}`, `suban_price_age_ms{symbol}`
- `suban_source_up{asset,source}`, `suban_source_staleness_ms{asset,source}`
- `suban_price_aggregations_total{asset,result}`
- `suban_oracle_push_total{asset,result}`
- `suban_oracle_trust_state{asset,state}`
- `suban_http_requests_total{method,route,status}`
- Node.js process metrics

### 4.9 Compatibility and analytics routes

| Route | Notes |
|---|---|
| `GET /data/pi-price` | Legacy PI price shape |
| `GET /data/mainnet-supply`, `/data/top-accounts`, `/data/accounts/distribution` | Pi analytics |
| `GET /data/data/unlocknext30d`, `/data/data/unlockfull`, `/data/data/nodemap` | Unlock/node analytics |
| `GET /api/v1/chain/stats`, `/chain/ledgers`, `/chain/accounts`, `/chain/transactions` | Local Horizon chain data |
| `GET /api/contracts`, `/api/contracts/:id`, `/api/contracts/:id/state` | Contract discovery (derived from recent effects) |
| `GET /api/escrow/:id`, `/api/gateway/events` | Escrow viewer backend |
| `GET /realtime/`, `/realtime/snapshot` | SSE live transactions/trades/orders |
| `GET /horizon/*` | Transparent proxy to local Horizon |

These are compatibility/analytics surfaces; they are not guaranteed to be stable
developer contracts until documented in a future API version.

## 5. Data API (Horizon-compatible)

Base: `http://localhost:4000`.

| Route | Purpose |
|---|---|
| `GET /api/v1/summary` | Latest ledger/transaction/operation summary |
| `GET /api/v1/accounts/:accountId` | Account details |
| `GET /api/v1/accounts/:accountId/transactions` | Account transactions |
| `GET /api/v1/transactions/:hash` | Transaction by hash |
| `GET /api/v1/ledgers` | Recent ledgers |
| `GET /api/v1/assets` | Assets known to Horizon |
| `ALL /horizon/*` | Raw Horizon proxy (`X-Cache: HIT|MISS`) |
| `GET /health` | Liveness |
| `GET /metrics` | Prometheus metrics (`suban_api_*`) |
| Arc routes | `/arc/health`, `/arc/block-number`, `POST /arc/rpc` |

Rate limit: default 100 requests/60s per IP; `/metrics` is exempt. Upstream
failures return `{ "error": "Horizon API error", "message": "..." }` with the
upstream status or `500`.

## 6. Data platform (indexed history)

Base: `http://localhost:3002`. This is the historical/indexed API. It answers
from SQLite and never calls Horizon or Soroban RPC in a query handler.

| Route | Purpose |
|---|---|
| `GET /` | Service metadata and endpoint list |
| `GET /status` | Counts, checkpoints, freshness, Horizon state |
| `GET /stats` | Counts and per-contract event totals |
| `GET /contracts` | Contract metadata registry (network/address/interface/verification) |
| `GET /contracts/:contractId` | One registry entry |
| `GET /contracts/:contractId/events` | Events for one contract |
| `GET /events` | Events (`contract`, `type`, `from`, `to`, `ledger_from`, `ledger_to`, `limit`, `offset`) |
| `GET /events/contract/:id` | Events by contract (legacy shape, freshness-wrapped) |
| `GET /events/type/:type` | Events by type (legacy shape, freshness-wrapped) |
| `GET /events/ledger/:ledger` | Events at a ledger (legacy shape, freshness-wrapped) |
| `GET /transactions` | Transactions (`account`, `ledger`, `from`, `to`, `successful`, `limit`, `offset`) |
| `GET /transactions/:hash` | Indexed transaction; `404` if not indexed |
| `GET /operations` | Operations (`account`, `tx_hash`, `ledger`, `type`, `from`, `to`, `limit`, `offset`) |
| `GET /accounts/:accountId` | Indexed account or activity stub |
| `GET /accounts/:id/transactions` | Transactions involving the account |
| `GET /accounts/:id/operations` | Operations involving the account |
| `GET /financial/pools` | Pool activity derived from swap/liquidity events |
| `GET /financial/pools/:contractId` | Raw volume, recent events and explicit gap flags |
| `GET /financial/pools/:contractId/trades` | Swap events with raw amounts |
| `GET /financial/pools/:contractId/liquidity` | Add/remove-liquidity events |
| `GET /financial/accounts/:accountId/activity` | Bounded recent account activity |
| `GET /financial/summary` | Financial event counts and latest event |
| `GET /ledgers` | Recent indexed ledgers |
| `GET /ledgers/:sequence` | One indexed ledger |
| `GET /health` | `200` healthy, `503` degraded/stale |
| `GET /metrics` | Prometheus metrics (`suban_indexer_*`) |

Financial routes expose raw event amounts only. `coverage.usd_volume` and
`coverage.lp_positions` are `false`: current events do not carry token
direction, decimals or LP balances. No USD value is inferred.

All data responses include `schema_version: "2.0"` and `freshness`:

```json
{
  "schema_version": "2.0",
  "freshness": {
    "as_of": "2026-10-21T11:47:52.494Z",
    "latest_indexed_ledger": 29329977,
    "latest_observed_ledger": 29329977,
    "lag_ledgers": 0,
    "lag_seconds": 3,
    "last_success_at": "2026-10-21T11:47:48.619Z",
    "stale": false,
    "stale_after_seconds": 120
  },
  "data": []
}
```

Pagination is `limit`/`offset` (`limit` max 1000, default 100). Time filters
accept ISO-8601 or Unix seconds. `stale` is true when ledger lag exceeds
`INDEXER_MAX_LAG_LEDGERS` (default 10), when the last success is older than
`INDEXER_STALE_AFTER_SECONDS` (default 120), or before the first success.
Consumers must check `freshness.stale`; do not assume real-time delivery.

Admin routes (`/admin/replay`, `/admin/checkpoints`, `/admin/contracts`,
`/admin/webhooks`, `/admin/api-keys/usage`) require
`INDEXER_ADMIN_API_KEY`. A replay reproduces state from a checkpoint; writes are
idempotent.

See [`DATA-PLATFORM.md`](./DATA-PLATFORM.md) for the schema and replay contract.

## 7. Bridge relayer

No public API. Internal endpoints:

| Route | Purpose |
|---|---|
| `GET http://<relayer>:9464/health` | Liveness |
| `GET http://<relayer>:9464/metrics` | Prometheus metrics (`suban_relayer_*`) |

Do not expose port `9464` to the public internet.

## 8. RPC

Base: `http://localhost:8000` (JSON-RPC 2.0). Admin API: `:8001`, internal only.
Supported methods include `getHealth`, `getNetwork`, `getLatestLedger`,
`getLedgers`, `getLedgerEntries`, `getEvents`, `getTransaction`,
`getTransactions`, `sendTransaction`, `simulateTransaction`, `getFeeStats` and
`getVersionInfo`. Params are always an object, never an array.

Compatibility limitation: `getLedgerEntries` is served by the Go path and is
supported. The `internal/preflight` tests are excluded from CI because their
fixture targets protocol 25 while the vendored host is protocol 23; this affects
preflight test coverage, not `getLedgerEntries`. See
[`../../suban-rpc/COMPATIBILITY.md`](../../suban-rpc/COMPATIBILITY.md).

## 9. Error and retry guidance

- `429`: back off; respect `Retry-After` where provided.
- `5xx` from a read route: retry with exponential backoff and jitter; do not
  retry non-idempotent writes automatically.
- `503` on a price route: the asset is not configured or has no usable sources.
  Treat it as "no price", never as zero or a peg.
- `401`/`403` on control routes: fix credentials; do not retry aggressively.
- Log the response `schema_version` and any `request_id`/trace header you add at
  the edge.

## 10. Related documents

- [ARCHITECTURE.md](./ARCHITECTURE.md) — service responsibilities and data flow.
- [DATA-PLATFORM.md](./DATA-PLATFORM.md) — indexed schema, freshness, replay and webhooks.
- [PRODUCTS.md](./PRODUCTS.md) — what each API is for.
- [SECURITY.md](./SECURITY.md) — auth, keys and threat model.
- [OBSERVABILITY.md](./OBSERVABILITY.md) — metrics and dashboards.
- [DEFERRED-OWNER-ACTIONS.md](./DEFERRED-OWNER-ACTIONS.md) — owner follow-ups.
- [DEVELOPER.md](./DEVELOPER.md) — contract addresses and SDK usage.
