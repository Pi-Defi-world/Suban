import {
  contract,
  Keypair,
  rpc,
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
   * Simulate a read-only contract call.
   */
  async simulate(contractId: string, method: string, args: unknown[] = []): Promise<unknown> {
    const keypair = Keypair.random();
    const client = await this.getContractClient(contractId, keypair.publicKey());
    // @ts-expect-error - dynamic method
    const tx = await client[method](...args);
    const result = await tx.simulate();
    return result;
  }

  /**
   * Build, simulate, and submit a transaction.
   * `signFn` is called with the built transaction XDR for the caller to sign.
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
    const result = await this.server.sendTransaction(builtTx);
    return { txHash: result.hash, result };
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
