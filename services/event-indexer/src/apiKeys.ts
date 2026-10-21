/**
 * API-key authentication and quota enforcement for the public data API.
 *
 * Keys are configured by hash. Usage is stored in SQLite so quotas survive a
 * restart and per-key consumption is visible to operators.
 */
import crypto from 'crypto';
import Database from 'better-sqlite3';
import { NextFunction, Request, Response } from 'express';
import { ApiKeyConfig, IndexerConfig } from './config.js';
import { apiKeyRequests } from './metrics.js';

interface UsageResult {
  allowed: boolean;
  minuteRemaining: number;
  dayRemaining: number;
}

function hashKey(raw: string): string {
  return crypto.createHash('sha256').update(raw).digest('hex');
}

function safeEqual(a: string, b: string): boolean {
  const left = Buffer.from(a);
  const right = Buffer.from(b);
  if (left.length !== right.length) return false;
  return crypto.timingSafeEqual(left, right);
}

export class QuotaStore {
  constructor(private readonly db: Database.Database) {}

  consume(key: ApiKeyConfig, now = Date.now()): UsageResult {
    const minuteStart = Math.floor(now / 60_000) * 60;
    const dayStart = Math.floor(now / 86_400_000) * 86_400;

    return this.db.transaction(() => {
      const minute = this.read(key.id, 'minute', minuteStart);
      const day = this.read(key.id, 'day', dayStart);
      const minuteRemaining = Math.max(0, key.perMinute - minute);
      const dayRemaining = Math.max(0, key.perDay - day);
      if (minuteRemaining <= 0 || dayRemaining <= 0) {
        return { allowed: false, minuteRemaining, dayRemaining };
      }

      this.increment(key.id, 'minute', minuteStart);
      this.increment(key.id, 'day', dayStart);
      return {
        allowed: true,
        minuteRemaining: minuteRemaining - 1,
        dayRemaining: dayRemaining - 1,
      };
    })();
  }

  usage(keys: ApiKeyConfig[], now = Date.now()) {
    const minuteStart = Math.floor(now / 60_000) * 60;
    const dayStart = Math.floor(now / 86_400_000) * 86_400;
    return keys.map((key) => ({
      key_id: key.id,
      name: key.name,
      per_minute: key.perMinute,
      per_day: key.perDay,
      minute_used: this.read(key.id, 'minute', minuteStart),
      day_used: this.read(key.id, 'day', dayStart),
    }));
  }

  prune(now = Date.now()): void {
    const cutoff = Math.floor(now / 86_400_000) * 86_400 - 2 * 86_400;
    this.db.prepare('DELETE FROM api_key_usage WHERE window_start < ?').run(cutoff);
  }

  private read(keyId: string, kind: string, windowStart: number): number {
    const row = this.db
      .prepare(
        `SELECT count FROM api_key_usage
         WHERE key_id = ? AND window_kind = ? AND window_start = ?`
      )
      .get(keyId, kind, windowStart) as { count: number } | undefined;
    return row?.count ?? 0;
  }

  private increment(keyId: string, kind: string, windowStart: number): void {
    this.db
      .prepare(
        `INSERT INTO api_key_usage (key_id, window_kind, window_start, count)
         VALUES (?, ?, ?, 1)
         ON CONFLICT(key_id, window_kind, window_start)
         DO UPDATE SET count = count + 1`
      )
      .run(keyId, kind, windowStart);
  }
}

export function extractApiKey(req: Request): string | null {
  const header = req.header('x-api-key');
  if (header) return header.trim();
  const authorization = req.header('authorization');
  if (authorization?.toLowerCase().startsWith('bearer ')) {
    return authorization.slice(7).trim();
  }
  return null;
}

export function findKey(raw: string, keys: ApiKeyConfig[]): ApiKeyConfig | null {
  const hash = hashKey(raw);
  return keys.find((key) => safeEqual(key.hash, hash)) || null;
}

export function createApiKeyMiddleware(
  config: IndexerConfig,
  quota: QuotaStore
): (req: Request, res: Response, next: NextFunction) => void {
  return (req, res, next) => {
    const raw = extractApiKey(req);
    if (!raw) {
      if (config.requireApiKey) {
        apiKeyRequests.inc({ key: 'anonymous', result: 'unauthorized' });
        res.status(401).json({
          error: 'API key required',
          hint: 'Send x-api-key or Authorization: Bearer <key>',
        });
        return;
      }
      next();
      return;
    }

    const key = findKey(raw, config.apiKeys);
    if (!key) {
      apiKeyRequests.inc({ key: 'unknown', result: 'unauthorized' });
      res.status(401).json({ error: 'Invalid API key' });
      return;
    }

    const usage = quota.consume(key);
    res.setHeader('X-API-Key-Name', key.name);
    res.setHeader('X-RateLimit-Limit-Minute', String(key.perMinute));
    res.setHeader('X-RateLimit-Remaining-Minute', String(usage.minuteRemaining));
    res.setHeader('X-RateLimit-Limit-Day', String(key.perDay));
    res.setHeader('X-RateLimit-Remaining-Day', String(usage.dayRemaining));

    if (!usage.allowed) {
      apiKeyRequests.inc({ key: key.name, result: 'quota_exceeded' });
      res.setHeader('Retry-After', '60');
      res.status(429).json({
        error: 'API key quota exceeded',
        key: key.name,
        per_minute: key.perMinute,
        per_day: key.perDay,
      });
      return;
    }

    apiKeyRequests.inc({ key: key.name, result: 'allowed' });
    next();
  };
}

export function createAdminMiddleware(
  config: IndexerConfig
): (req: Request, res: Response, next: NextFunction) => void {
  return (req, res, next) => {
    if (!config.adminApiKey) {
      res.status(503).json({ error: 'Admin API is not configured' });
      return;
    }
    const provided = extractApiKey(req) || '';
    if (!safeEqual(provided, config.adminApiKey)) {
      res.status(401).json({ error: 'Invalid admin API key' });
      return;
    }
    next();
  };
}
