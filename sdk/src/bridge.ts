import { Keypair } from '@stellar/stellar-sdk';
import type { SubanClient, TxResult } from './client.js';

export interface BridgeConfig {
  admin: string;
  threshold: number;
  signerCount: number;
  paused: boolean;
  volumeCap: string;
}

export interface Attestation {
  id: string;
  depositId: string;
  attestorPublicKey: string;
  signature: string;
  timestamp: number;
}

/**
 * Client for interacting with the Bridge Multi-Sig contract.
 */
export class BridgeClient {
  constructor(
    private readonly client: SubanClient,
    private readonly contractId: string,
  ) {}

  // ─── Read Methods ─────────────────────────────────────────────────

  async getConfig(): Promise<BridgeConfig> {
    const result = await this.client.simulate(this.contractId, 'config', []);
    return result as BridgeConfig;
  }

  async isDepositProcessed(depositIdHex: string): Promise<boolean> {
    const depositIdBytes = Uint8Array.from(Buffer.from(depositIdHex, 'hex'));
    const result = await this.client.simulate(this.contractId, 'is_deposit_processed', [depositIdBytes]);
    return result as boolean;
  }

  async getVolumeStats(): Promise<{
    volume: string;
    windowStart: number;
    windowLedgers: number;
    cap: string;
  }> {
    const result = await this.client.simulate(this.contractId, 'volume_stats', []);
    return result as { volume: string; windowStart: number; windowLedgers: number; cap: string };
  }

  // ─── Write Methods ────────────────────────────────────────────────

  /** Propose a new mint. Returns proposal ID. */
  async proposeMint(
    depositId: string,
    to: string,
    amount: string,
    depositorKeypair: Keypair,
  ): Promise<TxResult> {
    return this.client.invoke(
      this.contractId,
      'propose_mint',
      [depositorKeypair.publicKey(), depositId, to, amount],
      depositorKeypair,
    );
  }

  /** Approve a pending mint proposal. */
  async approveMint(
    proposalId: number,
    signerKeypair: Keypair,
  ): Promise<TxResult> {
    return this.client.invoke(
      this.contractId,
      'approve_mint',
      [signerKeypair.publicKey(), proposalId],
      signerKeypair,
    );
  }

  /** Pause the bridge. Admin only. */
  async pause(adminKeypair: Keypair): Promise<TxResult> {
    return this.client.invoke(
      this.contractId,
      'pause',
      [adminKeypair.publicKey()],
      adminKeypair,
    );
  }

  /** Unpause the bridge. Admin only. */
  async unpause(adminKeypair: Keypair): Promise<TxResult> {
    return this.client.invoke(
      this.contractId,
      'unpause',
      [adminKeypair.publicKey()],
      adminKeypair,
    );
  }
}
