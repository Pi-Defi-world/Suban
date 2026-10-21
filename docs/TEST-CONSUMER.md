# Test User Consumer

Status: **internal test user**, not production adoption. It exists so plan
execution can exercise the consumer contract without waiting for an external
app. It does not satisfy the Phase 3 production-consumer claim.

## What it does

`tools/test-consumer/consumer.mjs` makes five read-only checks:

1. `GET /api/v1/price?asset=PI` - price, confidence, freshness and trust state.
2. `GET /api/v1/statistics?asset=PI&window=24h` - TWAP, coverage and method
   version. A non-200 is reported as unavailable, never filled in.
3. `POST /api/v1/risk/lending` with test collateral/debt - versioned risk result
   and data-quality status. A non-200 is reported as unavailable.
4. `GET /financial/summary` - counts plus explicit `usd_volume=false` and
   `lp_positions=false` coverage flags.
5. `GET /financial/pools` - pool list plus the same coverage flags.

It sends `x-suban-consumer: test-user-v1` on every request. If `SUBAN_API_KEY`
is set, it also sends `x-api-key` so API-key usage metering applies.

## Run

```bash
CONTROLLER_URL=http://localhost:3000 \
DATA_PLATFORM_URL=http://localhost:3002 \
CONSUMER_REPORT_PATH=./consumer-report.json \
node tools/test-consumer/consumer.mjs
```

Optional environment:

- `SUBAN_API_KEY` - data-platform API key.
- `CONSUMER_REPORT_PATH` - write the JSON report to a file.
- `CONSUMER_STRICT=1` - exit non-zero when the run is degraded.
- `CONSUMER_TIMEOUT_MS` - per-request timeout (default 10s).

Exit code is non-zero for `failed` (malformed required responses or request
failures), and for `degraded` only in strict mode.

## Report

The report has `schema_version: consumer_report_v1`, a `consumer` block, the
target URLs, `status` (`ok`, `degraded` or `failed`), per-check results, observed
facts and warnings. It records explicit coverage limits instead of inventing USD
volume or LP positions.

## Verification

- Node contract tests: `node --test tools/test-consumer/consumer.test.mjs`
  (3 tests: healthy, degraded, malformed).
- CI job `Test consumer` runs those tests on Node 22.
- Live run on 21 October 2026 against the rebuilt controller and indexer:
  `status: ok`, 5/5 checks. The report showed `sample_count: 1`,
  `annualized_volatility: null`, `coverage.lp_positions: false` and
  `coverage.usd_volume: false`.

## Limits

- It is a synthetic user with fixed test amounts; it is not a product UI.
- It does not create a production API key or measure external adoption.
- It tolerates empty financial tables because coverage flags, not fabricated
  activity, are the contract being tested.
