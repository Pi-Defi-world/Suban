/**
 * Suban DeFi Protocol SDK
 *
 * Provides client libraries for interacting with Suban protocol contracts:
 * - AMM (pool-factory, cpmm-pool, stableswap, liquidity-mining)
 * - Lending (lending-pool, backstop)
 * - Oracle (price feeds)
 * - Escrow (milestone-based escrow)
 * - Bridge (wPi, bridge-multisig)
 * - Infrastructure (event-registry, node-staking, identity)
 */

export { SubanClient } from './client.js';
export type { SubanConfig, TxResult } from './client.js';

export {
  PoolFactoryClient,
  CpmmPoolClient,
  StableswapClient,
  LiquidityMiningClient,
} from './amm.js';
export type { PoolInfo, CpmmState, SwapResult } from './amm.js';

export { LendingClient } from './lending.js';
export type { LendingPosition, LendingPoolState } from './lending.js';

export { EscrowClient } from './escrow.js';
export type { EscrowConfig, EscrowState, Milestone } from './escrow.js';

export { OracleClient } from './oracle.js';
export type { Price, OracleConfig } from './oracle.js';

export { BridgeClient } from './bridge.js';
export type { BridgeConfig, Attestation } from './bridge.js';
