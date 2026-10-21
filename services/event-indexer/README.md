# Suban Data Platform (`event-indexer`)

Status: Phase 2 implementation, October 2026. Testnet/experimental. The service
is production-shaped (versioned schema, checkpoints, idempotent replay, quotas,
webhooks) but the public hostname and production-sized backfill are deployment
actions, not completed here.

The data platform indexes Pi ledgers, transactions, operations, accounts and
Suban contract events into SQLite, then serves historical queries without
touching Horizon or Soroban RPC. It is the indexed-data layer behind the
developer promise "consume Pi data without running a node".

## 1. What is implemented

| Capability | Implementation |
|---|---|
| Versioned schema | `src/db.ts` with `schema_migrations` and additive upgrades from Phase 1 |
| Event ingestion | Soroban RPC `getEvents`, per-contract checkpoints, deterministic event IDs |
| Ledger ingestion | Horizon ledgers, transactions, operations, account activity |
| Account snapshots | Horizon account records for newly observed accounts, bounded per poll |
| Replay | `POST /admin/replay` resets a stream checkpoint; all writes are idempotent |
| Historical API | `/events`, `/transactions`, `/operations`, `/accounts/:id`, `/ledgers` |
| Contract registry | SQLite `contracts` table, seeded from config and editable via admin API |
| API keys and quotas | SHA-256 key hashes, minute/day counters, `429` with `Retry-After` |
| Webhooks | Durable outbox, HMAC signatures, idempotency keys, exponential backoff |
| Freshness | `schema_version` + `freshness` on every data response |
| Operations | Lag/completeness/replay/reorg metrics and Prometheus alert rules |

## 2. Quick start

```bash
cp .env.example .env
# Set HORIZON_URL and, if you want contract events, the contract IDs.
npm ci
npm run build
npm test
npm start
```

The process listens on `INDEXER_API_PORT` (default `3002`) and exposes
`/health`, `/metrics` and the data routes. `/metrics` and `/health` are exempt
from public API-key auth.

The Docker image builds the native `better-sqlite3` binding on Node 22:

```bash
docker build -t suban-data-platform .
docker run --rm -p 3002:3002 --env-file .env suban-data-platform
```

Local development on Node 24 may not build the pinned native SQLite binding.
Use Node 22 (CI and Docker do).

## 3. Ingestion model

Two checkpointed streams feed the same schema:

- `events:<contractId>` - Soroban contract events from Stellar RPC.
- `horizon` - ledgers, transactions, operations and account activity from
  Horizon.

On first start with no checkpoint, event ingestion starts
`INDEXER_EVENT_BOOTSTRAP_LEDGERS` behind the current RPC ledger. Horizon starts
`INDEXER_HORIZON_BOOTSTRAP_LEDGERS` behind the current ledger (default `0`, i.e.
the current ledger). Set it higher for a deliberate backfill, then lower it.
Ingestion is bounded by `INDEXER_MAX_LEDGERS_PER_POLL`.

Every write is `INSERT OR IGNORE` or an explicit upsert. Replaying a checkpoint
therefore produces the same rows, not duplicates. Account rows are first
created as activity stubs; account balances/signers are filled by a bounded
Horizon snapshot fetch (`INDEXER_FETCH_ACCOUNT_SNAPSHOTS` and
`INDEXER_ACCOUNT_FETCH_LIMIT`). An account can legitimately have a stub without
balances when its snapshot has not been fetched yet; `last_modified_ledger`
distinguishes a full snapshot from a stub.

Pi/Stellar ledgers are treated as append-only. The ingester still compares the
previous ledger hash with `prev_hash` and emits
`suban_indexer_reorg_detected_total` if they disagree. Automatic rollback is not
implemented; a detected split is an operator event requiring replay from a
known-good ledger.

## 4. Schema (`schema_version` 2.0)

| Table | Purpose | Key |
|---|---|---|
| `schema_migrations` | Applied migration versions | `version` |
| `events` | Normalized contract events | `event_id` unique |
| `ledgers` | Horizon ledger headers and counts | `sequence` |
| `transactions` | Normalized transactions | `hash` |
| `operations` | Normalized operations | `id` |
| `transaction_accounts` | Account involvement index for transactions | `(tx_hash, account_id)` |
| `operation_accounts` | Account involvement index for operations | `(operation_id, account_id)` |
| `accounts` | Observed accounts, optional full snapshot | `account_id` |
| `contracts` | Network/address/interface/verification registry | `contract_id` |
| `checkpoints` | Per-stream replay position | `stream` |
| `indexer_meta` | Schema version, observed/indexed ledger, poll times | `key` |
| `api_key_usage` | Per-key minute/day counters | `(key_id, window_kind, window_start)` |
| `webhook_subscriptions` | Webhook endpoints and filters | `id` |
| `webhook_deliveries` | Durable outbox and retry state | `id`, unique `(subscription_id, event_id)` |

The upgrade path from the Phase 1 events database is additive: existing events
are preserved, and `event_id`, `ledger_closed_at` and `raw` columns are added.

## 5. Public API

All data routes return `schema_version` and a `freshness` object:

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

| Route | Description |
|---|---|
| `GET /status` | Counts, checkpoints, freshness, Horizon state |
| `GET /contracts` | Contract registry |
| `GET /contracts/:contractId` | One registry entry |
| `GET /contracts/:contractId/events` | Events for one contract |
| `GET /events` | `contract`, `type`, `from`, `to`, `ledger_from`, `ledger_to`, `limit`, `offset` |
| `GET /events/contract/:id` | Legacy shape, wrapped with freshness |
| `GET /events/type/:type` | Legacy shape, wrapped with freshness |
| `GET /events/ledger/:ledger` | Legacy shape, wrapped with freshness |
| `GET /transactions` | `account`, `ledger`, `from`, `to`, `successful`, `limit`, `offset` |
| `GET /transactions/:hash` | Indexed transaction only; `404` if absent (never live RPC) |
| `GET /operations` | `account`, `tx_hash`, `ledger`, `type`, `from`, `to`, `limit`, `offset` |
| `GET /accounts/:accountId` | Indexed account/stub and optional snapshot |
| `GET /accounts/:id/transactions` | Transactions involving an account |
| `GET /accounts/:id/operations` | Operations involving an account |
| `GET /ledgers` | Recent indexed ledgers |
| `GET /ledgers/:sequence` | One indexed ledger |

Pagination is limit/offset (`limit` max 1000, default 100). Time filters accept
ISO-8601 or Unix seconds. Response keys are snake_case.

`stale` is true when `lag_ledgers > INDEXER_MAX_LAG_LEDGERS` (default 10), when
the last successful poll is older than `INDEXER_STALE_AFTER_SECONDS` (default
120), or when no successful poll has happened yet. `/health` returns `503` with
`status: "degraded"` in that state.

## 6. Authentication and quotas

Set `INDEXER_REQUIRE_API_KEY=true` for a public deployment. Keys are supplied in
`INDEXER_API_KEYS`; only SHA-256 hashes are retained in memory and usage rows use
a non-secret key ID.

```bash
# JSON form
INDEXER_API_KEYS='[{"key":"...","name":"demo","perMinute":60,"perDay":10000}]'
# Compact form: key:name:perMinute:perDay
INDEXER_API_KEYS='key-one:demo:60:10000,key-two:partner:120:50000'
```

Clients send `x-api-key` or `Authorization: Bearer <key>`. Responses include
`X-RateLimit-Limit-Minute`, `X-RateLimit-Remaining-Minute`,
`X-RateLimit-Limit-Day` and `X-RateLimit-Remaining-Day`. Exhausted keys receive
`429` and `Retry-After: 60`. Usage is visible at `GET /admin/api-keys/usage`.

Admin routes require `INDEXER_ADMIN_API_KEY`. If it is unset, admin routes
return `503` rather than falling open.

## 7. Webhooks

Create a subscription with the admin key:

```bash
curl -X POST http://localhost:3002/admin/webhooks \
  -H 'x-api-key: <admin-key>' -H 'content-type: application/json' \
  -d '{"url":"https://example.com/hook","event_types":["swap"],"contract_ids":[]}'
```

The response contains the signing `secret` once. Store it server-side. Deliveries
include:

- `X-Suban-Signature: sha256=<hex HMAC of the raw body>`
- `X-Suban-Delivery: wd_...`
- `Idempotency-Key: delivery:wd_...`
- `X-Suban-Event: <event_id>`

The outbox is durable in SQLite. Failed deliveries retry with exponential
backoff from `INDEXER_WEBHOOK_BASE_BACKOFF_MS` up to
`INDEXER_WEBHOOK_MAX_BACKOFF_MS`, stopping after
`INDEXER_WEBHOOK_MAX_ATTEMPTS` attempts. Receivers should treat the idempotency
key as stable and return `2xx` only after the event is durably accepted.

`GET /admin/webhooks/deliveries?status=pending|failed|delivered` exposes
delivery state without payloads.

## 8. Admin operations

| Route | Action |
|---|---|
| `POST /admin/replay` | `{"stream":"horizon","from_ledger":29329000}` |
| `GET /admin/checkpoints` | Current stream positions |
| `POST /admin/contracts` | Add/update contract metadata |
| `DELETE /admin/contracts/:id` | Remove registry entry |
| `GET /admin/webhooks` | List subscriptions (secrets omitted) |
| `POST /admin/webhooks` | Create subscription |
| `DELETE /admin/webhooks/:id` | Delete subscription and deliveries |
| `GET /admin/webhooks/deliveries` | Delivery state |
| `GET /admin/api-keys/usage` | Per-key minute/day usage |

Replay sequence:

1. Choose a known-good ledger before the suspected gap.
2. `POST /admin/replay` with the stream and ledger.
3. Let the normal poll loop advance. Existing rows are ignored; missing rows are
   inserted.
4. Verify counts and `freshness.lag_ledgers` return to normal.

## 9. Metrics and alerts

The full catalog and alert thresholds are in
[`../../docs/OBSERVABILITY.md`](../../docs/OBSERVABILITY.md). Phase 2 adds:

- `suban_indexer_ledgers_indexed_total`
- `suban_indexer_transactions_indexed_total`
- `suban_indexer_operations_indexed_total`
- `suban_indexer_accounts_observed_total`
- `suban_indexer_ingest_lag_ledgers`
- `suban_indexer_ingest_lag_seconds`
- `suban_indexer_operation_completeness_ratio`
- `suban_indexer_reorg_detected_total`
- `suban_indexer_replay_resets_total`
- `suban_indexer_api_key_requests_total`
- `suban_indexer_webhook_deliveries_total`
- `suban_indexer_webhook_pending`
- `suban_indexer_webhook_duration_seconds`

## 10. Known limits

- This is testnet/experimental. The public `data.suban.org` hostname is a
  deployment target, not a claim that production DNS/TLS/backfill is complete.
- No automatic reorg rollback; a detected reorg requires operator replay.
- Account balances are snapshots, not a full historical balance ledger.
- The SQLite store is single-writer. Use one indexer process per database.
- Webhook secrets are stored in SQLite. Protect the database file as a secret.
- Contract registry verification is metadata only; it does not verify source
  code or bytecode.
- Full historical backfill from genesis is an operator choice; default bootstraps
  are intentionally small.
