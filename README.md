# Suban

Financial infrastructure for Pi Network: chain access, indexed data, market data
and oracle publication, escrow, and a gated cross-chain bridge.

Suban is built as small, independently deployable services rather than one
monolith. Start with the [architecture](docs/ARCHITECTURE.md) and
[product catalog](docs/PRODUCTS.md); use the [API index](docs/API.md) to
integrate. For the current build inventory, deployment IDs and honest
production/testnet classification, see
[PROJECT-STATUS.md](docs/PROJECT-STATUS.md).

## Repositories

| Repository | What it is |
|---|---|
| [`Suban`](https://github.com/Pi-Defi-world/Suban) (this repo) | Contracts, shared Rust crates, SDK, event indexer, bridge relayer, escrow viewer, deployment and observability assets |
| [`suban-rpc`](https://github.com/Pi-Defi-world/suban-rpc) | Pi JSON-RPC: network queries, transaction submission, status/history, simulation, Soroban interaction |
| [`Suban-api`](https://github.com/Pi-Defi-world/Suban-api) | Horizon-compatible REST API with caching, rate limiting and enhanced endpoints |
| [`suban-controller`](https://github.com/Pi-Defi-world/suban-controller) | Multi-asset price aggregation, provenance, trust states, history, TWAP/volatility, deterministic risk API and optional on-chain oracle publication |

## Services and ports

| Service | Port | Notes |
|---|---|---|
| Controller | `3000` | Market data/oracle; control routes authenticated |
| Data API | `4000` | Horizon-compatible reads |
| Event indexer / data platform | `3002` | Indexed ledgers, transactions, operations, accounts and contract events; public keys/quotas and webhooks. Financial projections include pool reserves, token direction, derived LP positions and lending position snapshots; USD valuation is opt-in and refuses stale prices |
| Bridge relayer | `9464` | Metrics/health only; no public API |
| RPC | `8000` / `8001` | Public JSON-RPC / private admin |
| Horizon | `31401` / `41401` | Testnet / mainnet local Horizon |

## Quick start (contracts and SDK)

```bash
# Contracts (Rust workspace)
cargo build --workspace
cargo test --workspace

# TypeScript SDK
cd sdk
npm install --no-audit --no-fund
npm run typecheck
npm test
```

Service quick starts live in each service directory:

- [`services/event-indexer`](services/event-indexer)
- [`services/bridge-relayer`](services/bridge-relayer)
- [`sdk`](sdk)
- [`frontend/escrow-viewer`](frontend/escrow-viewer)

## Documentation

| Document | Purpose |
|---|---|
| [PROJECT-STATUS.md](docs/PROJECT-STATUS.md) | Current project status, all builds, deployment IDs and verified limits |
| [ARCHITECTURE.md](docs/ARCHITECTURE.md) | Repositories, service boundaries, data flows, trust boundaries |
| [DATA-PLATFORM.md](docs/DATA-PLATFORM.md) | Indexed schema, historical API, freshness, replay, keys and webhooks |
| [PRODUCTS.md](docs/PRODUCTS.md) | Capability catalog with status and limits |
| [ROADMAP.md](docs/ROADMAP.md) | Phases, gates and exit criteria |
| [SECURITY.md](docs/SECURITY.md) | Threat model, controls, open gaps, reporting |
| [OWNERSHIP.md](docs/OWNERSHIP.md) | One owner and one deployment target per module |
| [API.md](docs/API.md) | Canonical API index and error/retry guidance |
| [OBSERVABILITY.md](docs/OBSERVABILITY.md) | Metrics, dashboard, SLOs and alerts |
| [DEVELOPER.md](docs/DEVELOPER.md) | Contract addresses, SDK usage and integration notes |
| [TEST-CONSUMER.md](docs/TEST-CONSUMER.md) | Internal test user for price, risk and financial APIs |
| [PHASE4-QUOTE-PREVIEW.md](docs/PHASE4-QUOTE-PREVIEW.md) | CPMM quote preview and explicit limits |
| [PHASE4-LIQUIDATION.md](docs/PHASE4-LIQUIDATION.md) | Testnet vault, oracle-aware lending and liquidation-bot evidence |
| [PHASE4-AUDIT-PREP.md](docs/PHASE4-AUDIT-PREP.md) | Phase 4.7 audit-preparation package (no auditor engaged) |
| [PLAN-EXECUTION.md](PLAN-EXECUTION.md) | Current execution plan and honest progress log |
| [plan.md](plan.md) | Canonical strategy and long-range narrative |

Operational runbooks:

- [INCIDENT-RUNBOOK.md](docs/INCIDENT-RUNBOOK.md)
- [SECRET-ROTATION.md](docs/SECRET-ROTATION.md)
- [DEFERRED-OWNER-ACTIONS.md](docs/DEFERRED-OWNER-ACTIONS.md)
- [AUDIT-PROGRAM.md](docs/AUDIT-PROGRAM.md)
- [PRODUCTION-ROADMAP.md](docs/PRODUCTION-ROADMAP.md)
- [PHASE2-BRIDGE-DESIGN.md](docs/PHASE2-BRIDGE-DESIGN.md)

## Status

Suban is testnet-first. Phase 0-2 are complete. Phase 3's technical/testnet
scope is complete, but there is no external production consumer. Phase 4.1-4.6
exist as testnet prototypes (quotes/routes, vault, oracle-aware lending and a
liquidation bot); Phase 4.7 is an audit-preparation package and **no auditor is
engaged**. The bridge supports Stellar→Arc on testnet, while Arc→Stellar
minting is disabled until signer authorization entries exist. No mainnet
capital product should be considered safe before the gates in
[ROADMAP.md](docs/ROADMAP.md) and `AUDIT-PROGRAM.md` are met. See
[PROJECT-STATUS.md](docs/PROJECT-STATUS.md) for the full inventory.

## Contributing

- Read [ARCHITECTURE.md](docs/ARCHITECTURE.md) and [SECURITY.md](docs/SECURITY.md)
  before changing a service or contract.
- Keep changes consistent with the surrounding code and update docs in the same
  change as behavior.
- Never commit secrets. Local environment and contract values are consolidated
  into gitignored `.env.local` files by `scripts/sync-local-env.ps1`; the script
  prints only paths and key counts. Gitleaks runs in CI; see
  [SECRET-ROTATION.md](docs/SECRET-ROTATION.md) if a key is exposed.
- CI must stay green on `main` for all four repositories.

## License

MIT (see individual package metadata).
