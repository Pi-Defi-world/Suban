# Phase 2: PUSD ↔ Arc Bridge Architecture

## 1. Current State

### What Exists (Stellar/Soroban)
| Contract | Status | Purpose |
|----------|--------|---------|
| `pusd-token` | Deployed (testnet) | PUSD stablecoin (1:1 backed by Pi) |
| `bridge-multisig` | Deployed (testnet) | 2-of-3 multisig proposal lifecycle (MintWpi, ReleaseUsdc) |
| `wpi-token` | Deployed (testnet) | Wrapped Pi token (Soroban, 7 decimals) |
| `usdc-vault` | Deployed (testnet) | Holds Stellar USDC for bridge releases |
| `pause-registry` | Deployed (testnet) | Emergency pause for all primitives |

### What Doesn't Exist Yet
| Component | Status | Required For |
|-----------|--------|-------------|
| PUSD ERC-20 (Arc/EVM) | **Not implemented** | PUSD representation on Arc |
| Arc bridge contract | **Not implemented** | Burn/mint coordination on EVM |
| Stellar bridge extension | **Not implemented** | Burn/mint coordination on Soroban |
| Unified relayer | **Not implemented** | Cross-chain event watching |

### Existing Pattern (Lock-and-Mint for Pi → wPi)
```
Pi Network → [deposit] → Bridge Relayer → [propose_mint] → bridge-multisig → [mint] → wpi-token
                                                                                         ↓
Stellar USDC ← [release] ← usdc-vault ← [propose_release] ← bridge-multisig ← [burn] ← wpi-token
```

### New Pattern (Burn-Mint for PUSD)
```
Stellar PUSD → [burn] → bridge-burn-mint → [emit event] → Relayer → [mint] → ArcBridge → Arc PUSD
       ↑                                                                              │
       └──────────────────────── [burn on Arc] ← [relay] ← [emit event] ←────────────┘
```

---

## 2. Design Decisions (Finalized)

### Decision 1: PUSD Collateral Model
**1:1 backed by Pi.** Each PUSD in circulation is backed by an equivalent amount of Pi held in reserve.

### Decision 2: Fee Structure
**Protocol pays bridge fees, charges users via transaction fees.**
- Relayer pays gas on both chains
- Users pay a small fee (e.g., 0.1-0.5%) deducted from their bridge transaction
- Revenue split: 90% relayer operator, 10% protocol treasury

### Decision 3: Signer Sets
**Separate signers per chain.**
- Stellar bridge: 2-of-3 Stellar keypairs
- Arc bridge: 2-of-3 EVM keypairs
- Independent rotation, independent compromise blast radius

### Decision 4: Deployment Target
**Arc testnet first, then mainnet after audit.**

### Decision 5: PUSD Token
**PUSD already exists on Stellar.** We deploy the Arc representation (ERC-20) and bridge infrastructure.

---

## 3. Contract Architecture

### Stellar/Pi Side (Soroban)

#### Existing: `pusd-token`
```
Already deployed on testnet.
SEP-41 token, admin-only mint/burn, 1:1 Pi-backed.
```

#### New: `bridge-burn-mint` Contract
```
purse/bridge-burn-mint
├── burn_pusd(amount, destination_chain) → emit Burned event
├── mint_pusd(recipient, amount, proof) → verify signatures + mint
├── set_chain_config(chain_id, endpoint, contract)
├── set_mint_cap(chain_id, cap)
├── get_chain_state(chain_id) → total_minted, total_burned, supply
├── Circuit breaker (volume threshold per window)
└── Admin: pause, set_signers, set_threshold, set_fee
```

### Arc/EVM Side (Solidity)

#### New: `PUSDToken` (ERC-20)
```
contracts/PUSDToken.sol
├── ERC-20 + AccessControl
├── MINTER_ROLE → ArcBridge contract only
├── BURNER_ROLE → ArcBridge contract only
├── MAX_SUPPLY = total PUSD supply cap on Arc
├── Pause mechanism
└── Events: Mint, Burn
```

#### New: `ArcBridge` (Validator Set)
```
contracts/ArcBridge.sol
├── validators: address[] (separate from Stellar signers)
├── threshold: uint256 (M-of-N, default 2-of-3)
├── nonces: mapping(address => uint256)
├── processedHashes: mapping(bytes32 => bool)
├── mintPusd(recipient, amount, sourceChain, sourceTxHash, signatures[])
│   ├── Verify M-of-N signatures
│   ├── Check nonce not replayed
│   ├── Check hash not processed
│   ├── Check amount within circuit breaker
│   ├── Mint PUSD to recipient
│   └── Mark hash as processed
├── burnPusd(amount, destinationChain)
│   ├── Burn caller's PUSD
│   └── Emit Burned event
├── Circuit breaker (volume cap per hour)
└── Admin: pause, addValidator, setThreshold, setMintCap
```

---

## 4. Cross-Chain Message Format

### Burn Event (Source Chain → Relayer)
```json
{
  "type": "pusd_burn",
  "source_chain": "stellar",
  "source_contract": "CBURN_MINT_...",
  "destination_chain": "arc",
  "recipient": "0x...",
  "amount": "1000000000",
  "fee": "500000",
  "nonce": 42,
  "tx_hash": "abc123...",
  "ledger": 28758800,
  "timestamp": 1727200000
}
```

### Mint Instruction (Relayer → Destination Chain)
```json
{
  "type": "pusd_mint",
  "source_chain": "stellar",
  "source_tx": "abc123...",
  "source_nonce": 42,
  "recipient": "0x...",
  "amount": "1000000000",
  "signatures": ["sig1", "sig2"]
}
```

---

## 5. Security Model

### Quorum
- **Stellar side:** 2-of-3 Stellar keypairs (separate from existing bridge-multisig)
- **Arc side:** 2-of-3 EVM keypairs (completely independent)
- **Relayer:** Watch-only, submits signed transactions but doesn't hold keys

### Replay Protection
- Nonce per user per direction
- Processed hash tracking on destination chain
- One-time use proofs

### Circuit Breakers
- Volume cap per time window (e.g., 100K PUSD per hour)
- Auto-pause on anomaly
- Manual pause via admin

### Supply Management
- Arc-side mint cap = total PUSD supply - Stellar-side supply
- Relayer checks cap before submitting mint
- Emergency pause if cap exceeded

---

## 6. Relayer Architecture

### Event Flow
```
1. User calls burn_pusd on Stellar bridge-burn-mint
2. Contract burns PUSD, emits Burned event with nonce
3. Relayer's Stellar watcher detects event
4. Relayer verifies:
   - Event is from valid bridge contract
   - Nonce not already processed
   - Amount within circuit breaker limits
   - Sufficient Arc mint cap remaining
5. Relayer signs transaction with Arc validator keys (2-of-3)
6. Relayer submits to Arc: ArcBridge.mintPusd(recipient, amount, sigs)
7. Arc bridge contract:
   - Verifies 2-of-3 signatures
   - Checks nonce not replayed
   - Checks hash not already processed
   - Mints PUSD to recipient via PUSDToken.mint()
   - Marks hash as processed
8. Relayer confirms mint, logs success
```

### Service Structure
```
bridge-relayer/
├── src/
│   ├── config.ts              # Chain configs, contract addresses, keys
│   ├── chains/
│   │   ├── stellar-watcher.ts # Horizon event polling
│   │   └── arc-watcher.ts     # EVM event polling (ethers.js)
│   ├── core/
│   │   ├── event-processor.ts # Verify, route, deduplicate
│   │   ├── tx-builder.ts      # Build cross-chain transactions
│   │   └── signer.ts          # Key management (HSM/KMS)
│   ├── fees/
│   │   └── fee-collector.ts   # Deduct user fees, track revenue
│   └── monitor.ts             # Health, metrics, alerts
├── docker/
│   └── Dockerfile
└── package.json
```

### Fee Collection
```
User burns 1000 PUSD
  ├── 0.5% fee = 5 PUSD
  ├── 90% to relayer operator = 4.5 PUSD
  ├── 10% to protocol treasury = 0.5 PUSD
  └── 995 PUSD arrives on Arc
```

---

## 7. Implementation Plan

### Phase 2a: Arc Testnet Contracts (2 weeks)
| # | Task | Chain | Days |
|---|------|-------|------|
| 1 | Write PUSDToken.sol (ERC-20) | Arc | 1 |
| 2 | Write ArcBridge.sol (validator set) | Arc | 3 |
| 3 | Deploy to Arc testnet | Arc | 1 |
| 4 | Write bridge-burn-mint (Soroban) | Stellar | 3 |
| 5 | Deploy to Pi testnet | Stellar | 1 |
| 6 | Unit tests for all contracts | Both | 3 |

### Phase 2b: Relayer (1.5 weeks)
| # | Task | Days |
|---|------|------|
| 1 | Build relayer service skeleton | 1 |
| 2 | Implement Stellar event watcher | 2 |
| 3 | Implement Arc event watcher | 2 |
| 4 | Implement signature generation + verification | 2 |
| 5 | Implement fee collection | 1 |
| 6 | Integration tests | 2 |

### Phase 2c: Testnet Integration (1 week)
| # | Task | Days |
|---|------|------|
| 1 | Deploy relayer to testnet | 1 |
| 2 | End-to-end: Stellar → Arc | 2 |
| 3 | End-to-end: Arc → Stellar | 2 |
| 4 | Circuit breaker testing | 1 |
| 5 | Documentation | 1 |

### Phase 2d: Audit + Mainnet (2-3 weeks)
| # | Task | Days |
|---|------|------|
| 1 | Fix testnet issues | 3 |
| 2 | Security audit engagement | 1 |
| 3 | Audit remediation | 5-10 |
| 4 | Mainnet deployment | 2 |
| 5 | Mainnet relaunch | 1 |

**Total: 6-8 weeks**

---

## 8. African Corridor Settlement

### Flow
```
Business A (Kenya)                    Business B (Nigeria)
    │                                      │
    │ Send 1000 PUSD on Pi                │
    │ (fee: 5 PUSD)                        │
    ▼                                      │
┌─────────────┐                           │
│ Stellar     │                           │
│ bridge-burn │                           │
│ -mint       │                           │
└──────┬──────┘                           │
       │ burn 1000 PUSD                    │
       ▼                                   │
┌─────────────┐                           │
│ Relayer     │                           │
│ (fee: 4.5)  │                           │
│ (proto: 0.5)│                           │
└──────┬──────┘                           │
       │ mint 995 PUSD on Arc             │
       ▼                                   │
┌─────────────┐    ┌─────────────┐        │
│ Arc AMM     │───▶│ USDC        │───────▶│
│ PUSD → USDC │    │ (via CCTP)  │        │
└─────────────┘    └─────────────┘        │
                                          │
                         Business B receives ~995 USDC
                         (or holds as PUSD on Arc)
```

### Benefits
- **Sub-second finality** on Arc for settlement
- **USDC-denominated fees** (predictable costs)
- **CCTP integration** for global USDC reach
- **1:1 Pi backing** maintains trust
- **Separate signers** limits compromise blast radius

---

## 9. Next Steps

1. Write `PUSDToken.sol` (ERC-20 with MINTER/BURNER roles)
2. Write `ArcBridge.sol` (validator set, mint/burn, circuit breaker)
3. Deploy both to Arc testnet
4. Write `bridge-burn-mint` Soroban contract
5. Deploy to Pi testnet
6. Build unified relayer service
7. End-to-end testnet testing
8. Security audit
9. Mainnet deployment
