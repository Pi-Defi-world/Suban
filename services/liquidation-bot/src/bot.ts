/**
 * One-shot and looped liquidation bot orchestration.
 *
 * Dry-run is the default. The execute path is a single, explicit branch that
 * requires `LIQUIDATION_MODE=execute` and a local liquidator secret.
 */

import { Keypair } from '@stellar/stellar-sdk';
import { LendingContract, type AccountRisk } from './client.js';
import { loadConfig, type BotConfig } from './config.js';
import { discoverAccounts } from './discovery.js';
import { planLiquidation, type LiquidationPlan } from './risk.js';

export interface BotReport {
  mode: 'dry-run' | 'execute';
  pool_id: string;
  candidates: number;
  inspected: number;
  liquidatable: number;
  executed: number;
  reviews: LiquidationPlan[];
  errors: Array<{ account?: string; message: string }>;
}

export interface BotDependencies {
  discover?: (config: BotConfig) => Promise<string[]>;
  read?: (client: LendingContract, account: string) => Promise<AccountRisk>;
  execute?: (
    client: LendingContract,
    plan: LiquidationPlan
  ) => Promise<{ hash: string; ledger: number; status: string }>;
}

async function defaultDiscover(config: BotConfig): Promise<string[]> {
  if (config.accounts.length > 0) return config.accounts;
  if (!config.dataPlatformUrl) return [];
  return discoverAccounts(config.dataPlatformUrl, config.maxCandidates);
}

export async function runOnce(
  config: BotConfig,
  deps: BotDependencies = {}
): Promise<BotReport> {
  const report: BotReport = {
    mode: config.mode,
    pool_id: config.poolId,
    candidates: 0,
    inspected: 0,
    liquidatable: 0,
    executed: 0,
    reviews: [],
    errors: [],
  };

  const candidates = (await (deps.discover || defaultDiscover)(config)).slice(
    0,
    config.maxCandidates
  );
  report.candidates = candidates.length;
  if (candidates.length === 0) return report;

  const signer =
    config.mode === 'execute'
      ? Keypair.fromSecret(config.liquidatorSecret!)
      : undefined;
  const client = await LendingContract.create(config, signer);
  const read = deps.read || ((c: LendingContract, a: string) => c.readAccount(a));
  const execute =
    deps.execute ||
    ((c: LendingContract, p: LiquidationPlan) =>
      c.liquidate({
        account: p.account,
        repayRaw: p.repayRaw,
        minCollateralRaw: p.minCollateralRaw,
      }));

  for (const account of candidates) {
    try {
      const risk = await read(client, account);
      report.inspected += 1;
      const plan = planLiquidation({
        account,
        healthBps: risk.healthBps,
        borrowRaw: risk.borrowRaw,
        collateralRaw: risk.collateralRaw,
        maxRepayRaw: config.maxRepayRaw,
        minHealthBps: config.minHealthBps,
        bonusBps: config.bonusBps,
        slippageBps: config.slippageBps,
      });
      if (!plan.liquidatable) continue;
      report.liquidatable += 1;

      if (config.mode === 'execute') {
        const result = await execute(client, plan);
        report.executed += 1;
        report.reviews.push({ ...plan, reason: `executed:${result.hash}` });
      } else {
        report.reviews.push(plan);
      }
    } catch (error) {
      report.errors.push({
        account,
        message: error instanceof Error ? error.message : String(error),
      });
    }
  }

  return report;
}

export function loadBotConfig(env: NodeJS.ProcessEnv = process.env): BotConfig {
  return loadConfig(env);
}
