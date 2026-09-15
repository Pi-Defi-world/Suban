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

  /** Propose a bridge operation (3.2/3.3). Maps to `propose(kind, target, amount, deposit_id)`. */
  async propose(
    kind: number,
    target: string,
    amount: string,
    depositId: Uint8Array,
    proposerKeypair: Keypair,
  ): Promise<TxResult> {
    return this.client.invoke(
      this.contractId,
      'propose',
      [proposerKeypair.publicKey(), kind, target, amount, depositId],
      proposerKeypair,
    );
  }

  /** Propose a wPi mint from a confirmed Pi deposit (kind = MintWpi = 0). */
  async proposeMint(
    to: string,
    amount: string,
    depositId: Uint8Array,
    proposerKeypair: Keypair,
  ): Promise<TxResult> {
    return this.propose(0, to, amount, depositId, proposerKeypair);
  }

  /** Propose releasing USDC from the vault for a redemption (kind = ReleaseUsdc = 1). */
  async proposeReleaseUsdc(
    from: string,
    amount: string,
    depositId: Uint8Array,
    proposerKeypair: Keypair,
  ): Promise<TxResult> {
    return this.propose(1, from, amount, depositId, proposerKeypair);
  }

  /** Approve a pending proposal (executes automatically once threshold is met). */
  async approveMint(
    proposalId: number,
    signerKeypair: Keypair,
  ): Promise<TxResult> {
    return this.client.invoke(
      this.contractId,
      'approve',
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
