import { Keypair } from '@stellar/stellar-sdk';
import type { SubanClient, TxResult } from './client.js';

export interface LendingPosition {
  deposit: string;
  borrow: string;
  collateral: string;
  healthFactor: number;
}

export interface LendingPoolState {
  totalSupply: string;
  totalBorrows: string;
  totalReserves: string;
  lastAccrual: number;
  collateralAsset: string;
}

/**
 * Client for interacting with the Lending Pool contract.
 */
export class LendingClient {
  constructor(
    private readonly client: SubanClient,
    private readonly contractId: string,
  ) {}

  // ─── Read Methods ─────────────────────────────────────────────────

  async getPoolState(): Promise<LendingPoolState> {
    const result = await this.client.simulate(this.contractId, 'get_pool_state', []);
    return result as LendingPoolState;
  }

  async getPosition(userAddress: string): Promise<LendingPosition> {
    const result = await this.client.simulate(this.contractId, 'get_position', [userAddress]);
    return result as LendingPosition;
  }

  async getHealthFactor(userAddress: string): Promise<number> {
    const result = await this.client.simulate(this.contractId, 'get_health_factor', [userAddress]);
    return result as number;
  }

  // ─── Write Methods ────────────────────────────────────────────────

  /** Deposit collateral (bTokens). */
  async depositCollateral(amount: string, userKeypair: Keypair): Promise<TxResult> {
    return this.client.invoke(
      this.contractId,
      'deposit',
      [userKeypair.publicKey(), amount],
      userKeypair,
    );
  }

  /** Withdraw collateral. */
  async withdrawCollateral(amount: string, userKeypair: Keypair): Promise<TxResult> {
    return this.client.invoke(
      this.contractId,
      'withdraw',
      [userKeypair.publicKey(), amount],
      userKeypair,
    );
  }

  /** Borrow assets. */
  async borrow(amount: string, userKeypair: Keypair): Promise<TxResult> {
    return this.client.invoke(
      this.contractId,
      'borrow',
      [userKeypair.publicKey(), amount],
      userKeypair,
    );
  }

  /** Repay borrowed assets. */
  async repay(amount: string, userKeypair: Keypair): Promise<TxResult> {
    return this.client.invoke(
      this.contractId,
      'repay',
      [userKeypair.publicKey(), amount],
      userKeypair,
    );
  }

  /** Liquidate an undercollateralized position. */
  async liquidate(
    userToLiquidate: string,
    repayAmount: string,
    minCollateral: string,
    keeperKeypair: Keypair,
  ): Promise<TxResult> {
    return this.client.invoke(
      this.contractId,
      'liquidate',
      [keeperKeypair.publicKey(), userToLiquidate, repayAmount, minCollateral],
      keeperKeypair,
    );
  }

  /** Set collateral asset. Admin only. */
  async setCollateralAsset(assetAddress: string, adminKeypair: Keypair): Promise<TxResult> {
    return this.client.invoke(
      this.contractId,
      'set_collateral_asset',
      [adminKeypair.publicKey(), assetAddress],
      adminKeypair,
    );
  }
}
