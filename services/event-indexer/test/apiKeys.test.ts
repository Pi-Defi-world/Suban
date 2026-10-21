import { describe, expect, it } from 'vitest';
import { findKey, QuotaStore } from '../src/apiKeys.js';
import { parseApiKeys } from '../src/config.js';
import { memoryDb } from './helpers.js';

describe('API keys and quotas', () => {
  it('parses compact and JSON key lists without retaining raw keys', () => {
    const compact = parseApiKeys('k1:alpha:1:2,k2:beta:10:100');
    expect(compact).toHaveLength(2);
    expect(compact[0].name).toBe('alpha');
    expect(compact[0].hash).not.toContain('k1');
    expect(compact[0].perMinute).toBe(1);

    const json = parseApiKeys(
      JSON.stringify([{ key: 'k3', name: 'gamma', per_minute: 5, per_day: 50 }])
    );
    expect(json[0].perDay).toBe(50);
    expect(findKey('k3', json)?.name).toBe('gamma');
    expect(findKey('wrong', json)).toBeNull();
  });

  it('enforces minute and day quotas and persists usage', () => {
    const db = memoryDb();
    const key = parseApiKeys('k1:alpha:1:2')[0];
    const quota = new QuotaStore(db);

    expect(quota.consume(key, 0).allowed).toBe(true);
    expect(quota.consume(key, 0).allowed).toBe(false);
    expect(quota.consume(key, 61_000).allowed).toBe(true);
    expect(quota.consume(key, 122_000).allowed).toBe(false);

    const reopened = new QuotaStore(db);
    const usage = reopened.usage([key], 61_000)[0];
    expect(usage.minute_used).toBe(1);
    expect(usage.day_used).toBe(2);
    db.close();
  });
});
