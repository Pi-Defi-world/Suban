# Suban - Project Status and Build Inventory

Status: **current as of 26 October 2026**. Testnet-first. This document is the
single-page answer to "what is Suban, what is built, and what actually works?".
It does not replace the execution plan, the audit package, or the per-service
runbooks.

## 1. What Suban is

Suban is blockchain and financial infrastructure for Pi Network. It is built as
small, independently deployable services and Soroban contracts rather than one
monolith. Applications can use four capabilities:

1. **Read the chain** - JSON-RPC and Horizon-compatible access to Pi state.
2. **Understand the chain** - indexed ledgers, transactions, operations,
   accounts, contract events and replayable financial projections.
3. **Understand markets** - multi-source price aggregation with provenance,
   confidence, freshness and explicit oracle trust states.
4. **Move value safely** - tested contracts with explicit authorization
   boundaries, escrow primitives and a gated cross-chain bridge.

The platform is intentionally layered. Chain access and indexed data are useful
on their own. Market data and oracle publication build on that. Execution and
capital products are only supposed to be built after demand is demonstrated and
the security gates are met.

### Repository map

| Repository | Runtime | Responsibility | Current state |
|---|---|---|---|
| `Suban` (this repo) | Rust + TypeScript + Solidity | Contracts, shared crates, SDK, event indexer, bridge relayer, liquidation bot, escrow viewer, deployment and observability assets | Testnet prototypes; CI green |
| `suban-rpc` | Go + Rust preflight | Pi JSON-RPC, transaction submission/status/history, simulation, Soroban interaction | Implemented; preflight simulation tests gated by a host/protocol mismatch |
| `Suban-api` | Node.js/TypeScript | Horizon-compatible REST wrapper with caching, rate limiting and enhanced endpoints | Implemented; CI green |
| `suban-controller` | Node.js/TypeScript | Price aggregation, provenance, trust states, history, TWAP/volatility, risk APIs, quote/route previews, optional oracle publication | Implemented; CI green |

## 2. Current status at a glance

- **Phase 0-2:** complete.
- **Phase 3:** technical/testnet scope complete. The enriched contracts are
  deployed and reindexed; the internal test consumer passed 5/5 live checks.
  External production adoption and measured third-party usage are still absent,
  so the production-consumer exit criterion is not claimed.
- **Phase 4.1-4.3:** CPMM quote, read-only on-chain simulation and CPMM route
  discovery are implemented and matched to the enriched deployed pool.
- **Phase 4.4-4.6:** testnet vault, oracle-aware lending/liquidation and the
  liquidation bot were deployed and exercised.
- **Phase 4.7:** audit-preparation package is complete; **no auditor is
  engaged** and no audit report exists.
- **Phase 4 gate:** not satisfied. No mainnet capital, vault capital allocation
  or production keeper is authorized.
- **Latest CI:** `Suban` run `40361766711` on `04a5ea3` is green. The preceding
  docs-only run `40360487990` on `42004e3` failed on two new dependency
  advisories; both were fixed in `04a5ea3` (indexer `body-parser` upgrade and a
  documented `RUSTSEC-2026-0387` exception). The controller run `39820587698`
  on `d0db18d` is green. `Suban-api` and `suban-rpc` latest runs are green, with
  the RPC preflight caveat noted below.
- **Local runtime at the last check:** the event indexer container
  `suban-indexer-phase3` was running on port `3102`; the controller was not
  listening on port `3101`. This is a local verification environment, not a
  production deployment.

### Honest classification

| Classification | What it means |
|---|---|
| **Production-ready** | Nothing in this repository should be described as production-ready financial infrastructure. |
| **Testnet / experimental** | Deployed and exercised on Pi or Arc testnet, with documented limits. |
| **Implemented, deployment owner-managed** | Code and tests exist; a live address may be old, unverified or absent. |
| **Planned** | Design only; do not depend on it. |

## 3. How the pieces fit

```text
Pi applications / wallets / explorers / DeFi protocols
                          |
        +-----------------+------------------+
        |                 |                  |
        v                 v                  v
  Suban-api         suban-controller     suban-rpc
  (Horizon/data)    (market data/oracle) (JSON-RPC/execution)
        |                 |                  |
        +--------+--------+---------+--------+
                 |                  |
                 v                  v
          Pi Horizon / node     Soroban contracts
                 |                  |
                 +-- event-indexer / bridge-relayer --+
```

Trust boundaries:

- On-chain contracts are the source of truth for token balances, reserves,
  debt, collateral and authorization.
- `event-indexer` is a derived, replayable view. It is not live contract storage
  and may lag the chain. Every data response carries freshness and source-ledger
  metadata.
- `suban-controller` is an off-chain price aggregator. It must never invent a
  price. Missing, stale or non-`HEALTHY` data is returned as an error or marked
  degraded rather than converted to a zero.
- Controller control routes and oracle publication are opt-in and
  authenticated. They return `503` when no admin key is configured.
- The bridge is gated. Stellar -> Arc is the supported testnet direction;
  Arc -> Stellar mint remains disabled until signer authorization entries exist.
- Indexed risk and route previews are explicitly labelled as indexed/preview
  data. They are not execution guarantees.

## 4. Build inventory

### 4.1 Rust workspace

The Cargo workspace contains **21 Soroban contracts and 5 shared crates**.
`contracts/arc-bridge` is a separate Hardhat/Solidity build.
`contracts/pusd-token` is currently an empty placeholder directory; the deployed
PUSD token address is recorded below, but its source is not present in that
directory.

| Build | Path | What it does | Current status |
|---|---|---|---|
| `backstop` | `contracts/backstop` | Emergency backstop for lending | Builds/tests; earlier testnet address recorded, not re-verified in the enriched deployment |
| `bridge-burn-mint` | `contracts/bridge-burn-mint` | Stellar/Arc PUSD burn-mint bridge | Testnet deployment recorded; Arc -> Stellar mint disabled |
| `bridge-multisig` | `contracts/bridge-multisig` | N-of-M approval for bridge operations | Testnet deployment recorded; not re-verified this phase |
| `cpmm-pool` | `contracts/cpmm-pool` | Constant-product AMM with real SEP-41 transfers and enriched events | **Deployed current**: `CBN77...` |
| `escrow` | `contracts/escrow` | Milestone escrow with dispute/release lifecycle | Testnet address recorded; not re-verified this phase |
| `escrow-factory` | `contracts/escrow-factory` | Deploys and catalogs escrow instances | Builds/tests; no current deployment recorded |
| `event-registry` | `contracts/event-registry` | Cross-primitive event storage/query | Builds/tests; no current deployment recorded |
| `fee-router` | `contracts/fee-router` | Protocol fee routing | Builds/tests; no current deployment recorded |
| `identity` | `contracts/identity` | Maps Pi accounts to on-chain addresses | Builds/tests; no current deployment recorded |
| `lending-pool` | `contracts/lending-pool` | Collateralized lending with oracle-aware health and liquidation | **Deployed current**: `CAKDET...` |
| `liquidity-mining` | `contracts/liquidity-mining` | LP reward emissions | Testnet address recorded; not re-verified this phase |
| `lp-token` | `contracts/lp-token` | SEP-41-like LP share token | Builds/tests; pools deploy instances |
| `node-staking` | `contracts/node-staking` | Node-operator staking and slashing | Builds/tests; no current deployment recorded |
| `oracle` | `contracts/oracle` | On-chain price feed with staleness, breaker and deviation checks | **Deployed current**: `CATCIF...` |
| `pause-registry` | `contracts/pause-registry` | Shared pause registry | Builds/tests; not wired into every value-moving contract |
| `pool-factory` | `contracts/pool-factory` | Deploys and catalogs AMM pools | Testnet address recorded; not re-verified this phase |
| `stableswap` | `contracts/stableswap` | Curve-style low-slippage stable pool with enriched events | **Deployed current**: `CD4HZ...` |
| `strategy-vault` | `contracts/strategy-vault` | Testnet-only single-strategy vault with caps, pause and isolation | **Deployed current**: `CD4RHS...` |
| `swap-router` | `contracts/swap-router` | Pool registry and multi-hop path finder | Testnet address recorded; not re-verified this phase |
| `usdc-vault` | `contracts/usdc-vault` | USDC bridge vault for Stellar Asset Contract USDC | Builds/tests; no current deployment recorded |
| `wpi-token` | `contracts/wpi-token` | Wrapped Pi SEP-41 token | **Deployed current token**: `CCODZ...` |
| `arc-bridge` | `contracts/arc-bridge` | Solidity `ArcBridge` + `PUSDToken`; M-of-N validator mint/burn | Hardhat build/test; Arc testnet addresses recorded, not re-verified this phase |
| `pusd-token` | `contracts/pusd-token` | Placeholder directory; no source in this path | Deployed PUSD token address recorded below; source not present here |

Shared crates:

| Crate | Purpose |
|---|---|
| `hub-types` | Shared `Price`, asset and protocol types |
| `hub-errors` | Shared contract error enum (`HubError`) |
| `hub-events` | Standard event topics for indexer compatibility |
| `hub-oracle` | Shared oracle interface/helpers |
| `hub-token` | Shared SEP-41-style token interface/helpers |

Build and test:

```bash
cargo build --workspace
cargo test --workspace
```

The current enriched WASM artifacts are generated locally under `artifacts/`
(ignored by git via `*.wasm`): `cpmm_pool.wasm`, `stableswap.wasm`,
`lending_pool.wasm`, `strategy_vault.wasm` and `oracle.wasm`. Build them from
the workspace when reproducing a deployment; do not assume the local artifacts
are committed.

### 4.2 TypeScript services

| Service | Path | Runtime / port | What it does | Build/test |
|---|---|---|---|---|
| Event indexer / data platform | `services/event-indexer` | Node 22, `:3002` | Ingests Soroban events and Horizon data into SQLite; serves historical, financial, freshness, key/quota and webhook APIs | `npm ci`, `npm run typecheck`, `npm test`, `npm run build` |
| Bridge relayer | `services/bridge-relayer` | Node 22, `:9464` metrics/health | Watches Stellar/Arc burn events and builds destination mints; no public API | `npm ci`, `npm run build` |
| Liquidation bot | `services/liquidation-bot` | Node 22, no public port | Testnet-only discovery and liquidation prototype; dry-run by default, env-only key, collateral-bounded repayment | `npm ci`, `npm run typecheck`, `npm test`, `npm run build` |

The event indexer's financial layer derives pool state, trades, liquidity
events, LP positions, lending events, lending account snapshots and bounded
account activity. It excludes vault `deposit`/`withdraw` events from lending
projections by contract-name filter. USD valuation is opt-in and refuses stale
or degraded prices.

The liquidation bot re-reads health and positions from the contract before
acting. Indexed candidates are hints only. It never reads or prints the secret,
and it has no flash-loan or capital source.

### 4.3 SDK

`@suban/sdk` is a typed TypeScript client. It is pre-1.0 and network-specific.

Modules:

- `amm.ts` - CPMM, stableswap, pool factory and liquidity operations
- `quote.ts` - deterministic CPMM quote preview
- `lending.ts` - lending pool reads and transactions
- `oracle.ts` - oracle reads
- `escrow.ts` - escrow lifecycle
- `bridge.ts` - bridge operations
- `client.ts` - Soroban RPC/client wrapper
- `index.ts` - public exports

Build and test:

```bash
cd sdk
npm ci --no-audit --no-fund
npm run typecheck
npm test
npm run build
npm run parity:testnet   # requires a deployed pool and env configuration
```

### 4.4 Escrow viewer

`frontend/escrow-viewer` is a read-only React/Vite dashboard for escrow
auditability. Build:

```bash
cd frontend/escrow-viewer
npm ci --no-audit --no-fund
npm run build
```

It is experimental and is not a custody or legal-enforceability product.

### 4.5 Arc Solidity bridge

`contracts/arc-bridge` contains `ArcBridge.sol` and `PUSDToken.sol`, built with
Hardhat. It implements M-of-N validator minting, replay protection, mint caps
and pausability. Build/test:

```bash
cd contracts/arc-bridge
npm install --no-audit --no-fund
npm test
```

The Arc addresses in `docs/DEVELOPER.md` are recorded testnet addresses. They
were not re-verified during the 23 October enriched deployment.

### 4.6 Deployment, operations and tools

| Path | What it contains |
|---|---|
| `deploy/` | Docker Compose files for Pi testnet/mainnet nodes, Horizon, RPC, suban services, Cloudflare tunnel config and verification scripts |
| `ops/observability/` | Prometheus config/alerts and a provisioned Grafana dashboard |
| `scripts/sync-local-env.ps1` | Consolidates local deployer, faucet, relayer, Arc, controller, indexer and liquidation-bot values into gitignored `.env.local` files without printing values |
| `tools/test-consumer/` | Internal test user for price, statistics, risk and financial coverage |
| `artifacts/` | Local enriched WASM build output (gitignored; not committed) |
| `.github/workflows/ci.yml` | Contracts, SDK, consumer, services, frontend, Solidity, secret scan and dependency-audit jobs |

### 4.7 CI jobs

| Job | What it checks |
|---|---|
| `Contracts (Rust)` | `cargo build --workspace`, `cargo test --workspace` |
| `Rust dependency audit` | RustSec audit |
| `SDK (TypeScript)` | Node 22, production dependency audit, typecheck, 20 Vitest tests |
| `Test consumer` | 3 Node contract tests |
| `Service services/event-indexer` | Node 22, production audit, build, 22 Vitest tests |
| `Service services/bridge-relayer` | Node 22, production audit, build |
| `Service services/liquidation-bot` | Node 22, production audit, typecheck/build, 18 Vitest tests |
| `Escrow viewer` | Node 22, production audit, Vite build |
| `Arc bridge (Solidity)` | Hardhat tests |
| `Secret scan` | Gitleaks working-tree scan is blocking; git-history scan is report-only |

### 4.8 Sibling repository builds

These repositories are separate checkouts, not workspace members:

| Repository | Build/test commands | Caveat |
|---|---|---|
| `suban-rpc` | `make build`, `make test` | Preflight simulation tests are gated by the protocol/host mismatch; the Go `getLedgerEntries` path is supported |
| `Suban-api` | `npm ci`, `npm run build`, `npm run lint` | No test script in `package.json`; CI builds the service |
| `suban-controller` | `pnpm install`, `pnpm build`, `pnpm test` | Node 22 is required; 56 Jest tests pass |

## 5. Current testnet deployment inventory

### Current enriched deployment (Pi testnet, 23 October 2026)

| Component | Contract ID |
|---|---|
| Oracle | `CATCIFIYSSMIH7AHZXFXTRPIB2MAKZQCJRZNRP3G4ULOQBBAPIW2SL32` |
| Oracle-aware lending | `CAKDET6TED6DQWLIPB6ZB2URIVDTFOZKL62ALN5DDUTDN7LDELEKGKRC` |
| Strategy vault | `CD4RHSQM5IWBIXO6MEN6U4DHEANH6HFSD5FETB3HBMSLGLPXLLW6VMDP` |
| Enriched CPMM | `CBN77D4T6EPPVHLCNU2XCAEWMRPGTDH67RSGVKKJSIPHT3W23KB6QNCM` |
| Enriched stableswap | `CD4HZJPLPHMKHMHBZ5VTP4VTISJGXIP7CMCEUG2GZOHCR2EQK5DGM6MC` |
| wPi token | `CCODZXYOZMKBOKCCMOKSVW3FIGPJRILM7GH4MKFOS4IWZ56EBX5MVONV` |
| PUSD token | `CAPDFYOFXSQTVCZ7KPUACHVNMOO3TWLSLHTPQ3H64EBJVRFMAWIBEUAY` |

### Historical or superseded addresses

| Component | Address | Note |
|---|---|---|
| Earlier CPMM pool | `CBE7DHJDS6HZHVY7AWU42XDUA3WAG7GRFAEYEBRM5J3XG2KUYDBAO2NZ` | Earlier parity run; superseded for enriched tests |
| Earlier stableswap | `CAETZYW4ADMRCQIVLONFTAEP4WQGIVP2V5DZ7XXSPPVEK5CNHXOJOHMX` | Earlier deployment; not re-verified |
| Earlier lending pool | `CAZETGC2BYWX5KY653BF4J2WDXFTAQAKSW7CBKDEJVXJG2KRBOHXLJXI` | Superseded by oracle-aware lending |
| Earlier enriched lending | `CCTSJAKLI42YMDXUI6BGWJTDDB27LCSLBUR4XWB7R5LKVXIAG7C5HPPK` | Superseded by `CAKDET...` |
| Earlier oracle | `CBLQKWBP2TV3CPM26DDNMEMDEMLGYAJHJ5O2ZM4UD32OVXBOCA25DE3M` | Earlier oracle; current oracle is `CATCIF...` |

### Recorded testnet contracts not re-verified this phase

These addresses appear in `docs/DEVELOPER.md` or the historical project report.
Treat them as recorded integrations, not as freshly verified deployments:

- Pool Factory: `CCQKGU54YX6UBHRANG4JNDR3TMDA4HGJ4NLP5KGJC2JLQRGUC7KIMOAW`
- Backstop: `CD4HGZAQN5C2O53K2OMV5HTDSJNJM56V7X6CGBIOJJ2A3QMV7FPYNMRX`
- Escrow Manager: `CC3JNU7LAGOLF5TAZ2BP23ZDFSDMVM6IR7O3MWHGM65QII3UINQNM4EU`
- Swap Router: `CA3AECFKZSGBODPXWWTOBVKUEWQLAGWAIO5E2RMAAVYMLCERSSEU4KYZ`
- Liquidity Mining: `CDC6LFSPVE7XOAAVU2JBUQQNFHAAIH2DESW4M4KU6KXS2I2CHFFGA555`
- Bridge Burn-Mint: `CAM33E3NNPHO5OGNU6YVMUJYIVFLHDCRB7P3IXET4EDZHYVYM2JAX54S`
- Bridge Multi-Sig: `CBNGDXVUGRHQPTHYUOTQJVZKYSPZI2DCEWA6JUJCTV7IYF7Z2STLPXVA`
- ArcBridge (EVM): `0x765c4AdF71CFA7f1e9F6358Ca25A8216DC70B409`
- PUSDToken (EVM): `0x7534400f6F725326D5668d85d76b7bA0029aFEd8`

## 6. What works today

Verified on 23 October 2026 unless noted:

- **Price controller:** multi-asset aggregation, persistence, history, TWAP,
  volatility, trust states, risk calculator and metrics. Missing/stale prices
  return `503`; non-`HEALTHY` fresh prices are marked degraded.
- **Data platform:** normalized ledger/transaction/operation/account/event
  history, replay checkpoints, freshness, contract registry, API keys/quotas,
  webhooks and enriched financial projections.
- **Enriched pool state:** `/financial/pools` returned CPMM reserve/token/fee
  state at ledger `27319581`.
- **LP and lending projections:** LP positions, lending events, lending account
  snapshots and `coverage.route_discovery` are live for the enriched deployment.
- **CPMM quote parity:** enriched pool `CBN77...` matched SDK/HTTP/on-chain
  `get_quote` in both directions at ledger `27319628`
  (`96783046` and `100687090` raw units).
- **Route discovery:** `/api/v1/route/cpmm` returned `96783046` at source ledger
  `27319581`; indexed quote returned the same; read-only on-chain simulation
  returned `96783046` at ledger `27319714` with `parity.matches=true`.
- **Vault:** deposit `10` PUSD, withdraw `5`, pause blocked a deposit with
  `Error(Contract, #2)`, withdrawal while paused succeeded, first strategy set
  succeeded and a second set failed with `Error(Contract, #6)`.
- **Lending/liquidation:** health moved from `17000` to `4250` bps after a wPi
  price drop. Testnet liquidation transaction
  `ee6ae5c8c0a74ef9e3c0a1fae229bde61f75c26fe7156a707fbacbd51e5d4a36`
  repaid `952380952` raw PUSD and seized `999999999` raw wPi.
- **Liquidation bot:** dry run detected the health-`4250` position and the
  execute run produced the transaction above. 18 Vitest tests, typecheck and
  build pass on Node 22.
- **Internal consumer:** `status: ok`, 5/5 live checks, including LP positions,
  lending positions and route discovery.

### Test counts

| Area | Evidence |
|---|---|
| Rust contracts | 43 targeted tests: 10 CPMM, 10 stableswap, 15 lending, 8 vault |
| Event indexer | 22 Vitest tests |
| Controller | 56 Jest tests |
| SDK | 20 Vitest tests |
| Liquidation bot | 18 Vitest tests |
| Test consumer | 3 Node tests plus 5/5 live checks |
| CI | `Suban` run `40361766711` on `04a5ea3` green; controller run `39820587698` on `d0db18d` green |

## 7. What is not production-ready

- No independent audit and no bug bounty.
- No external production consumer or measured third-party adoption.
- No mainnet deployment or mainnet capital.
- No vault capital allocation.
- No production liquidation keeper, uptime guarantee or MEV strategy.
- Liquidation leaves dust: `1` raw wPi collateral and `47,619,048` raw PUSD
  debt after the test liquidation. There is no bad-debt/dust write-off policy.
- No stableswap quote, transaction construction or execution API.
- Public data-platform deployment at `data.suban.org` is not provisioned.
- Secret rotation and git-history purge remain owner actions. Working-tree
  gitleaks is clean; history scanning still reports historical findings.
- Rust audit has one explicit tracked exception, `RUSTSEC-2026-0387`
  (`crypto-bigint 0.5.5` via `k256 0.13.4` / `soroban-env-host 27.0.1`). The
  patched release is a semver-incompatible major; see `.cargo/audit.toml` and
  `docs/SECURITY.md`.
- Arc -> Stellar mint remains disabled pending signer authorization entries.
- Named maintainers remain `TBD`.
- `suban-rpc` preflight simulation tests are gated by a host/protocol mismatch
  (`soroban-env-host 23` vs protocol 25).
- The current local controller was not running at the last runtime check; only
  the indexer container was confirmed up.

## 8. Build and test commands

```bash
# Rust contracts and shared crates
cargo build --workspace
cargo test --workspace

# SDK
cd sdk
npm ci --no-audit --no-fund
npm run typecheck
npm test
npm run build

# Event indexer
cd services/event-indexer
npm ci --no-audit --no-fund
npm run typecheck
npm test
npm run build

# Liquidation bot
cd services/liquidation-bot
npm ci --no-audit --no-fund
npm run typecheck
npm test
npm run build

# Bridge relayer
cd services/bridge-relayer
npm ci --no-audit --no-fund
npm run build

# Escrow viewer
cd frontend/escrow-viewer
npm ci --no-audit --no-fund
npm run build

# Arc Solidity bridge
cd contracts/arc-bridge
npm install --no-audit --no-fund
npm test
```

Node services are pinned to Node 22 in CI. The local host may be newer; use the
Node 22 Docker pattern from CI when reproducing service tests.

## 9. Documentation map

| Document | Purpose |
|---|---|
| `README.md` | Project entry point |
| `PLAN-EXECUTION.md` | Canonical phase status, evidence, waivers and remaining gaps |
| `docs/ARCHITECTURE.md` | Repositories, service boundaries, data flows and trust boundaries |
| `docs/DATA-PLATFORM.md` | Indexed schema, historical API, freshness, replay, keys and webhooks |
| `docs/PRODUCTS.md` | Capability catalog with status and limits |
| `docs/ROADMAP.md` | Phases, gates and exit criteria |
| `docs/DEVELOPER.md` | Current and historical contract addresses, SDK usage |
| `docs/API.md` | Canonical API index and error/retry guidance |
| `docs/SECURITY.md` | Threat model, controls and open gaps |
| `docs/AUDIT-PROGRAM.md` | Audit plan and scope |
| `docs/PHASE4-QUOTE-PREVIEW.md` | CPMM quote/route/simulation evidence and limits |
| `docs/PHASE4-LIQUIDATION.md` | Vault, oracle-aware lending and liquidation-bot evidence |
| `docs/PHASE4-AUDIT-PREP.md` | Phase 4.7 audit-preparation package (no auditor engaged) |
| `docs/TEST-CONSUMER.md` | Internal test-user contract and live evidence |
| `docs/DEFERRED-OWNER-ACTIONS.md` | Owner-blocked follow-ups |
| `docs/OBSERVABILITY.md` | Metrics, dashboards, SLOs and alerts |
| `docs/OWNERSHIP.md` | Module ownership matrix |

## 10. Maintenance rule

Update this document when a deployment ID, build result, CI gate, audit status
or production claim changes. Keep the distinction between:

- code that builds,
- code that is tested,
- code that is deployed on testnet,
- code that has been exercised end-to-end, and
- code that is approved for mainnet capital.

Those are different claims. Do not collapse them.
