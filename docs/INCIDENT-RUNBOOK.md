# Incident Response Runbook

## Overview

This runbook documents procedures for responding to security incidents, oracle failures, and protocol emergencies. All procedures are tied to the Emergency Pause Registry contract.

## Severity Levels

| Level | Description | Response Time | Example |
|-------|-------------|---------------|---------|
| **P0 — Critical** | Active exploit, funds at risk | Immediate (< 5 min) | Oracle manipulation, contract drained |
| **P1 — High** | Potential exploit, risk of fund loss | < 30 min | Abnormal price feed, failed liquidations |
| **P2 — Medium** | Degraded service, no immediate fund risk | < 2 hours | Oracle stale, bridge delayed |
| **P3 — Low** | Minor issue, service impacted | < 24 hours | UI bug, minor miscalculation |

## Emergency Contacts

| Role | Name | Contact |
|------|------|---------|
| Protocol Lead | — | — |
| Oracle Owner | — | — |
| Bridge Admin | — | — |
| Security Lead | — | — |

## Procedures

### P0: Active Exploit

**Immediate actions (first 5 minutes):**

1. **PAUSE ALL PRIMITIVES**
   ```bash
   # Pause AMM
   soroban contract invoke --id <pause-registry> -- pause --primitive amm
   # Pause Lending
   soroban contract invoke --id <pause-registry> -- pause --primitive lending
   # Pause Escrow
   soroban contract invoke --id <pause-registry> -- pause --primitive escrow
   # Pause Bridge
   soroban contract invoke --id <pause-registry> -- pause --primitive bridge
   ```

2. **VERIFY PAUSE**
   ```bash
   soroban contract invoke --id <pause-registry> -- is_paused --primitive amm
   soroban contract invoke --id <pause-registry> -- is_paused --primitive lending
   soroban contract invoke --id <pause-registry> -- is_paused --primitive escrow
   soroban contract invoke --id <pause-registry> -- is_paused --primitive bridge
   ```

3. **ASSESS DAMAGE**
   - Check contract balances
   - Review recent transactions
   - Identify affected addresses

4. **NOTIFY TEAM**
   - Alert all team members
   - Begin incident log

**Investigation (next 30 minutes):**

5. **TRACE ATTACK VECTOR**
   - Pull event logs from event-registry
   - Review transaction history
   - Identify root cause

6. **DETERMINE SCOPE**
   - Which contracts affected?
   - How much funds at risk?
   - Which users impacted?

7. **PLAN REMEDIATION**
   - Fix the vulnerability
   - Prepare patch deployment
   - Plan fund recovery (if possible)

**Recovery:**

8. **DEPLOY FIX**
   - Deploy patched contract
   - Verify fix on testnet

9. **UNPAUSE SELECTIVELY**
   - Unpause primitives one by one
   - Monitor for issues

10. **POST-INCIDENT**
    - Write incident report
    - Update runbook
    - Consider external audit

### P1: Oracle Failure

**Symptoms:** Oracle price stale (> 60s), circuit breaker triggered, abnormal price deviation

**Actions:**

1. Check oracle health:
   ```bash
   soroban contract invoke --id <oracle> -- get_config
   ```

2. Check circuit breaker:
   ```bash
   # Via suban-controller API
   curl https://api.zyrachain.org/api/v1/circuit-breaker
   ```

3. If circuit breaker tripped:
   - Do NOT manually reset until source is verified
   - Check price sources (MEXC, OKX, Bitget, CoinGecko)

4. If oracle completely down:
   - Pause lending (liquidations depend on oracle)
   - AMM can continue (uses pool reserves)
   - Escrow can continue (milestone-based, not price-based)

5. Reset circuit breaker only after confirming price source health

### P2: Bridge Delayed

**Symptoms:** Deposits not being minted, relayer offline

**Actions:**

1. Check bridge-multisig status:
   ```bash
   soroban contract invoke --id <bridge-multisig> -- config
   ```

2. Check if bridge is paused:
   ```bash
   soroban contract invoke --id <pause-registry> -- is_paused --primitive bridge
   ```

3. If relayer offline:
   - Check worker fleet logs
   - Restart relayer process
   - Monitor for catch-up

4. If quorum issue:
   - Contact signers
   - Verify attestation signatures

### P3: UI/UX Issue

**Actions:**

1. Document the issue
2. Check if it affects on-chain state (usually no)
3. Deploy frontend fix
4. No protocol pause needed

## Post-Incident Checklist

- [ ] Incident report written
- [ ] Root cause identified
- [ ] Fix deployed and verified
- [ ] Affected users notified
- [ ] Runbook updated
- [ ] Team retrospective scheduled
- [ ] External audit considered (for P0/P1)

## Monitoring Commands

```bash
# Check all primitive pause status
for p in amm lending escrow bridge oracle; do
  echo "$p: $(soroban contract invoke --id <pause-registry> -- is_paused --primitive $p)"
done

# Check oracle price freshness
soroban contract invoke --id <oracle> -- get_price --asset <wpi-address>

# Check lending health factors
soroban contract invoke --id <lending-pool> -- get_pool_state

# Check bridge volume
soroban contract invoke --id <bridge-multisig> -- volume_stats
```

## Communication Templates

### P0 Notification
```
[URGENT] Security incident detected on Suban Protocol.
All primitives have been paused as a precaution.
Investigation is underway. Updates will follow.
Do NOT interact with contracts until further notice.
```

### P1 Notification
```
[ALERT] Oracle issue detected. Lending operations paused.
AMM and Escrow remain operational.
Investigating root cause. Updates will follow.
```

### All Clear
```
[RESOLVED] Incident from [DATE] has been resolved.
Root cause: [DESCRIPTION]
Fix: [DESCRIPTION]
All primitives are now operational.
```
