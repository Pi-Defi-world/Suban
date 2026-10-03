# PUSD Bridge — Production Roadmap

**Date:** September 28, 2026  
**Status:** Testnet (E2E Bridge Verified)  
**Target:** Mainnet Launch Q1 2027

---

## Executive Summary

The PUSD bridge has achieved end-to-end functionality on testnet. This document outlines the production requirements, including multi-sig signing, address mapping, and Arc threshold restoration.

---

## 1. Multi-Sig Production Signing

### Current State (Testing)
- **Arc threshold:** 1 (relayer only)
- **Stellar threshold:** 2 (but only 1 signer used for minting)

### Production Requirement
- **Threshold:** 2-of-3 (both chains)
- **Signers:** 3 validators per chain (independent rotation)

### Implementation Steps

#### Arc Side (EVM)

1. **Raise threshold to 2**
   ```solidity
   // Call on ArcBridge contract
   bridge.setThreshold(2);
   ```

2. **Implement threshold signing in relayer**
   - Use `@ethereumjs/tx` for EIP-191 message signing
   - Collect 2+ signatures before submitting
   - Verify M-of-N in `tx-builder.ts`

3. **Add validator key management**
   - Store validator keys in HSM or encrypted keystore
   - Never expose private keys in environment variables
   - Use key rotation schedule (monthly)

#### Stellar Side (Soroban)

1. **Update bridge contract**
   - Ensure `mint_pusd` verifies 2-of-3 signatures
   - Add signature aggregation logic

2. **Implement threshold signing**
   - Use `@stellar/stellar-sdk` for signing
   - Collect 2+ signatures before submitting
   - Verify M-of-N in `bridge-relayer`

3. **Add signer rotation**
   - Admin function to rotate signers
   - Time-locked rotation (24h delay)
   - Emergency rotation capability

### Timeline
- **Week 1-2:** Implement threshold signing on Arc
- **Week 3-4:** Implement threshold signing on Stellar
- **Week 5:** Integration testing
- **Week 6:** Security review

---

## 2. Stellar → EVM Address Mapping

### Current State (Placeholder)
The relayer uses its own EVM address as a placeholder for the recipient:
```typescript
// event-processor.ts
const evmRecipient = config.relayerEvmAddress; // PLACEHOLDER
```

### Production Requirement
A proper mapping between Stellar accounts and EVM addresses.

### Implementation Options

#### Option A: On-Chain Registry (Recommended)

**Contract:** `StellarEvmRegistry.sol` on Arc

```solidity
contract StellarEvmRegistry {
    mapping(bytes32 => address) public stellarToEvm;
    mapping(address => bytes32) public evmToStellar;
    
    function register(bytes32 stellarPubKey, address evmAddress) external {
        require(keccak256(abi.encodePacked(evmAddress)) == stellarPubKey, "Invalid mapping");
        stellarToEvm[stellarPubKey] = evmAddress;
        evmToStellar[evmAddress] = stellarPubKey;
    }
    
    function getEvmAddress(bytes32 stellarPubKey) external view returns (address) {
        return stellarToEvm[stellarPubKey];
    }
}
```

**Flow:**
1. User calls `register()` with their Stellar pubkey and EVM address
2. Registry stores the mapping on-chain
3. Relayer reads mapping when processing bridge events

**Pros:** Decentralized, on-chain, trustless  
**Cons:** Requires user action, gas costs

#### Option B: ENS-like Resolution

**Contract:** `StellarENS.sol` on Stellar

```rust
pub fn register(env: Env, name: Symbol, stellar_addr: Address, evm_addr: Bytes) {
    // Store mapping on Stellar
    env.storage().persistent().set(
        &DataKey::Name(name),
        &Mapping { stellar_addr, evm_addr },
    );
}
```

**Flow:**
1. User registers a name (e.g., "alice.piusd") on Stellar
2. Mapping stored on Stellar blockchain
3. Relayer queries Stellar for EVM address

**Pros:** User-friendly names, on-chain  
**Cons:** Requires Stellar contract, more complex

#### Option C: Off-Chain Database (Simplest)

**Database:** PostgreSQL or SQLite

```sql
CREATE TABLE address_mapping (
    stellar_pubkey BYTEA PRIMARY KEY,
    evm_address ADDRESS NOT NULL,
    verified BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP DEFAULT NOW()
);
```

**Flow:**
1. User submits mapping via API
2. Backend verifies ownership (sign message with both keys)
3. Mapping stored in database
4. Relayer queries database

**Pros:** Simple, fast, easy to update  
**Cons:** Centralized, requires trust in backend

### Recommendation
**Option A (On-Chain Registry)** for production. It's decentralized, trustless, and aligns with Web3 principles.

### Timeline
- **Week 1:** Design registry contract
- **Week 2:** Implement on Arc testnet
- **Week 3:** Build relayer integration
- **Week 4:** User registration frontend
- **Week 5:** Testing and security review

---

## 3. Arc Threshold Restoration

### Current State (Testing)
- **Threshold:** 1 (relayer only)
- **Validators:** 4 (3 original + relayer)

### Production Requirement
- **Threshold:** 2-of-3
- **Validators:** 3 (remove relayer from validator set)

### Implementation Steps

1. **Remove relayer from validator set**
   ```solidity
   bridge.removeValidator(0xc0D202faFEe21541E6Ee18D3be434004dcAF919C);
   ```

2. **Raise threshold to 2**
   ```solidity
   bridge.setThreshold(2);
   ```

3. **Verify validator set**
   ```solidity
   // Should return 3 validators
   bridge.getValidators();
   ```

4. **Test multi-sig signing**
   - Submit mint with 1 signature → should fail
   - Submit mint with 2 signatures → should succeed

### Timeline
- **Day 1:** Remove relayer validator, raise threshold
- **Day 2-3:** Test multi-sig signing
- **Day 4-5:** Integration testing with relayer

---

## 4. Additional Production Requirements

### 4.1 Reserve Attestation

**Requirement:** Prove USDC reserves match PUSD supply

**Implementation:**
- On-chain oracle reports USDC balance in vault
- Compare with PUSD total supply
- Alert if ratio deviates from 1:1

### 4.2 Bridge Insurance Fund

**Requirement:** Cover losses from exploits or bugs

**Implementation:**
- Allocate 5% of bridge fees to insurance vault
- Use multi-sig for fund access
- Establish claims process

### 4.3 Rate Limiting

**Requirement:** Prevent single user from draining bridge

**Implementation:**
- Per-user daily limit (e.g., 10,000 PUSD)
- Per-transaction limit (e.g., 1,000 PUSD)
- Global rate limit (e.g., 100,000 PUSD/hour)

### 4.4 Monitoring & Alerting

**Requirement:** Detect issues in real-time

**Implementation:**
- Prometheus metrics for bridge volume, latency, errors
- Grafana dashboards for visualization
- PagerDuty alerts for critical issues

### 4.5 Security Audit

**Requirement:** Independent verification of smart contracts

**Implementation:**
- Engage auditor (OtterSec, Trail of Bits, or Halborn)
- Fix critical/high findings
- Publish audit report

### Timeline
| Task | Priority | Effort | Target |
|------|----------|--------|--------|
| Multi-sig production signing | HIGH | 6 weeks | Week 6 |
| Address mapping | HIGH | 5 weeks | Week 5 |
| Arc threshold restoration | HIGH | 1 week | Week 1 |
| Reserve attestation | MEDIUM | 3 weeks | Week 8 |
| Bridge insurance | MEDIUM | 2 weeks | Week 9 |
| Rate limiting | MEDIUM | 1 week | Week 10 |
| Monitoring/alerting | MEDIUM | 2 weeks | Week 12 |
| Security audit | HIGH | 4 weeks | Week 16 |

---

## 5. Production Checklist

### Pre-Production
- [ ] Multi-sig signing implemented (2-of-3)
- [ ] Address mapping implemented (on-chain registry)
- [ ] Arc threshold restored to 2
- [ ] Bridge insurance fund created
- [ ] Rate limiting implemented
- [ ] Monitoring/alerting configured
- [ ] Security audit completed
- [ ] Documentation updated

### Mainnet Deployment
- [ ] Contracts deployed to Pi mainnet
- [ ] Contracts deployed to Arc mainnet
- [ ] Bridge relayer running in production
- [ ] Monitoring dashboards live
- [ ] Alerting configured
- [ ] Incident runbook tested
- [ ] Support team trained

### Post-Launch
- [ ] Monitor for 48 hours
- [ ] Verify reserve attestation
- [ ] Collect user feedback
- [ ] Address critical issues
- [ ] Publish transparency report

---

## 6. Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Relayer key compromise | LOW | HIGH | Multi-sig, HSM, key rotation |
| Smart contract exploit | MEDIUM | CRITICAL | Security audit, bug bounty |
| Reserve depletion | LOW | HIGH | Circuit breakers, monitoring |
| Bridge drain attack | LOW | HIGH | Rate limiting, per-user caps |
| Oracle manipulation | LOW | MEDIUM | Multi-source, staleness checks |

---

## 7. Timeline Summary

```
Week 1-2:  Arc threshold restoration + testing
Week 3-6:  Multi-sig production signing (Arc + Stellar)
Week 7-11: Address mapping + reserve attestation
Week 12-16: Security audit + mainnet prep
Week 17+:  Mainnet launch + monitoring
```

**Target Mainnet Launch:** Q1 2027

---

*Document generated by Suban Data Platform*  
*Last updated: September 28, 2026*
