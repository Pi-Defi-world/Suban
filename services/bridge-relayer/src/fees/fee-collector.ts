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
    // 0.5% = 50 basis points = 50/10000
    const feeBps = BigInt(Math.round(config.fees.percentage * 100)); // 0.5 -> 50
    const protocolShareBps = BigInt(Math.round(config.fees.protocolShare * 100)); // 0.1 -> 10
    // Minimum fee: 1 PUSD = 1_000_000 base units (6 decimals)
    const minimumFee = BigInt(Math.round(config.fees.minimumFee * 1_000_000));

    // bridgeFee = amount * feeBps / 10000
    let bridgeFee = (amountBigInt * feeBps) / 10000n;

    // Apply minimum fee
    if (bridgeFee < minimumFee) {
      bridgeFee = minimumFee;
    }

    // Split fee: protocol gets protocolShare portion, relayer gets rest
    const protocolFee = (bridgeFee * protocolShareBps) / 100n;
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
