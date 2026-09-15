import type { SubanClient, TxResult } from './client.js';

export interface Price {
  price: number;
  decimals: number;
  confidence: number;
  timestamp: number;
}

export interface OracleConfig {
  maxAgeLedgers: number;
  breakerThresholdBps: number;
}

/**
 * Client for interacting with the Oracle contract.
 */
export class OracleClient {
  private constructor(
    private readonly client: SubanClient,
    private readonly contractId: string,
    private readonly networkPassphrase: string,
  ) {}

  static async connect(
    client: SubanClient,
    contractId: string,
    networkPassphrase: string,
  ): Promise<OracleClient> {
    return new OracleClient(client, contractId, networkPassphrase);
  }

  /**
   * Get the last price for an asset.
   */
  async getPrice(assetAddress: string): Promise<Price | null> {
    try {
      const result = await this.client.simulate(this.contractId, 'get_price', [assetAddress]);
      return result as Price;
    } catch {
      return null;
    }
  }

  /**
   * Check if price is stale.
   */
  async isStale(assetAddress: string, maxAge: number): Promise<boolean> {
    const result = await this.client.simulate(this.contractId, 'is_stale', [assetAddress, maxAge]);
    return result as boolean;
  }

  /**
   * Get oracle config.
   */
  async getConfig(): Promise<OracleConfig> {
    const result = await this.client.simulate(this.contractId, 'config', []);
    return result as OracleConfig;
  }

  /**
   * Push a new price for an asset (admin-only, called by the off-chain oracle
   * service). Maps to `set_price(admin, asset, price, decimals, confidence)`.
   */
  async setPrice(
    asset: string,
    price: string,
    decimals: number,
    confidence: number,
    adminKeypair: import('@stellar/stellar-sdk').Keypair,
  ): Promise<TxResult> {
    return this.client.invoke(
      this.contractId,
      'set_price',
      [adminKeypair.publicKey(), asset, price, decimals, confidence],
      adminKeypair,
    );
  }

  /**
   * Commit the current price as the last-known price (circuit breaker).
   * Maps to `commit_price(admin, asset)`.
   */
  async commitPrice(
    asset: string,
    adminKeypair: import('@stellar/stellar-sdk').Keypair,
  ): Promise<TxResult> {
    return this.client.invoke(
      this.contractId,
      'commit_price',
      [adminKeypair.publicKey(), asset],
      adminKeypair,
    );
  }
}
