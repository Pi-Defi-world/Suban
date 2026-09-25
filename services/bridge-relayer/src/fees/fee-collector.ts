import { config } from "../config";
import { logger } from "../logger";

export interface FeeCalculation {
  totalAmount: string;
  bridgeFee: string;
  protocolFee: string;
  relayerFee: string;
  netAmount: string;
}

export class FeeCollector {
  /**
   * Calculate bridge fees for a transfer
   * @param amount - Amount in PUSD base units (6 decimals)
   * @returns FeeCalculation breakdown
   */
  static calculate(amount: string): FeeCalculation {
    const amountBigInt = BigInt(amount);
    const feePercentage = BigInt(Math.floor(config.fees.percentage * 100)); // basis points
    const protocolShare = BigInt(Math.floor(config.fees.protocolShare * 100)); // basis points
    const minimumFee = BigInt(Math.floor(config.fees.minimumFee * 1e6)); // 6 decimals

    // Calculate fee: amount * feePercentage / 10000
    let bridgeFee = (amountBigInt * feePercentage) / 10000n;

    // Apply minimum fee
    if (bridgeFee < minimumFee) {
      bridgeFee = minimumFee;
    }

    // Split fee between protocol and relayer
    const protocolFee = (bridgeFee * protocolShare) / 100n;
    const relayerFee = bridgeFee - protocolFee;

    // Net amount after fee
    const netAmount = amountBigInt - bridgeFee;

    return {
      totalAmount: amount.toString(),
      bridgeFee: bridgeFee.toString(),
      protocolFee: protocolFee.toString(),
      relayerFee: relayerFee.toString(),
      netAmount: netAmount.toString(),
    };
  }

  /**
   * Log fee collection for accounting
   */
  static logFee(
    direction: "stellar_to_arc" | "arc_to_stellar",
    sender: string,
    amount: string,
    fees: FeeCalculation
  ): void {
    logger.info("Bridge fee collected", {
      direction,
      sender,
      totalAmount: fees.totalAmount,
      bridgeFee: fees.bridgeFee,
      protocolFee: fees.protocolFee,
      relayerFee: fees.relayerFee,
      netAmount: fees.netAmount,
    });
  }
}
