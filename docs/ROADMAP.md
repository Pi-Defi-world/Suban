# Suban Roadmap

Status: canonical roadmap (Phase 2, October 2026).

Suban is sequenced **risk removal → data → financial products → execution and
capital**. Later phases are gated by earlier exit criteria; they are not
calendar promises. The canonical strategy narrative remains
[`plan.md`](../plan.md); this document tracks what is actually being executed in
[`PLAN-EXECUTION.md`](../PLAN-EXECUTION.md).

## 1. Current position

| Phase | Theme | Status (21 Oct 2026) | Exit criterion |
|---|---|---|---|
| 0 | Stabilize and make claims true | **Complete except owner actions** | No known critical defect; CI green; no secrets in the tree |
| 1 | Consolidate L0/L2 | **Complete** | Docs match code; multi-asset persistent prices; all services have metrics; no contradictions |
| 2 | Data platform (L1) | **Implemented; deployment/owner actions pending** | External developer can build an explorer/analytics app using only Suban APIs |
| 3 | Financial data and risk (L2) | **Partial (21 Oct 2026)** | At least one application consumes Suban financial data/oracle with freshness/confidence visible |
| 4 | Execution and controlled capital | Not started | Measurable usage justifies one capital/execution product; audit complete before mainnet capital |
| 5 | 2027 portfolio selection | Not started | Choose the winning product from evidence |

## 2. Phase 0 — Stabilize (complete except owner actions)

| Task | Status | Evidence |
|---|---|---|
| Freeze new bridge/mainnet capital work | Process decision | Recorded in `PLAN-EXECUTION.md`; no new capital features merged |
| Fix or disable oracle push | Done | Soroban prepare/sign/send/poll, all five `set_price` args, opt-in |
| Fix bridge mint verification | Done | Per-signer `require_auth`, duplicate/non-signer rejection, 9 tests; Arc→Stellar disabled |
| Rotate leaked secrets | **Owner action pending** | Tree clean; keys still need rotation and history purge (`docs/SECRET-ROTATION.md`) |
| Protect controller control routes | Done | Constant-time API key, per-IP rate limit, `503` if unset |
| Bridge contract in workspace | Done | `bridge-burn-mint` is a workspace member on `soroban-sdk 27` |
| CI in all four repos | Done | Green on `main` |
| Secret scanning and dependency audit | Partial | gitleaks working-tree blocking, history report-only; dependency audit pending |

**Blocking owner actions:** rotate the Stellar deployer, PUSD/oracle admin, Arc
relayer, Pi node seeds, Cloudflare tunnel token and faucet keys; then approve the
destructive git-history purge. Until then Phase 0 is not fully closed.

## 3. Phase 1 — Consolidate L0/L2 (current)

| # | Task | Status | Remaining work |
|---|---|---|---|
| 1.1 | Canonical docs | **Done** | `ARCHITECTURE.md`, `ROADMAP.md`, `PRODUCTS.md`, `SECURITY.md`, `API.md`, `OBSERVABILITY.md` |
| 1.2 | Ownership matrix | **Done** | `OWNERSHIP.md`: one accountable role and one deployment target per module |
| 1.3 | Multi-asset price model | **Done** | PI/USDC/USDT/oUSD registry, per-asset sources, cache, breaker and trust |
| 1.4 | Persist price observations | Done | SQLite, 30-day retention, `/history` |
| 1.5 | Provenance and freshness | Done | Versioned schema, `age_ms`, `is_fresh`, per-source freshness |
| 1.6 | Oracle trust states | Done | `HEALTHY`/`DEGRADED`/`BREAKER`/`RECOVERY`, transitions exposed |
| 1.7 | Metrics and dashboards | **Done** | `/metrics` on controller, API, indexer and relayer; Prometheus + Grafana pack |
| 1.8 | Resolve `getLedgerEntries` | **Done** | `suban-rpc/COMPATIBILITY.md` and README aligned with the Go runtime path; preflight test limitation documented |

**Phase 1 close-out checklist**

- [x] Controller serves `/api/v1/assets` and `?asset=` on all price routes.
- [x] oUSD has no fabricated peg; unconfigured assets return `503`.
- [x] Four services expose `/metrics`; dashboard JSON validates.
- [x] Canonical docs exist and are linked from the root README.
- [x] Production dependency audits are blocking in CI; full/dev audits are report-only.
- [ ] Add an on-chain testnet integration test for the controller oracle push (owner-blocked, see `DEFERRED-OWNER-ACTIONS.md`).
- [ ] Complete owner secret rotation and history purge (owner-blocked, see `DEFERRED-OWNER-ACTIONS.md`).

## 4. Phase 2 — Data platform (L1)

**Goal:** developers can consume Pi data without running a node.

| # | Task | Status | Evidence |
|---|---|---|---|
| 2.1 | Normalize event/account/tx/operation data | **Done** | `services/event-indexer/src/db.ts` versioned schema `2.0` with additive Phase 1 upgrade; migrations tested |
| 2.2 | Expand indexer ingestion | **Done** | Horizon ledgers/transactions/operations/accounts + Soroban events; checkpointed idempotent replay tested |
| 2.3 | Indexer operations | **Done** | Lag/completeness/reorg/replay metrics; Grafana panels + Prometheus alert rules |
| 2.4 | Contract metadata registry | **Done** | `contracts` table + admin API; discovery independent of event scans |
| 2.5 | Public API keys and quotas | **Done** | SHA-256 keys, minute/day counters, `429` + `Retry-After`, `/admin/api-keys/usage` |
| 2.6 | Webhooks | **Done** | Durable outbox, HMAC, idempotency key, exponential backoff, delivery state API |
| 2.7 | Historical query API | **Done** | `/events`, `/transactions`, `/operations`, `/accounts`, `/ledgers`; SQLite-only handlers |
| 2.8 | Freshness guarantees | **Done** | `schema_version` + `freshness` on every data response; `/health` `503` when stale |

**Exit:** an external developer can build an explorer or analytics app using only
Suban APIs. Implementation is complete and the route contract is documented in
[`DATA-PLATFORM.md`](./DATA-PLATFORM.md) and
[`../services/event-indexer/README.md`](../services/event-indexer/README.md).
Public deployment (DNS/TLS/backfill/capacity) and the owner actions in
[`DEFERRED-OWNER-ACTIONS.md`](./DEFERRED-OWNER-ACTIONS.md) remain open; this
document does not claim a live public data platform.

## 5. Phase 3 — Financial data and risk (L2)

**Goal:** make financial data real, but derived from a reliable data layer.

Status: **partial**. The calculation and projection layer is implemented and
tested; an internal test-user consumer exercises it, but the production
consumer/exit criterion is not met.

| # | Task | Status | Evidence / gap |
|---|---|---|---|
| 3.1 | Financial data API | Partial | `/financial/pools`, `/trades`, `/liquidity`, `/accounts/:id/activity`, `/summary` project indexed events. USD volume and LP positions are unavailable because events do not carry token direction/decimals or LP balances. Lending events do not exist yet. |
| 3.2 | Historical price series | Done | SQLite persistence with 30-day retention and `/history`; restart test covers reopen. |
| 3.3 | TWAP and volatility | Done | `twap_volatility_v1` in `suban-controller/src/services/priceStats.ts`; pure deterministic tests and `/statistics`, `/twap`, `/volatility`. |
| 3.4 | Oracle extension | Partial | Provenance, confidence, freshness and trust are exposed; `/methodology` is versioned. `liquidity_aware` is explicitly `false`; no pool-depth input exists. |
| 3.5 | Risk engine v0 | Done | `lending_risk_v1` in `suban-controller/src/services/risk.ts`; deterministic vectors for health factor, collateral, max borrow and liquidation price. |
| 3.6 | Risk API | Done | `/api/v1/risk/methods` and `POST /api/v1/risk/lending`; unknown/unconfigured/missing/stale prices return `404`/`503` without partial fabricated values; fresh non-HEALTHY prices are labelled degraded. |
| 3.7 | Consumer integration | Partial | `tools/test-consumer/consumer.mjs` is an internal test user. It checks price provenance, TWAP, risk and financial coverage, records a JSON report and runs in CI. A live run passed 5/5 checks. Production/external usage is still absent. |

**Exit:** at least one financial application consumes Suban financial data or
oracle services, with freshness/confidence visible to it. Partially met by the
internal test user; production/external usage is not claimed. Phase 4 is limited
to a preview under an explicit gate override.

**Known gaps (accepted, not hidden):**

- Pool events omit pool address, reserve state, token direction/decimals and
  reliable initial LP positions.
- `lending-pool` has no events, so positions, liquidations and on-chain risk
  inputs cannot be derived.
- The risk API is a calculator over caller-supplied amounts and live prices; it
  does not read lending state.
- The internal test user records consumer-side usage, but there is no
  production/external consumer or adoption measurement.


## 6. Phase 4 — Execution and controlled capital

**Gate:** Phase 3 exit met, data platform stable, oracle reliable, at least one
consumer, no open P0/P1 defects.

Status: **preview started (21 Oct 2026) under an explicit owner-requested gate
override**. The owner asked to use an internal test user instead of waiting for
an external consumer and to skip owner-only actions. This does not satisfy the
gate and does not authorize capital. Only the deterministic SDK CPMM quote
preview exists; no deployed `/quote` service, vault, liquidation or capital work
is claimed.

| # | Task | Acceptance |
|---|---|---|
| 4.1 | Quote service | Matches on-chain simulation within tolerance; SDK preview only |
| 4.2 | Simulation API | Reproducible preflight + price impact; SDK impact preview only |
| 4.3 | Route discovery | Only across real liquidity; no synthetic venues |
| 4.4 | Vault framework (testnet) | Caps, pause, strategy isolation; deposit/withdraw/pause tests pass |
| 4.5 | Lending hardening | Oracle integration and liquidation tests end-to-end |
| 4.6 | Liquidation bot prototype | Detects and executes a test liquidation on testnet |
| 4.7 | Audit prep | Scope, threat model, test evidence, auditor engaged |

**Exit:** measurable usage justifies one capital/execution product; audit
complete before any mainnet capital.

## 7. Phase 5 - 2027 portfolio selection

Status: **not started**. It depends on the unmet Phase 3 exit and Phase 4 gate.

| Quarter | Focus |
|---|---|
| Q1 2027 | Data platform exit |
| Q2 2027 | Financial data + risk exit |
| Q3 2027 | Execution + capital experiments |
| Q4 2027 | Choose the winning product from evidence |

Do not start a later phase before the previous exit criterion is met.

## 8. Explicit non-goals until gates pass

- No mainnet capital deployment.
- No yield promises or vault launches.
- No new repository unless lifecycle, security boundary, runtime or ownership
  genuinely differs.
- No additional bridge direction until signer authorization entries exist.
- No "TVL" or "volume" marketing metric without an indexing source of truth.
- No external protocol (Aquarius, Blend, etc.) declared the winning product in
  advance.

## 9. Roadmap change control

A phase is marked complete only when its exit criterion is evidenced in
`PLAN-EXECUTION.md`, CI is green, and the relevant docs are updated in the same
change. Roadmap changes that move a gate require an explicit note in
`PLAN-EXECUTION.md`; silent re-scoping is not allowed.
