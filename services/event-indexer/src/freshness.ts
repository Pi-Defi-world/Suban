import Database from 'better-sqlite3';
import { IndexerConfig } from './config.js';
import { getMeta, getNumberMeta } from './meta.js';

export interface Freshness {
  as_of: string;
  latest_indexed_ledger: number;
  latest_observed_ledger: number;
  lag_ledgers: number;
  lag_seconds: number | null;
  last_success_at: string | null;
  stale: boolean;
  stale_after_seconds: number;
}

export function getFreshness(
  db: Database.Database,
  config: IndexerConfig
): Freshness {
  const row = db
    .prepare('SELECT MAX(last_ledger) AS ledger FROM checkpoints')
    .get() as { ledger: number | null };
  const latestIndexed = row?.ledger || 0;
  const latestObserved = getNumberMeta(db, 'latest_observed_ledger') || latestIndexed;
  const lagLedgers = Math.max(0, latestObserved - latestIndexed);
  const lastSuccessAt = getMeta(db, 'last_success_at');
  const lastSuccessMs = lastSuccessAt ? Date.parse(lastSuccessAt) : NaN;
  const lagSeconds = Number.isFinite(lastSuccessMs)
    ? Math.max(0, Math.floor((Date.now() - lastSuccessMs) / 1000))
    : null;

  return {
    as_of: new Date().toISOString(),
    latest_indexed_ledger: latestIndexed,
    latest_observed_ledger: latestObserved,
    lag_ledgers: lagLedgers,
    lag_seconds: lagSeconds,
    last_success_at: lastSuccessAt,
    stale:
      lagLedgers > config.maxLagLedgers ||
      lagSeconds === null ||
      lagSeconds > config.staleAfterSeconds,
    stale_after_seconds: config.staleAfterSeconds,
  };
}
