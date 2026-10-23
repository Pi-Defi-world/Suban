/**
 * Environment-only configuration for the testnet liquidation bot.
 *
 * There are no private-key defaults and no contract defaults. A missing or
 * malformed value fails closed before any network call.
 */

export type LiquidationMode = 'dry-run' | 'execute';

export interface BotConfig {
  poolId: string;
  rpcUrl: string;
  networkPassphrase: string;
  txFee: string;
  mode: LiquidationMode;
  liquidatorSecret?: string;
  liquidatorPublicKey?: string;
  accounts: string[];
  dataPlatformUrl?: string;
  maxRepayRaw: bigint;
  minHealthBps: bigint;
  bonusBps: bigint;
  slippageBps: bigint;
  pollMs: number;
  maxCandidates: number;
}

export class ConfigError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'ConfigError';
  }
}

const CONTRACT_ID = /^C[A-Z2-7]{55}$/;
const ACCOUNT_ID = /^G[A-Z2-7]{55}$/;

function required(env: NodeJS.ProcessEnv, key: string): string {
  const value = env[key]?.trim();
  if (!value) throw new ConfigError(`${key} is required`);
  return value;
}

function positiveBigInt(
  env: NodeJS.ProcessEnv,
  key: string,
  fallback: string
): bigint {
  const raw = env[key]?.trim() || fallback;
  if (!/^\d+$/.test(raw)) {
    throw new ConfigError(`${key} must be a non-negative integer`);
  }
  const value = BigInt(raw);
  if (value < 0n) throw new ConfigError(`${key} must be non-negative`);
  return value;
}

function positiveInt(
  env: NodeJS.ProcessEnv,
  key: string,
  fallback: number
): number {
  const raw = env[key]?.trim() || String(fallback);
  if (!/^\d+$/.test(raw)) {
    throw new ConfigError(`${key} must be a positive integer`);
  }
  const value = Number(raw);
  if (!Number.isSafeInteger(value) || value <= 0) {
    throw new ConfigError(`${key} must be a positive integer`);
  }
  return value;
}

function parseAccounts(raw: string | undefined): string[] {
  if (!raw?.trim()) return [];
  const accounts = raw
    .split(',')
    .map((value) => value.trim())
    .filter(Boolean);
  for (const account of accounts) {
    if (!ACCOUNT_ID.test(account)) {
      throw new ConfigError(`LIQUIDATION_ACCOUNTS contains an invalid address: ${account}`);
    }
  }
  return [...new Set(accounts)];
}

export function loadConfig(env: NodeJS.ProcessEnv = process.env): BotConfig {
  const poolId = required(env, 'LENDING_POOL_CONTRACT_ID');
  if (!CONTRACT_ID.test(poolId)) {
    throw new ConfigError('LENDING_POOL_CONTRACT_ID must be a Soroban contract id');
  }

  const mode = (env.LIQUIDATION_MODE?.trim() || 'dry-run') as LiquidationMode;
  if (mode !== 'dry-run' && mode !== 'execute') {
    throw new ConfigError('LIQUIDATION_MODE must be dry-run or execute');
  }

  const secret = env.LIQUIDATOR_SECRET?.trim() || undefined;
  const publicKey = env.LIQUIDATOR_PUBLIC_KEY?.trim() || undefined;
  if (mode === 'execute' && !secret) {
    throw new ConfigError('LIQUIDATOR_SECRET is required in execute mode');
  }
  if (!secret && !publicKey) {
    throw new ConfigError(
      'LIQUIDATOR_PUBLIC_KEY (or LIQUIDATOR_SECRET) is required for simulation'
    );
  }
  if (publicKey && !ACCOUNT_ID.test(publicKey)) {
    throw new ConfigError('LIQUIDATOR_PUBLIC_KEY must be a Stellar account address');
  }

  const accounts = parseAccounts(env.LIQUIDATION_ACCOUNTS);
  const dataPlatformUrl = env.DATA_PLATFORM_URL?.trim() || undefined;
  if (accounts.length === 0 && !dataPlatformUrl) {
    throw new ConfigError(
      'LIQUIDATION_ACCOUNTS or DATA_PLATFORM_URL is required for candidate discovery'
    );
  }

  return {
    poolId,
    rpcUrl: env.STELLAR_RPC_URL?.trim() || 'https://rpc.testnet.minepi.com',
    networkPassphrase: env.STELLAR_NETWORK_PASSPHRASE?.trim() || 'Pi Testnet',
    txFee: (() => {
      const raw = env.STELLAR_TX_FEE?.trim() || '2000000';
      if (!/^\d+$/.test(raw) || BigInt(raw) <= 0n) {
        throw new ConfigError('STELLAR_TX_FEE must be a positive integer in stroops');
      }
      return raw;
    })(),
    mode,
    liquidatorSecret: secret,
    liquidatorPublicKey: publicKey,
    accounts,
    dataPlatformUrl,
    maxRepayRaw: positiveBigInt(env, 'LIQUIDATION_MAX_REPAY_RAW', '1000000000'),
    minHealthBps: positiveBigInt(env, 'LIQUIDATION_MIN_HEALTH_BPS', '10000'),
    bonusBps: positiveBigInt(env, 'LIQUIDATION_BONUS_BPS', '500'),
    slippageBps: positiveBigInt(env, 'LIQUIDATION_SLIPPAGE_BPS', '100'),
    pollMs: positiveInt(env, 'LIQUIDATION_POLL_MS', 300_000),
    maxCandidates: positiveInt(env, 'LIQUIDATION_MAX_CANDIDATES', 100),
  };
}
