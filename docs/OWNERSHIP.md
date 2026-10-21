# Suban Ownership Matrix

Status: canonical ownership map (Phase 2, October 2026).

Every module has **one accountable role** and **one deployment target**. The
role is accountable until a named maintainer is recorded in the repository's
`CODEOWNERS` or team settings. "TBD" means the role exists but a named person has
not yet been recorded; that is an open action, not an orphan module.

## 1. Ownership rules

1. One accountable owner per module. Reviewers and backups do not dilute
   accountability.
2. A change to a module's public contract, deployment target or security
   boundary requires the owner's review.
3. No module is "owned by everyone". Shared crates still have a single owner.
4. Ownership transfers are recorded here in the same change that updates
   `CODEOWNERS`.
5. Every owner must be able to answer: what does this run, where does it deploy,
   how is it monitored, and how is it rolled back?
6. Security incidents escalate to the Security lead regardless of module owner.

## 2. Matrix

| Module | Repository / path | Accountable owner | Named maintainer | Deployment target | Observability / runbook | Status |
|---|---|---|---|---|---|---|
| Core Soroban contracts + shared crates | `Suban/contracts/*`, `Suban/crates/*` | Contracts owner | TBD | Pi testnet (addresses in `docs/DEVELOPER.md`); Arc testnet for Solidity | `cargo test --workspace`, `docs/AUDIT-PROGRAM.md` | Testnet |
| Bridge contracts | `Suban/contracts/bridge-burn-mint`, `Suban/contracts/arc-bridge` | Bridge owner | TBD | Pi testnet + Arc testnet | `docs/PHASE2-BRIDGE-DESIGN.md`, `docs/PRODUCTION-ROADMAP.md` | Testnet; Arc→Stellar mint disabled |
| PUSD bridge relayer | `Suban/services/bridge-relayer` | Bridge owner | TBD | Internal worker; metrics `:9464` | `docs/OBSERVABILITY.md`, `docs/INCIDENT-RUNBOOK.md` | Implemented; gated |
| RPC | `suban-rpc` | RPC owner | TBD | `rpc.suban.org`, `testrpc.suban.org` (ports `8000`/`8001`) | `suban-rpc` README + monitoring stack | Implemented |
| Horizon-compatible data API | `Suban-api` | Data API owner | TBD | `horizon.suban.org` (`:4000`) | `docs/API.md`, `docs/OBSERVABILITY.md` | Implemented |
| Market data + oracle controller | `suban-controller` | Oracle owner | TBD | `oracle.suban.org` (`:3000`) | `suban-controller/docs/API.md`, `docs/OBSERVABILITY.md` | Implemented; TWAP/volatility and risk calculator; oracle push opt-in |
| Data platform (`event-indexer`) | `Suban/services/event-indexer` | Indexing owner | TBD | `data.suban.org` (target; not yet provisioned); local `:3002` | `docs/DATA-PLATFORM.md`, `docs/OBSERVABILITY.md` | Implemented; financial projections partial; testnet/experimental |
| SDK | `Suban/sdk` | SDK owner | TBD | Published package `@suban/sdk` (release artifact) | `sdk` tests (`vitest`), `docs/DEVELOPER.md` | Implemented; pre-1.0 |
| Escrow viewer | `Suban/frontend/escrow-viewer` | Frontend owner | TBD | `testnet.suban.org` (testnet-facing frontend) | Vite build; read-only viewer | Experimental |
| Deployment + observability stack | `Suban/deploy`, `Suban/ops/observability` | SRE owner | TBD | Docker Compose / Pi node stack; Prometheus + Grafana | `docs/OBSERVABILITY.md`, `docs/INCIDENT-RUNBOOK.md` | Implemented |
| CI, release process and platform docs | All four repositories | Platform lead | TBD | GitHub Actions on `main` | `.github/workflows/*`, `PLAN-EXECUTION.md` | Implemented |
| Secret management and rotation | Deployment secret stores | Security lead | TBD | Per-environment secret store | `docs/SECRET-ROTATION.md`, `docs/SECURITY.md` | Partial; rotation pending |
| Price source configuration | `suban-controller/.env` | Oracle owner | TBD | Controller deployment env | `suban-controller/README.md`, `docs/API.md` | Implemented |

## 3. Escalation

| Situation | First responder | Escalation |
|---|---|---|
| Oracle price wrong/stale or push failure | Oracle owner | Security lead + Platform lead |
| Bridge mint anomaly or stuck transfer | Bridge owner | Security lead + Protocol lead |
| RPC/Horizon outage | RPC owner or Data API owner | SRE owner |
| Indexer lag or data loss | Indexing owner | SRE owner |
| Leaked key or secret | Security lead | Protocol lead + all affected owners |
| CI/release blocker | Platform lead | Module owner |
| Contract exploit or pause decision | Contracts owner / Bridge owner | Protocol lead + Security lead |

Contacts are intentionally role-based in this public document. Named contacts
and phone numbers belong in the private `INCIDENT-RUNBOOK.md` deployment copy,
not in git.

## 4. Deployment targets at a glance

| Target | Modules | Public? |
|---|---|---|
| `oracle.suban.org` | Controller | Read routes public; control routes private |
| `horizon.suban.org` | Suban-api + Pi Horizon | Public read |
| `testnet.suban.org` | Escrow viewer / testnet frontend | Public |
| `rpc.suban.org` | RPC | Public JSON-RPC; admin `:8001` private |
| `testrpc.suban.org` | Test RPC | Public testnet JSON-RPC |
| Internal Compose network | Bridge relayer, Prometheus/Grafana | No public listener except dashboards |
| `data.suban.org` (target) | Data platform (`event-indexer`) | Not yet provisioned; `/metrics` private |

## 5. Open ownership actions

These are tracked in [`DEFERRED-OWNER-ACTIONS.md`](./DEFERRED-OWNER-ACTIONS.md)
so they are not lost or reported as complete.

- [ ] Record named maintainers in each repository's `CODEOWNERS` or team
      settings; replace the "TBD" cells above.
- [ ] Fill the private emergency contact table in the deployment copy of
      `INCIDENT-RUNBOOK.md`.
- [ ] Confirm the `testnet.suban.org` frontend owner and deployment pipeline.
- [x] Assign the dependency-audit CI work to the Platform lead.
- [ ] Assign the `suban-rpc` protocol-25 host upgrade to the RPC owner.
- [ ] Rotate leaked secrets and approve the git-history purge (owner-blocked).
- [ ] Add the on-chain oracle-push integration test (owner-blocked).
- [ ] Provision the public data-platform hostname and production backfill.
