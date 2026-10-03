# Pi Horizon & Suban API Services

REST API servers for Pi Network blockchain data, running locally via Docker.

**Part of the [Suban Data Platform](https://suban.org)**

---

## Services Overview

### Pi Horizon (Core Infrastructure)

| Service | Network | URL | Protocol | Browser? |
|---------|---------|-----|----------|----------|
| Horizon API | Mainnet | `http://localhost:41401` | REST (JSON) | Yes |
| Horizon API | Testnet | `http://localhost:31401` | REST (JSON) | Yes |
| Stellar RPC | Mainnet | `http://localhost:41403` | JSON-RPC (POST) | No |
| Stellar RPC | Testnet | `http://localhost:31403` | JSON-RPC (POST) | No |
| Stellar Core | Mainnet | `http://localhost:41402` | HTTP admin | Yes |
| Stellar Core | Testnet | `http://localhost:31402` | HTTP admin | Yes |

### Suban Platform (Enhanced Services)

| Service | URL | Domain | Browser? | Description |
|---------|-----|--------|----------|-------------|
| Suban API | `http://localhost:4000` | api.suban.org | Yes | Horizon wrapper with caching + rate limiting |
| Suban Controller | `http://localhost:3000` | oracle.suban.org | Yes | Price oracle + on-chain analytics |
| Suban RPC | `http://localhost:8000` | rpc.suban.org | No | JSON-RPC for smart contracts |

---

## What Is Each Service?

### Horizon API (ports 41401 / 31401)

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

### Stellar RPC (ports 41403 / 31403)

Stellar RPC is a **JSON-RPC server** for interacting with smart contracts (Soroban) and advanced blockchain features. It handles:

- **Smart contract calls** — Execute and query Soroban contracts
- **Transaction simulation** — Simulate transactions before submitting
- **Event streaming** — Subscribe to contract events
- **Health checks** — Monitor node status

**Does NOT work in a browser.** Browsers send GET requests; Stellar RPC requires POST requests with a JSON body. Use `curl`, an SDK, or an HTTP client instead.

Example usage:
```bash
# Health check
curl -X POST http://localhost:41403/ \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}'

# Get latest ledger
curl -X POST http://localhost:41403/ \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"getLatestLedger"}'
```

Full documentation: https://developers.stellar.org/docs/smart-contracts/

### Stellar Core (ports 41402 / 31402)

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
http://localhost:31401/ledgers?order=desc&limit=1
```

### Test RPC (must use POST)
```bash
# Mainnet
curl -X POST http://localhost:41403/ -H "Content-Type: application/json" -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}'

# Testnet
curl -X POST http://localhost:31403/ -H "Content-Type: application/json" -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}'
```

### PowerShell
```powershell
# Horizon
Invoke-RestMethod "http://localhost:41401/ledgers?order=desc&limit=1"

# RPC
Invoke-RestMethod -Uri "http://localhost:41403/" -Method Post -ContentType "application/json" -Body '{"jsonrpc":"2.0","id":1,"method":"getHealth"}'
```

---

## Architecture

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

# Start services (after container restart)
docker exec pi-mainnet supervisorctl start horizon rpc
docker exec testnet2 supervisorctl start horizon rpc

# Check service status
docker exec pi-mainnet supervisorctl status
docker exec testnet2 supervisorctl status

# Stop services
docker exec pi-mainnet supervisorctl stop horizon rpc
docker exec testnet2 supervisorctl stop horizon rpc

# Restart everything
docker restart pi-mainnet testnet2
```

---

## Docker Compose Files

| File | Description |
|------|-------------|
| `docker-compose.pi-mainnet-node.yml` | Mainnet only |
| `docker-compose.pi-testnet-node.yml` | Testnet only |
| `docker-compose.pi-dual.yml` | Both mainnet + testnet |

```bash
# Start mainnet only
docker compose -f docker-compose.pi-mainnet-node.yml up -d

# Start testnet only
docker compose -f docker-compose.pi-testnet-node.yml up -d

# Start both
docker compose -f docker-compose.pi-dual.yml up -d
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
| **History Archive** | `https://history.mainnet.minepi.com/` | `https://history.testnet2.minepi.com/` |
| **Docker Image** | `pinetwork/pi-node-docker:community-v1.0-p27.1.0` | Same |
| **Container Name** | `pi-mainnet` | `testnet2` |
| **Horizon Port** | 41401 | 31401 |
| **RPC Port** | 41403 | 31403 |
| **Core Peer Port** | 41402 | 31402 |

---

## Troubleshooting

### "405 Method Not Allowed" in browser
You're accessing a Stellar RPC endpoint (`41403`/`31403`) with a browser GET request. This is expected — RPC requires POST requests. Use `curl` or an SDK instead. For browser-friendly endpoints, use Horizon (`41401`/`31401`).

### Horizon not starting
```bash
docker exec pi-mainnet supervisorctl status
# If horizon is STOPPED:
docker exec pi-mainnet supervisorctl start horizon
```

### RPC not responding
```bash
# Check if RPC is running
docker exec pi-mainnet supervisorctl status rpc

# If STOPPED, start it
docker exec pi-mainnet supervisorctl start rpc

# If keeps crashing, check logs
docker exec pi-mainnet tail -50 /var/log/supervisor/rpc-stdout---supervisor-*.log
```

### Container won't start (port already in use)
Another container is using the same port. Stop conflicting containers:
```bash
docker ps  # see what's running
docker stop <container-name>  # stop the conflicting one
```
