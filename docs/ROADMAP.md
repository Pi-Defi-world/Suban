# Suban Roadmap

Status: canonical roadmap (Phase 1, October 2026).

Suban is sequenced **risk removal → data → financial products → execution and
capital**. Later phases are gated by earlier exit criteria; they are not
calendar promises. The canonical strategy narrative remains
[`plan.md`](../plan.md); this document tracks what is actually being executed in
[`PLAN-EXECUTION.md`](../PLAN-EXECUTION.md).

## 1. Current position

| Phase | Theme | Status (20 Oct 2026) | Exit criterion |
|---|---|---|---|
| 0 | Stabilize and make claims true | **Complete except owner actions** | No known critical defect; CI green; no secrets in the tree |
| 1 | Consolidate L0/L2 | **In progress** | Docs match code; multi-asset persistent prices; all services have metrics; no contradictions |
| 2 | Data platform (L1) | Not started | External developer can build an explorer/analytics app using only Suban APIs |
| 3 | Financial data and risk (L2) | Not started | At least one application consumes Suban financial data/oracle with freshness/confidence visible |
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
- [ ] Add dependency audit (`npm audit` / `cargo audit`) to CI (carry-over from 0.8).
- [ ] Add an on-chain testnet integration test for the controller oracle push.
- [ ] Complete owner secret rotation and history purge.

## 4. Phase 2 — Data platform (L1)

**Goal:** developers can consume Pi data without running a node.

| # | Task | Acceptance |
|---|---|---|
| 2.1 | Normalize event/account/tx/operation data | Versioned schema + migrations, documented and tested |
| 2.2 | Expand indexer ingestion | Accounts, transactions, operations, events; replay from checkpoint reproduces state |
| 2.3 | Indexer operations | Lag, completeness, reorg/replay metrics with dashboard and alert thresholds |
| 2.4 | Contract metadata registry | Network/address/interface/version/verification; discovery no longer depends on event scans |
| 2.5 | Public API keys and quotas | Per-key usage visible and enforceable |
| 2.6 | Webhooks | Subscription + outbox + retries; idempotent delivery |
| 2.7 | Historical query API | Time-range queries served from indexed data |
| 2.8 | Freshness guarantees | Freshness/lag fields on every response |

**Exit:** an external developer can build an explorer or analytics app using only
Suban APIs.

## 5. Phase 3 — Financial data and risk (L2)

**Goal:** make financial data real, but derived from a reliable data layer.

| # | Task | Acceptance |
|---|---|---|
| 3.1 | Financial data API | Pools, liquidity, volume, trades, LP positions; at least one real consumer |
| 3.2 | Historical price series | 30-day query works after restart |
| 3.3 | TWAP and volatility | Methodology versioned and reproducible from stored observations |
| 3.4 | Oracle extension | Provenance/confidence/liquidity-aware pricing, documented |
| 3.5 | Risk engine v0 | Health factor, collateral value, liquidation price; deterministic vectors |
| 3.6 | Risk API | `/risk/*`; degraded when inputs are missing, never fabricated |
| 3.7 | Consumer integration | One internal or external app; usage recorded |

**Exit:** at least one financial application consumes Suban financial data or
oracle services, with freshness/confidence visible to it.

## 6. Phase 4 — Execution and controlled capital

**Gate:** Phase 3 exit met, data platform stable, oracle reliable, at least one
consumer, no open P0/P1 defects.

| # | Task | Acceptance |
|---|---|---|
| 4.1 | Quote service | Matches on-chain simulation within tolerance |
| 4.2 | Simulation API | Reproducible preflight + price impact |
| 4.3 | Route discovery | Only across real liquidity; no synthetic venues |
| 4.4 | Vault framework (testnet) | Caps, pause, strategy isolation; deposit/withdraw/pause tests pass |
| 4.5 | Lending hardening | Oracle integration and liquidation tests end-to-end |
| 4.6 | Liquidation bot prototype | Detects and executes a test liquidation on testnet |
| 4.7 | Audit prep | Scope, threat model, test evidence, auditor engaged |

**Exit:** measurable usage justifies one capital/execution product; audit
complete before any mainnet capital.

## 7. Phase 5 — 2027 portfolio selection

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
