# Suban Observability

Status: canonical observability guide (Phase 2, October 2026).

All four runtime services expose Prometheus metrics and a health endpoint. The
observability pack in [`ops/observability/`](../ops/observability/) runs
Prometheus and Grafana with a provisioned **Suban Platform** dashboard.

## 1. Endpoints

| Service | Metrics | Health | Default port |
|---|---|---|---|
| Controller | `/metrics` | `/api/v1/health` | `3000` |
| Data API (`Suban-api`) | `/metrics` | `/health` | `4000` |
| Data platform (`event-indexer`) | `/metrics` | `/health` | `3002` |
| Bridge relayer | `/metrics` | `/health` | `9464` |
| RPC | admin metrics | admin health | `8001` admin; `8000` JSON-RPC |

`/metrics` is exempt from the data API rate limiter. Metrics should be scraped
from a private network; do not expose the relayer's `9464` port publicly.

## 2. Metric catalog

### 2.1 Controller (`suban_*`)

| Metric | Labels | Meaning |
|---|---|---|
| `suban_price_usd` | `symbol` | Latest aggregated price |
| `suban_price_confidence` | `symbol` | Confidence (0–1) |
| `suban_price_age_ms` | `symbol` | Age of the observation |
| `suban_source_up` | `asset`, `source` | Source usable (1/0) |
| `suban_source_staleness_ms` | `asset`, `source` | Time since last success |
| `suban_price_aggregations_total` | `asset`, `result` | Outcomes (`success`, `cache_hit`, `insufficient_sources`, `circuit_breaker`, `circuit_breaker_triggered`) |
| `suban_oracle_push_total` | `asset`, `result` | Push outcomes (`success`, `failure`, `skipped`, `untrusted`, `unconfigured_asset`) |
| `suban_oracle_trust_state` | `asset`, `state` | 1 for the active trust state |
| `suban_http_requests_total` | `method`, `route`, `status` | HTTP request counts |
| `process_*`, `nodejs_*` | — | Node.js runtime metrics |

### 2.2 Data API (`suban_api_*`)

| Metric | Labels | Meaning |
|---|---|---|
| `suban_api_http_requests_total` | `method`, `route`, `status` | HTTP requests (route normalized; proxy paths collapsed) |
| `suban_api_http_request_duration_seconds` | `method`, `route`, `status` | Request duration histogram |
| `suban_api_cache_events_total` | `result` | `hit`, `miss`, `set` |
| `suban_api_horizon_requests_total` | `method`, `status` | Upstream Horizon requests |
| `suban_api_horizon_request_duration_seconds` | `method`, `status` | Upstream Horizon duration |
| `suban_api_process_*`, `suban_api_nodejs_*` | — | Runtime metrics |

### 2.3 Data platform (`suban_indexer_*`)

| Metric | Labels | Meaning |
|---|---|---|
| `suban_indexer_polls_total` | - | Poll cycles |
| `suban_indexer_events_indexed_total` | `contract`, `event_type` | Events written |
| `suban_indexer_ledgers_indexed_total` | - | Horizon ledgers written |
| `suban_indexer_transactions_indexed_total` | - | Transactions written |
| `suban_indexer_operations_indexed_total` | - | Operations written |
| `suban_indexer_accounts_observed_total` | - | Account identifiers observed |
| `suban_indexer_errors_total` | `stage` | Indexer errors (`events`, `horizon`) |
| `suban_indexer_fetch_errors_total` | `source` | Upstream fetch errors (`soroban-rpc`, `horizon`, `horizon-account`) |
| `suban_indexer_last_ledger` | `contract` | Last indexed ledger per contract |
| `suban_indexer_latest_observed_ledger` | - | Latest ledger observed upstream |
| `suban_indexer_ingest_lag_ledgers` | - | Observed minus indexed ledger |
| `suban_indexer_ingest_lag_seconds` | - | Seconds since the last successful poll |
| `suban_indexer_operation_completeness_ratio` | - | Stored operations / latest ledger operation count |
| `suban_indexer_reorg_detected_total` | `stream` | Parent-hash mismatches |
| `suban_indexer_replay_resets_total` | `stream` | Checkpoint resets requested for replay |
| `suban_indexer_stored_events` | - | Total stored events |
| `suban_indexer_stored_transactions` | - | Total stored transactions |
| `suban_indexer_stored_operations` | - | Total stored operations |
| `suban_indexer_stored_accounts` | - | Total stored accounts |
| `suban_indexer_running` | - | 1 while the poll loop runs |
| `suban_indexer_api_key_requests_total` | `key`, `result` | Public API key outcomes (`allowed`, `quota_exceeded`, `unauthorized`) |
| `suban_indexer_webhook_deliveries_total` | `result` | Webhook outcomes (`delivered`, `retry`, `failed`) |
| `suban_indexer_webhook_duration_seconds` | `result` | Webhook delivery duration histogram |
| `suban_indexer_webhook_pending` | - | Deliveries waiting for first attempt or retry |
| `suban_indexer_http_requests_total` | `method`, `route`, `status` | API requests |
| `suban_indexer_process_*`, `suban_indexer_nodejs_*` | - | Runtime metrics |

### 2.4 Bridge relayer (`suban_relayer_*`)

| Metric | Labels | Meaning |
|---|---|---|
| `suban_relayer_events_seen_total` | `chain` | Burn events observed (`stellar`, `arc`) |
| `suban_relayer_transfers_total` | `direction`, `status` | Transfer outcomes (`stellar->arc`, `arc->stellar`; `completed`, `failed`) |
| `suban_relayer_last_event_timestamp_seconds` | `chain` | Last event time |
| `suban_relayer_running` | — | 1 while running |
| `suban_relayer_processed_total` | — | Events tracked by the processor |
| `suban_relayer_processed_completed` | — | Completed events |
| `suban_relayer_processed_failed` | — | Failed events |
| `suban_relayer_processed_pending` | — | In-flight events |
| `suban_relayer_process_*`, `suban_relayer_nodejs_*` | — | Runtime metrics |

## 3. Prometheus and Grafana

From [`ops/observability/`](../ops/observability/):

```bash
docker compose up -d
# Prometheus: http://localhost:9090
# Grafana:    http://localhost:3001  (admin / value of GRAFANA_ADMIN_PASSWORD)
```

- Prometheus config: `prometheus/prometheus.yml`. Targets use
  `host.docker.internal` for services running on the host. In a Compose
  deployment, replace them with the service DNS names (`controller:3000`,
  `suban-api:4000`, `event-indexer:3002`, `bridge-relayer:9464`).
- Grafana provisioning: datasource `prometheus`, dashboard folder `Suban`.
- Dashboard: `grafana/dashboards/suban-platform.json`, UID `suban-platform`.
  Panels cover service up, HTTP rate/5xx, price by asset, trust state, source
  availability, aggregation/push outcomes, indexer polls/ledger/events/errors,
  ingestion lag, operation completeness, webhook retry/failure pressure, API-key
  quota pressure, relayer events/transfers/last-event age and process memory.
- Alert rules: `prometheus/alerts.yml`, loaded by `prometheus.yml`. Thresholds
  match `DATA-PLATFORM.md` section 5.

The dashboard is provisioned read-only by default; `allowUiUpdates` is enabled
for local editing. Export changes back to the JSON file if you want them kept.

## 4. SLO targets

From `PLAN-EXECUTION.md` §10:

| Metric | Target |
|---|---|
| RPC availability | 99.5% monthly |
| RPC p95 latency | < 500 ms |
| API availability | 99.5% |
| API p95 latency | < 300 ms |
| Controller availability | 99.0% |
| Price freshness | < 60 s for PI/USD; < 5 min for stablecoins |
| Oracle push success | > 99% of attempts |
| Indexer lag | < 30 s |
| Webhook delivery success | > 99% within 5 min (Phase 2) |
| Bridge transfer time | < 5 min p95 (testnet) |
| Critical alerts acknowledged | < 15 min |

These are targets, not current measurements. Measure them before claiming them.

## 5. Suggested alerts

Expressions assume the scrape jobs in `prometheus.yml`. Tune windows for the
deployment.

| Alert | Expression | For |
|---|---|---|
| Controller down | `up{job="suban-controller"} == 0` | 5m |
| Data API down | `up{job="suban-api"} == 0` | 5m |
| Indexer down | `up{job="suban-event-indexer"} == 0 or suban_indexer_running == 0` | 5m |
| Relayer down | `up{job="suban-bridge-relayer"} == 0 or suban_relayer_running == 0` | 5m |
| PI price stale | `suban_price_age_ms{symbol="PI"} > 60000` | 5m |
| Stablecoin price stale | `suban_price_age_ms{symbol=~"USDC|USDT|OUSD"} > 300000` | 5m |
| PI trust not healthy | `suban_oracle_trust_state{asset="PI",state="HEALTHY"} == 0` | 10m |
| Oracle push failing | `increase(suban_oracle_push_total{result="failure"}[10m]) > 0` | 5m |
| Source down | `suban_source_up == 0` | 10m |
| HTTP 5xx ratio high | `sum by (job) (rate({__name__=~"suban_(api_|indexer_)?http_requests_total",status=~"5.."}[5m])) / sum by (job) (rate({__name__=~"suban_(api_|indexer_)?http_requests_total"}[5m])) > 0.01` | 10m |
| Indexer not advancing | `changes(suban_indexer_last_ledger[15m]) == 0 and suban_indexer_running == 1` | 15m |
| Indexer ledger lag high | `suban_indexer_ingest_lag_ledgers > 10` | 5m |
| Indexer stale | `suban_indexer_ingest_lag_seconds > 120` | 5m |
| Indexer operation completeness low | `suban_indexer_operation_completeness_ratio < 0.99` | 15m |
| Indexer reorg signal | `increase(suban_indexer_reorg_detected_total[10m]) > 0` | 1m |
| Indexer fetch errors | `rate(suban_indexer_fetch_errors_total[10m]) > 0.1` | 10m |
| Webhook failures/retries | `sum(rate(suban_indexer_webhook_deliveries_total{result=~"failed|retry"}[10m])) > 0.05` | 10m |
| API-key quota pressure | `sum(rate(suban_indexer_api_key_requests_total{result="quota_exceeded"}[10m])) > 1` | 10m |
| Relayer event starvation | `time() - max(suban_relayer_last_event_timestamp_seconds) > 900 and suban_relayer_running == 1` | 10m |
| Relayer failures | `increase(suban_relayer_transfers_total{status="failed"}[15m]) > 0` | 5m |

## 6. Logs and traces

- All Node services log structured JSON-ish lines with timestamp, level and
  service. Do not log secrets, signatures or full signed transactions.
- Controller logs source failures, aggregation outcomes, trust transitions,
  oracle push results and HTTP errors.
- Relayer logs each observed burn event and each completed/failed transfer.
- Indexer logs poll errors, reorg signals, webhook delivery failures and account
  snapshot failures. Event payloads are stored, not logged by default.
- Distributed tracing is not implemented; use request correlation at the edge
  when added.

## 7. Operational checklist

- [ ] Prometheus scrapes all four jobs and `up == 1`.
- [ ] Grafana dashboard loads and shows the four services.
- [ ] Alerts route to a real on-call channel before claiming an SLO.
- [ ] `/metrics` is reachable only from the monitoring network.
- [ ] Dashboard JSON is reviewed when metric names or labels change.
- [ ] Metrics retention is sized for the incident-review window (15 days here).
