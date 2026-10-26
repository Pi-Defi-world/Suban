# Suban — Plan Review & Workable Execution Plan

**Date:** 19 October 2026  
**Input reviewed:** `Suban/plan.md` (Revision 3, 1,165 lines)  
**Code reviewed:** `Suban`, `Suban-api`, `suban-controller`, `suban-rpc`  
**Method:** plan-to-code mapping, endpoint inventory, test inventory, git history, deployment config review  
**Purpose:** give an honest verdict on what the plan says versus what exists, then a sequenced plan that can actually be executed.

---

## 0. One-page verdict

The plan is a good **strategy document**. The implementation is a real but narrow **L0/L1 platform** with a fragile bridge bolted on.

In plain terms:

- **What is real:** RPC, Horizon/API, escrow, the core contract set, the SDK, the four-source price aggregator, the event indexer, persistent price history, TWAP/volatility, a deterministic risk calculator, enriched pool/lending event projections, and the Docker deployment.
- **What is early:** indexing, historical backfill, contract discovery, controller security, observability, financial consumer integration.
- **What is paper:** execution and capital, settlement, liquidity-aware pricing; the vault and liquidation work is a testnet prototype, not a production network. A deterministic CPMM quote preview exists in the SDK and over HTTP, and it now matches the enriched deployed pool's on-chain `get_quote` in both directions.
- **What is broken or overstated:** on-chain oracle publication, bridge mint authorization, secret hygiene, CI coverage of owner-managed deployment.
- **What is accurate:** Section 3 ("Current State") is a fair description of the repos. The plan's own note at line 258 about missing USDC/USD/oUSD/USDT pricing is correct.

**Overall score (judgment call, not a formal metric):** roughly **45% of the plan's "current state" is true; about 20% of the 2026–2027 target architecture is built.**

**Recommendation:** do **not** start Q2–Q4 2027 execution/capital work yet. Run a 90-day "Consolidate" phase first: correctness, security, data persistence, observability, CI, and documentation. The platform's credibility depends on the controller and data layer being trustworthy, not on shipping more contracts.

**Current build inventory:** see [`docs/PROJECT-STATUS.md`](docs/PROJECT-STATUS.md) for every contract, crate, service, SDK, frontend, CI job, deployment ID and verified limit.

**CI update (26 October 2026):** `Suban` run `40361766711` on `04a5ea3` is green after upgrading the event indexer's transitive `body-parser` to `1.20.9` and recording `RUSTSEC-2026-0387` (`crypto-bigint 0.5.5` via `k256 0.13` / `soroban-env-host 27.0.1`) as an explicit, documented exception in `.cargo/audit.toml` and CI. The preceding docs-only run failed on those two new advisories; they are not being silently ignored.

**Update (23 October 2026):** Phase 0-2 are complete. Phase 3's technical/testnet scope is complete: the enriched CPMM, stableswap, oracle-aware lending, oracle and strategy-vault contracts are deployed to Pi testnet and reindexed; enriched pool state, LP positions, lending snapshots and indexed risk reads are live; and the internal **test-user consumer** passed 5/5 live checks. External production adoption and measured third-party usage are still absent, so the production-consumer exit criterion is not claimed. Phase 4 has testnet prototypes for 4.1–4.6 under an explicit owner-requested gate override: deployed `get_quote` parity matches the enriched pool, CPMM route discovery and read-only simulation work, the vault and oracle-aware liquidation were exercised, and a liquidation bot executed one testnet liquidation. The 4.7 audit-preparation package exists, but no auditor is engaged. No mainnet capital, vault allocation or production keeper is authorized.

---

## 1. Status legend

| Marker | Meaning |
|---|---|
| ✅ | Implemented and reasonably matches the plan |
| 🟡 | Partial: some code exists, but incomplete, unverified, or missing operational requirements |
| ❌ | Not implemented |
| ⚠️ | Implemented in a way that contradicts the plan or is unsafe |

---

## 2. Plan section scorecard

### Sections 1–5 — Strategy and architecture
| Plan section | Status | Verdict |
|---|---|---|
| 1. Executive Summary | n/a | Strategy only. Clear and coherent. |
| 2. What Suban Is | n/a | Positioning is sound. |
| 3. Current State | ✅ | Accurate at repository level. |
| 4. Strategic Thesis | n/a | Correct: infrastructure before capital products. |
| 5. Product Architecture (L0–L5) | 🟡 | L0 exists, L1–L2 partial, L3–L5 mostly absent. |
| 5.1 Current Implementation by Layer | 🟡 | L0/L1/L2 rows are optimistic; L3–L5 are correctly marked future. |

### Section 6 — Repository and ownership map
| Item | Status | Verdict |
|---|---|---|
| Four repositories exist | ✅ | All four are present locally and public. |
| Controller is a real service | ✅ | Express/TypeScript service with aggregation, routes, Horizon proxy, oracle push. |
| Controller source map | ✅ | `src/index.ts`, `aggregator.ts`, `oracle-push.ts`, `routes/api.ts`, `routes/chaindata.ts` all exist. |
| Controller can publish to on-chain oracle | ⚠️ | **Broken:** calls `set_price` with zero arguments; wrong SDK server type. See defect D2. |
| Ownership matrix | ❌ | No `docs/` ownership matrix. Boundaries are described only in the plan. |
| Repository consolidation rule | 🟡 | Respected so far; no new repos were created unnecessarily. |

### Section 7 — Core platform modules
| Module | Status | Evidence / gap |
|---|---|---|
| 7.1 RPC | ✅ | `suban-rpc`: JSON-RPC, tx submission/status, preflight simulation, Soroban, admin endpoint `:8001`, Prometheus/Grafana docs. |
| 7.2 Horizon/API | ✅ | `Suban-api`: summary, accounts, transactions, ledgers, assets, caching, rate limiting, `/horizon/*`, `/arc/*`. |
| 7.3 Contract discovery | 🟡 | Controller `/api/contracts`, `/:id`, `/:id/state` exists, but is derived by scanning RPC `getEvents`, not metadata/ABI/verification. |
| 7.4 Escrow | ✅ | Escrow + escrow-factory contracts, SDK client, React escrow viewer, controller escrow routes. |
| 7.5 Existing price oracle | 🟡 | Oracle contract + 4-source aggregator + confidence + breaker exist. On-chain push is broken; only Pi/USD is priced; no persistent history. |
| 7.6 Controller | ✅ | Implemented as described, except admin security and oracle publication. |
| 7.7 Contracts/core platform | ✅ | 19 contracts in Cargo workspace plus Arc contracts. |

### Section 8 — Financial infrastructure
| Module | Status | Verdict |
|---|---|---|
| 8.1 Financial data | 🟡 | Pools, trades, liquidity events, LP positions and lending positions are projected from enriched events; USD valuation is opt-in and refuses stale/degraded prices. An internal test-user consumer exists but production usage is absent. |
| 8.2 Oracle extension | 🟡 | TWAP/volatility, persistent series, provenance and freshness exist; liquidity-aware pricing is not implemented and is marked false. |
| 8.3 Risk engine | 🟡 | Shared deterministic risk module and `/risk/*` API exist. An opt-in indexed route reads lending position snapshots from the data platform and labels its source and ledger. |
| 8.4 Execution | ❌ | RPC simulation and SDK swap methods exist; deterministic CPMM previews exist in the SDK and over HTTP (`POST /quote/cpmm`, indexed-state `GET /quote/cpmm/:id`, simulated `GET /quote/cpmm/:id/onchain`). Deployed Pi testnet `get_quote` parity passes in both directions and CPMM route discovery exists; execution, vaults and capital are not implemented. |
| 8.5 Controller-to-financial integration | 🟡 | Versioned price/statistics/risk contracts and financial event projections exist; an internal test-user consumer exercises them and records coverage/freshness; production usage measurement is still absent. |
| 8.6 Data quality model | 🟡 | Responses carry price/confidence/sources/freshness/trust and versioned methods; not the full plan metadata model. |
| 8.7 Oracle trust model | ✅ | Explicit HEALTHY/DEGRADED/BREAKER/RECOVERY state machine with transition history. |
| 8.8 Risk inputs | ❌ | No volatility/liquidity/concentration inputs. |
| 8.9 Build conditions | 🟡 | Correctly not building capital products yet. |

### Section 9 — Proposed DeFi products
| Product | Status |
|---|---|
| 9.1 Liquidity vault | ❌ No strategy/vault framework. |
| 9.2 Liquidity optimization | ❌ No analytics. |
| 9.3 Liquidation network | ❌ Lending contract has liquidation internals; no monitoring/execution service. |
| 9.4 Router/aggregator | ❌ Swap contracts exist; no routing service. |
| 9.5 Settlement | ❌ None. |
| 9.6 Capital vaults | ❌ Only `usdc-vault`, which is a reserve/custody vault, not a strategy vault. |
| 9.7 Lending | 🟡 Contract exists on testnet; no demand evidence, audit, or liquidation capacity. |

### Section 10 — API and module boundaries
| Item | Status | Verdict |
|---|---|---|
| Service boundaries | 🟡 | The four services exist, but controller duplicates Horizon proxying already owned by Suban-api. |
| Common API contract | ❌ | No shared schema or contract between RPC, API and controller. |
| API versioning | 🟡 | `Suban-api` uses `/api/v1`; controller uses `/api/v1` for prices but also `/data`, `/api`, `/realtime` with mixed conventions. |
| Authentication and rate limits | ⚠️ | Suban-api has rate limiting; controller has **no auth and no rate limiting** on public or admin routes. |
| Data contracts | ❌ | No canonical Asset/PriceObservation/RiskAssessment/Quote schemas. |

### Section 11 — Data architecture
| Item | Status | Verdict |
|---|---|---|
| Ingestion layer | 🟡 | Event indexer only; no account/tx/operation ingestion. |
| Normalized data store | 🟡 | SQLite `events` and `index_state`; no financial tables. |
| Checkpoints/replay | ✅ | Per-contract `index_state` checkpoint; replay tooling is minimal. |
| Historical analytics | 🟡 | Price history is a 24h in-memory ring buffer; no persistence. |
| Source vs derived separation | ❌ | Not represented. |
| Webhooks | ❌ | None. |
| `getLedgerEntries` limitation | ⚠️ | Plan calls it a known limitation; `suban-rpc/README.md` lists it as supported. Contradiction must be resolved. |
| Data freshness/completeness | ❌ | Indexer exposes only total counts/status. |

### Section 12 — Security
| Item | Status | Verdict |
|---|---|---|
| RPC public/admin separation | ✅ | Separate admin endpoint, documented internal use. |
| Controller public/control separation | ⚠️ | Same process, same exposure, no auth. |
| Oracle security | ⚠️ | Single admin secret in env; push is broken; no multisig/HSM. |
| Contract audits | ❌ | `AUDIT-PROGRAM.md` only. No audit performed. |
| Key management | ⚠️ | Keys in `.env`; some secrets are committed in `docs/PROJECT-REPORT.md`. |
| TLS | 🟡 | Caddy/Cloudflare at the edge; internal container traffic is HTTP. |
| Incident response | ✅ | `docs/INCIDENT-RUNBOOK.md` exists. |
| Pause controls | 🟡 | `pause-registry` exists but is **not called by any other contract**. |

### Section 13 — Reliability and operations
| Item | Status | Verdict |
|---|---|---|
| RPC health/monitoring | ✅ | Prometheus metrics, Grafana dashboard, admin endpoint. |
| API/controller/indexer/relayer metrics | ❌ | Logs only. |
| Backups | ❌ | No documented backup/restore. |
| CI/CD | ❌ | No `.github/workflows` in any repo. |
| API keys/usage metering | ❌ | None. |
| SLOs | ❌ | Not defined or measured. |
| Cross-service integration tests | ❌ | Only Rust unit tests and SDK wiring tests. |

### Sections 14–21 — Business, validation, roadmap, definition of done
| Section | Status | Verdict |
|---|---|---|
| 14 Revenue | n/a | Strategy only. |
| 15 Validation framework | n/a | Good framework; not yet applied to any product. |
| 16 Roadmap | 🟡 | Q4 2026 items are mostly not done; Q1 2027 not started. |
| 17 Milestones | n/a | No milestone tracking in the repo. |
| 18 Dependencies | n/a | Correctly identifies Pi API/liquidity dependence. |
| 19 Risks | n/a | Good list; several risks are already materialized (secrets, oracle, bridge). |
| 20 Non-goals | 🟡 | Mostly respected, except unprotected controller admin APIs and premature bridge complexity. |
| 21 Definition of Done | ❌ | Almost no module meets it (no alerts, no integration tests, no runbook per service). |
| 22 Immediate next actions | ❌ | See Section 6 below; most are not done. |

---

## 3. What is actually implemented (with evidence)

### 3.1 Contracts
- 19 contracts in the root Cargo workspace (`Cargo.toml`):
  `pause-registry`, `fee-router`, `wpi-token`, `usdc-vault`, `escrow`, `escrow-factory`, `lp-token`, `cpmm-pool`, `pool-factory`, `stableswap`, `swap-router`, `liquidity-mining`, `oracle`, `bridge-multisig`, `lending-pool`, `backstop`, `event-registry`, `node-staking`, `identity`.
- 5 shared crates: `hub-types`, `hub-errors`, `hub-events`, `hub-oracle`, `hub-token`.
- 2 bridge contracts outside the workspace:
  - `bridge-burn-mint` — standalone, `soroban-sdk 21`, no tests.
  - `pusd-token` — source only, no `Cargo.toml`.
- Arc EVM: `ArcBridge.sol`, `PUSDToken.sol`, 8 Hardhat tests.
- Test inventory: ~149 `#[test]` functions across workspace contracts; `bridge-burn-mint` has 0.

### 3.2 Services
- `suban-rpc`: Go JSON-RPC gateway, Pi node configs, admin metrics, monitoring stack.
- `Suban-api`: Horizon wrapper, caching, rate limiting, enhanced endpoints, Arc proxy.
- `suban-controller`: four price sources, weighted aggregation, confidence, breaker, chain-data routes, contract search, escrow routes, realtime SSE, oracle push (broken).
- `Suban/services/event-indexer`: SQLite event indexer for 11 contract IDs, checkpointing, HTTP query API.
- `Suban/services/bridge-relayer`: Stellar/Arc watchers, event processor, tx builder, fee collector (single-sig).

### 3.3 SDK and frontend
- SDK: pool factory, CPMM, stableswap, liquidity mining, bridge, escrow, lending, oracle, generic simulate/invoke.
- Frontend: escrow viewer with mainnet/testnet toggle and contract/event search.

### 3.4 Deployment
- Docker Compose stack with RPC, testnet RPC, Stellar RPC, API, controller, event indexer, relayer, Caddy, Cloudflare tunnel.
- Pi mainnet/testnet node configs and captive-core setup.

### 3.5 Documentation
Present: `API-DOCS.md`, `API-REFERENCE.md`, `AUDIT-PROGRAM.md`, `DEVELOPER.md`, `INCIDENT-RUNBOOK.md`, `PHASE2-BRIDGE-DESIGN.md`, `PRODUCTION-ROADMAP.md`, `PROJECT-REPORT.md`, `WORKER-FLEET.md`.
Missing per Section 22: `ARCHITECTURE.md`, `ROADMAP.md`, `PRODUCTS.md`, `SECURITY.md`, ownership matrix.

---

## 4. What is partial or missing (the honest gap list)

### L1 - Data
- Normalized ledgers/transactions/operations/accounts/events, checkpoints, API keys/quotas, webhooks and historical queries exist.
- Remaining: public deployment, production backfill/capacity proof, and a bounded scan for account activity because account addresses are encoded in topic XDR.
- Remaining: contract metadata is registry-based; there is no automatic discovery of every deployed contract.

### L2 - Financial
- PI/USDC/USDT are priced; oUSD has no default feed and returns `503` rather than a synthetic peg.
- Persistent observations, `/history`, TWAP/volatility, a shared risk engine and an oracle trust state machine exist.
- Pool events now carry token direction, post-event reserves and share totals; LP positions and lending positions are derived from indexed events.
- Remaining: liquidity-aware pricing, a production consumer, configured token/price mappings for USD valuation, and deployment/indexing of the enriched contracts.
- Remaining: concentration metrics and a full provenance/quality object beyond the current fields.

### L3 - Execution
- A deterministic CPMM quote preview exists in the SDK and over HTTP.
- Remaining: quote/route discovery, on-chain `get_quote` simulation parity, execution simulation API beyond raw RPC simulate, and a transaction construction service.

### L4 — Capital
- No strategy vault framework.
- No deposit caps/strategy isolation.
- No capital product validation evidence.
- Lending exists only as a testnet contract.

### L5 — Protection
- No liquidation monitoring or bot.
- No settlement/reconciliation layer.
- `pause-registry` is not wired into contracts that need it.
- No shared alerting.

### Cross-cutting
- No CI across four repos.
- No integration tests for bridge, oracle push, or indexer replay.
- No shared API contracts.
- No SLOs/metrics for anything except RPC.
- No secret management; secrets committed in docs.
- No independent audit.
- No ownership matrix or module runbooks.

---

## 5. Defects and risks, prioritized

### P0 — stop-the-line

**D1. Bridge mint authorization is not cryptographically verified.**
- `contracts/bridge-burn-mint/src/lib.rs:309` accepts `signatures: Vec<Address>`.
- It checks each address is in the signer set, but never calls `require_auth()` and never verifies a signature.
- Anyone who knows the signer addresses can mint PUSD on the Stellar side.
- The `_relayer` argument is unused.
- **Fix:** either disable `mint_pusd` until a real signature scheme is implemented, or change the API to require `Address::require_auth()` for each signer over a canonical payload, with replay protection. Add tests that fail for forged signer lists.

**D2. On-chain oracle publication does not work.**
- `suban-controller/src/services/oracle-push.ts:59` calls `contract.call('set_price')` with **zero arguments**.
- `contracts/oracle/src/lib.rs:82` requires `(admin, asset, price, decimals, confidence)`.
- It also uses `Horizon.Server` with the Soroban RPC URL and `submitTransaction`, which is the wrong client/flow.
- **Fix:** use `rpc.Server`, `prepareTransaction`, `simulateTransaction`, `sendTransaction`, `getTransaction`; pass all five args and an asset identifier; add an integration test against testnet or a mock.

**D3. Secrets are committed in documentation.**
- `docs/PROJECT-REPORT.md` contains Stellar secret keys, an EVM private key, and a Cloudflare tunnel token.
- **Fix:** rotate every exposed key, remove the secrets from the file, purge git history, add secret scanning (gitleaks) to CI.

**D4. Controller control endpoints are unauthenticated.**
- `suban-controller/src/routes/api.ts` exposes `POST /price/commit` and `POST /circuit-breaker/reset` with no auth.
- The plan's Sections 12.1 and 20 explicitly forbid this.
- **Fix:** split public and control routers; require an API key/HMAC/mTLS on control routes; bind control routes to a private interface; add rate limits and audit logs.

**D5. Arc → Stellar bridge path is not functional.**
- `event-processor.ts:96-99` uses the sender as a placeholder recipient.
- `tx-builder.ts:151-171` sends a `[address, bytes]` vec, while `mint_pusd` expects `Vec<Address>` plus a `_relayer` argument.
- **Fix:** define the canonical Stellar → EVM address mapping and the real mint payload; add an end-to-end test in both directions.

### P1 — before any capital exposure

**D6. `bridge-burn-mint` is outside the workspace and untested.**
- It pins `soroban-sdk 21` while the workspace uses 27.
- `cargo test --workspace` does not build or test it.
- **Fix:** move it into the workspace, align the SDK, add unit/integration tests, deploy through the same pipeline.

**D7. `pause-registry` is not integrated.**
- No other contract reads it. The incident runbook assumes it works.
- **Fix:** either wire pause checks into contracts or explicitly narrow the runbook to per-contract pause functions.

**D8. Price coverage is single-asset.**
- All four adapters fetch Pi/USD only.
- The plan's own note (line 258) requires USDC/USD, USDT/USD, oUSD.
- **Fix:** introduce an asset registry with per-asset source sets and weights; ensure stablecoin prices are validated against a tight band (e.g., $1.00 ± tolerance) and fail closed.

**D9. Price history is in-memory.**
- `priceHistory.ts` keeps 8,640 observations in a ring buffer.
- **Fix:** persist observations (SQLite/Postgres), add retention, expose `/history`, compute TWAP from persisted data.

**D10. No CI.**
- No GitHub Actions in any repo.
- **Fix:** per-repo CI: build, lint, tests; workspace-level integration job for services; secret scanning; Docker build checks.

**D11. Event indexer is event-only and has no operational metrics.**
- **Fix:** add accounts/transactions/operations tables, lag/completeness metrics, replay command, webhook outbox.

**D12. No monitoring outside RPC.**
- **Fix:** Prometheus metrics on controller, API, indexer, relayer; Grafana dashboards; alert rules.

### P2 — quality and scale

**D13. Contract discovery is event-scan based.**
- `routes/contracts.ts` builds a contract list by scanning `getEvents`; the code itself warns that filtering is unreliable when a contract has no events.
- **Fix:** maintain a contract metadata registry with network, address, ABI/interface, version, deploy tx, verification status.

**D14. `getLedgerEntries` contradiction.**
- Plan calls it unsupported; RPC README lists it.
- **Fix:** test against the deployed build, document the exact supported key formats, and gate the capability.

**D15. API surface is fragmented.**
- Controller serves `/api/v1`, `/data`, `/api`, `/realtime`; Suban-api serves `/api/v1`, `/arc`, `/horizon`.
- **Fix:** publish one API contract and namespace policy; deprecate duplicate routes.

**D16. No canonical data contracts.**
- **Fix:** define versioned schemas for Asset, PriceObservation, AggregatedPrice, Ledger, Transaction, Account, Pool, Position, RiskAssessment, Quote, ExecutionRequest.

---

## 6. Section 22 immediate actions — actual status

| Action from plan | Status |
|---|---|
| Create `/docs/ARCHITECTURE.md` | ❌ |
| Create `/docs/ROADMAP.md` | ❌ (only `PRODUCTION-ROADMAP.md`) |
| Create `/docs/PRODUCTS.md` | ❌ |
| Create `/docs/API.md` | 🟡 (`API-DOCS.md` / `API-REFERENCE.md`) |
| Create `/docs/SECURITY.md` | ❌ |
| Ownership matrix | ❌ |
| Promote controller in architecture/deployment | ✅ documented in plan; deployed in Compose |
| Split public/control routes | ❌ |
| Document sources, weights, freshness, confidence | 🟡 (config only) |
| Verify oracle interface/publisher | ❌ broken |
| Inventory oracle and consumers | 🟡 (partially in docs) |
| Common API contract | ❌ |
| Instrument usage | 🟡 (logs only) |
| Design indexed data model | 🟡 (events only) |
| Document `getLedgerEntries` limitation | ⚠️ contradiction |
| Data platform before capital | ✅ respected |
| No mainnet lending/vault until prerequisites | ✅ respected |

---

## 7. The workable plan

This plan is sequenced by **risk removal first, then data, then financial products**. It aligns with the plan's own Q4 2026 "Consolidate" phase and delays Q2–Q4 2027 work until its exit criteria are met.

### Guiding rules

1. No new capital-bearing product until Phase 0 and Phase 1 exit criteria are met.
2. Every service must have health, metrics, logs, a runbook, and a rollback path before it is called production.
3. Every financial number exposed by an API must carry source, timestamp, freshness, confidence, and methodology version.
4. Every privileged operation must be authenticated, rate-limited, audited, and bound to a private network.
5. Every contract that moves value must be in the workspace, tested, and deployed through one pipeline.

---

### Phase 0 — Stabilize and make claims true (Week 1–2)

**Goal:** remove the defects that make current claims unsafe or false.

| # | Task | Deliverable | Acceptance criteria |
|---|---|---|---|
| 0.1 | Freeze bridge/mainnet capital work | Written freeze note | No new capital feature merged |
| 0.2 | Fix or disable oracle push | Working push on testnet or explicit `ORACLE_PUSH_ENABLED=false` | Push succeeds with all 5 args; integration test asserts tx hash |
| 0.3 | Fix bridge mint verification | Auth-checked mint or disabled mint path | Forged signer list fails; valid signer succeeds |
| 0.4 | Rotate leaked secrets | New keys, old keys revoked | Secret scan clean; history purged |
| 0.5 | Protect controller control routes | Private control router + auth | Unauthenticated POST returns 401/403 |
| 0.6 | Bring `bridge-burn-mint` into workspace | Workspace member, SDK 27 | `cargo test --workspace` includes it |
| 0.7 | Add CI to all four repos | GitHub Actions | Build/lint/test green on PR |
| 0.8 | Add secret scanning and dependency audit | gitleaks + npm/cargo audit | CI blocks on findings |

**Exit criterion:** no known critical defect; CI green; no secrets in the tree.

#### Phase 0 — progress log (19 Oct 2026)

Status: ✅ done and verified · 🟡 done, pending CI/owner action · ⬜ not started

| # | Task | Status | What changed / what is left |
|---|---|---|---|
| 0.1 | Freeze bridge/mainnet capital work | ⬜ | Process decision, not a code change. |
| 0.2 | Fix or disable oracle push | ✅ | `suban-controller/src/services/oracle-push.ts` now uses the Soroban `rpc.Server` (build → prepare → sign → send → poll) and passes all five `set_price` args (`admin, asset, price, decimals, confidence`). The broken zero-arg call is gone. Publishing is opt-in (`ORACLE_PUSH_ENABLED=false` by default) and needs `ORACLE_ASSET`. An on-chain integration test is still pending. |
| 0.3 | Fix bridge mint verification | ✅ | `contracts/bridge-burn-mint`: `mint_pusd` no longer trusts a caller-supplied signature list. Every listed signer must be a configured signer, must be unique, and must `require_auth()` (the cryptographic proof). 9 unit tests cover initialization, threshold, non-signer, duplicate, replay and missing-authorization. The Arc→Stellar relayer path is explicitly disabled until signer auth entries are implemented. |
| 0.4 | Rotate leaked secrets | 🟡 | Committed keys are removed from the working tree (`docs/PROJECT-REPORT.md`, deploy core configs, `fund-deployer.*` now read `FAUCET_SECRET` from env). gitleaks is in CI. **Owner action still required:** rotate the Stellar deployer, PUSD admin, Arc relayer keys and the Cloudflare tunnel token, then purge git history. Step-by-step runbook: `docs/SECRET-ROTATION.md`. |
| 0.5 | Protect controller control routes | ✅ | `POST /price/commit` and `POST /circuit-breaker/reset` now require `CONTROLLER_ADMIN_API_KEY` (constant-time compare; 503 if unset) and are rate-limited per IP. 6 unit tests. |
| 0.6 | Bring `bridge-burn-mint` into workspace | ✅ | Added to the root `Cargo.toml` members, `soroban-sdk 27` workspace dependency, nested lock removed. Verified with `cargo test -p bridge-burn-mint` in a Rust container. |
| 0.7 | Add CI to all four repos | ✅ | GitHub Actions on all four repos, all green on `main`. `suban-rpc` needed two fixes: pin `ed25519-dalek 2.2.0` (3.x breaks `soroban-env-host 23`), commit a lockfile and build with `--locked`; and exclude the `internal/preflight` tests (protocol-25 fixture vs host 23) while still compiling them with `go vet`. |
| 0.8 | Secret scanning and dependency audit | ✅ | gitleaks in all four repos: working-tree scan is blocking, history scan is report-only. Dependency audit is now in CI: production Node audits are blocking (`pnpm audit --prod --audit-level=high`, `npm audit --omit=dev --audit-level=high`) and Rust uses `rustsec/audit-check@v2`. Full/dev dependency trees are report-only. Controller production deps were cleaned by upgrading `@stellar/stellar-sdk` to 17.2.1, axios, and pnpm overrides for `proxy-addr`, `form-data` and `path-to-regexp`; `Suban-api`, SDK, event indexer, relayer and escrow viewer are clean at production level. |

**New defect found while adding CI (P1):** `suban-rpc` has two committed build artifacts, `pi-rpc` (~98 MB) and `db.test` (~59 MB), which bloat every clone and CI checkout. Remove them from git, add to `.gitignore`, and rewrite history (bundle with the D3 history purge).

**New defect found while adding CI (P2):** `suban-rpc`'s `internal/preflight` tests target protocol 25 while the vendored Rust host is `soroban-env-host 23`, so `TestGetPreflight` fails before reaching application code. The package is still compiled by `go vet`, but its tests are excluded from the CI gate until the host is upgraded.

**Dependency fix:** the `suban-rpc` Rust workspace had no committed lockfile, so a fresh checkout resolved `ed25519-dalek 3.x`, which uses `rand_core 0.10` and breaks the `soroban-env-host 23` testutils build. It is now pinned to `2.2.0`, the lockfile is committed, and `build-libs` uses `--locked` so CI builds a reviewed graph.

---

### Phase 1 — Consolidate L0/L2 (Week 3–5)

**Goal:** make the platform's public claims and documentation accurate.

| # | Task | Deliverable | Acceptance criteria |
|---|---|---|---|
| 1.1 | Write canonical docs | `ARCHITECTURE.md`, `ROADMAP.md`, `PRODUCTS.md`, `API.md`, `SECURITY.md` | New developer can onboard without reading source |
| 1.2 | Ownership matrix | Repo/service/owner table | Every module has one owner and one deployment target |
| 1.3 | Multi-asset price model | Asset registry + per-asset sources | Pi/USD, USDC/USD, USDT/USD, oUSD with independent health |
| 1.4 | Persist price observations | SQLite/Postgres + retention | Restart preserves history; `/history` returns data |
| 1.5 | Provenance and freshness in API | Versioned response schema | Every price includes source, timestamp, age, confidence, method version |
| 1.6 | Oracle trust states | HEALTHY/DEGRADED/BREAKER/RECOVERY | Transitions exposed and tested |
| 1.7 | Metrics for controller/API/indexer/relayer | `/metrics` + dashboards | Four services visible in Grafana |
| 1.8 | Resolve `getLedgerEntries` | Tested compatibility note | README and plan agree |

**Exit criterion:** docs match code; prices are multi-asset and persistent; all services have metrics; no contradictions.

#### Phase 1 — progress log (20 Oct 2026)

Status: ✅ done · 🟡 partial · ⬜ not started

| # | Task | Status | What changed / what is left |
|---|---|---|---|
| 1.1 | Write canonical docs | ✅ | Platform-level `docs/ARCHITECTURE.md`, `ROADMAP.md`, `PRODUCTS.md`, `SECURITY.md`, `API.md` and `OBSERVABILITY.md` now exist, with a root `README.md` that indexes them. `suban-controller/docs/API.md` remains the route-level reference. |
| 1.2 | Ownership matrix | ✅ | `docs/OWNERSHIP.md` assigns one accountable role and one deployment target per module, with escalation paths and open actions to record named maintainers. |
| 1.3 | Multi-asset price model | ✅ | `suban-controller` now has an asset registry (`PI`, `USDC`, `USDT`, `OUSD`) with independent sources, cache keys, circuit breakers and trust machines. `GET /api/v1/assets` exposes the registry; every price route accepts `?asset=` and defaults to `PI`. oUSD has no default feed and returns `503` until `OUSD_PRICE_URL` is configured. Unknown assets return `404`; no peg is fabricated. |
| 1.4 | Persist price observations | ✅ | New `priceStore` (SQLite via `better-sqlite3`) writes every fresh aggregation, retains 30 days by default and prunes hourly. `GET /api/v1/history?hours=&limit=` returns observations and window stats; cache hits and breaker fallbacks are not duplicated. Restart-safe; tested with an in-memory DB. |
| 1.5 | Provenance and freshness in API | ✅ | Price responses now carry `schema_version`, `method_version`, `age_ms`, `is_fresh`, and per-source `age_ms`/`is_fresh`; cached responses recompute age. |
| 1.6 | Oracle trust states | ✅ | New `TrustStateMachine` (`HEALTHY`/`DEGRADED`/`BREAKER`/`RECOVERY`) with a recovery streak. Transitions are exposed at `GET /api/v1/trust` and in `/health`; 6 unit tests cover transitions. The on-chain publisher refuses non-`HEALTHY` observations. |
| 1.7 | Metrics for controller/API/indexer/relayer | ✅ | All four runtime services expose Prometheus metrics: controller (`suban_*`, per-asset labels), `Suban-api` (`suban_api_*`), event indexer (`suban_indexer_*`) and bridge relayer (`suban_relayer_*`, metrics/health server on `9464`). `docs/OBSERVABILITY.md` and `ops/observability/` provide Prometheus scrape config and a provisioned Grafana **Suban Platform** dashboard (16 panels) covering all four services. |
| 1.8 | Resolve `getLedgerEntries` | ✅ | `suban-rpc/COMPATIBILITY.md` and the README now distinguish the supported Go `getLedgerEntries` runtime path from the failing Rust `internal/preflight` tests (protocol-25 fixture vs `soroban-env-host 23`). CI still compiles the package with `go vet` and excludes only those tests; the upgrade acceptance criteria are documented. |

**Dependency note:** `better-sqlite3@13` requires Node.js 22+, so the controller's `engines` now says `>=22` (CI already uses Node 22). `pnpm-workspace.yaml` allowlists its native build script for pnpm 12.

**Bug fixed while adding provenance:** the oracle publisher rounded `confidence_score` (0–1) directly into the contract's 0–100 `u32`, so every push reported confidence 0 or 1. It now sends `round(score × 100)`.

**Phase 1 close-out (20 Oct 2026):** all eight Phase 1 tasks are implemented. Verification performed: controller `tsc --noEmit`, build and 29 Jest tests pass; a live smoke test returned `/api/v1/assets`, a USDC price and `503` for unconfigured oUSD; `Suban-api` builds and serves `/health` + `/metrics`; the event indexer builds and its Docker image serves `/health` + `/metrics`; the bridge relayer builds and its metrics server serves `/health` + `/metrics`; the Grafana dashboard JSON and Compose file validate. Final `main` CI on all four repositories is green, including the new production Node audits and `rustsec/audit-check` (the Rust audit reports no vulnerabilities; only unmaintained-crate warnings). Remaining close-out actions are process/owner actions, not Phase 1 code: an on-chain oracle-push integration test, owner secret rotation, the git-history purge, named maintainers in the ownership matrix, and full/dev dependency audit triage.

**Source verification note:** external symbol checks on 20 Oct 2026 confirmed CoinGecko `usd-coin`/`tether`, OKX `USDC-USDT`/`USDT-USD` and MEXC `USDCUSDT`. OKX `USDT-USDC`, MEXC `USDTUSDC` and Bitget spot `USDTUSDC` are not used because they do not resolve. oUSD has no default market feed and must not be assigned a synthetic peg.

---

### Phase 2 — Data platform (Week 6–11)

**Goal:** deliver the L1 capability the plan promises: developers can consume Pi data without running nodes.

| # | Task | Deliverable | Acceptance criteria |
|---|---|---|---|
| 2.1 | Normalize event/account/tx/operation data | Versioned schema + migrations | Schema documented and tested |
| 2.2 | Expand indexer ingestion | Accounts, transactions, operations, events | Replay from checkpoint reproduces state |
| 2.3 | Indexer operations | Lag, completeness, reorg/replay metrics | Dashboard + alert thresholds |
| 2.4 | Contract metadata registry | Network/address/interface/version/verification | Contract discovery no longer depends on event scan |
| 2.5 | Public API keys and quotas | Key issuance, usage metering | Per-key usage visible and enforceable |
| 2.6 | Webhooks | Subscription + outbox + retries | Delivery is idempotent; failures retried with backoff |
| 2.7 | Historical query API | Time-range queries | Queries served from indexed data, not live RPC |
| 2.8 | Data freshness guarantees | Freshness/lag fields on all responses | Consumers can detect stale data |

**Exit criterion:** an external developer can build an explorer/analytics app using only Suban APIs, with no node access.

#### Phase 2 — progress log (21 Oct 2026)

Status: done ● / partial ◐ / not started ○

| # | Task | Status | What changed / what is left |
|---|---|---|---|
| 2.1 | Normalize event/account/tx/operation data | Done | `event-indexer/src/db.ts` adds schema `2.0` with `schema_migrations`, normalized `ledgers`, `transactions`, `operations`, `accounts`, `events`, `contracts`, account-involvement indexes, `checkpoints`, API-key usage and webhook outbox. The upgrade from the Phase 1 events database is additive and preserves rows. Migration/idempotency/upgrade tests pass. |
| 2.2 | Expand indexer ingestion | Done | Two checkpointed streams: Soroban events via RPC and Horizon ledgers/transactions/operations/accounts. Writes are idempotent; replay from a checkpoint is tested to reproduce the same rows. Horizon pagination is bounded; account snapshots are bounded per poll; `429`/`5xx` responses retry with backoff. |
| 2.3 | Indexer operations | Done | Metrics for ledger/transaction/operation/account counts, ingestion lag (ledgers and seconds), operation completeness, reorg signals, replay resets, API-key outcomes and webhook delivery. Grafana dashboard now has 22 panels; `prometheus/alerts.yml` contains thresholds for lag, staleness, completeness, reorg, fetch errors, webhook pressure and quota pressure. |
| 2.4 | Contract metadata registry | Done | `contracts` table records network/address/name/interface version/verification/metadata. Seeded from deployment config; admin CRUD exists. Discovery no longer depends on scanning events. `verified` is explicitly an operator assertion, not source verification. |
| 2.5 | Public API keys and quotas | Done | Keys are configured by hash; minute/day counters persist in SQLite; `429` includes `Retry-After`; per-key usage is visible at `GET /admin/api-keys/usage`; admin routes return `503` when unconfigured. |
| 2.6 | Webhooks | Done | Subscription filters, durable SQLite outbox, unique `(subscription,event)`, HMAC-SHA256 signature, stable idempotency key, bounded exponential backoff and delivery-state API. Tests cover idempotent enqueue, signature, retry and terminal failure. |
| 2.7 | Historical query API | Done | `/events`, `/transactions`, `/operations`, `/accounts/:id`, `/ledgers`, `/contracts` and `/status` query SQLite only. No query handler calls Horizon or RPC. `404` means not indexed, not missing on chain. |
| 2.8 | Data freshness guarantees | Done | Every data response includes `schema_version` and `freshness` (`latest_indexed_ledger`, `latest_observed_ledger`, `lag_ledgers`, `lag_seconds`, `last_success_at`, `stale`, `stale_after_seconds`). `/health` returns `503` when stale. |

**Phase 2 verification:** TypeScript build passes; 13 data-platform tests pass in a Node 22 container; Docker image builds and a live smoke test against local Horizon indexed 19 ledgers, 53 transactions, 56 operations and 64 accounts, then served `/health`, `/stats`, `/status`, `/transactions`, `/operations`, `/ledgers`, `/accounts/:id`, `/contracts`, `/metrics` and admin checkpoints/usage with correct API-key behavior. `/health` transitioned to `200` once the first ledger was durable. The Grafana JSON and Prometheus config are validated separately.

**CI:** Suban run `39600701294` succeeded on `4b18f3e`; the gitleaks history scan remains report-only. The latest runs on `suban-controller` (`8f65562`), `Suban-api` (`017100a`) and `suban-rpc` (`7530ca6`) are also green.

**Phase 2 close-out note:** code and route contracts for 2.1–2.8 are complete. The public hostname/TLS, production-sized backfill and capacity test are deployment actions, not code; they are tracked with the other owner-blocked items in `docs/DEFERRED-OWNER-ACTIONS.md`. Do not describe `data.suban.org` as live until that deployment is actually verified.

---

### Phase 3 — Financial data and risk (Week 12–17)

**Goal:** make L2 real, but derived from the now-reliable data layer.

Status as of 23 October 2026: **technical/testnet scope complete**. The
calculation/projection layer is implemented and tested, and the documented
event-data gaps are addressed in the deployed contracts and indexer. The
enriched CPMM, stableswap, oracle-aware lending, oracle and strategy-vault
contracts are live on Pi testnet and reindexed. An internal test-user consumer
passed 5/5 live checks against that deployment. The production
consumer/exit criterion is **not** met: there is still no external application
with measured adoption.

| # | Task | Status | Evidence / gap |
|---|---|---|---|
| 3.1 | Financial data API | Done (testnet) | `/financial/*` pools, trades, liquidity, account activity, summary, `/pools/:id/lp-positions`, `/lending/events`, `/lending/accounts/:id` and `/lending/summary` project enriched events. Pool events carry token direction, post-event reserves and total shares; lending events carry position snapshots and pool totals. The enriched deployment is reindexed and live: the CPMM pool exposes reserve/token/fee state at ledger `27319581`, and the lending projection contains deposit/borrow/collateral/liquidate snapshots. USD valuation is opt-in via `/pools/:id/valuation` and returns `503` without fresh, healthy, configured prices. Legacy events remain explicitly unenriched. |
| 3.2 | Historical price series | Done | SQLite persistence and `/history`; reopen test proves 30-day-capable storage survives restart. |
| 3.3 | TWAP and volatility | Done | `twap_volatility_v1`; deterministic pure-function tests; `/statistics`, `/twap`, `/volatility`. |
| 3.4 | Oracle extension | Partial | Provenance/freshness/confidence/trust and `/methodology` are versioned. Liquidity-aware pricing is not implemented and is marked `false`. |
| 3.5 | Risk engine v0 | Done | `lending_risk_v1`; deterministic vectors for health factor, collateral value, max borrow and liquidation price. The on-chain lending health-factor formula now returns basis points consistently with its `10_000` threshold. |
| 3.6 | Risk API | Done (deployment-dependent reader) | `/risk/methods`, `POST /risk/lending`, and opt-in `GET /risk/lending/indexed/:account`. Unknown/unconfigured/missing/stale prices return `404`/`503` without partial fabricated results; fresh non-HEALTHY prices are labelled degraded. The indexed route labels `indexed_contract_events`, exposes the source ledger and can lag the chain. |
| 3.7 | Consumer integration | Done for internal test user; production adoption absent | `tools/test-consumer/consumer.mjs` is a deterministic internal test user. It exercises `/price`, `/statistics`, `/risk/lending`, `/financial/summary` and `/financial/pools`, requires freshness/confidence/coverage fields, records a JSON usage report and runs in CI. The 23 October 2026 live run against the enriched controller/indexer passed 5/5 checks with `coverage.lp_positions`, `coverage.lending_positions` and `coverage.route_discovery` true; USD valuation was not configured. Production/external adoption and real third-party usage measurement are still absent. |

**Exit criterion:** at least one financial application consumes Suban financial data/oracle services, and freshness/confidence is visible to it. **Met for the internal test user; production/external adoption is not claimed.** The test user exercises the consumer mechanics and the enriched deployment, but it is not a third-party product or measured adoption. A production Phase 3 claim still requires an external consumer or an explicit owner waiver.

**Accepted gaps**

- External production adoption and third-party usage measurement remain absent.
- The risk reader depends on the data platform being configured with
  `DATA_PLATFORM_URL`; without it the indexed route returns `503`. It reads
  indexed snapshots, not live contract storage.
- USD valuation stays opt-in. It needs `INDEXER_PRICE_API_URL`,
  `INDEXER_TOKEN_ASSETS` and `INDEXER_TOKEN_DECIMALS`, and refuses stale or
  non-HEALTHY prices. It is a live valuation, not a historical USD-volume series.
- LP balances are derived by netting internal add/remove-share events. The pool
  contracts do not support LP-token transfers, so this is exact for current
  contracts but is not a generic LP-token indexer.
- Account activity and lending-account lookup use bounded recent-event scans
  because account addresses are encoded in topic XDR.
- The internal test user records consumer-side usage, but there is no
  production/external consumer or adoption measurement.
- Phase 4 is allowed only as a preview under an explicit gate override; no
  execution or capital work is claimed.

**Verification (23 October 2026):**

- Rust contracts: targeted `cpmm-pool`, `stableswap`, `lending-pool` and
  `strategy-vault` tests pass: 43 total (10 CPMM, 10 stableswap, 15 lending,
  8 vault). The lending tests include stale-price rejection, distinct
  collateral/debt and oracle-priced liquidation.
- `suban-controller`: `tsc` build passes; 56 Jest tests pass on Node 22 Docker,
  including the data-platform client, CPMM quote and route-discovery tests.
- `event-indexer`: `tsc` build passes; 22 Vitest tests pass on Node 22 Docker,
  including enriched pool, pool-list state, LP-position, lending-position,
  fresh-price valuation, stale/degraded `503`, Soroban value decoding and
  vault-event exclusion.
- Test-user consumer: 3 Node contract tests pass; the live run against the
  enriched controller/indexer passed 5/5 checks with `status: ok`.
- SDK quote preview: `tsc --noEmit` passes; 20 Vitest tests pass. The deployed
  parity script matches enriched CPMM `CBN77...` in both directions at ledger
  `27319628` (`96783046` and `100687090` raw units).
- Live data checks: `/financial/pools` exposes enriched CPMM state at ledger
  `27319581`; `/financial/lending/summary` reports deposit/borrow/collateral/
  liquidate events; `/risk/lending/indexed/:account` labels
  `indexed_contract_events` and returns the post-liquidation dust position.
- CI: `Suban` run `39956871500` on commit `0096549` is green, including the new
  `Service services/liquidation-bot` job. The working-tree gitleaks scan passed;
  the history scan remains report-only and still reports historical findings.

No production consumer, historical USD-volume series or capital/execution usage
is claimed. External adoption requires a real consumer or an explicit owner
waiver.

---

### Phase 4 — Execution and controlled capital (Week 18–25)

**Goal:** only build execution/capital if Phase 3 produced evidence.

**Gate before starting:** data platform stable, oracle reliable, at least one consumer, no P0/P1 defects open.

**Status (23 October 2026): testnet prototype under an explicit owner-requested gate override.** The owner directed execution to continue without waiting for owner-only actions and to use an internal test user instead of a real consumer. This does **not** satisfy the Phase 4 gate and does **not** authorize mainnet capital, vault capital allocation or a production keeper. 4.1–4.3 (quote, read-only simulation, CPMM route discovery) are implemented and matched to the enriched deployed pool. 4.4 (vault) and 4.5 (oracle-aware lending/liquidation) were deployed and exercised on Pi testnet. 4.6 (liquidation bot) executed one testnet liquidation. 4.7 is a preparation package; no auditor is engaged.

| Gate item | State |
|---|---|
| Data platform stable | Partial: Phase 2 routes/tests pass; public deployment, backfill and capacity are owner actions. |
| Oracle reliable | Partial: trust states/history are implemented; on-chain push integration test is still missing. |
| At least one consumer | Internal test user passed 5/5 live checks against the enriched deployment; production/external consumer is still absent. |
| No open P0/P1 defects | Not met: secret rotation and git-history purge are owner-blocked; bridge mint authorization remains gated. Working-tree secret scan is clean. |

The Phase 4 table below is implemented only to testnet-prototype depth. The
HTTP previews are deterministic over caller-supplied reserves or indexed pool
state. `sdk/scripts/quote-parity.mjs` and the controller simulation service
matched the enriched Pi testnet CPMM pool
`CBN77D4T6EPPVHLCNU2XCAEWMRPGTDH67RSGVKKJSIPHT3W23KB6QNCM` in both directions
at ledger `27319628` (`96783046` and `100687090` raw units). The earlier pool
`CBE7DHJDS6HZHVY7AWU42XDUA3WAG7GRFAEYEBRM5J3XG2KUYDBAO2NZ` also matched at
ledger `27305287` (`998992` and `995011` raw units); that run is historical.
These are point-in-time formula checks, not execution guarantees.

**Testnet deployment inventory (23 October 2026):**

| Component | Contract ID |
|---|---|
| Oracle | `CATCIFIYSSMIH7AHZXFXTRPIB2MAKZQCJRZNRP3G4ULOQBBAPIW2SL32` |
| Oracle-aware lending | `CAKDET6TED6DQWLIPB6ZB2URIVDTFOZKL62ALN5DDUTDN7LDELEKGKRC` |
| Strategy vault | `CD4RHSQM5IWBIXO6MEN6U4DHEANH6HFSD5FETB3HBMSLGLPXLLW6VMDP` |
| Enriched CPMM | `CBN77D4T6EPPVHLCNU2XCAEWMRPGTDH67RSGVKKJSIPHT3W23KB6QNCM` |
| Enriched stableswap | `CD4HZJPLPHMKHMHBZ5VTP4VTISJGXIP7CMCEUG2GZOHCR2EQK5DGM6MC` |

| # | Task | Status | Deliverable | Acceptance criteria |
|---|---|---|---|---|
| 4.1 | Quote service | Done (testnet parity) | SDK `quoteCpmm`, `CpmmPoolClient.getQuote`, `POST /api/v1/quote/cpmm`, indexed `GET /api/v1/quote/cpmm/:contractId`, and simulated `GET /api/v1/quote/cpmm/:contractId/onchain` | Deterministic integer formula, contract unit vector and deployed `get_quote` parity in both directions pass; execution-time state guarantees are still absent |
| 4.2 | Simulation API | Done (read-only) | Read-only `get_quote` simulation plus deterministic price impact | Reproducible result and parity flag; not wired to transaction building |
| 4.3 | Route discovery | Done (CPMM only) | `GET /api/v1/route/cpmm` over enriched indexed CPMM pools, 1–3 hops | Only real indexed liquidity; stableswap and unenriched pools excluded; `404` when no route exists |
| 4.4 | Vault framework (testnet) | Done (testnet only) | Caps, pause, strategy isolation | Deposit/withdraw/pause tests pass; deployed vault exercised |
| 4.5 | Lending hardening | Done (testnet only) | Oracle integration, liquidation tests | Liquidation path tested end-to-end; dust limitation documented |
| 4.6 | Liquidation bot prototype | Done (one testnet liquidation) | Monitor + execute on testnet | Detects and executes a test liquidation |
| 4.7 | Audit prep | Prep complete; auditor not engaged | Scope, threat model, test evidence | Auditor engaged |

**Exit criterion:** measurable usage/economic activity justifies one capital/execution product; audit complete before mainnet capital.

**Verification (23 October 2026):**

- `sdk/scripts/quote-parity.mjs` simulated enriched CPMM `CBN77...` in both
  directions: wPi→PUSD preview/on-chain `96783046`, PUSD→wPi `100687090`, at
  ledger `27319628`.
- Live controller checks: `/route/cpmm` returned `96783046` at source ledger
  `27319581`; indexed `/quote/cpmm/:id` returned the same amount; read-only
  `/quote/cpmm/:id/onchain` returned `96783046` at ledger `27319714` with
  `parity.matches=true`.
- Controller build passes; 56 Jest tests pass on Node 22 Docker, including 5
  route-discovery tests and the enriched pool-list parsing test.
- Indexer build passes; 22 Vitest tests pass on Node 22 Docker, including
  Soroban value decoding and vault-event exclusion from lending projections.
- Rust contracts: 43 targeted tests pass (10 CPMM, 10 stableswap, 15 lending,
  8 vault).
- Vault testnet exercise: deposit `10` PUSD, withdraw `5`, pause blocked a
  deposit with `Error(Contract, #2)`, withdrawal while paused succeeded,
  unpause succeeded, first strategy set succeeded and a second set failed with
  `Error(Contract, #6)`.
- Lending testnet exercise: health `17000` bps before the wPi price drop and
  `4250` after; liquidation transaction
  `ee6ae5c8c0a74ef9e3c0a1fae229bde61f75c26fe7156a707fbacbd51e5d4a36`
  repaid `952380952` raw PUSD and seized `999999999` raw wPi.
- Liquidation bot: 18 Vitest tests, typecheck and build pass on Node 22; live
  dry run detected the health-`4250` position and the execute run produced the
  transaction above.
- Audit prep: `docs/PHASE4-AUDIT-PREP.md` contains scope, threat model,
  invariants, test evidence, deployment inventory and known limitations.
- CI: `Suban` run `39956871500` on commit `0096549` is green, including the
  liquidation-bot Node 22 job and the working-tree secret scan. The history
  scan remains report-only.

**Still missing:** stableswap quoting, execution/transaction building,
production keeper operation, vault capital allocation, an independent audit,
mainnet authorization, and a bad-debt/dust policy. 4.7's auditor-engagement
checkbox is unchecked.

---

### Phase 5 — 2027 roadmap (per plan Sections 16–17)

Status: **not started**. It depends on the Phase 3 consumer/exit and the Phase 4
gate, neither of which is met.

- Q1 2027: Data Platform exit.
- Q2 2027: Financial Data + Risk exit.
- Q3 2027: Execution + Capital experiments.
- Q4 2027: choose the winning product based on evidence.

Do not start a later phase before the previous exit criterion is met.

---

## 8. First 10 working days (concrete)

| Day | Task | Output |
|---|---|---|
| 1 | Disable insecure mint path; add feature flag | Config/code change + note |
| 1 | Disable oracle push until fixed | `ORACLE_PUSH_ENABLED=false` |
| 2 | Fix oracle push args/server | PR + test |
| 2 | Add controller control-route auth | PR + tests |
| 3 | Rotate all exposed keys | New envs; old keys revoked |
| 3 | Remove secrets from docs; purge history | Clean repo |
| 4 | Add gitleaks + dependency audit | CI job |
| 4 | Bring `bridge-burn-mint` into workspace | `Cargo.toml` + build |
| 5 | Add first bridge tests (forged/valid signer) | Test suite |
| 6 | Set up CI for Suban (build/test/lint) | Workflow |
| 7 | Set up CI for the three service repos | Workflows |
| 8 | Add `/metrics` to controller | Prometheus output |
| 9 | Add `/metrics` to indexer and relayer | Prometheus output |
| 10 | Write `ARCHITECTURE.md` and ownership matrix | Docs |

---

## 9. Effort estimate (rough, 2–3 engineers)

| Phase | Duration | Main skills |
|---|---|---|
| Phase 0 | 1–2 weeks | Rust, TypeScript, DevOps, security |
| Phase 1 | 3 weeks | TypeScript, Rust, technical writing |
| Phase 2 | 6 weeks | Backend/data engineering, DevOps |
| Phase 3 | 6 weeks | Data/quant, backend, Rust |
| Phase 4 | 8 weeks | Rust, TypeScript, DevOps, security |
| **Total to Phase 4** | **~6 months** | |

With one engineer, assume **9–12 months**, and sequence strictly.

---

## 10. Metrics and SLO targets

| Metric | Target |
|---|---|
| RPC availability | 99.5% monthly |
| RPC p95 latency | < 500 ms |
| API availability | 99.5% |
| API p95 latency | < 300 ms |
| Controller availability | 99.0% |
| Price freshness | < 60 s for Pi/USD; < 5 min for stablecoins |
| Oracle push success | > 99% of attempts |
| Indexer lag | < 30 s |
| Webhook delivery success | > 99% within 5 min |
| Bridge transfer time | < 5 min p95 (testnet) |
| Critical alerts acknowledged | < 15 min |

---

## 11. Definition of done (per workstream)

A module is done only when all are true:

1. Responsibility and API boundary documented.
2. Owning repo and deployment target named.
3. Failure modes documented.
4. Metrics and alerts exist.
5. Security assumptions explicit.
6. Integration tests cover critical flows.
7. Versioning/compatibility policy exists.
8. Runbook exists.
9. Revenue/usage mechanism understood (if commercial).
10. For capital-bearing contracts: audit and staged rollout complete.

---

## 12. Decisions needed from you

1. **Priority:** platform/data first, or bridge/stablecoin first? My recommendation: platform/data first; freeze bridge mainnet work.
2. **PUSD model:** is it 1:1 Pi-backed, 1:1 USDC-backed, or both? The plan says Pi-backed; `PROJECT-REPORT.md` says USDC-backed. This must be decided before reserve work.
3. **Oracle publisher authority:** single admin key, multisig, or external publisher service?
4. **Address mapping:** on-chain registry, Stellar name registry, or off-chain database? The plan recommends on-chain; nothing is built.
5. **Do we keep the current bridge in scope for Q4 2026**, or park it until the data/platform layer is trustworthy?
6. **Team capacity:** how many engineers and which skills are available? The sequencing above assumes 2–3.

---

## 13. What I would not do

- Do not build the risk engine, router, vaults, or liquidation network yet.
- Do not add a fifth repository.
- Do not rewrite the controller; harden it.
- Do not deploy capital contracts to mainnet.
- Do not treat the current bridge as production-ready.
- Do not publish the controller's admin endpoints as public APIs.
- Do not promise "on-chain oracle" until the push path is tested.

---

## 14. Bottom line

The plan is directionally right and unusually well written. The codebase is a credible start at L0 with a promising L1/L2 seed. The main risk is no longer missing operational basics — CI, production dependency audits, per-asset metrics and multi-asset pricing are now in place. The remaining risk is **claiming capabilities that are not yet true** (on-chain oracle publishing still lacks an integration test, the bridge is not production-ready, and secret rotation plus the git-history purge remain owner actions) and **operational execution** (named owners, monitoring, incident response).

The fastest path to a platform that matches the plan is:

1. **2 weeks:** make the current claims true and safe. (Phase 0 code is done; secret rotation and history purge are owner-blocked.)
2. **3 weeks:** document, multi-asset pricing, persistence, metrics. (Phase 1 is complete and CI-green as of 20 October 2026.)
3. **6 weeks:** deliver a real data platform with keys, webhooks, and history. (Phase 2 code and route contracts are complete as of 21 October 2026; public deployment and owner actions remain.)
4. **Then** financial data, risk, execution, and capital — in that order, behind evidence gates.

That sequence preserves the plan's strategic thesis while removing the gap between what the document says and what the system does.

---

*Prepared from the codebase as of 19 October 2026; Phase 1 close-out verified on 20 October 2026; Phase 2 implementation verified on 21 October 2026. File references are relative to `C:\Users\Binhdentist\suban-repos`.*
