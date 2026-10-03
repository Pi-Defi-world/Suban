# Suban API Documentation

Complete reference for all Suban Data Platform endpoints.

**Base URL:** `https://<service>.suban.org`

All responses are JSON. All endpoints accept GET requests unless noted.

---

## Table of Contents

1. [Services Overview](#services-overview)
2. [Horizon API (Mainnet)](#horizon-api-mainnet)
3. [Horizon API (Testnet)](#horizon-api-testnet)
4. [JSON-RPC (Mainnet)](#json-rpc-mainnet)
5. [JSON-RPC (Testnet)](#json-rpc-testnet)
6. [Oracle](#oracle)
7. [Error Handling](#error-handling)
8. [Rate Limits](#rate-limits)

---

## Services Overview

| Service | URL | Description |
|---------|-----|-------------|
| Horizon (Mainnet) | `https://horizon.suban.org` | Read blockchain data from Pi mainnet |
| Horizon (Testnet) | `https://testnet.suban.org` | Read blockchain data from Pi testnet |
| RPC (Mainnet) | `https://rpc.suban.org` | Submit transactions, call smart contracts on mainnet |
| RPC (Testnet) | `https://testrpc.suban.org` | Submit transactions, call smart contracts on testnet |
| Oracle | `https://oracle.suban.org` | Pi price feeds + on-chain analytics |

---

## Horizon API (Mainnet)

**URL:** `https://horizon.suban.org`

Horizon is the REST API for reading data from the Pi mainnet blockchain.

### Ledgers

#### `GET /ledgers`

List ledgers (blocks) on the network, ordered by sequence number.

**Query Parameters:**
| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `cursor` | string | - | Pagination cursor |
| `order` | string | `asc` | Sort order: `asc` or `desc` |
| `limit` | int | `10` | Number of results (max 200) |

**Response:**
```json
{
  "_links": { "self": "...", "next": "...", "prev": "..." },
  "_embedded": {
    "records": [
      {
        "id": "abc123...",
        "paging_token": "28500001",
        "hash": "def456...",
        "prev_hash": "...",
        "sequence": 28500001,
        "transaction_count": 5,
        "operation_count": 12,
        "closed_at": "2026-09-02T12:00:00Z",
        "total_coins": "100000000000",
        "fee_pool": "1000",
        "base_fee": "100",
        "base_reserve": "10",
        "max_tx_set_size": 100,
        "protocol_version": 21
      }
    ]
  }
}
```

#### `GET /ledgers/:sequence`

Get a single ledger by sequence number.

---

### Transactions

#### `GET /transactions`

List transactions. Results are ordered by ledger close time.

**Query Parameters:**
| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `cursor` | string | - | Pagination cursor |
| `order` | string | `asc` | Sort order |
| `limit` | int | `10` | Number of results |
| `include_failed` | bool | `false` | Include failed transactions |

**Response:**
```json
{
  "_embedded": {
    "records": [
      {
        "id": "tx123...",
        "hash": "abcdef...",
        "ledger": 28500001,
        "created_at": "2026-09-02T12:00:00Z",
        "source_account": "GABC...",
        "source_account_sequence": 123456,
        "fee_charged": 100,
        "max_fee": 1000,
        "operation_count": 1,
        "envelope_xdr": "...",
        "result_xdr": "...",
        "result_meta_xdr": "...",
        "fee_meta_xdr": "...",
        "memo_type": "none",
        "signatures": ["sig1...", "sig2..."],
        "valid_after": "...",
        "valid_before": "..."
      }
    ]
  }
}
```

#### `GET /transactions/:hash_or_id`

Get a transaction by its hash or transaction ID.

---

### Operations

#### `GET /operations`

List operations (the individual instructions within a transaction).

**Query Parameters:** Same as transactions.

**Response:**
```json
{
  "_embedded": {
    "records": [
      {
        "id": 12345,
        "paging_token": "12345",
        "source_account": "GABC...",
        "type": "payment",
        "type_i": 1,
        "created_at": "2026-09-02T12:00:00Z",
        "transaction_hash": "tx123...",
        "amount": "100.0000000",
        "asset_type": "native",
        "from": "GABC...",
        "to": "GDEF..."
      }
    ]
  }
}
```

---

### Accounts

#### `GET /accounts`

List all accounts on the network.

**Query Parameters:**
| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `sponsor` | string | - | Filter by sponsor account |
| `cursor` | string | - | Pagination cursor |
| `order` | string | `asc` | Sort order |
| `limit` | int | `10` | Number of results |

**Response:**
```json
{
  "_embedded": {
    "records": [
      {
        "id": "GABC123...",
        "account_id": "GABC123...",
        "sequence": 123456,
        "subentry_count": 3,
        "home_domain": "example.com",
        "thresholds": { "low_threshold": 0, "med_threshold": 0, "high_threshold": 0 },
        "flags": { "auth_required": false, "auth_revocable": false, "auth_immutable": false },
        "balances": [
          { "balance": "100.0000000", "asset_type": "native" },
          { "balance": "50.0000000", "asset_type": "credit_alphanum4", "asset_code": "PI", "asset_issuer": "GDEF..." }
        ],
        "signers": [
          { "weight": 1, "key": "GABC123...", "type": "ed25519_public_key" }
        ],
        "data": {},
        "paging_token": "GABC123..."
      }
    ]
  }
}
```

#### `GET /accounts/:account_id`

Get a single account by public key.

**Response fields:**
- `id` - Account public key (G...)
- `sequence` - Current sequence number
- `balances` - Array of asset balances
- `signers` - Array of signers
- `thresholds` - Multi-sig thresholds
- `data` - Key-value data attached to account
- `subentry_count` - Number of sub-entries
- `home_domain` - Domain name for stellar.toml

---

### Payments

#### `GET /payments`

List payment operations across the network.

**Query Parameters:**
| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `cursor` | string | - | Pagination cursor |
| `order` | string | `asc` | Sort order |
| `limit` | int | `10` | Number of results |

---

### Trades

#### `GET /trades`

List trades (asset exchanges).

**Query Parameters:**
| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `base_asset_type` | string | - | Filter: `native`, `credit_alphanum4`, `credit_alphanum12` |
| `base_asset_code` | string | - | Base asset code |
| `base_asset_issuer` | string | - | Base asset issuer |
| `counter_asset_type` | string | - | Counter asset type |
| `counter_asset_code` | string | - | Counter asset code |
| `counter_asset_issuer` | string | - | Counter asset issuer |

---

### Assets

#### `GET /assets`

List all assets issued on the network.

**Query Parameters:**
| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `asset_code` | string | - | Filter by asset code |
| `asset_issuer` | string | - | Filter by issuer |

---

### Offers

#### `GET /offers`

List open offers (decentralized exchange orders).

---

### Effects

#### `GET /effects`

List all effects (changes to the ledger state).

---

### Streams (SSE)

Horizon supports Server-Sent Events for real-time updates:

```
GET /ledgers?cursor=now
GET /transactions?cursor=now
GET /payments?cursor=now
```

This opens a persistent connection that streams new events as they occur.

---

## Horizon API (Testnet)

**URL:** `https://testnet.suban.org`

Same endpoints as mainnet, but queries the Pi testnet blockchain. Use this for testing before going to mainnet.

All endpoints are identical to the mainnet Horizon API above.

---

## JSON-RPC (Mainnet)

**URL:** `https://rpc.suban.org`

The Stellar RPC server provides a JSON-RPC 2.0 interface for submitting transactions and interacting with smart contracts.

**Protocol:** JSON-RPC 2.0 over HTTP (POST only)

**Content-Type:** `application/json`

### Request Format

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "method_name",
  "params": { ... }
}
```

### Methods

#### `getHealth`

Check if the RPC server is healthy.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "getHealth"
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "status": "healthy"
  }
}
```

---

#### `getNetwork`

Get information about the network.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "getNetwork"
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "friendbotUrl": "https://friendbot-pi.stellar.org",
    "passphrase": "Pi Network",
    "protocolVersion": 21
  }
}
```

---

#### `getLatestLedger`

Get the latest ledger info.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "getLatestLedger"
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "hash": "...",
    "sequence": 28500001,
    "protocolVersion": 21,
    "baseFee": 100,
    "base Reserve": 10,
    "maxTxSetSize": 100,
    "closedAt": "2026-09-02T12:00:00Z"
  }
}
```

---

#### `getTransaction`

Get a transaction by hash.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "getTransaction",
  "params": {
    "hash": "abc123..."
  }
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "status": "SUCCESS",
    "ledger": 28500001,
    "createdAt": "2026-09-02T12:00:00Z",
    "maxFee": 1000,
    "feeCharged": 100,
    "hash": "abc123...",
    "envelopeXdr": "...",
    "resultXdr": "...",
    "resultMetaXdr": "..."
  }
}
```

---

#### `simulateTransaction`

Simulate a transaction to estimate fees and check for errors before submitting.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "simulateTransaction",
  "params": {
    "transaction": "base64_encoded_tx..."
  }
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "id": "...",
    "status": "SUCCESS",
    "error": null,
    "fees": 100,
    "results": [...],
    "transactionData": "...",
    "events": [...]
  }
}
```

---

#### `sendTransaction`

Submit a signed transaction to the network.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "sendTransaction",
  "params": {
    "transaction": "base64_encoded_signed_tx...",
    "simulate": true
  }
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "hash": "abc123...",
    "status": "PENDING",
    "ledger": null
  }
}
```

---

#### `getAccount`

Get account details.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "getAccount",
  "params": {
    "address": "GABC123..."
  }
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "id": "GABC123...",
    "sequence": 123456,
    "subentryCount": 3,
    "thresholds": { "low": 0, "med": 0, "high": 0 },
    "flags": 0,
    "balances": [...],
    "signers": [...],
    "data": {}
  }
}
```

---

#### `getEvents`

Query contract events.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "getEvents",
  "params": {
    "startLedger": 28500000,
    "endLedger": 28500100,
    "filters": [
      { "type": "contract", "contractIds": ["..."] },
      { "type": "topic", "topics": ["..."] }
    ]
  }
}
```

---

### Error Response Format

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32600,
    "message": "Invalid Request",
    "data": {
      "code": "TRANSACTION_FAILED",
      "message": "...",
      "data": { ... }
    }
  }
}
```

Common error codes:
| Code | Description |
|------|-------------|
| `-32600` | Invalid Request |
| `-32601` | Method Not Found |
| `-32602` | Invalid Params |
| `-32603` | Internal Error |

---

## JSON-RPC (Testnet)

**URL:** `https://testrpc.suban.org`

Same JSON-RPC methods as mainnet, but operates on the Pi testnet. Use this for development and testing.

---

## Oracle

**URL:** `https://oracle.suban.org`

The Oracle provides aggregated Pi price data and on-chain analytics.

### Price Endpoints

#### `GET /api/v1/price`

Get the aggregated Pi price from multiple exchanges.

**Response:**
```json
{
  "symbol": "PI",
  "price_usd": 1.23,
  "timestamp": "2026-09-02T12:00:00.000Z",
  "sources_used": 3,
  "total_sources": 3,
  "aggregation_method": "weighted_average",
  "source_prices": {
    "coingecko": { "price": 1.22, "weight": 1.5, "timestamp": "..." },
    "okx": { "price": 1.23, "weight": 2.0, "timestamp": "..." },
    "bitget": { "price": 1.24, "weight": 2.0, "timestamp": "..." }
  },
  "confidence_score": 0.95,
  "cache_hit": false
}
```

**Fields:**
- `price_usd` - Weighted average price in USD
- `confidence_score` - 0.0 to 1.0 based on source agreement
- `sources_used` - Number of sources that responded
- `source_prices` - Individual prices from each exchange

---

#### `GET /api/v1/sources`

Get status of each price data source.

**Response:**
```json
{
  "sources": [
    {
      "name": "coingecko",
      "weight": 1.5,
      "enabled": true,
      "last_fetch": "2026-09-02T12:00:00.000Z",
      "success_count": 100,
      "fail_count": 2,
      "avg_response_ms": 450
    }
  ],
  "total_sources": 3
}
```

---

#### `GET /api/v1/health`

Health check with uptime.

**Response:**
```json
{
  "status": "healthy",
  "uptime": 3600,
  "timestamp": "2026-09-02T12:00:00.000Z"
}
```

---

### On-Chain Analytics

#### `GET /api/v1/chain/stats`

Network statistics from the local Horizon node.

**Response:**
```json
{
  "network": "mainnet",
  "latest_ledger": 28500001,
  "ledger_count": 28500001,
  "account_count": 1500000,
  "tx_count": 5000000,
  "operation_count": 12000000
}
```

---

#### `GET /api/v1/chain/ledgers`

Latest ledgers with transaction/operation counts.

**Query Parameters:**
| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `limit` | int | `10` | Number of ledgers (max 50) |

**Response:**
```json
{
  "ledgers": [
    {
      "sequence": 28500001,
      "hash": "abc123...",
      "closed_at": "2026-09-02T12:00:00Z",
      "transaction_count": 5,
      "operation_count": 12,
      "base_fee": 100
    }
  ]
}
```

---

#### `GET /api/v1/chain/accounts`

Top accounts by balance.

**Query Parameters:**
| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `limit` | int | `10` | Number of accounts (max 50) |

---

#### `GET /api/v1/chain/accounts/:accountId`

Get account details including balances and recent transactions.

---

#### `GET /api/v1/chain/transactions`

Recent transactions.

**Query Parameters:**
| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `limit` | int | `10` | Number of transactions |
| `cursor` | string | - | Pagination cursor |

---

#### `GET /api/v1/chain/network`

Network configuration (passphrase, protocol version, etc.).

---

### Horizon Proxy

All Horizon mainnet endpoints are also available via:

```
GET /horizon/*
```

This proxies to the local Horizon node with caching.

---

## Error Handling

All services return errors in a consistent format:

```json
{
  "error": "Error type",
  "message": "Human-readable description"
}
```

Common HTTP status codes:
| Code | Description |
|------|-------------|
| `200` | Success |
| `400` | Bad request (invalid parameters) |
| `404` | Resource not found |
| `429` | Rate limit exceeded |
| `500` | Internal server error |

---

## Rate Limits

### Suban API
- 100 requests per minute per IP

### Horizon API
- Standard Stellar Horizon rate limits apply

### Oracle
- Price data cached for 10 seconds
- Chain data cached for 30 seconds

### JSON-RPC
- Rate limits vary by method
- `sendTransaction` has stricter limits
