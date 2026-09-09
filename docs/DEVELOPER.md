# Suban Protocol — Developer Documentation

## Overview

Suban is a DeFi protocol hub built on Stellar/Soroban (Protocol 27), providing:
- **AMM Core** — Constant-product and stableswap pools with factory deployment
- **Lending Core** — Isolated lending pools with health factors and liquidation
- **Oracle** — Decentralized price feeds with staleness checks and circuit breakers
- **Escrow** — Milestone-based escrow for freelancing and payments
- **Bridge** — wPi bridge with multi-sig attestation

## Quick Start

```bash
pnpm add @suban/sdk
```

```typescript
import { SubanClient, LendingClient, OracleClient, PoolFactoryClient } from '@suban/sdk';

const client = new SubanClient({
  rpcUrl: 'https://rpc.testnet.minepi.com',
  networkPassphrase: 'Pi Testnet',
});

// Read pool state
const lending = new LendingClient(client, '<lending-pool-id>');
const state = await lending.getPoolState();

// Read oracle price
const oracle = new OracleClient(client, '<oracle-contract-id>');
const price = await oracle.getPrice('<asset-address>');

// Create a pool via factory
const factory = new PoolFactoryClient(client, '<pool-factory-id>');
const pools = await factory.listPools();
```

## Contract Addresses (Pi Testnet)

| Contract | Address |
|----------|---------|
| Pool Factory | `CCQKGU54YX6UBHRANG4JNDR3TMDA4HGJ4NLP5KGJC2JLQRGUC7KIMOAW` |
| Backstop | `CD4HGZAQN5C2O53K2OMV5HTDSJNJM56V7X6CGBIOJJ2A3QMV7FPYNMRX` |
| Lending Pool | `CAZETGC2BYWX5KY653BF4J2WDXFTAQAKSW7CBKDEJVXJG2KRBOHXLJXI` |
| Bridge Multi-Sig | `CBNGDXVUGRHQPTHYUOTQJVZKYSPZI2DCEWA6JUJCTV7IYF7Z2STLPXVA` |
| Oracle | `CBLQKWBP2TV3CPM26DDNMEMDEMLGYAJHJ5O2ZM4UD32OVXBOCA25DE3M` |
| Escrow Manager | `CC3JNU7LAGOLF5TAZ2BP23ZDFSDMVM6IR7O3MWHGM65QII3UINQNM4EU` |
| CPMM Pool (wPi/MockUSDC) | `CBE7DHJDS6HZHVY7AWU42XDUA3WAG7GRFAEYEBRM5J3XG2KUYDBAO2NZ` |
| Stableswap | `CAETZYW4ADMRCQIVLONFTAEP4WQGIVP2V5DZ7XXSPPVEK5CNHXOJOHMX` |
| Swap Router | `CA3AECFKZSGBODPXWWTOBVKUEWQLAGWAIO5E2RMAAVYMLCERSSEU4KYZ` |
| Liquidity Mining | `CDC6LFSPVE7XOAAVU2JBUQQNFHAAIH2DESW4M4KU6KXS2I2CHFFGA555` |
| wPi Token | `CCODZXYOZMKBOKCCMOKSVW3FIGPJRILM7GH4MKFOS4IWZ56EBX5MVONV` |
| MockUSDC | `CAPDFYOFXSQTVCZ7KPUACHVNMOO3TWLSLHTPQ3H64EBJVRFMAWIBEUAY` |

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Suban Hub                              │
├──────────────┬──────────────┬──────────────┬────────────────┤
│  AMM Core    │ Lending Core │   Oracle     │    Escrow      │
│  ──────────  │  ──────────  │  ──────────  │  ────────────  │
│  pool-factory│ lending-pool │   oracle     │   escrow       │
│  cpmm-pool   │  backstop    │              │                │
│  stableswap  │              │              │                │
│  swap-router │              │              │                │
│  liq-mining  │              │              │                │
├──────────────┴──────────────┴──────────────┴────────────────┤
│                    Shared Primitives                         │
│  ─────────────────────────────────────────────────────────  │
│  hub-types  hub-errors  hub-events  hub-token               │
├─────────────────────────────────────────────────────────────┤
│                  Bridge Infrastructure                       │
│  ─────────────────────────────────────────────────────────  │
│  wpi-token  usdc-vault  bridge-multisig                     │
└─────────────────────────────────────────────────────────────┘
```

---

## Pool Factory — Third-Party Pool Creation

The Pool Factory allows anyone to deploy isolated AMM pools. Two paths:

### Path 1: Deploy a new pool (create_pool)

```typescript
import { PoolFactoryClient } from '@suban/sdk';
import { Keypair } from '@stellar/stellar-sdk';

const factory = new PoolFactoryClient(client, POOL_FACTORY_ID);

// 1. Upload pool WASM (cpmm-pool or stableswap)
// 2. Call create_pool with the WASM hash + salt
const result = await factory.createPool(
  wasmHash,        // BytesN<32> from upload_contract_wasm()
  salt,            // BytesN<32> unique salt
  tokenA,          // Address of token A
  tokenB,          // Address of token B
  0,               // pool_type: 0 = ConstantProduct, 1 = Stableswap
  30,              // fee_bps: 30 = 0.3%
  'My Pool',       // name: short display name
  callerKeypair,   // signer
);
// result.txHash, result.result contains (pool_id, pool_address)
```

### Path 2: Register a pre-deployed pool (admin only)

```typescript
const result = await factory.registerPool(
  poolAddress,     // existing pool contract address
  tokenA,
  tokenB,
  0,               // pool_type
  30,              // fee_bps
  adminKeypair,    // admin signer
);
```

### Pool Management

```typescript
// List active pools
const pools = await factory.listPools();

// List all pools including inactive
const all = await factory.listAllPools();

// Deactivate (creator or admin)
await factory.removePool(poolId, callerKeypair);

// Reactivate (admin only)
await factory.reactivatePool(poolId, adminKeypair);
```

---

## AMM — Constant-Product Pool

```typescript
import { CpmmPoolClient } from '@suban/sdk';

const pool = new CpmmPoolClient(client, cpmmPoolAddress);

// Initialize (first time)
await pool.initialize(tokenA, tokenB, 30, adminKeypair); // 0.3% fee

// Add liquidity
const tx = await pool.addLiquidity('1000000000', '500000000', providerKeypair);

// Swap
const swap = await pool.swap(tokenA, '100000000', '90000000', traderKeypair);

// Remove liquidity
await pool.removeLiquidity('500000000', providerKeypair);

// Read state
const state = await pool.getState();
const lpBalance = await pool.getLpBalance(userAddress);
```

---

## Lending Pool

```typescript
import { LendingClient } from '@suban/sdk';

const lending = new LendingClient(client, lendingPoolAddress);

// Read
const state = await lending.getPoolState();
const position = await lending.getPosition(userAddress);
const health = await lending.getHealthFactor(userAddress);

// Deposit collateral
await lending.depositCollateral('1000000000', userKeypair);

// Borrow
await lending.borrow('500000000', userKeypair);

// Repay
await lending.repay('500000000', userKeypair);

// Liquidate (keeper)
await lending.liquidate(undercollateralizedUser, '200000000', keeperKeypair);

// Set collateral asset (admin)
await lending.setCollateralAsset(pusdAddress, adminKeypair);
```

---

## Oracle

```typescript
import { OracleClient } from '@suban/sdk';

const oracle = new OracleClient(client, oracleAddress);

// Get price
const price = await oracle.getPrice(assetAddress);

// Check staleness
const stale = await oracle.isStale(assetAddress, maxAgeLedgers);

// Get config
const config = await oracle.getConfig();
```

---

## Escrow

```typescript
import { EscrowClient } from '@suban/sdk';

const escrow = new EscrowClient(client, escrowAddress);

// Read
const state = await escrow.getEscrow(escrowId);
const count = await escrow.getEscrowCount();

// Create
const milestones = [
  { description: 'Design', approver: funderAddress, amount: '1000000000' },
  { description: 'Build', approver: funderAddress, amount: '2000000000' },
];
await escrow.createEscrow(receiver, arbitrator, asset, milestones, 100, deadline, funderKeypair);

// Fund
await escrow.fundEscrow(escrowId, '3000000000', funderKeypair);

// Submit milestone (receiver)
await escrow.submitMilestone(escrowId, 0, receiverKeypair);

// Approve milestone (designated approver)
await escrow.approveMilestone(escrowId, 0, approverKeypair);

// Release funds (funder)
await escrow.releaseFunds(escrowId, funderKeypair);

// Dispute (funder or receiver)
await escrow.dispute(escrowId, 'reason', disputerKeypair);

// Resolve (arbitrator)
await escrow.resolve(escrowId, true, arbitratorKeypair); // true = release, false = refund
```

---

## Bridge

```typescript
import { BridgeClient } from '@suban/sdk';

const bridge = new BridgeClient(client, bridgeAddress);

// Read
const config = await bridge.getConfig();
const processed = await bridge.isDepositProcessed(depositIdHex);
const stats = await bridge.getVolumeStats();

// Propose mint (after deposit observed)
await bridge.proposeMint(depositId, toAddress, amount, depositorKeypair);

// Approve mint (signer)
await bridge.approveMint(proposalId, signerKeypair);

// Pause/unpause (admin)
await bridge.pause(adminKeypair);
await bridge.unpause(adminKeypair);
```

---

## Error Codes

### FactoryError (pool-factory)

| Code | Name | Description |
|------|------|-------------|
| 1 | NotAdmin | Caller is not the admin |
| 2 | AlreadyInitialized | Contract already initialized |
| 3 | PoolNotFound | Pool ID does not exist |
| 4 | PairAlreadyExists | Token pair already registered |
| 5 | InvalidFee | Fee exceeds 1000 bps (10%) |
| 6 | NotCreator | Caller is not pool creator or admin |
| 7 | PoolNotActive | Pool has been deactivated |
| 8 | WasmDeployFailed | WASM deployment failed |

### PoolError (cpmm-pool / stableswap)

| Code | Name | Description |
|------|------|-------------|
| 1 | NotAdmin | Caller is not the admin |
| 2 | InsufficientLiquidity | Not enough liquidity in pool |
| 3 | SlippageExceeded | Output below minimum |
| 4 | ZeroAmount | Amount must be > 0 |
| 5 | SameToken | Cannot pair same token |
| 6 | InvalidFee | Fee too high |
| 7 | InsufficientLpShares | Not enough LP shares |

### LendingError (lending-pool)

| Code | Name | Description |
|------|------|-------------|
| 1 | NotAdmin | Caller is not the admin |
| 2 | InsufficientLiquidity | Not enough liquidity |
| 3 | Undercollateralized | Health factor below threshold |
| 4 | InsufficientBalance | Not enough tokens |
| 5 | ZeroAmount | Amount must be > 0 |
| 6 | NotLiquidatable | Position is healthy |
| 7 | InvalidCollateral | Collateral asset not set |

### BackstopError (backstop)

| Code | Name | Description |
|------|------|-------------|
| 1 | NotAdmin | Caller is not the admin |
| 2 | BelowMinDeposit | Deposit below minimum |
| 3 | AboveMaxDeposit | Deposit above maximum |
| 4 | NoShares | No shares to claim |
| 5 | InsufficientBalance | Not enough balance |

### EscrowError (escrow)

| Code | Name | Description |
|------|------|-------------|
| 1 | NotAuthorized | Caller not authorized |
| 2 | InvalidState | Escrow not in correct state |
| 3 | MilestoneNotFound | Milestone index invalid |
| 4 | DeadlinePassed | Escrow deadline has passed |
| 5 | NotDisputed | Escrow is not disputed |
| 6 | AlreadyResolved | Escrow already resolved |

---

## REST API

### AMM

| Endpoint | Method | Description |
|----------|--------|-------------|
| `GET /api/v1/pools` | GET | List all pools |
| `GET /api/v1/pools/:id` | GET | Get pool details |
| `GET /api/v1/pools/:id/quote` | GET | Get swap quote |
| `POST /api/v1/pools/:id/swap` | POST | Execute swap |

### Lending

| Endpoint | Method | Description |
|----------|--------|-------------|
| `GET /api/v1/lending/health` | GET | Lending health check |
| `GET /api/v1/lending/position/:address` | GET | Get user position |
| `POST /api/v1/lending/deposit` | POST | Deposit collateral |
| `POST /api/v1/lending/borrow` | POST | Borrow assets |
| `POST /api/v1/lending/repay` | POST | Repay loan |

### Escrow

| Endpoint | Method | Auth | Description |
|----------|--------|------|-------------|
| `GET /api/escrow/health` | GET | No | Health check |
| `GET /api/escrow/:id` | GET | No | Get escrow details |
| `GET /api/escrow/user/:address` | GET | No | Get user escrows |
| `POST /api/escrow/create` | POST | Yes | Create escrow |
| `POST /api/escrow/:id/fund` | POST | Yes | Fund escrow |
| `POST /api/escrow/:id/submit-milestone` | POST | Yes | Submit milestone |
| `POST /api/escrow/:id/approve-milestone` | POST | Yes | Approve milestone |
| `POST /api/escrow/:id/release` | POST | Yes | Release funds |
| `POST /api/escrow/:id/refund` | POST | Yes | Refund remaining |
| `POST /api/escrow/:id/dispute` | POST | Yes | Dispute escrow |
| `POST /api/escrow/:id/resolve` | POST | Yes | Resolve dispute |

### Bridge

| Endpoint | Method | Description |
|----------|--------|-------------|
| `GET /api/v1/bridge/health` | GET | Bridge health |
| `GET /api/v1/bridge/config` | GET | Bridge config |
| `GET /api/v1/bridge/volume` | GET | Volume stats |

---

## Network Configuration

### Pi Testnet
- RPC: `https://rpc.testnet.minepi.com`
- Passphrase: `Pi Testnet`

### Stellar Testnet
- RPC: `https://soroban-testnet.stellar.org`
- Passphrase: `Test SDF Network ; September 2015`

---

## ZyraPay Integration Guide

ZyraPay is a conditional payment application built on Suban's Escrow Core. It enables trusted, milestone-based payments between parties who don't know each other.

### How ZyraPay Works

```
Funder (buyer) → Creates Escrow → Funds Escrow → Milestones Approved → Funds Released → Receiver (seller)
                    ↓                                    ↓
              Escrow Contract                    Arbitrator resolves
              holds PUSD                         disputes if needed
```

### Integration Options

#### Option 1: Use the Escrow API (Recommended)

```typescript
// 1. Create an escrow for a freelance gig
const response = await fetch('https://api.zyrachain.org/api/escrow/create', {
  method: 'POST',
  headers: {
    'Content-Type': 'application/json',
    'Authorization': `Bearer ${token}`,
  },
  body: JSON.stringify({
    receiverAddress: freelancerAddress,
    assetAddress: PUSD_CONTRACT_ID,
    milestones: [
      { description: 'Design mockups', approver: buyerAddress, amount: '5000000000' },
      { description: 'Frontend implementation', approver: buyerAddress, amount: '10000000000' },
      { description: 'Backend integration', approver: buyerAddress, amount: '10000000000' },
    ],
    feeBps: 100, // 1% platform fee
    deadlineLedger: currentLedger + 10000, // ~1 week
  }),
});

// 2. Fund the escrow
await fetch(`https://api.zyrachain.org/api/escrow/${escrowId}/fund`, {
  method: 'POST',
  headers: { 'Authorization': `Bearer ${token}` },
  body: JSON.stringify({ amount: '25000000000' }), // 2500 PUSD
});

// 3. Freelancer submits milestone work
await fetch(`https://api.zyrachain.org/api/escrow/${escrowId}/submit-milestone`, {
  method: 'POST',
  headers: { 'Authorization': `Bearer ${token}` },
  body: JSON.stringify({ milestoneIdx: 0 }),
});

// 4. Buyer approves milestone
await fetch(`https://api.zyrachain.org/api/escrow/${escrowId}/approve-milestone`, {
  method: 'POST',
  headers: { 'Authorization': `Bearer ${token}` },
  body: JSON.stringify({ milestoneIdx: 0 }),
});

// 5. Funds auto-release after all milestones approved
```

#### Option 2: Use the SDK Directly

```typescript
import { EscrowClient, SubanClient } from '@suban/sdk';

const client = new SubanClient({
  rpcUrl: 'https://rpc.testnet.minepi.com',
  networkPassphrase: 'Pi Testnet',
});

const escrow = new EscrowClient(client, ESCROW_CONTRACT_ID);

// Create escrow
await escrow.createEscrow(
  receiverAddress,
  arbitratorAddress,
  PUSD_ADDRESS,
  milestones,
  100, // 1% fee
  deadlineLedger,
  funderKeypair,
);

// Fund
await escrow.fundEscrow(escrowId, '25000000000', funderKeypair);

// Submit milestone
await escrow.submitMilestone(escrowId, 0, receiverKeypair);

// Approve
await escrow.approveMilestone(escrowId, 0, approverKeypair);

// Release
await escrow.releaseFunds(escrowId, funderKeypair);
```

### ZyraPay Features

| Feature | Description |
|---------|-------------|
| Milestone-based | Funds released in stages as work is completed |
| Role-gated | Only designated approver can approve each milestone |
| Dispute resolution | Arbitrator can resolve disputes (release or refund) |
| PUSD settlement | All escrows denominated in PUSD |
| Fee routing | Platform fees routed to treasury via fee-router |
| Pause mechanism | Admin can pause escrow operations in emergencies |
| Audit trail | All events logged via event-registry |

### Configuring ZyraPay for Your App

1. **Set up escrow contract** — deploy or use existing escrow contract
2. **Configure roles** — define funder, receiver, approver, arbitrator
3. **Set milestones** — define work stages and amounts
4. **Choose fee structure** — platform fee (default 1%)
5. **Set deadline** — maximum ledger for escrow completion

### Example: Marketplace Integration

```typescript
// Marketplace creates escrow for each transaction
const createMarketplaceEscrow = async (order) => {
  const milestones = order.items.map((item, idx) => ({
    description: `Item ${idx + 1}: ${item.name}`,
    approver: order.buyerAddress,
    amount: item.price.toString(),
  }));

  return escrow.createEscrow(
    order.sellerAddress,
    order.marketplaceArbitrator,
    PUSD_ADDRESS,
    milestones,
    order.marketplaceFeeBps,
    order.deadlineLedger,
    order.buyerKeypair,
  );
};
```

---

## Security

1. **Oracle** — Always check staleness and circuit breaker before using prices
2. **Health Factor** — Never allow health factor below 1.0 (10000 bps)
3. **Slippage** — Set min amounts for swaps and liquidations
4. **Authorization** — All state-changing operations require `require_auth()`
5. **Pause** — Admin can pause any contract in emergencies
6. **Multisig** — Bridge operations require 2-of-3 signer quorum

## Support

- GitHub: https://github.com/provenalabs/suban
- Issues: https://github.com/provenalabs/suban/issues
