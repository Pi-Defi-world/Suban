#!/usr/bin/env node
/**
 * Compare the SDK CPMM preview with a deployed contract's `get_quote` view.
 *
 * This is the deployed-parity check required before describing the quote
 * preview as matching on-chain behaviour. It is read-only: simulation builds
 * a transaction envelope but nothing is signed or submitted.
 *
 * Required environment:
 *   QUOTE_PARITY_POOL_ID         deployed CPMM contract id
 *   QUOTE_PARITY_SOURCE_ACCOUNT  existing account used as the simulation source
 *
 * Optional:
 *   QUOTE_PARITY_RPC_URL              default https://rpc.testnet.minepi.com
 *   QUOTE_PARITY_NETWORK_PASSPHRASE   default "Pi Testnet"
 *   QUOTE_PARITY_AMOUNT_IN            default 1000000 (raw units)
 *
 * Run from sdk/ after `npm run build`:
 *   npm run parity:testnet
 */
import { contract, Keypair } from '@stellar/stellar-sdk';
import { quoteCpmm } from '../dist/quote.js';

const rpcUrl = process.env.QUOTE_PARITY_RPC_URL || 'https://rpc.testnet.minepi.com';
const networkPassphrase = process.env.QUOTE_PARITY_NETWORK_PASSPHRASE || 'Pi Testnet';
const poolId = process.env.QUOTE_PARITY_POOL_ID;
const source = process.env.QUOTE_PARITY_SOURCE_ACCOUNT;
const amountIn = process.env.QUOTE_PARITY_AMOUNT_IN || '1000000';

function fail(message) {
  console.error(message);
  process.exit(2);
}

if (!poolId) fail('QUOTE_PARITY_POOL_ID is required');
if (!source) fail('QUOTE_PARITY_SOURCE_ACCOUNT is required');
if (!/^\d+$/.test(amountIn) || BigInt(amountIn) <= 0n) {
  fail('QUOTE_PARITY_AMOUNT_IN must be a positive integer string');
}

const client = await contract.Client.from({
  contractId: poolId,
  networkPassphrase,
  rpcUrl,
  publicKey: Keypair.fromPublicKey(source).publicKey(),
});

async function simulate(method, args) {
  const tx = await client[method](...args);
  const simulated = await tx.simulate();
  return {
    value: simulated?.result?.value ?? simulated?.result,
    ledger: Number(simulated?.simulation?.latestLedger || 0),
  };
}

const [tokenA, tokenB, feeBps, reserves] = await Promise.all([
  simulate('get_token_a', []).then((r) => String(r.value)),
  simulate('get_token_b', []).then((r) => String(r.value)),
  simulate('get_fee_bps', []).then((r) => Number(r.value)),
  simulate('get_reserves', []).then((r) => r.value),
]);

const checks = [];
for (const [tokenIn, tokenOut, reserveIn, reserveOut] of [
  [tokenA, tokenB, String(reserves[0]), String(reserves[1])],
  [tokenB, tokenA, String(reserves[1]), String(reserves[0])],
]) {
  const preview = quoteCpmm({ reserveIn, reserveOut, amountIn, feeBps });
  const onChain = await simulate('get_quote', [
    { token_in: tokenIn, amount_in: BigInt(amountIn) },
  ]);
  checks.push({
    token_in: tokenIn,
    token_out: tokenOut,
    reserve_in_raw: reserveIn,
    reserve_out_raw: reserveOut,
    preview_amount_out_raw: preview.amountOut,
    on_chain_amount_out_raw: String(onChain.value),
    matches: preview.amountOut === String(onChain.value),
    ledger: onChain.ledger,
  });
}

const report = {
  ok: checks.every((check) => check.matches),
  rpc_url: rpcUrl,
  network_passphrase: networkPassphrase,
  pool_id: poolId,
  amount_in_raw: amountIn,
  fee_bps: feeBps,
  reserves_raw: [String(reserves[0]), String(reserves[1])],
  checks,
};

console.log(JSON.stringify(report, null, 2));
process.exit(report.ok ? 0 : 1);
