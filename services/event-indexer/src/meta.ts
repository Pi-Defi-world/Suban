import Database from 'better-sqlite3';

export function nowIso(): string {
  return new Date().toISOString();
}

export function getMeta(db: Database.Database, key: string): string | null {
  const row = db.prepare('SELECT value FROM indexer_meta WHERE key = ?').get(key) as
    | { value: string }
    | undefined;
  return row?.value ?? null;
}

export function getNumberMeta(db: Database.Database, key: string): number | null {
  const value = getMeta(db, key);
  if (value === null) return null;
  const parsed = parseInt(value, 10);
  return Number.isFinite(parsed) ? parsed : null;
}

export function setMeta(db: Database.Database, key: string, value: string | number): void {
  db.prepare(
    `INSERT INTO indexer_meta (key, value, updated_at)
     VALUES (?, ?, CURRENT_TIMESTAMP)
     ON CONFLICT(key) DO UPDATE SET value = excluded.value,
       updated_at = CURRENT_TIMESTAMP`
  ).run(key, String(value));
}

export function getCheckpoint(db: Database.Database, stream: string): number {
  const row = db.prepare('SELECT last_ledger FROM checkpoints WHERE stream = ?').get(stream) as
    | { last_ledger: number }
    | undefined;
  return row?.last_ledger ?? 0;
}

export function setCheckpoint(
  db: Database.Database,
  stream: string,
  lastLedger: number,
  cursor: string | null = null
): void {
  db.prepare(
    `INSERT INTO checkpoints (stream, cursor, last_ledger, updated_at)
     VALUES (?, ?, ?, CURRENT_TIMESTAMP)
     ON CONFLICT(stream) DO UPDATE SET cursor = excluded.cursor,
       last_ledger = excluded.last_ledger,
       updated_at = CURRENT_TIMESTAMP`
  ).run(stream, cursor, lastLedger);
}

export function resetCheckpoint(
  db: Database.Database,
  stream: string,
  lastLedger: number
): void {
  setCheckpoint(db, stream, lastLedger, null);
}
