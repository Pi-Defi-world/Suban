# Shared Worker Fleet — Design Document

## Overview

A single worker infrastructure running three distinct jobs, reusing the same node operator base:
1. **Liquidation Bot** — monitors lending pool health factors, executes liquidations
2. **Milestone Monitor** — watches escrow milestones, triggers approvals
3. **Bridge Relayer** — observes bridge deposits, proposes/approves mints

## Architecture

```
┌─────────────────────────────────────────────────────┐
│              Shared Worker Fleet                     │
│                                                     │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐   │
│  │ Liquidation  │ │  Milestone  │ │   Bridge    │   │
│  │    Bot       │ │  Monitor    │ │  Relayer    │   │
│  └──────┬──────┘ └──────┬──────┘ └──────┬──────┘   │
│         │               │               │           │
│  ┌──────┴───────────────┴───────────────┴──────┐   │
│  │          Shared Event Bus (SSE)              │   │
│  └──────┬───────────────┬───────────────┬──────┘   │
│         │               │               │           │
│  ┌──────┴──────┐ ┌──────┴──────┐ ┌──────┴──────┐   │
│  │  Soroban    │ │   Pi RPC    │ │  Stellar    │   │
│  │    RPC      │ │   Node      │ │   RPC       │   │
│  └─────────────┘ └─────────────┘ └─────────────┘   │
└─────────────────────────────────────────────────────┘
```

## Jobs

### 1. Liquidation Bot

**Trigger:** Health factor < 1.0 (10000 bps)
**Action:** Call `liquidate()` on lending-pool contract
**Revenue:** Keeper receives liquidation bonus (default 5%)

```
Loop:
  1. Query all lending pools for positions
  2. Calculate health factor for each position
  3. If health_factor < 10000 (undercollateralized):
     a. Build liquidate transaction
     b. Submit to Soroban RPC
     c. Collect liquidation bonus
```

### 2. Milestone Monitor

**Trigger:** Milestone submitted but not approved
**Action:** Notify approver, or auto-approve if criteria met
**Revenue:** Small fee from escrow

```
Loop:
  1. Query escrow contract for submitted milestones
  2. Check if approval criteria are met
  3. If auto-approve enabled:
     a. Build approve_milestone transaction
     b. Submit to Soroban RPC
  4. Else: send notification to approver
```

### 3. Bridge Relayer

**Trigger:** New deposit observed on chain
**Action:** Propose mint via bridge-multisig
**Revenue:** Bridge fee

```
Loop:
  1. Watch Pi Horizon for deposits to bridge address
  2. Verify deposit amount and destination
  3. Build propose_mint transaction
  4. Submit to bridge-multisig contract
  5. Wait for quorum (2-of-3 signers)
```

## Configuration

```yaml
worker:
  # Shared
  rpc_url: https://rpc.testnet.minepi.com
  network_passphrase: "Pi Testnet"
  poll_interval_ms: 10000
  
  # Liquidation
  liquidation:
    enabled: true
    health_threshold: 9500  # liquidate at 95% collateralization
    min_profit_stroops: 1000000
    
  # Milestone
  milestone:
    enabled: true
    auto_approve: false
    notification_webhook: https://hooks.example.com/milestone
    
  # Bridge
  bridge:
    enabled: true
    bridge_contract_id: CBNGDXVUGRHQPTHYUOTQJVZKYSPZI2DCEWA6JUJCTV7IYF7Z2STLPXVA
    min_deposit_stroops: 1000000
```

## Node Operator Economics

Operators stake tokens via the `node-staking` contract:
- **Min stake:** 100 tokens
- **Reward rate:** 0.5% per period (configurable)
- **Slashing:** Misbehavior (double-signing, downtime) → stake slashed

Revenue split per job:
| Job | Keeper Share | Protocol Share |
|-----|-------------|----------------|
| Liquidation | 80% | 20% |
| Milestone | 70% | 30% |
| Bridge | 90% | 10% |

## Deployment

The worker fleet runs as a single Node.js process with three concurrent job runners:

```bash
# Start all jobs
pnpm worker start --all

# Start specific job
pnpm worker start --job liquidation
pnpm worker start --job milestone
pnpm worker start --job bridge
```

## Monitoring

- Each job emits metrics to Prometheus
- Grafana dashboard shows:
  - Liquidation profit/loss
  - Milestone approval latency
  - Bridge relay success rate
  - Worker uptime and error rates
