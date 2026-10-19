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

    // Get signatures from validator set (use netAmount to match what contract receives)
    const signatures = await this.getValidatorSignatures(event, recipient, fees.netAmount);

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
   * Build and submit burn transaction on Stellar after Arc burn.
   *
   * DISABLED (Phase 0): the Stellar bridge now requires each signer to
   * authorize the invocation (soroban require_auth). The relayer does not yet
   * collect and attach those auth entries, so submitting a mint would fail.
   * Re-enable only after signer authorization collection is implemented.
   */
  async buildStellarMint(
    event: ArcBurnEvent,
    recipient: string
  ): Promise<string> {
    void event;
    void recipient;
    throw new Error(
      "Arc->Stellar mint is disabled: signer authorization entries are not implemented"
    );
  }

  /**
   * Get validator signatures for a cross-chain event
   * For testing: single-sig with relayer key
   * For mainnet: implement M-of-N threshold signing
   */
  private async getValidatorSignatures(
    event: StellarBurnEvent,
    recipientAddress: string,
    netAmount: string
  ): Promise<string[]> {
    // Build message to match ArcBridge's bridgeHash:
    // keccak256(abi.encodePacked(sourceChain, sourceTxHash, sourceNonce, amount, recipient))
    const txHashBytes = ethers.getBytes(event.txHash);
    const message = ethers.solidityPacked(
      ["string", "bytes32", "uint256", "uint256", "address"],
      [event.sourceChain, txHashBytes, event.nonce, netAmount, recipientAddress]
    );
    const messageHash = ethers.keccak256(message);

    // Sign with relayer EVM key (single-sig for testing)
    const signingWallet = new ethers.Wallet(config.keys.evmSigner);
    const signature = await signingWallet.signMessage(
      ethers.getBytes(messageHash)
    );

    logger.info(`Signed cross-chain message: ${event.txHash}`);
    return [signature];
  }
}
