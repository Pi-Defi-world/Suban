# Suban API

Status: canonical API index (Phase 1, October 2026).

This is the entry point for API consumers. Detailed per-service documents are
linked below; when they disagree with this index, the service document wins for
its own routes and this index is updated in the same change.

## 1. Services and base URLs

| Service | Default local base URL | Public target | Detailed docs |
|---|---|---|---|
| Controller (market data/oracle) | `http://localhost:3000` | `https://oracle.suban.org` | [`suban-controller/docs/API.md`](../../suban-controller/docs/API.md) |
| Data API (Horizon-compatible) | `http://localhost:4000` | `https://horizon.suban.org` | [`../Suban-api/README.md`](../../Suban-api/README.md) |
| Event indexer | `http://localhost:3002` | internal service | [`../services/event-indexer/README.md`](../services/event-indexer/README.md) |
| Bridge relayer (metrics/health only) | `http://localhost:9464` | internal worker | [`../docs/OBSERVABILITY.md`](./OBSERVABILITY.md) |
| RPC | `http://localhost:8000` | `https://rpc.suban.org` | [`../../suban-rpc/README.md`](../../suban-rpc/README.md) |

All HTTP APIs return JSON unless noted. Timestamps are ISO-8601 UTC strings.
Endpoints marked **authenticated** require a shared secret and are not part of
the public developer surface.

## 2. Versioning and compatibility

- Public HTTP routes are versioned under `/api/v1` (data API and controller).
- The controller price payload carries `schema_version` (`1.0` today) and
  `method_version` (`weighted_average_v1`). Breaking shape changes bump
  `schema_version`; methodology changes bump `method_version`.
- Adding a field is backward-compatible. Removing/renaming a field is not.
- Error responses use `{ "error": "message" }`; controller price errors may add
  `asset` and `available_assets`.
- Unknown assets return `404`; known but unconfigured assets return `503`. Do
  not treat `503` as a zero price.

## 3. Authentication and rate limits

| Surface | Auth | Limits |
|---|---|---|
| Public read routes | None | Per-IP rate limit at the edge and in the service |
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

## 6. Event indexer

Base: `http://localhost:3002`.

| Route | Purpose |
|---|---|
| `GET /` | Service metadata and endpoint list |
| `GET /status` | Indexer status (running, contracts, counts) |
| `GET /stats` | Event counts and per-contract stats |
| `GET /events` | Recent events (`limit`, `offset`) |
| `GET /events/contract/:contractId` | Events by contract |
| `GET /events/type/:eventType` | Events by type |
| `GET /events/ledger/:ledger` | Events at a ledger |
| `GET /health` | Liveness/degraded |
| `GET /metrics` | Prometheus metrics (`suban_indexer_*`) |

The indexer is a derived, replayable view. Consumers should use the `ledger` and
`timestamp` fields to detect lag rather than assuming real-time delivery.

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
- [PRODUCTS.md](./PRODUCTS.md) — what each API is for.
- [SECURITY.md](./SECURITY.md) — auth, keys and threat model.
- [OBSERVABILITY.md](./OBSERVABILITY.md) — metrics and dashboards.
- [DEVELOPER.md](./DEVELOPER.md) — contract addresses and SDK usage.
