import { Keypair } from '@stellar/stellar-sdk';
import type { SubanClient, TxResult } from './client.js';

export interface Milestone {
  description: string;
  approver: string;
  amount: string;
}

export interface EscrowConfig {
  funder: string;
  receiver: string;
  arbitrator: string;
  asset: string;
  milestones: Milestone[];
  feeBps: number;
  deadlineLedger: number;
}

export interface EscrowState {
  escrowId: number;
  config: EscrowConfig;
  totalDeposited: string;
  totalReleased: string;
  milestonesCompleted: number;
  milestoneStatuses: string[];
  status: string;
}

/**
 * Client for interacting with the Escrow Manager contract.
 */
export class EscrowClient {
  constructor(
    private readonly client: SubanClient,
    private readonly contractId: string,
  ) {}

  // ─── Read Methods ─────────────────────────────────────────────────

  async getEscrow(escrowId: number): Promise<EscrowState> {
    const result = await this.client.simulate(this.contractId, 'get_escrow_state', [escrowId]);
    return result as EscrowState;
  }

  async getEscrowCount(): Promise<number> {
    const result = await this.client.simulate(this.contractId, 'get_escrow_count', []);
    return result as number;
  }

  // ─── Write Methods ────────────────────────────────────────────────

  /** Create a new escrow. Returns escrow ID. */
  async createEscrow(
    receiver: string,
    arbitrator: string,
    asset: string,
    milestones: Milestone[],
    feeBps: number,
    deadlineLedger: number,
    funderKeypair: Keypair,
  ): Promise<TxResult> {
    return this.client.invoke(
      this.contractId,
      'create_escrow',
      [funderKeypair.publicKey(), receiver, arbitrator, asset, milestones, feeBps, deadlineLedger],
      funderKeypair,
    );
  }

  /** Fund an escrow. */
  async fundEscrow(escrowId: number, amount: string, funderKeypair: Keypair): Promise<TxResult> {
    return this.client.invoke(
      this.contractId,
      'fund_escrow',
      [funderKeypair.publicKey(), escrowId, amount],
      funderKeypair,
    );
  }

  /** Submit milestone work. */
  async submitMilestone(
    escrowId: number,
    milestoneIdx: number,
    receiverKeypair: Keypair,
  ): Promise<TxResult> {
    return this.client.invoke(
      this.contractId,
      'submit_milestone',
      [receiverKeypair.publicKey(), escrowId, milestoneIdx],
      receiverKeypair,
    );
  }

  /** Approve a milestone. */
  async approveMilestone(
    escrowId: number,
    milestoneIdx: number,
    approverKeypair: Keypair,
  ): Promise<TxResult> {
    return this.client.invoke(
      this.contractId,
      'approve_milestone',
      [approverKeypair.publicKey(), escrowId, milestoneIdx],
      approverKeypair,
    );
  }

  /** Reject a milestone. */
  async rejectMilestone(
    escrowId: number,
    milestoneIdx: number,
    reason: string,
    approverKeypair: Keypair,
  ): Promise<TxResult> {
    return this.client.invoke(
      this.contractId,
      'reject_milestone',
      [approverKeypair.publicKey(), escrowId, milestoneIdx, reason],
      approverKeypair,
    );
  }

  /** Release funds for an approved milestone. */
  async releaseFunds(escrowId: number, funderKeypair: Keypair): Promise<TxResult> {
    return this.client.invoke(
      this.contractId,
      'release_funds',
      [funderKeypair.publicKey(), escrowId],
      funderKeypair,
    );
  }

  /** Refund remaining funds. */
  async refund(escrowId: number, funderKeypair: Keypair): Promise<TxResult> {
    return this.client.invoke(
      this.contractId,
      'refund',
      [funderKeypair.publicKey(), escrowId],
      funderKeypair,
    );
  }

  /** Dispute the escrow. */
  async dispute(escrowId: number, reason: string, disputerKeypair: Keypair): Promise<TxResult> {
    return this.client.invoke(
      this.contractId,
      'dispute',
      [disputerKeypair.publicKey(), escrowId, reason],
      disputerKeypair,
    );
  }

  /** Arbitrator resolves dispute. */
  async resolve(
    escrowId: number,
    release: boolean,
    arbitratorKeypair: Keypair,
  ): Promise<TxResult> {
    return this.client.invoke(
      this.contractId,
      'resolve',
      [arbitratorKeypair.publicKey(), escrowId, release],
      arbitratorKeypair,
    );
  }
}
