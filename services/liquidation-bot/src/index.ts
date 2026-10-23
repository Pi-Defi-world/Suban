#!/usr/bin/env node
/**
 * Testnet liquidation bot entry point.
 *
 * Usage:
 *   npm run scan          one dry-run pass
 *   npm start             loop (dry-run unless LIQUIDATION_MODE=execute)
 *
 * The process prints decisions and post-execution hashes, never secrets.
 */

import { loadBotConfig, runOnce } from './bot.js';

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function jsonSafe(_key: string, value: unknown): unknown {
  return typeof value === 'bigint' ? value.toString() : value;
}

async function main(): Promise<void> {
  const config = loadBotConfig();
  const once =
    process.argv.includes('--once') || process.env.LIQUIDATION_ONCE === 'true';

  do {
    const started = Date.now();
    try {
      const report = await runOnce(config);
      console.log(
        JSON.stringify(
          {
            source: 'lending_contract_simulation',
            observed_at: new Date().toISOString(),
            duration_ms: Date.now() - started,
            ...report,
          },
          jsonSafe,
          2
        )
      );
    } catch (error) {
      console.error(
        JSON.stringify({
          source: 'lending_contract_simulation',
          observed_at: new Date().toISOString(),
          error: error instanceof Error ? error.message : String(error),
        })
      );
      if (once) process.exitCode = 1;
    }

    if (once) return;
    await sleep(config.pollMs);
  } while (true);
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
