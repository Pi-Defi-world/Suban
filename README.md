# Suban

Financial infrastructure for Pi Network: chain access, indexed data, market data
and oracle publication, escrow, and a gated cross-chain bridge.

Suban is built as small, independently deployable services rather than one
monolith. Start with the [architecture](docs/ARCHITECTURE.md) and
[product catalog](docs/PRODUCTS.md); use the [API index](docs/API.md) to
integrate.

## Repositories

| Repository | What it is |
|---|---|
| [`Suban`](https://github.com/Pi-Defi-world/Suban) (this repo) | Contracts, shared Rust crates, SDK, event indexer, bridge relayer, escrow viewer, deployment and observability assets |
| [`suban-rpc`](https://github.com/Pi-Defi-world/suban-rpc) | Pi JSON-RPC: network queries, transaction submission, status/history, simulation, Soroban interaction |
| [`Suban-api`](https://github.com/Pi-Defi-world/Suban-api) | Horizon-compatible REST API with caching, rate limiting and enhanced endpoints |
| [`suban-controller`](https://github.com/Pi-Defi-world/suban-controller) | Multi-asset price aggregation, provenance, trust states, history and optional on-chain oracle publication |

## Services and ports

| Service | Port | Notes |
|---|---|---|
| Controller | `3000` | Market data/oracle; control routes authenticated |
| Data API | `4000` | Horizon-compatible reads |
| Event indexer | `3002` | Indexed contract events |
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
| [ARCHITECTURE.md](docs/ARCHITECTURE.md) | Repositories, service boundaries, data flows, trust boundaries |
| [PRODUCTS.md](docs/PRODUCTS.md) | Capability catalog with status and limits |
| [ROADMAP.md](docs/ROADMAP.md) | Phases, gates and exit criteria |
| [SECURITY.md](docs/SECURITY.md) | Threat model, controls, open gaps, reporting |
| [OWNERSHIP.md](docs/OWNERSHIP.md) | One owner and one deployment target per module |
| [API.md](docs/API.md) | Canonical API index and error/retry guidance |
| [OBSERVABILITY.md](docs/OBSERVABILITY.md) | Metrics, dashboard, SLOs and alerts |
| [DEVELOPER.md](docs/DEVELOPER.md) | Contract addresses, SDK usage and integration notes |
| [PLAN-EXECUTION.md](PLAN-EXECUTION.md) | Current execution plan and honest progress log |
| [plan.md](plan.md) | Canonical strategy and long-range narrative |

Operational runbooks:

- [INCIDENT-RUNBOOK.md](docs/INCIDENT-RUNBOOK.md)
- [SECRET-ROTATION.md](docs/SECRET-ROTATION.md)
- [AUDIT-PROGRAM.md](docs/AUDIT-PROGRAM.md)
- [PRODUCTION-ROADMAP.md](docs/PRODUCTION-ROADMAP.md)
- [PHASE2-BRIDGE-DESIGN.md](docs/PHASE2-BRIDGE-DESIGN.md)

## Status

Suban is testnet-first. The controller, data API, indexer and relayer are
implemented and observable; the bridge supports Stellar→Arc on testnet, while
Arc→Stellar minting is disabled until signer authorization entries exist. No
mainnet capital product should be considered safe before the gates in
[ROADMAP.md](docs/ROADMAP.md) and `AUDIT-PROGRAM.md` are met.

## Contributing

- Read [ARCHITECTURE.md](docs/ARCHITECTURE.md) and [SECURITY.md](docs/SECURITY.md)
  before changing a service or contract.
- Keep changes consistent with the surrounding code and update docs in the same
  change as behavior.
- Never commit secrets. Gitleaks runs in CI; see
  [SECRET-ROTATION.md](docs/SECRET-ROTATION.md) if a key is exposed.
- CI must stay green on `main` for all four repositories.

## License

MIT (see individual package metadata).
