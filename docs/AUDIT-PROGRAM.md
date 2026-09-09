# Audit Program — Per-Primitive Security Plan

## Scope

Every contract that touches user funds must be audited before mainnet. The audit program covers:

1. **Per-primitive audits** — each primitive independently
2. **Integration audit** — cross-primitive interactions
3. **Operational audit** — key management, pause mechanisms, upgrade paths

## Audit Schedule

| Primitive | Contracts | Priority | Status |
|-----------|-----------|----------|--------|
| AMM Core | cpmm-pool, stableswap, swap-router, pool-factory, lp-token, liquidity-mining | HIGH | Pending |
| Lending Core | lending-pool, backstop | CRITICAL | Pending |
| Escrow Core | escrow | HIGH | Pending |
| Bridge | wpi-token, usdc-vault, bridge-multisig | CRITICAL | Pending |
| Infrastructure | oracle, pause-registry, fee-router, event-registry, node-staking, identity | MEDIUM | Pending |

## Audit Checklist

### Smart Contract Audit

- [ ] Formal verification of critical math (invariant checks, fee calculations)
- [ ] Reentrancy analysis
- [ ] Integer overflow/underflow analysis
- [ ] Access control review (admin, operator, user permissions)
- [ ] Front-running resistance
- [ ] MEV resistance
- [ ] Upgrade path analysis (if applicable)
- [ ] Gas optimization review

### Integration Audit

- [ ] Cross-contract call safety
- [ ] Token transfer atomicity
- [ ] Event emission consistency
- [ ] Pause mechanism propagation
- [ ] Fee routing correctness

### Operational Audit

- [ ] Key management procedures
- [ ] Emergency pause procedures
- [ ] Incident response runbook
- [ ] Monitoring and alerting coverage
- [ ] Backup and recovery procedures

## Recommended Auditors

| Auditor | Specialization | Estimate |
|---------|---------------|----------|
| OtterSec | Soroban/Rust contracts | $50-100K per primitive |
| Trail of Bits | DeFi protocol security | $100-200K per primitive |
| Halborn | Multi-chain DeFi | $30-80K per primitive |
| Neodyme | Solana/Soroban | $40-90K per primitive |

## Pre-Audit Preparation

Before engaging auditors:

1. **Complete test coverage** — aim for 100% line coverage
2. **Documentation** — contract specs, threat model, known limitations
3. **Formal specs** — mathematical invariants for AMM and lending math
4. **Deployment scripts** — reproducible testnet deployments
5. **Bug bounty program** — prepare for post-audit bounty

## Post-Audit

1. **Fix all critical/high findings** before mainnet
2. **Document accepted risks** for medium/low findings
3. **Re-audit** if critical changes are made after initial audit
4. **Continuous monitoring** via oracle circuit breakers and pause registry
