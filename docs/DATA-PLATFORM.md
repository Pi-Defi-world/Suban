# Suban Data Platform

Status: canonical data-platform architecture (Phase 2 complete, Phase 3 financial projections partial, October 2026).

The data platform is the indexed-data layer behind Suban's developer promise:
consume Pi and Suban data without running a node. It is **implemented and
tested** in `services/event-indexer`, but it is still **testnet/experimental**:
the public hostname, TLS, production backfill and capacity plan are deployment
actions, not completed production claims.

Service detail and the full route reference live in
[`../services/event-indexer/README.md`](../services/event-indexer/README.md).
This document is the platform-level contract.

## 1. Position in the architecture

```text
Pi Horizon ──┐
             ├──> event-indexer ──> SQLite normalized store ──> data API
Soroban RPC ─┘         │                     │
                       │                     └──> webhook outbox ──> subscribers
                       └──> Prometheus metrics / health
```

- `Suban-api` remains the Horizon-compatible convenience/proxy API.
- `event-indexer` is the indexed historical API. It answers from SQLite and
  never calls Horizon or RPC in a query handler.
- `suban-controller` consumes aggregate market data; it is not part of this
  ingestion path.
- `suban-rpc` is the node-facing JSON-RPC surface; the data platform is useful
  to developers who cannot run it.

## 2. What "normalized" means

One row per ledger, transaction, operation, account observation and contract
event. The schema version is `2.0`; `schema_migrations` records applied
migrations and `indexer_meta.schema_version` is exposed in API responses.

| Table | Meaning |
|---|---|
| `ledgers` | Ledger hash, parent hash, close time, transaction/operation counts, protocol version |
| `transactions` | Hash, ledger, success flag, source/fee account, fee, memo, XDR fields |
| `operations` | Operation id, transaction, ledger, source, type, normalized details |
| `accounts` | Observed account ID, last-seen ledger, optional full snapshot (sequence, balances, signers, thresholds, flags) |
| `events` | Contract event id, contract, type, transaction, ledger, timestamp, decoded data |
| `contracts` | Network/address/name/interface version/verification metadata |
| `checkpoints` | Per-stream replay position |
| `webhook_*` | Subscriptions, durable outbox and delivery state |

Accounts are first observed through transaction/operation activity. A bounded
snapshot fetch fills balances and signers for newly observed accounts. A stub
with no `last_modified_ledger` is an activity observation, not a complete
account state; the API exposes that distinction instead of fabricating balances.

## 3. Ingestion and replay guarantees

Two streams share the schema:

1. `events:<contractId>` from Soroban `getEvents`, starting behind the tip by
   `INDEXER_EVENT_BOOTSTRAP_LEDGERS`.
2. `horizon` from Horizon ledgers/transactions/operations, starting at the tip
   by default (`INDEXER_HORIZON_BOOTSTRAP_LEDGERS=0`).

Every write is idempotent. A replay from a checkpoint reproduces the same rows;
it does not duplicate them. `POST /admin/replay` resets a stream checkpoint.
The replay procedure is documented in the service README.

The indexer compares `prev_hash` against the previously stored ledger hash and
increments `suban_indexer_reorg_detected_total` if they disagree. Pi/Stellar
ledgers are treated as append-only; automatic rollback is not implemented, so a
detected reorg is an operator event requiring replay from a known-good ledger.

## 4. Public API contract

Base (local): `http://localhost:3002`.

All data responses carry:

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

Routes:

| Route | Data source | Notes |
|---|---|---|
| `GET /status` | SQLite | Counts, checkpoints, freshness |
| `GET /contracts` and `/contracts/:id` | `contracts` | Discovery does not depend on scanning events |
| `GET /contracts/:id/events` | `events` | Contract-filtered history |
| `GET /events` | `events` | Time/ledger range + pagination |
| `GET /transactions`, `/transactions/:hash` | `transactions` | `404` if not indexed; never live RPC |
| `GET /operations` | `operations` | Account/transaction/ledger/type filters |
| `GET /accounts/:id` | `accounts` | Stub or full snapshot |
| `GET /accounts/:id/transactions` | join | Account involvement index |
| `GET /accounts/:id/operations` | join | Account involvement index |
| `GET /financial/pools` and `/financial/pools/:id` | `events` | Pool activity, latest reserves, token direction and derived LP positions |
| `GET /financial/pools/:id/trades` | `events` | Swap amounts, token direction and post-trade reserves; USD value is opt-in |
| `GET /financial/pools/:id/liquidity` | `events` | Add/remove-liquidity events with share totals |
| `GET /financial/pools/:id/lp-positions` | `events` | Net LP shares derived from add/remove events |
| `GET /financial/pools/:id/valuation` | `events` + price API | Opt-in USD reserves/volume; `503` unless prices are fresh, HEALTHY and configured |
| `GET /financial/lending/events` | `events` | Deposit/withdraw/borrow/repay/collateral/liquidation/accrual events |
| `GET /financial/lending/accounts/:id` | `events` | Latest indexed position snapshot for one account |
| `GET /financial/lending/summary` | `events` | Lending event counts and latest event |
| `GET /financial/accounts/:id/activity` | `events` | Bounded recent pool activity |
| `GET /financial/summary` | `events` | Pool and lending event counts plus coverage flags |
| `GET /ledgers`, `/ledgers/:sequence` | `ledgers` | Indexed ledger history |

Pagination is `limit`/`offset` (`limit` max 1000). Time filters accept ISO-8601
or Unix seconds. The legacy `/events/contract/:id`, `/events/type/:type` and
`/events/ledger/:ledger` shapes are preserved and wrapped with freshness.

### 4.1 Financial projections (Phase 3, partial)

The `/financial/*` routes project enriched swap, liquidity and lending events.
They remain deliberately conservative:

- CPMM/stableswap swap events carry token direction, post-trade reserves, fee
  and total shares. Liquidity events carry token addresses, post-event reserves
  and total shares.
- LP positions are derived by netting internal add/remove-share events. The
  current pool contracts do not support LP-token transfers, so this is exact for
  them but is not a generic transferable-LP indexer.
- Lending events carry the account position snapshot and pool totals after each
  state change. `/financial/lending/accounts/:id` returns the newest snapshot in
  a bounded scan; it does not read live contract storage.
- `coverage.usd_volume` is `true` only when `INDEXER_PRICE_API_URL`,
  `INDEXER_TOKEN_ASSETS` and `INDEXER_TOKEN_DECIMALS` are configured. The
  valuation route returns `503` for stale, non-HEALTHY or unmapped prices and
  does not backfill a historical USD-volume series.
- Legacy events without the enriched tuple remain readable but report missing
  direction/reserves/LP state rather than inferring them.
- `GET /financial/accounts/:id/activity` scans at most the 5,000 most recent
  financial events because the account is encoded in topic XDR.
- Lending-account lookup scans at most the 10,000 most recent lending events.

These are Phase 3 limits, not implied capabilities. The controller's
`POST /risk/lending` remains a deterministic calculator over supplied amounts;
`GET /risk/lending/indexed/:account` reads the indexed snapshot above when
`DATA_PLATFORM_URL` is configured.

## 5. Freshness and SLOs

`stale` is true when any of these holds:

- `lag_ledgers > INDEXER_MAX_LAG_LEDGERS` (default 10),
- `now - last_success_at > INDEXER_STALE_AFTER_SECONDS` (default 120),
- no successful poll has completed yet.

`/health` returns `200` when healthy and `503` with `status: "degraded"` when
stale. `/metrics` is separate and is not part of the stale decision.

Phase 2 targets:

| Signal | Target / alert |
|---|---|
| Indexer lag | `< 30 s` steady state; page if `suban_indexer_ingest_lag_seconds > 120` for 5m |
| Ledger lag | Alert if `suban_indexer_ingest_lag_ledgers > 10` for 5m |
| Operation completeness | Alert if `suban_indexer_operation_completeness_ratio < 0.99` for 15m |
| Fetch errors | Alert if error rate `> 0.1/s` for 10m |
| Webhook failures | Alert if retry/failure rate `> 0.05/s` for 10m |
| API-key quota pressure | Alert if quota-exceeded rate `> 1/s` for 10m |

The provisioned Grafana dashboard and Prometheus alert rules are in
[`../ops/observability/`](../ops/observability/).

## 6. Authentication, quotas and webhooks

Public data routes support per-key quotas. Keys are configured as hashes
(`INDEXER_API_KEYS`), usage is stored in SQLite, and exhausted keys get `429`
with `Retry-After: 60`. Per-key usage is visible at `GET
/admin/api-keys/usage` (admin key required). Admin routes return `503` when
`INDEXER_ADMIN_API_KEY` is unset.

Webhooks use a durable SQLite outbox. A delivery is unique per
`(subscription, event)`; receivers get `sha256=<HMAC>` signatures, a stable
`Idempotency-Key` and exponential backoff retries. Delivery state is exposed to
operators without payloads.

## 7. Contract metadata registry

`contracts` is independent of event scanning. It records network, address,
name, interface version, verification flag and free-form metadata. The registry
is seeded from deployment config and can be updated through admin routes.
`verified=true` is an operator assertion; it is not source-code verification.

## 8. Known limits

- Testnet/experimental; public DNS/TLS/backfill are not provisioned by code.
- Single-writer SQLite: run one indexer per database.
- Account balances are snapshots, not a historical balance ledger.
- No automatic reorg rollback.
- Webhook secrets live in SQLite; protect the database as a secret.
- No production-sized backfill has been run; default bootstrap windows are
  deliberately small.
- Contract verification is metadata-only.
- Enriched financial events require redeploying the updated contracts and
  reindexing; the existing testnet deployment may still emit legacy payloads.
- USD valuation is opt-in and live-only; it is not a historical USD-volume
  series and returns `503` without fresh, HEALTHY, configured prices.
- LP positions are derived from internal share events, not transferable LP-token
  balances.
- Lending positions are bounded indexed snapshots, not live contract storage.

Owner-blocked actions that are intentionally deferred are tracked in
[`DEFERRED-OWNER-ACTIONS.md`](./DEFERRED-OWNER-ACTIONS.md).
