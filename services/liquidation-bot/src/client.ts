/**
 * Thin Soroban adapter for the lending pool.
 *
 * Reads are always simulations of the deployed contract. Execution uses the
 * SDK's `KeypairSigner` path and only happens when the caller passes a signer.
 */

import { contract, Keypair } from '@stellar/stellar-sdk';
import type { BotConfig } from './config.js';

export interface AccountRisk {
  account: string;
  depositRaw: bigint;
  borrowRaw: bigint;
  collateralRaw: bigint;
  healthBps: bigint;
}

export interface ExecutionResult {
  hash: string;
  ledger: number;
  status: string;
}

function toBigInt(value: unknown): bigint {
  if (typeof value === 'bigint') return value;
  if (typeof value === 'number' && Number.isInteger(value)) return BigInt(value);
  if (typeof value === 'string' && /^-?\d+$/.test(value)) return BigInt(value);
  if (value && typeof value === 'object') {
    const record = value as Record<string, unknown>;
    if ('value' in record) return toBigInt(record.value);
  }
  throw new Error(`Expected an integer contract result, got ${typeof value}`);
}

function unwrapResult(result: unknown): unknown {
  if (result && typeof result === 'object') {
    const record = result as Record<string, unknown>;
    if ('error' in record && record.error !== undefined) {
      throw new Error(`Contract simulation returned an error: ${String(record.error)}`);
    }
    if ('value' in record) return unwrapResult(record.value);
  }
  return result;
}

function field(position: unknown, name: string): bigint {
  if (!position || typeof position !== 'object') {
    throw new Error('Contract returned a malformed position');
  }
  return toBigInt((position as Record<string, unknown>)[name]);
}

export class LendingContract {
  private constructor(
    private readonly client: any,
    private readonly config: BotConfig
  ) {}

  static async create(config: BotConfig, signer?: Keypair): Promise<LendingContract> {
    const publicKey =
      signer?.publicKey() ||
      config.liquidatorPublicKey ||
      (config.liquidatorSecret
        ? Keypair.fromSecret(config.liquidatorSecret).publicKey()
        : undefined);
    if (!publicKey) {
      throw new Error('No liquidator public key is available for simulation');
    }

    // The SDK's ClientOptions type is not exported from the package root in
    // every release; the shape is documented and checked at runtime by the SDK.
    const options: any = {
      contractId: config.poolId,
      networkPassphrase: config.networkPassphrase,
      rpcUrl: config.rpcUrl,
      publicKey,
      fee: config.txFee,
    };
    // Passing the Keypair as the signer makes `signAndSend` sign the envelope
    // and the invoker auth entry. It is omitted for read-only/dry-run clients.
    if (signer) options.signTransaction = signer;

    const client = await contract.Client.from(options);
    return new LendingContract(client, config);
  }

  private async simulate(method: string, args: unknown[]): Promise<unknown> {
    const transaction = await this.client[method](...args);
    const simulated = await transaction.simulate();
    return unwrapResult(simulated?.result);
  }

  async readAccount(account: string): Promise<AccountRisk> {
    const [position, health] = await Promise.all([
      this.simulate('get_position', [{ user: account }]),
      this.simulate('get_health_factor', [{ user: account }]),
    ]);
    return {
      account,
      depositRaw: field(position, 'deposit'),
      borrowRaw: field(position, 'borrow'),
      collateralRaw: field(position, 'collateral'),
      healthBps: toBigInt(health),
    };
  }

  async liquidate(plan: {
    account: string;
    repayRaw: bigint;
    minCollateralRaw: bigint;
  }): Promise<ExecutionResult> {
    const transaction = await this.client.liquidate({
      liquidator: this.publicKey(),
      borrower: plan.account,
      repay_amount: plan.repayRaw,
      min_collateral: plan.minCollateralRaw,
    });
    const sent = await transaction.signAndSend();
    const status =
      sent.getTransactionResponse?.status ||
      sent.sendTransactionResponse?.status ||
      'unknown';
    if (status !== 'SUCCESS') {
      throw new Error(`Liquidation transaction ended with status ${status}`);
    }
    return {
      hash:
        sent.sendTransactionResponse?.hash ||
        sent.getTransactionResponse?.txHash ||
        '',
      ledger: Number(sent.getTransactionResponse?.ledger || 0),
      status,
    };
  }

  private publicKey(): string {
    if (this.config.liquidatorSecret) {
      return Keypair.fromSecret(this.config.liquidatorSecret).publicKey();
    }
    if (this.config.liquidatorPublicKey) return this.config.liquidatorPublicKey;
    throw new Error('No liquidator public key configured');
  }
}
