import {
  contract,
  Keypair,
  rpc,
  scValToNative,
} from '@stellar/stellar-sdk';

export interface SubanConfig {
  rpcUrl: string;
  networkPassphrase: string;
}

export interface TxResult {
  txHash: string;
  result: unknown;
}

/**
 * Main client for interacting with Suban protocol.
 */
export class SubanClient {
  readonly server: rpc.Server;

  constructor(private readonly config: SubanConfig) {
    this.server = new rpc.Server(config.rpcUrl);
  }

  /**
   * Get the health of the Soroban RPC node.
   */
  async getHealth(): Promise<{ status: string }> {
    return this.server.getHealth();
  }

  /**
   * Get the current ledger sequence.
   */
  async getLedgerSequence(): Promise<number> {
    const response = await this.server.getLatestLedger();
    return response.sequence;
  }

  /**
   * Build a Soroban client for a given contract.
   */
  async getContractClient(contractId: string, publicKey: string): Promise<contract.Client> {
    return contract.Client.from({
      contractId,
      networkPassphrase: this.config.networkPassphrase,
      rpcUrl: this.config.rpcUrl,
      publicKey,
    });
  }

  /**
   * Simulate a read-only contract call and decode the return value into a
   * native JS value (3.1).
   */
  async simulate(contractId: string, method: string, args: unknown[] = []): Promise<unknown> {
    const keypair = Keypair.random();
    const client = await this.getContractClient(contractId, keypair.publicKey());
    // @ts-expect-error - dynamic method
    const tx = await client[method](...args);
    const sim = await tx.simulate();
    const retval = (sim as unknown as { simulationResult?: { result?: unknown } }).simulationResult?.result;
    if (retval === undefined) {
      return null;
    }
    return scValToNative(retval as never);
  }

  /**
   * Build, simulate, sign, submit, and **confirm** a transaction (3.5).
   * Polls `getTransaction` until the tx reaches SUCCESS / FAILED / NOT_FOUND,
   * so the caller knows the outcome instead of just the submission receipt.
   */
  async invoke(
    contractId: string,
    method: string,
    args: unknown[],
    sourceKeypair: Keypair,
  ): Promise<TxResult> {
    const client = await this.getContractClient(contractId, sourceKeypair.publicKey());
    // @ts-expect-error - dynamic method
    const tx = await client[method](...args);
    const builtTx = await tx.build();
    builtTx.sign(sourceKeypair);
    const sendResult = await this.server.sendTransaction(builtTx);

    // If the network already resolved it (unusual), return as-is.
    if (sendResult.status && sendResult.status !== 'PENDING') {
      return { txHash: sendResult.hash, result: sendResult };
    }

    const hash = sendResult.hash;
    const maxAttempts = 30;
    for (let attempt = 0; attempt < maxAttempts; attempt++) {
      await new Promise((r) => setTimeout(r, 1000));
      const txr = await this.server.getTransaction(hash);
      if (txr.status === 'SUCCESS' || txr.status === 'FAILED') {
        return { txHash: hash, result: txr };
      }
      // NOT_FOUND means not yet included; keep polling.
    }
    // Timed out waiting; return the hash so the caller can re-check later.
    return { txHash: hash, result: sendResult };
  }

  /**
   * Invoke with explicit sign function (for wallet-based signing).
   */
  async invokeWithSigner(
    contractId: string,
    method: string,
    args: unknown[],
    signerKeypair: Keypair,
  ): Promise<TxResult> {
    return this.invoke(contractId, method, args, signerKeypair);
  }
}
