# Suban Local System and Portable Setup

Status: verified by read-only inspection on **29 October 2026**. This document
describes the Docker stack running on the current host and how to reproduce it
on another device. It is not a claim that every public endpoint is healthy.

The assistant does not host these services. They run as Docker containers on the
machine that owns this checkout. A different device must either run the same
containers or call a reachable public endpoint.

## 1. Short answer: what is actually working

| Capability | Public endpoint | Local upstream | Status at the check |
|---|---|---|---|
| Pi mainnet Horizon | `https://horizon.suban.org/` | `suban-api:4000` -> `pi-mainnet:8000` | Working. Horizon `v28.0.1`, core `v28.0.1`, network `Pi Network`, latest ledger `29,466,843`. |
| Pi testnet Horizon | `https://testnet.suban.org/` | `pi-testnet:8000` | Working. Horizon `v28.0.1`, core `v28.0.1`, network `Pi Testnet`, latest ledger `27,421,371`. |
| Pi testnet Soroban RPC | `https://testrpc.suban.org/` | `stellar-rpc-testnet:8000` | Working. `getHealth` returned `healthy`, latest ledger `27,421,338`. |
| Official Pi-node RPC ports | `41403` / `42403` | `pi-mainnet` / `pi-testnet` supervisor `rpc` | **Not working.** `supervisorctl status` showed `rpc STOPPED` on both nodes. Use `testrpc.suban.org` for testnet Soroban RPC. |
| Pi mainnet Suban JSON-RPC | `https://rpc.suban.org/` | `suban-rpc:8000` | **Not working.** `getHealth` returned a stale-ledger error: `latency (304h49m51s) since last known ledger closed is too high (>5m0s)`. |
| Pi testnet Suban JSON-RPC | internal `suban-rpc-testnet:8000` | `suban-rpc-testnet:8000` | **Not working.** `getHealth` returned `data stores are not initialized: DB is empty`; the container was crash-looping. |
| Price oracle / controller | `https://oracle.suban.org/api/v1/health` | `suban-controller:3000` | Working. Health `healthy`, `4/4` price sources active. |
| Data platform | `data.suban.org` (not provisioned) | local phase3 indexer on `:3102` | **Not public and not current.** DNS does not resolve; the local enriched indexer is stale. |

**Do not say "all RPCs are working."** The accurate statement is:

- The **official Pi-node Horizon APIs** are working for mainnet and testnet.
- The **official Stellar testnet RPC** is working.
- The **price controller** is healthy.
- The **Suban Go JSON-RPC services** are not working at this check.
- The **public data platform** is not provisioned.

### Which software is used where

- **Horizon**: the `pi-mainnet` and `pi-testnet` containers use the official
  `pinetwork/pi-node-docker:community-v1.0-p28.0.1` image. They run a local
  stellar-core and Horizon. `testnet2` is a separate official testnet node
  started by the Pi Network desktop app.
- **Mainnet/testnet JSON-RPC**: the `suban-rpc` and `suban-rpc-testnet`
  containers are built from the separate `suban-rpc` repository. They are not
  the official node's Horizon API. Their runtime image is pinned to
  `pinetwork/pi-node-docker:community-v1.0-p27.1.0` (stellar-core `v27.1.0`),
  while the live Pi network is protocol 28. That mismatch is visible in the
  logs and is the likely cause of the RPC failures.
- **Testnet Soroban RPC**: `stellar-rpc-testnet` uses
  `stellar/stellar-rpc:latest` with `Pi Testnet` and the Pi testnet history
  archive. This is the working testnet contract-RPC path.
- **Official node RPC supervisor**: both `pi-mainnet` and `pi-testnet` report
  `rpc STOPPED` in `supervisorctl status`. Their `41403`/`42403` ports are not
  serving JSON-RPC. Do not use them as working RPC endpoints.
- **Suban services**: `suban-api` wraps mainnet Horizon; `suban-controller`
  aggregates prices and serves oracle/risk routes; the event indexers and bridge
  relayer are separate workers.

## 2. Verified container inventory

`docker compose ls` showed one Compose project named `docker` with these three
files:

- `Suban/deploy/docker-compose.pi-mainnet-node.yml`
- `Suban/deploy/docker-compose.pi-testnet-node.yml`
- `Suban/deploy/docker-compose.suban.yml`

The Pi Network desktop app also had its own Compose project, `pi-network`, with
the `testnet2` container.

| Container | Image / source | Host ports | Role | Observed state |
|---|---|---|---|---|
| `pi-mainnet` | official `pinetwork/pi-node-docker:community-v1.0-p28.0.1` | `41401` Horizon, `41402` peer, `41403` RPC | Pi mainnet core + Horizon | Running and current; supervisor `rpc` is STOPPED |
| `pi-testnet` | same official image, Suban testnet entrypoint | `42401` Horizon, `42402` peer, `42403` RPC | Pi testnet core + Horizon | Running and current; supervisor `rpc` is STOPPED |
| `testnet2` | same official image, started by Pi Network desktop app | `31401` Horizon, `31402` peer, `31403` RPC | Separate Pi testnet node | Running; not used by the Suban Caddy routes |
| `suban-api` | `Suban-api` repository | internal `4000` | Mainnet Horizon wrapper with caching and rate limiting | `/health` returned `200` |
| `suban-controller` | `suban-controller` repository | internal `3000` | Price aggregation, oracle/risk routes | `/api/v1/health` returned `healthy`; `4/4` sources active |
| `suban-rpc` | `suban-rpc` repository, captive core | internal `8000` JSON-RPC, `8001` admin | Mainnet Suban JSON-RPC | Running but stale; `getHealth` failed |
| `suban-rpc-testnet` | `suban-rpc` repository, captive core | internal `8000` JSON-RPC, `8001` admin | Testnet Suban JSON-RPC | Crash-looping; DB empty; `RestartCount` was `92,582` at the check |
| `stellar-rpc-testnet` | `stellar/stellar-rpc:latest` | internal `8000` | Pi testnet Soroban RPC | Healthy; latest ledger `27,421,338` |
| `suban-event-indexer` | `services/event-indexer` from `docker-compose.suban.yml` | internal `3002` | Older event indexer in the main Compose stack | Running, but `0` events and `0` monitored contracts; points at `suban-rpc` |
| `suban-indexer-phase3` | ad-hoc `suban-event-indexer:phase3` container | host `3102` | Enriched Phase 3 indexer | Running but stale; latest indexed ledger `27,319,581` vs observed `27,421,333` |
| `suban-bridge-relayer` | `services/bridge-relayer` | internal `3003` | Stellar/Arc bridge worker | Running and polling; no HTTP health listener was reachable on `3003` or `9464` at the check |
| `suban-caddy` | `caddy:2-alpine` | internal `80`/`443` | Reverse proxy and static escrow viewer | Running |
| `suban-cloudflared` | `cloudflare/cloudflared:latest` | none | Cloudflare tunnel | Running |
| `event-indexer-test` | `node:22-bookworm-slim` | none | Test shell (`sleep infinity`) | Running; not a service |
| `controller-test` | `node:22-bookworm-slim` | none | Test shell (`sleep infinity`) | Running; not a service |

The `suban-indexer-phase3` container is not part of the three Compose files. It
was started separately with:
- `PI_SOROBAN_RPC_URL=https://rpc.testnet.minepi.com`
- `PI_NETWORK_PASSPHRASE=Pi Testnet`
- `INDEXER_DB_PATH=/data/events.db`
- `INDEXER_API_PORT=3002`
- `INDEXER_POLL_MS=5000`
- `INDEXER_CONTRACTS_JSON` set to the current enriched contract registry

Its host port is `3102`, mapped to container port `3002`.

## 3. Public routing

Caddy and Cloudflare provide the public hostnames:

| Public host | Caddy upstream | What it serves |
|---|---|---|
| `horizon.suban.org` | `suban-api:4000` | Suban API and `/horizon/*` proxy to mainnet Horizon |
| `testnet.suban.org` | `pi-testnet:8000` | Raw Pi testnet Horizon |
| `testrpc.suban.org` | `stellar-rpc-testnet:8000` | Testnet Soroban JSON-RPC |
| `rpc.suban.org` | `suban-rpc:8000` | Mainnet Suban JSON-RPC; currently unhealthy |
| `oracle.suban.org` | `suban-controller:3000` | Price/oracle/risk read routes |
| `suban.org` and `www.suban.org` | Caddy static files | Escrow viewer; selected `/api/*` paths proxy to the controller |
| `data.suban.org` | none | Not provisioned; DNS does not resolve |

`horizon.suban.org/` returns the Suban API index, not raw Horizon. The raw
Horizon proxy is available at `horizon.suban.org/horizon/*`, and the
`/api/v1/ledgers` convenience route also worked at the check.

## 4. Reproduce the working stack on another device

### Prerequisites

- Git.
- Docker Desktop on Windows/macOS, or Docker Engine plus Compose v2 on Linux.
- Enough disk for the Pi node data. The first mainnet/testnet sync is large and
  takes time.
- Node 22 only if you run the TypeScript services outside Docker.
- Go and Rust only if you build `suban-rpc` outside Docker.

The Docker path below does not require Node, Go or Rust on the host.

### Clone the four repositories side by side

```bash
mkdir suban-repos
cd suban-repos
git clone https://github.com/Pi-Defi-world/Suban.git Suban
git clone https://github.com/Pi-Defi-world/suban-rpc.git suban-rpc
git clone https://github.com/Pi-Defi-world/Suban-api.git Suban-api
git clone https://github.com/Pi-Defi-world/suban-controller.git suban-controller
```

The Compose files build from `../../suban-rpc`, `../../Suban-api` and
`../../suban-controller`, so the directory layout matters.

### Start the official Pi nodes first

```bash
cd Suban/deploy
docker compose -f docker-compose.pi-mainnet-node.yml up -d
docker compose -f docker-compose.pi-testnet-node.yml up -d
```

This creates the external `docker_default` network used by the Suban services.
Wait for the nodes to sync before expecting useful Horizon data. The official
node's `rpc` supervisor service is not started by these files; Horizon is the
working API. Use `stellar-rpc-testnet` for testnet Soroban RPC.

Check the local Horizon endpoints:

```bash
curl -s http://localhost:41401/ | grep -E 'horizon_version|core_version|history_latest_ledger|network_passphrase'
curl -s http://localhost:42401/ | grep -E 'horizon_version|core_version|history_latest_ledger|network_passphrase'
```

On Windows PowerShell without `grep`:

```powershell
Invoke-RestMethod http://localhost:41401/ |
  Select-Object horizon_version, core_version, history_latest_ledger, network_passphrase
Invoke-RestMethod http://localhost:42401/ |
  Select-Object horizon_version, core_version, history_latest_ledger, network_passphrase
```

### Create local configuration

Copy the examples and edit them. Never commit `.env` or `.env.relayer`.

```bash
cd Suban/deploy
cp .env.example .env
cp ../services/bridge-relayer/.env.example .env.relayer
```

For a local-only run you can leave `TUNNEL_TOKEN` blank and skip the
`cloudflared` service. The bridge relayer is optional; if you do not run it,
you do not need to fill in its signer keys.

The service-specific examples are:

- `Suban-api/.env.example`
- `suban-controller/.env.example`
- `Suban/services/event-indexer/.env.example`
- `Suban/services/bridge-relayer/.env.example`
- `Suban/services/liquidation-bot/.env.example`

`suban-rpc` uses the checked-in `Suban/deploy/suban-rpc-mainnet.toml` and
`Suban/deploy/suban-rpc-testnet.toml`; those files contain no signer secrets.

### Start only the working subset locally

The optional `docker-compose.local.yml` override publishes ports on the host
without Cloudflare:

```bash
cd Suban/deploy
docker compose \
  -f docker-compose.suban.yml \
  -f docker-compose.local.yml \
  up -d suban-api suban-controller stellar-rpc-testnet
```

This starts the mainnet Horizon wrapper, the price controller and the testnet
Soroban RPC. It does **not** start the broken Suban Go RPC containers or the
older indexer.

Local checks:

```bash
curl -s http://localhost:4000/health
curl -s http://localhost:3000/api/v1/health
curl -s -X POST http://localhost:8200/ \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"getHealth","params":{}}'
```

The controller still has mainnet contract-RPC settings pointing at
`http://suban-rpc:8000` and `https://rpc.suban.org`; those mainnet contract
routes will fail while the Suban Go RPC is unhealthy. Price and Horizon-backed
routes work.

### Start the full Compose stack

```bash
cd Suban/deploy
docker compose -f docker-compose.suban.yml up -d --build
```

This also starts `suban-rpc`, `suban-rpc-testnet`, the older indexer, the bridge
relayer, Caddy and Cloudflare. It is the complete stack, not the healthy subset.
The public hostnames require a valid `TUNNEL_TOKEN` and Cloudflare DNS records.

### Start the enriched indexer separately

The enriched Phase 3 indexer is not the `event-indexer` service in the main
Compose file. Build and run it separately when you need the financial
projections:

```bash
docker build -t suban-event-indexer:local Suban/services/event-indexer
docker run -d --name suban-indexer-local \
  --network docker_default \
  -p 3102:3002 \
  -e PI_SOROBAN_RPC_URL=https://rpc.testnet.minepi.com \
  -e PI_NETWORK_PASSPHRASE='Pi Testnet' \
  -e HORIZON_URL=http://pi-testnet:8000 \
  -e INDEXER_DB_PATH=/data/events.db \
  -e INDEXER_API_PORT=3002 \
  -e INDEXER_POLL_MS=5000 \
  -v suban-indexer-local-data:/data \
  suban-event-indexer:local
```

Set `INDEXER_CONTRACTS_JSON` to the contract registry you intend to index. The
current enriched testnet addresses are in
[DEVELOPER.md](./DEVELOPER.md). Without a registry the indexer will run but will
not ingest contract events.

### Stop the stack

```bash
cd Suban/deploy
docker compose -f docker-compose.suban.yml down
```

`down` keeps named volumes. Do not add `-v` unless you intend to delete the Pi
node and service databases. The Pi nodes use large named volumes.

## 5. Public endpoint checks

From any device with internet access:

```bash
curl -s https://horizon.suban.org/horizon/ledgers?limit=1
curl -s https://testnet.suban.org/ledgers?limit=1
curl -s https://oracle.suban.org/api/v1/health

curl -s -X POST https://testrpc.suban.org/ \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"getHealth","params":{}}'

curl -s -X POST https://rpc.suban.org/ \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"getHealth","params":{}}'
```

Expected result at the check:

- `horizon.suban.org`, `testnet.suban.org`, `oracle.suban.org` and
  `testrpc.suban.org` respond successfully.
- `rpc.suban.org` returns a JSON-RPC error about a stale ledger.
- `data.suban.org` does not resolve.

## 6. Known rough edges

- **Mainnet Suban RPC is stale.** `suban-rpc` logs show
  `History: Skipping catchup: incompatible core version or invalid local state`
  and `getHealth` reports a ledger about 304 hours old. The container image is
  based on `community-v1.0-p27.1.0`; the live Pi nodes are protocol 28. Rebuild
  the RPC image with a protocol-28-compatible stellar-core before treating
  `rpc.suban.org` as usable. This was not changed during this documentation pass.
- **Testnet Suban RPC is crash-looping.** `suban-rpc-testnet` logs show
  `stellar-core catchup: exit status 3`, an empty database and a very high
  restart count. Use `testrpc.suban.org` or the official
  `https://rpc.testnet.minepi.com` for testnet Soroban RPC until the Suban RPC
  image is rebuilt.
- **The official node RPC supervisor is stopped.** `pi-mainnet` and
  `pi-testnet` both show `rpc STOPPED` in `supervisorctl status`. The
  `41403`/`42403` ports are not working RPC endpoints. The working testnet
  Soroban RPC is `stellar-rpc-testnet` at `testrpc.suban.org` (or local
  `:8200` with the Compose override).
- **The main Compose indexer is not the enriched indexer.**
  `suban-event-indexer` reports `0` events and `0` monitored contracts and points
  at the unhealthy `suban-rpc`. The separate `suban-indexer-phase3` container has
  the enriched data, but its `/status` showed `stale: true` with a lag of
  `101,752` ledgers at the check.
- **The public data platform is not provisioned.** `data.suban.org` does not
  resolve. Do not describe the indexed API as a public service.
- **`Suban-api` enhanced summary returned `400`.** `/health`, `/horizon/*` and
  `/api/v1/ledgers` worked; `/api/v1/summary` returned an
  `invalid_accounts_params` error at the check. Treat the Horizon proxy as the
  verified path and investigate the summary route before relying on it.
- **Bridge relayer health port was not reachable.** The container was running
  and polling Arc blocks and Stellar ledgers, but neither `3003` nor `9464`
  answered a health request from the Docker network at the check. The bridge is
  not a public API.
- **`testnet2` is not the Suban testnet node.** The Pi Network desktop app runs
  it on `31401`-`31403`. The Suban Compose testnet node is `pi-testnet` on
  `42401`-`42403`. Do not mix the ports.

## 7. Security notes

- Keep `.env`, `.env.relayer`, signer keys and API keys out of Git.
- The local bridge relayer and deployment accounts are testnet credentials.
  Rotate them if a transcript or log containing them is shared.
- Do not expose the controller admin routes, RPC admin port `8001`, the bridge
  relayer metrics port or the indexer admin routes to the public internet.
- Public read traffic should go through Caddy/Cloudflare; privileged control
  traffic should stay on a private network.
- This document records observed testnet state. It is not a production
  readiness claim, an audit, or evidence of external adoption.
