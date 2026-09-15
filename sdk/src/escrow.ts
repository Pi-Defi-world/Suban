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
    const result = await this.client.simulate(this.contractId, 'get_escrow', [escrowId]);
    return result as EscrowState;
  }

  async getEscrowCount(): Promise<number> {
    const result = await this.client.simulate(this.contractId, 'escrow_count', []);
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
      [escrowId, amount],
      funderKeypair,
    );
  }

  /** Submit milestone work. */
  async submitMilestone(
    escrowId: number,
    milestoneIdx: number,
    workerKeypair: Keypair,
  ): Promise<TxResult> {
    return this.client.invoke(
      this.contractId,
      'submit_milestone',
      [escrowId, milestoneIdx],
      workerKeypair,
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
      [escrowId, milestoneIdx],
      approverKeypair,
    );
  }

  /** Reject a milestone. */
  async rejectMilestone(
    escrowId: number,
    milestoneIdx: number,
    approverKeypair: Keypair,
  ): Promise<TxResult> {
    return this.client.invoke(
      this.contractId,
      'reject_milestone',
      [escrowId, milestoneIdx],
      approverKeypair,
    );
  }

  /** Release funds for an approved milestone. */
  async releaseFunds(escrowId: number, funderKeypair: Keypair): Promise<TxResult> {
    return this.client.invoke(
      this.contractId,
      'release_funds',
      [escrowId],
      funderKeypair,
    );
  }

  /** Refund remaining funds. */
  async refund(escrowId: number, funderKeypair: Keypair): Promise<TxResult> {
    return this.client.invoke(
      this.contractId,
      'refund',
      [escrowId],
      funderKeypair,
    );
  }

  /** Dispute the escrow. */
  async dispute(escrowId: number, disputerKeypair: Keypair): Promise<TxResult> {
    return this.client.invoke(
      this.contractId,
      'dispute',
      [escrowId, disputerKeypair.publicKey()],
      disputerKeypair,
    );
  }

  /** Arbitrator resolves dispute. `release` pays the receiver; otherwise refunds. */
  async resolve(
    escrowId: number,
    release: boolean,
    arbitratorKeypair: Keypair,
  ): Promise<TxResult> {
    return this.client.invoke(
      this.contractId,
      'resolve',
      [escrowId, release ? 'release' : 'refund'],
      arbitratorKeypair,
    );
  }
}
