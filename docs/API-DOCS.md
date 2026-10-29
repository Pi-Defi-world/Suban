# Pi Horizon & Suban API Services

REST API servers for Pi Network blockchain data, running locally via Docker.

**Part of the [Suban Data Platform](https://suban.org)**

> Status: local Pi node/API runbook. For the verified 29 October 2026 container
> state and portable setup, see [LOCAL-SYSTEM.md](./LOCAL-SYSTEM.md). The Suban
> Go RPC is currently unhealthy; the official Pi Horizon APIs and the Stellar
> testnet RPC are the working paths.

---

## Services Overview

### Pi Horizon (Core Infrastructure)

| Service | Network | URL | Protocol | Browser? | Status |
|---------|---------|-----|----------|----------|--------|
| Horizon API | Mainnet | `http://localhost:41401` | REST (JSON) | Yes | Working |
| Horizon API | Testnet | `http://localhost:42401` | REST (JSON) | Yes | Working |
| Stellar RPC (node service) | Mainnet | `http://localhost:41403` | JSON-RPC (POST) | No | `rpc` supervisor STOPPED |
| Stellar RPC (node service) | Testnet | `http://localhost:42403` | JSON-RPC (POST) | No | `rpc` supervisor STOPPED |
| Soroban RPC (`stellar-rpc-testnet`) | Testnet | `http://localhost:8200` or `https://testrpc.suban.org` | JSON-RPC (POST) | No | Working |
| Stellar Core | Mainnet | `http://localhost:41402` | HTTP admin | Yes | Running |
| Stellar Core | Testnet | `http://localhost:42402` | HTTP admin | Yes | Running |

The `31401`-`31403` ports belong to the separate Pi Network desktop-app
`testnet2` node. The Suban Compose testnet node is `pi-testnet` on
`42401`-`42403`.

### Suban Platform (Enhanced Services)

| Service | URL | Domain | Browser? | Description |
|---------|-----|--------|----------|-------------|
| Suban API | `http://localhost:4000` | horizon.suban.org | Yes | Horizon wrapper with caching + rate limiting |
| Suban Controller | `http://localhost:3000` | oracle.suban.org | Yes | Price oracle + on-chain analytics |
| Suban RPC | `http://localhost:8000` | rpc.suban.org | No | JSON-RPC for smart contracts; currently unhealthy, see LOCAL-SYSTEM.md |

---

## What Is Each Service?

### Horizon API (ports 41401 / 42401)

Horizon is the **main REST API** for reading data from the Pi blockchain. It provides endpoints for:

- **Ledgers** — Browse blocks on the chain
- **Transactions** — Look up any transaction by hash or sequence
- **Accounts** — Check balances, trustlines, and account data
- **Payments** — Track payment history
- **Assets** — List issued assets and their holders
- **Operations** — Browse individual operations within transactions

**Works in your browser.** Open `http://localhost:41401` and you'll see a JSON response with links to explore the API.

Example endpoints:
```
GET /ledgers?order=desc&limit=10
GET /transactions/{hash}
GET /accounts/{address}
GET /accounts/{address}/transactions
GET /payments?order=desc&limit=20
GET /assets
```

Full documentation: https://developers.stellar.org/api/

### Stellar RPC (ports 41403 / 42403) and the working testnet Soroban RPC

Stellar RPC is a **JSON-RPC server** for interacting with smart contracts (Soroban) and advanced blockchain features. It handles:

- **Smart contract calls** — Execute and query Soroban contracts
- **Transaction simulation** — Simulate transactions before submitting
- **Event streaming** — Subscribe to contract events
- **Health checks** — Monitor node status

**Does NOT work in a browser.** Browsers send GET requests; Stellar RPC requires POST requests with a JSON body. Use `curl`, an SDK, or an HTTP client instead.

The official Pi-node supervisor services on `41403`/`42403` are currently
`STOPPED`. The working testnet Soroban RPC is the separate
`stellar-rpc-testnet` container at `https://testrpc.suban.org` (or local
`http://localhost:8200` with the Compose override). The mainnet Suban JSON-RPC
on `8000` is stale; see [LOCAL-SYSTEM.md](./LOCAL-SYSTEM.md).

Example usage:
```bash
# Working testnet Soroban RPC, local override
curl -X POST http://localhost:8200/ \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"getHealth","params":{}}'

# Get latest ledger
curl -X POST https://testrpc.suban.org/ \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"getLatestLedger"}'
```

Full documentation: https://developers.stellar.org/docs/smart-contracts/

### Stellar Core (ports 41402 / 42402)

Stellar Core is the **underlying node software** that validates transactions and maintains the ledger. Horizon and RPC sit on top of it. You typically don't need to interact with it directly.

---

## Why Two Different Protocols?

| | Horizon (REST) | Stellar RPC (JSON-RPC) |
|---|---|---|
| **Method** | GET / POST | POST only |
| **Browser** | Works | Does not work |
| **Format** | `?key=value` params | `{"jsonrpc":"2.0", ...}` body |
| **Use case** | Reading chain data | Smart contracts, Soroban |
| **Example** | `GET /ledgers?limit=1` | `POST {"method":"getHealth"}` |

---

## Quick Test

### Test Horizon (works in browser)
Open in browser or curl:
```
http://localhost:41401/ledgers?order=desc&limit=1
http://localhost:42401/ledgers?order=desc&limit=1
```

### Test RPC (must use POST)
```bash
# Working testnet Soroban RPC (local override)
curl -X POST http://localhost:8200/ -H "Content-Type: application/json" -d '{"jsonrpc":"2.0","id":1,"method":"getHealth","params":{}}'

# Working testnet Soroban RPC (public)
curl -X POST https://testrpc.suban.org/ -H "Content-Type: application/json" -d '{"jsonrpc":"2.0","id":1,"method":"getHealth","params":{}}'
```

The official node `rpc` services on `41403`/`42403` are STOPPED at the check.
Do not use them as working RPC endpoints.

### PowerShell
```powershell
# Horizon
Invoke-RestMethod "http://localhost:41401/ledgers?order=desc&limit=1"

# RPC
Invoke-RestMethod -Uri "http://localhost:8200/" -Method Post -ContentType "application/json" -Body '{"jsonrpc":"2.0","id":1,"method":"getHealth","params":{}}'
```

---

## Architecture

The diagram below labels the older desktop-app testnet node `testnet2` on
`31401`-`31403`. For the Suban Compose testnet node, substitute `pi-testnet` and
`42401`-`42403`. The diagram's Stellar RPC boxes are the official node supervisor
services, which are STOPPED; the working testnet Soroban RPC is
`stellar-rpc-testnet` at `testrpc.suban.org`.

```
┌─────────────────────────────────────────────────────────────────────┐
│  Pi Mainnet (pi-mainnet container)                                 │
│                                                                     │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────────────────────┐ │
│  │ Stellar Core │  │   Horizon    │  │       Stellar RPC          │ │
│  │  :41402      │  │   :41401     │  │          :41403            │ │
│  └──────┬───────┘  └──────┬───────┘  └───────────┬────────────────┘ │
│         │                 │                      │                  │
│         └─────────────────┴──────────────────────┘                  │
│                         ↓ metadata                                  │
│                   PostgreSQL                                        │
└─────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────┐
│  Pi Testnet (testnet2 container)                                   │
│                                                                     │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────────────────────┐ │
│  │ Stellar Core │  │   Horizon    │  │       Stellar RPC          │ │
│  │  :31402      │  │   :31401     │  │          :31403            │ │
│  └──────────────┘  └──────────────┘  └────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Running / Stopping Services

```bash
# Check container status
docker ps

# Start Horizon (the rpc supervisor service is left stopped)
docker exec pi-mainnet supervisorctl start horizon
docker exec pi-testnet supervisorctl start horizon

# Check service status
docker exec pi-mainnet supervisorctl status
docker exec pi-testnet supervisorctl status

# Stop Horizon
docker exec pi-mainnet supervisorctl stop horizon
docker exec pi-testnet supervisorctl stop horizon

# Restart everything
docker restart pi-mainnet pi-testnet
```

The official node `rpc` supervisor service is intentionally left stopped. Use
`stellar-rpc-testnet` / `testrpc.suban.org` for testnet Soroban RPC.

---

## Docker Compose Files

| File | Description |
|------|-------------|
| `docker-compose.pi-mainnet-node.yml` | Current mainnet node (`p28.0.1`) |
| `docker-compose.pi-testnet-node.yml` | Current testnet node (`p28.0.1`, ports `42401`-`42403`) |
| `docker-compose.pi-dual.yml` | Older dual-node file (`p27.1.0`, testnet on `31401`-`31403`); use the two current files instead |

```bash
# Start mainnet only
docker compose -f docker-compose.pi-mainnet-node.yml up -d

# Start testnet only
docker compose -f docker-compose.pi-testnet-node.yml up -d

# Start both (current p28 files)
docker compose -f docker-compose.pi-mainnet-node.yml up -d
docker compose -f docker-compose.pi-testnet-node.yml up -d
```

After starting containers, wait for stellar-core to sync, then start services:
```bash
docker exec pi-mainnet supervisorctl start horizon rpc
docker exec pi-testnet supervisorctl start horizon rpc
```

---

## Network Details

| | Mainnet | Testnet |
|---|---|---|
| **Network Passphrase** | `Pi Network` | `Pi Testnet` |
| **History Archive** | `https://history.mainnet.minepi.com/` | `https://history.testnet.minepi.com/` |
| **Docker Image** | `pinetwork/pi-node-docker:community-v1.0-p28.0.1` | Same |
| **Container Name** | `pi-mainnet` | `pi-testnet` |
| **Horizon Port** | 41401 | 42401 |
| **RPC Port** | 41403 (supervisor `rpc` STOPPED) | 42403 (supervisor `rpc` STOPPED) |
| **Core Peer Port** | 41402 | 42402 |

The Pi Network desktop-app `testnet2` node uses `31401`-`31403` and is separate
from the Suban Compose `pi-testnet` node.

---

## Troubleshooting

### "405 Method Not Allowed" in browser
You're accessing a Stellar RPC endpoint (`41403`/`42403`) with a browser GET request. This is expected - RPC requires POST requests. Use `curl` or an SDK instead. For browser-friendly endpoints, use Horizon (`41401`/`42401`). If the connection is refused, the official node's supervisor `rpc` service is STOPPED; use `https://testrpc.suban.org` for testnet Soroban RPC.

### Horizon not starting
```bash
docker exec pi-mainnet supervisorctl status
# If horizon is STOPPED:
docker exec pi-mainnet supervisorctl start horizon
```

### RPC not responding

The official Pi-node `rpc` supervisor service is intentionally left stopped.
Use `stellar-rpc-testnet` / `https://testrpc.suban.org` for testnet Soroban
RPC. If you are investigating the official node service, inspect it without
treating it as the supported path:

```bash
# Check the official node rpc service
docker exec pi-mainnet supervisorctl status rpc
docker exec pi-testnet supervisorctl status rpc

# Inspect container logs
docker logs --tail 100 pi-mainnet
docker logs --tail 100 pi-testnet
```

### Container won't start (port already in use)
Another container is using the same port. Stop conflicting containers:
```bash
docker ps  # see what's running
docker stop <container-name>  # stop the conflicting one
```
