import * as StellarSdk from "@stellar/stellar-sdk";
import { ethers, Wallet, Contract, JsonRpcProvider } from "ethers";
import { config } from "../config";
import { logger } from "../logger";
import { StellarBurnEvent } from "../chains/stellar-watcher";
import { ArcBurnEvent } from "../chains/arc-watcher";
import { FeeCollector, FeeCalculation } from "../fees/fee-collector";

const ARC_BRIDGE_ABI = [
  "function mintPusd(address recipient, uint256 amount, string sourceChain, bytes32 sourceTxHash, uint256 sourceNonce, bytes[] signatures) external",
  "function burnPusd(string destChain, uint256 amount) external",
];

const STELLAR_BRIDGE合约 = "bridge-burn-mint";

export class TransactionBuilder {
  private stellarServer: StellarSdk.rpc.Server;
  private arcProvider: JsonRpcProvider;
  private arcSigner: Wallet;
  private stellarKeypair: StellarSdk.Keypair;

  constructor() {
    this.stellarServer = new StellarSdk.rpc.Server(config.stellar.rpcUrl, {
      allowHttp: true,
    });
    this.arcProvider = new JsonRpcProvider(
      config.arc.rpcUrl,
      config.arc.chainId
    );
    this.arcSigner = new Wallet(config.keys.evmSigner, this.arcProvider);
    this.stellarKeypair = StellarSdk.Keypair.fromSecret(
      config.keys.stellarSigner
    );
  }

  /**
   * Build and submit mint transaction on Arc after Stellar burn
   */
  async buildArcMint(
    event: StellarBurnEvent,
    recipient: string
  ): Promise<string> {
    const fees = FeeCollector.calculate(event.amount);
    FeeCollector.logFee("stellar_to_arc", event.sender, event.amount, fees);

    logger.info(`Building Arc mint: ${fees.netAmount} PUSD to ${recipient}`);

    // Get signatures from validator set
    const signatures = await this.getValidatorSignatures(event);

    // Build contract call
    const bridgeContract = new Contract(
      config.arc.bridgeContract,
      ARC_BRIDGE_ABI,
      this.arcSigner
    );

    const tx = await bridgeContract.mintPusd(
      recipient,
      fees.netAmount,
      event.sourceChain,
      event.txHash,
      event.nonce,
      signatures
    );

    const receipt = await tx.wait();
    logger.info(`Arc mint confirmed: ${receipt.hash}`);
    return receipt.hash;
  }

  /**
   * Build and submit burn transaction on Stellar after Arc burn
   */
  async buildStellarMint(
    event: ArcBurnEvent,
    recipient: string
  ): Promise<string> {
    const fees = FeeCollector.calculate(event.amount);
    FeeCollector.logFee("arc_to_stellar", event.sender, event.amount, fees);

    logger.info(
      `Building Stellar mint: ${fees.netAmount} PUSD to ${recipient}`
    );

    // Build Soroban transaction
    const account = await this.stellarServer.loadAccount(
      this.stellarKeypair.publicKey()
    );

    const contract = new StellarSdk.Contract(config.stellar.bridgeContract);

    const transaction = new StellarSdk.TransactionBuilder(account, {
      fee: StellarSdk.BASE_FEE,
      networkPassphrase: config.stellar.networkPassphrase,
    })
      .addOperation(
        contract.call(
          "mint_pusd",
          new StellarSdk.Address(recipient).toScVal(),
          StellarSdk.nativeToScVal(fees.netAmount, { type: "i128" }),
          StellarSdk.nativeToScVal(event.sourceChain, { type: "string" }),
          StellarSdk.nativeToScVal(event.nonce, { type: "u64" }),
          StellarSdk.nativeToScVal(event.txHash, { type: "bytes32" }),
          this.buildScValSignatures(event)
        )
      )
      .setTimeout(StellarSdk.TimeoutInfinite)
      .build();

    // Sign
    transaction.sign(this.stellarKeypair);

    // Submit
    const result = await this.stellarServer.sendTransaction(transaction);
    logger.info(`Stellar mint submitted: ${result.hash}`);
    return result.hash;
  }

  /**
   * Get validator signatures for a cross-chain event
   * In production, this would call a signing service or HSM
   */
  private async getValidatorSignatures(
    event: StellarBurnEvent
  ): Promise<string[]> {
    // Placeholder: In production, this would:
    // 1. Build the message to sign
    // 2. Send to M-of-N validators
    // 3. Collect signatures
    // 4. Return array of signatures

    logger.warn("Using placeholder signatures - implement signing service");
    return ["0x" + "0".repeat(65)]; // placeholder
  }

  /**
   * Build ScVal array of signatures for Soroban
   */
  private buildScValSignatures(event: ArcBurnEvent): any {
    // Placeholder: Build Soroban Vec of signer addresses
    return StellarSdk.nativeToScVal([], { type: "vec" });
  }
}
