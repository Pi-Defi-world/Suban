# Phase 2: PUSD ↔ Arc Bridge Architecture

## 1. Current State

### What Exists (Stellar/Soroban)
| Contract | Status | Purpose |
|----------|--------|---------|
| `bridge-multisig` | Deployed (testnet) | 2-of-3 multisig proposal lifecycle (MintWpi, ReleaseUsdc) |
| `wpi-token` | Deployed (testnet) | Wrapped Pi token (Soroban, 7 decimals) |
| `usdc-vault` | Deployed (testnet) | Holds Stellar USDC for bridge releases |
| `pause-registry` | Deployed (testnet) | Emergency pause for all primitives |

### What Doesn't Exist Yet
| Component | Status | Required For |
|-----------|--------|-------------|
| `pusd-token` (Soroban) | **Not implemented** | Settlement asset on Stellar side |
| PUSD ERC-20 (Arc/EVM) | **Not implemented** | Settlement asset on Arc side |
| Arc bridge contract | **Not implemented** | Burn/mint coordination on EVM |
| Stellar bridge extension | **Not implemented** | Burn/mint coordination on Soroban |
| Unified relayer | **Not implemented** | Cross-chain event watching |
| CCTP integration | **Not implemented** | USDC flows on Arc |

### Existing Pattern (Lock-and-Mint)
```
Pi Network → [deposit] → Bridge Relayer → [propose_mint] → bridge-multisig → [mint] → wpi-token
                                                                                         ↓
Stellar USDC ← [release] ← usdc-vault ← [propose_release] ← bridge-multisig ← [burn] ← wpi-token
```

---

## 2. Design Decisions

### Decision 1: PUSD on Arc — Separate ERC-20 (Path B)

**Choice:** PUSD on Arc is a standalone ERC-20 with privileged mint/burn controlled by the bridge signer set.

**Why not CCTP-compatible (Path A):**
- Circle's CCTP requires their messaging layer integration
- PUSD is not USDC — it's a Pi-ecosystem stablecoin
- CCTP compatibility would require Circle's cooperation (not feasible now)
- Path B works today, Path A requires Circle partnership

**Implementation:**
```solidity
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import "@openzeppelin/contracts/access/AccessControl.sol";

contract PUSDToken is ERC20, AccessControl {
    bytes32 public constant MINTER_ROLE = keccak256("MINTER_ROLE");
    bytes32 public constant BURNER_ROLE = keccak256("BURNER_ROLE");
    
    uint256 public constant MAX_SUPPLY = 1_000_000_000 * 1e6; // 1B PUSD, 6 decimals
    
    constructor() ERC20("Pi USD", "PUSD") {
        _grantRole(DEFAULT_ADMIN_ROLE, msg.sender);
    }
    
    function mint(address to, uint256 amount) external onlyRole(MINTER_ROLE) {
        require(totalSupply() + amount <= MAX_SUPPLY, "Exceeds max supply");
        _mint(to, amount);
    }
    
    function burn(address from, uint256 amount) external onlyRole(BURNER_ROLE) {
        _burn(from, amount);
    }
}
```

### Decision 2: Stellar/Pi Side — New Contract (Not Extend bridge-multisig)

**Choice:** Deploy a new `bridge-burn-mint` contract for PUSD cross-chain operations.

**Why not extend bridge-multisig:**
- bridge-multisig is designed for lock-and-mint (Pi → wPi)
- Burn-mint is a fundamentally different pattern
- Separation of concerns: Pi bridge vs PUSD bridge
- Different security model: Pi bridge needs custodian, PUSD bridge needs signer set

**New contract responsibilities:**
- Burn PUSD on Stellar side → emit cross-chain event
- Mint PUSD on Stellar side ← receive cross-chain message
- Manage per-chain mint caps
- Circuit breaker for large transfers

### Decision 3: Relayer — Unified Multi-Chain

**Choice:** Single relayer service that watches both chains.

**Why unified:**
- Shared quorum verification logic
- Shared nonce management
- Single deployment, single monitoring
- Reduces code duplication

**Architecture:**
```
┌─────────────────────────────────────────────────────┐
│                  Unified Relayer                     │
│                                                     │
│  ┌──────────────┐    ┌──────────────┐              │
│  │ Pi/Stellar   │    │ Arc/EVM      │              │
│  │ Chain Watcher│    │ Chain Watcher│              │
│  └──────┬───────┘    └──────┬───────┘              │
│         │                   │                       │
│         └─────────┬─────────┘                       │
│                   │                                 │
│         ┌─────────▼─────────┐                       │
│         │  Event Processor  │                       │
│         │  (verify + route) │                       │
│         └─────────┬─────────┘                       │
│                   │                                 │
│         ┌─────────▼─────────┐                       │
│         │  Transaction      │                       │
│         │  Builder + Submit │                       │
│         └───────────────────┘                       │
└─────────────────────────────────────────────────────┘
```

### Decision 4: CCTP Integration Path

**Choice:** Path B now (PUSD swaps for USDC on Arc AMMs), Path A later (if Circle partnership secured).

**Current play:**
1. PUSD on Arc → swap for USDC on Arc DEX (Uniswap V3 fork)
2. USDC flows anywhere via CCTP (Ethereum, Base, Arbitrum, etc.)
3. PUSD gets "global reach" through USDC as intermediate

**Future play (if Circle partnership):**
1. PUSD on Arc becomes CCTP-compatible
2. PUSD flows directly to any CCTP-supported chain
3. No swap step needed

### Decision 5: Supply Management

**Choice:** Per-chain caps with oracle sync.

**Mechanism:**
- Stellar side: PUSD supply cap (e.g., 100M)
- Arc side: PUSD supply cap (e.g., 100M)
- Total cross-chain supply tracked via oracle
- If one side hits cap, bridge pauses until rebalance
- Admin can adjust caps via governance

---

## 3. Contract Architecture

### Stellar/Pi Side (Soroban)

#### New: `pusd-token` Contract
```
purse/stablecoin
├── Standard SEP-41 token interface
├── Admin-only mint/burn
├── Supply cap enforcement
├── Pause mechanism
└── Events: Transfer, Mint, Burn
```

#### New: `bridge-burn-mint` Contract
```
purse/bridge-burn-mint
├── burn_pusd(amount, destination_chain) → emit Burned event
├── mint_pusd(recipient, amount, proof) → verify + mint
├── set_chain_config(chain_id, config)
├── set_mint_cap(chain_id, cap)
├── get_chain_state(chain_id) → total_minted, total_burned
├── Circuit breaker (volume threshold)
└── Admin functions (pause, set_signers, set_threshold)
```

#### Existing (Modified): `bridge-multisig`
- Add `BurnPusd` proposal kind
- Add `MintPusd` proposal kind
- Keep existing `MintWpi` and `ReleaseUsdc`

### Arc/EVM Side (Solidity)

#### New: `PUSDToken` (ERC-20)
```
contracts/PUSDToken.sol
├── ERC-20 + AccessControl
├── MINTER_ROLE → bridge signer set
├── BURNER_ROLE → bridge signer set
├── MAX_SUPPLY cap
├── Pause mechanism
└── Events: Mint, Burn
```

#### New: `ArcBridge` (Validator Set)
```
contracts/ArcBridge.sol
├── validatorSet: address[] (signers)
├── threshold: uint256 (M-of-N)
├── nonce: mapping(address => uint256)
├── processedHashes: mapping(bytes32 => bool)
├── mintPusd(recipient, amount, signatures)
├── burnPusd(amount, destination)
├── Circuit breaker (volume threshold)
└── Admin functions (pause, addValidator, setThreshold)
```

---

## 4. Cross-Chain Message Format

### Burn Event (Source Chain → Relayer)
```json
{
  "type": "burn",
  "source_chain": "stellar",
  "source_contract": "CBURN_MINT_...",
  "destination_chain": "arc",
  "recipient": "0x...",
  "amount": "1000000000",
  "nonce": 42,
  "tx_hash": "abc123...",
  "ledger": 28758800,
  "timestamp": 1727200000
}
```

### Mint Instruction (Relayer → Destination Chain)
```json
{
  "type": "mint",
  "source_chain": "stellar",
  "source_tx": "abc123...",
  "source_nonce": 42,
  "recipient": "0x...",
  "amount": "1000000000",
  "proof": ["sig1", "sig2", "sig3"]
}
```

---

## 5. Security Model

### Quorum
- **Stellar side:** 2-of-3 multisig (existing bridge-multisig signers)
- **Arc side:** 2-of-3 validator set (same keys, different chain)
- **Relayer:** Watch-only, no signing authority

### Replay Protection
- Nonce per source chain per user
- Processed hash tracking on destination chain
- One-time use proofs

### Circuit Breakers
- Volume cap per time window (e.g., 1M PUSD per hour)
- Auto-pause on anomaly
- Manual pause via admin

### Supply Management
- Per-chain mint caps
- Total supply oracle sync
- Emergency pause if caps exceeded

---

## 6. Relayer Architecture

### Event Flow
```
1. User burns PUSD on Stellar
2. Stellar chain emits Burned event
3. Relayer's Stellar watcher detects event
4. Relayer verifies:
   - Event is from valid bridge contract
   - Nonce not already processed
   - Amount within circuit breaker limits
   - Sufficient supply cap remaining
5. Relayer builds Arc transaction:
   - Calls ArcBridge.mintPusd() with signatures
6. Relayer submits to Arc chain
7. Arc bridge contract:
   - Verifies signatures (M-of-N)
   - Checks nonce not replayed
   - Mints PUSD to recipient
   - Marks hash as processed
8. Relayer confirms mint on both chains
```

### Worker Fleet Integration
```
bridge-relayer/
├── src/
│   ├── config.ts          # Chain configs, contract addresses
│   ├── stellar-watcher.ts # Horizon event polling
│   ├── arc-watcher.ts     # EVM event polling (ethers.js)
│   ├── event-processor.ts # Verify, route, deduplicate
│   ├── tx-builder.ts      # Build cross-chain transactions
│   ├── signer.ts          # Key management (HSM/KMS)
│   └── monitor.ts         # Health, metrics, alerts
├── docker/
│   └── Dockerfile
└── package.json
```

### Key Management
- Signer keys stored in HSM or cloud KMS
- Relayer has read-only access to public keys
- Signing happens in secure enclave
- Key rotation via governance proposal

---

## 7. Implementation Plan

### Phase 2a: Contracts (2-3 weeks)
| Task | Chain | Effort |
|------|-------|--------|
| Deploy `pusd-token` on Soroban | Stellar | 3 days |
| Deploy `PUSDToken` on Arc | EVM | 2 days |
| Deploy `ArcBridge` on Arc | EVM | 5 days |
| Extend `bridge-multisig` with BurnPusd/MintPusd | Stellar | 3 days |
| Deploy `bridge-burn-mint` on Stellar | Stellar | 5 days |
| Write contract tests | Both | 5 days |

### Phase 2b: Relayer (1-2 weeks)
| Task | Effort |
|------|--------|
| Build unified relayer service | 5 days |
| Implement Stellar event watcher | 2 days |
| Implement Arc event watcher | 2 days |
| Implement transaction builder | 3 days |
| Implement key management | 2 days |
| Write integration tests | 3 days |

### Phase 2c: Integration (1 week)
| Task | Effort |
|------|--------|
| Deploy relayer to production | 1 day |
| Configure monitoring + alerts | 1 day |
| End-to-end testing | 3 days |
| Documentation | 1 day |

---

## 8. African Corridor Settlement

### Flow
```
Business A (Kenya)                    Business B (Nigeria)
    │                                      │
    │ Send PUSD on Pi/Stellar              │
    ▼                                      │
┌─────────────┐                           │
│ Bridge      │                           │
│ Burn PUSD   │                           │
│ on Stellar  │                           │
└──────┬──────┘                           │
       │                                   │
       ▼                                   │
┌─────────────┐                           │
│ Relayer     │                           │
│ Mint PUSD   │                           │
│ on Arc      │                           │
└──────┬──────┘                           │
       │                                   │
       ▼                                   │
┌─────────────┐    ┌─────────────┐        │
│ Arc AMM     │───▶│ USDC        │───────▶│
│ PUSD → USDC │    │ (via CCTP)  │        │
└─────────────┘    └─────────────┘        │
                                          │
                         Business B receives USDC
                         (or swaps back to PUSD)
```

### Benefits
- **Sub-second finality** on Arc
- **USDC-denominated fees** (predictable costs)
- **CCTP integration** (global USDC reach)
- **No compliance overhead** (Circle handles it)

---

## 9. Open Questions

1. **PUSD collateral model:** Is PUSD 1:1 backed by Pi? Or algorithmic? This affects the mint/burn economics.

2. **Signer key distribution:** Should the same 3 signers control both chains? Or separate sets?

3. **Fee structure:** Who pays bridge fees? Sender? Recipient? Protocol?

4. **Testing strategy:** Should we deploy to Arc testnet first? Or go straight to mainnet?

5. **Audit timing:** When should we engage auditors? Before or after testnet deployment?

---

## 10. Next Steps

1. **Answer open questions** with stakeholders
2. **Deploy PUSD token** on Pi testnet (Soroban)
3. **Deploy PUSD token** on Arc testnet (Solidity)
4. **Build bridge contracts** on both chains
5. **Build unified relayer** service
6. **End-to-end test** on testnets
7. **Security audit** before mainnet
8. **Mainnet deployment** with gradual rollout
