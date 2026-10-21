/**
 * Deterministic CPMM quote preview.
 *
 * The integer arithmetic intentionally mirrors `cpmm-pool::get_quote`:
 *
 *   amount_in_after_fee = amount_in * (10_000 - fee_bps) / 10_000
 *   amount_out          = reserve_out * amount_in_after_fee
 *                         / (reserve_in + amount_in_after_fee)
 *
 * Values are integer token units, not decimal display units. Callers must
 * apply the token's decimals themselves. This module does not discover routes
 * and does not invent liquidity.
 */

export interface CpmmQuoteInput {
  reserveIn: string | bigint;
  reserveOut: string | bigint;
  amountIn: string | bigint;
  feeBps: number;
}

export interface CpmmQuote {
  amountIn: string;
  amountInAfterFee: string;
  feeAmount: string;
  amountOut: string;
  feeBps: number;
  /** Reserve ratio before the trade, in output units per input unit. */
  spotPriceOutPerIn: number;
  /** Realized ratio including fee and slippage. */
  executionPriceOutPerIn: number;
  /** Total cost versus spot, in basis points. Includes the fee. */
  priceImpactBps: number;
}

function toUnits(value: string | bigint, field: string): bigint {
  if (typeof value === 'bigint') {
    if (value < 0n) throw new RangeError(`${field} must be non-negative`);
    return value;
  }
  if (!/^\d+$/.test(value)) {
    throw new RangeError(`${field} must be a non-negative integer string`);
  }
  return BigInt(value);
}

function toNumber(value: bigint): number {
  const number = Number(value);
  if (!Number.isFinite(number)) throw new RangeError('quote value is too large to represent');
  return number;
}

/**
 * Quote a two-asset constant-product swap with the same rounding as the
 * on-chain `get_quote` view. This is a preview helper; callers that need
 * state-at-execution guarantees should simulate `get_quote` on-chain.
 */
export function quoteCpmm({ reserveIn, reserveOut, amountIn, feeBps }: CpmmQuoteInput): CpmmQuote {
  const inReserve = toUnits(reserveIn, 'reserveIn');
  const outReserve = toUnits(reserveOut, 'reserveOut');
  const input = toUnits(amountIn, 'amountIn');

  if (inReserve <= 0n || outReserve <= 0n) {
    throw new RangeError('reserves must be positive');
  }
  if (input <= 0n) {
    throw new RangeError('amountIn must be positive');
  }
  if (!Number.isInteger(feeBps) || feeBps < 0 || feeBps > 1000) {
    throw new RangeError('feeBps must be an integer between 0 and 1000');
  }

  const amountInAfterFee = (input * BigInt(10_000 - feeBps)) / 10_000n;
  const amountOut = (outReserve * amountInAfterFee) / (inReserve + amountInAfterFee);
  const feeAmount = input - amountInAfterFee;

  // impact = 1 - (amount_out / amount_in) / (reserve_out / reserve_in)
  //        = (amount_in * reserve_out - amount_out * reserve_in)
  //          / (amount_in * reserve_out)
  const numerator = input * outReserve - amountOut * inReserve;
  const denominator = input * outReserve;
  const rawImpactBps = denominator === 0n ? 0n : (numerator * 10_000n) / denominator;
  const priceImpactBps = rawImpactBps < 0n ? 0 : toNumber(rawImpactBps);

  return {
    amountIn: input.toString(),
    amountInAfterFee: amountInAfterFee.toString(),
    feeAmount: feeAmount.toString(),
    amountOut: amountOut.toString(),
    feeBps,
    spotPriceOutPerIn: toNumber(outReserve) / toNumber(inReserve),
    executionPriceOutPerIn: amountOut === 0n ? 0 : toNumber(amountOut) / toNumber(input),
    priceImpactBps,
  };
}
