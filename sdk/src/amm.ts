import { Keypair } from '@stellar/stellar-sdk';
import type { SubanClient, TxResult } from './client.js';

// ─── Types ────────────────────────────────────────────────────────────

export interface PoolInfo {
  poolId: number;
  poolAddress: string;
  tokenA: string;
  tokenB: string;
  poolType: number; // 0 = ConstantProduct, 1 = Stableswap
  feeBps: number;
  creator: string;
  name: string;
  active: boolean;
}

export interface CpmmState {
  reserveA: string;
  reserveB: string;
  totalShares: string;
  feeBps: number;
}

export interface SwapResult {
  amountIn: string;
  amountOut: string;
  feeBps: number;
}

// ─── Pool Factory Client ──────────────────────────────────────────────

export class PoolFactoryClient {
  constructor(
    private readonly client: SubanClient,
    private readonly contractId: string,
  ) {}

  /** Deploy a new pool. Returns (poolId, poolAddress). */
  async createPool(
    wasmHash: string,
    salt: string,
    tokenA: string,
    tokenB: string,
    poolType: number,
    feeBps: number,
    name: string,
    callerKeypair: Keypair,
  ): Promise<TxResult> {
    return this.client.invoke(
      this.contractId,
      'create_pool',
      [callerKeypair.publicKey(), wasmHash, salt, tokenA, tokenB, poolType, feeBps, name],
      callerKeypair,
    );
  }

  /** Register a pre-deployed pool. Admin only. */
  async registerPool(
    poolAddress: string,
    tokenA: string,
    tokenB: string,
    poolType: number,
    feeBps: number,
    adminKeypair: Keypair,
  ): Promise<TxResult> {
    return this.client.invoke(
      this.contractId,
      'register_pool',
      [adminKeypair.publicKey(), poolAddress, tokenA, tokenB, poolType, feeBps],
      adminKeypair,
    );
  }

  /** Deactivate a pool. */
  async removePool(poolId: number, callerKeypair: Keypair): Promise<TxResult> {
    return this.client.invoke(
      this.contractId,
      'remove_pool',
      [callerKeypair.publicKey(), poolId],
      callerKeypair,
    );
  }

  /** Reactivate a pool. Admin only. */
  async reactivatePool(poolId: number, adminKeypair: Keypair): Promise<TxResult> {
    return this.client.invoke(
      this.contractId,
      'reactivate_pool',
      [adminKeypair.publicKey(), poolId],
      adminKeypair,
    );
  }

  /** Get pool info by ID. */
  async getPool(poolId: number): Promise<PoolInfo> {
    const result = await this.client.simulate(this.contractId, 'get_pool', [poolId]);
    return result as PoolInfo;
  }

  /** Find pool by token pair. */
  async findPool(tokenA: string, tokenB: string): Promise<PoolInfo> {
    const result = await this.client.simulate(this.contractId, 'find_pool', [tokenA, tokenB]);
    return result as PoolInfo;
  }

  /** List all active pools. */
  async listPools(): Promise<PoolInfo[]> {
    const result = await this.client.simulate(this.contractId, 'list_pools', []);
    return result as PoolInfo[];
  }

  /** List all pools including inactive. */
  async listAllPools(): Promise<PoolInfo[]> {
    const result = await this.client.simulate(this.contractId, 'list_all_pools', []);
    return result as PoolInfo[];
  }

  /** Get total pool count. */
  async poolCount(): Promise<number> {
    const result = await this.client.simulate(this.contractId, 'pool_count', []);
    return result as number;
  }
}

// ─── CPMM Pool Client ─────────────────────────────────────────────────

export class CpmmPoolClient {
  constructor(
    private readonly client: SubanClient,
    private readonly contractId: string,
  ) {}

  /** Initialize the pool. */
  async initialize(
    tokenA: string,
    tokenB: string,
    feeBps: number,
    adminKeypair: Keypair,
  ): Promise<TxResult> {
    return this.client.invoke(
      this.contractId,
      'initialize',
      [adminKeypair.publicKey(), tokenA, tokenB, feeBps],
      adminKeypair,
    );
  }

  /** Add liquidity. Returns LP shares minted. */
  async addLiquidity(
    amountA: string,
    amountB: string,
    providerKeypair: Keypair,
  ): Promise<TxResult> {
    return this.client.invoke(
      this.contractId,
      'add_liquidity',
      [providerKeypair.publicKey(), amountA, amountB],
      providerKeypair,
    );
  }

  /** Remove liquidity. Burns LP shares, returns tokens. */
  async removeLiquidity(
    shares: string,
    providerKeypair: Keypair,
  ): Promise<TxResult> {
    return this.client.invoke(
      this.contractId,
      'remove_liquidity',
      [providerKeypair.publicKey(), shares],
      providerKeypair,
    );
  }

  /** Swap tokenA for tokenB (or reverse). */
  async swap(
    tokenIn: string,
    amountIn: string,
    minAmountOut: string,
    traderKeypair: Keypair,
  ): Promise<TxResult> {
    return this.client.invoke(
      this.contractId,
      'swap',
      [traderKeypair.publicKey(), tokenIn, amountIn, minAmountOut],
      traderKeypair,
    );
  }

  /** Get pool state. */
  async getState(): Promise<CpmmState> {
    const result = await this.client.simulate(this.contractId, 'get_pool_state', []);
    return result as CpmmState;
  }

  /** Get LP balance. */
  async getLpBalance(userAddress: string): Promise<string> {
    const result = await this.client.simulate(this.contractId, 'get_lp_balance', [userAddress]);
    return result as string;
  }
}

// ─── Stableswap Client ────────────────────────────────────────────────

export class StableswapClient {
  constructor(
    private readonly client: SubanClient,
    private readonly contractId: string,
  ) {}

  /** Initialize the pool. */
  async initialize(
    tokenA: string,
    tokenB: string,
    amplification: string,
    adminKeypair: Keypair,
  ): Promise<TxResult> {
    return this.client.invoke(
      this.contractId,
      'initialize',
      [adminKeypair.publicKey(), tokenA, tokenB, amplification],
      adminKeypair,
    );
  }

  /** Add liquidity. */
  async addLiquidity(
    amountA: string,
    amountB: string,
    providerKeypair: Keypair,
  ): Promise<TxResult> {
    return this.client.invoke(
      this.contractId,
      'add_liquidity',
      [providerKeypair.publicKey(), amountA, amountB],
      providerKeypair,
    );
  }

  /** Remove liquidity. */
  async removeLiquidity(
    shares: string,
    providerKeypair: Keypair,
  ): Promise<TxResult> {
    return this.client.invoke(
      this.contractId,
      'remove_liquidity',
      [providerKeypair.publicKey(), shares],
      providerKeypair,
    );
  }

  /** Swap tokens. */
  async swap(
    tokenIn: string,
    amountIn: string,
    minAmountOut: string,
    traderKeypair: Keypair,
  ): Promise<TxResult> {
    return this.client.invoke(
      this.contractId,
      'swap',
      [traderKeypair.publicKey(), tokenIn, amountIn, minAmountOut],
      traderKeypair,
    );
  }

  /** Get pool state. */
  async getState(): Promise<CpmmState> {
    const result = await this.client.simulate(this.contractId, 'get_pool_state', []);
    return result as CpmmState;
  }
}

// ─── Liquidity Mining Client ──────────────────────────────────────────

export class LiquidityMiningClient {
  constructor(
    private readonly client: SubanClient,
    private readonly contractId: string,
  ) {}

  /** Stake LP tokens. */
  async stake(amount: string, userKeypair: Keypair): Promise<TxResult> {
    return this.client.invoke(
      this.contractId,
      'stake',
      [userKeypair.publicKey(), amount],
      userKeypair,
    );
  }

  /** Unstake LP tokens. */
  async unstake(amount: string, userKeypair: Keypair): Promise<TxResult> {
    return this.client.invoke(
      this.contractId,
      'unstake',
      [userKeypair.publicKey(), amount],
      userKeypair,
    );
  }

  /** Claim accrued rewards. */
  async claim(userKeypair: Keypair): Promise<TxResult> {
    return this.client.invoke(
      this.contractId,
      'claim',
      [userKeypair.publicKey()],
      userKeypair,
    );
  }

  /** Get staked balance. */
  async getStaked(userAddress: string): Promise<string> {
    const result = await this.client.simulate(this.contractId, 'get_staked', [userAddress]);
    return result as string;
  }

  /** Get pending rewards. */
  async getRewards(userAddress: string): Promise<string> {
    const result = await this.client.simulate(this.contractId, 'get_rewards', [userAddress]);
    return result as string;
  }
}
