import { scValToNative, xdr } from '@stellar/stellar-sdk';

/**
 * Normalize a Soroban value from RPC or storage.
 *
 * RPC servers differ: some return base64 XDR strings, while others (including
 * the Pi testnet RPC) return already-decoded objects such as `{ symbol }`,
 * `{ address }`, `{ vec }` or `{ i128 }`. Financial projections must see the
 * same primitive values either way, and the result must be JSON-safe because
 * it is persisted in SQLite.
 */
function normalize(value: unknown): unknown {
  if (typeof value === 'bigint') return value.toString();
  if (Array.isArray(value)) return value.map((entry) => normalize(entry));
  if (!value || typeof value !== 'object') return value;
  return Object.fromEntries(
    Object.entries(value as Record<string, unknown>).map(([key, entry]) => [
      key,
      normalize(entry),
    ])
  );
}

function decodeRaw(value: unknown): unknown {
  if (value instanceof xdr.ScVal) return scValToNative(value as xdr.ScVal);
  if (typeof value === 'string') {
    try {
      return scValToNative(xdr.ScVal.fromXDR(value, 'base64'));
    } catch {
      return value;
    }
  }
  if (Array.isArray(value)) return value.map((entry) => decodeRaw(entry));
  if (!value || typeof value !== 'object') return value;

  const record = value as Record<string, unknown>;
  if ('symbol' in record) return record.symbol;
  if ('address' in record) return record.address;
  if ('vec' in record && Array.isArray(record.vec)) {
    const vec = record.vec as unknown[];
    return vec.map((entry) => decodeRaw(entry));
  }
  if ('map' in record && Array.isArray(record.map)) {
    const map = record.map as unknown[];
    return map.map((entry) => decodeRaw(entry));
  }
  if ('bool' in record) return record.bool;
  if ('void' in record) return null;
  for (const key of ['i128', 'u128', 'i64', 'u64', 'i32', 'u32', 'i256', 'u256']) {
    if (key in record) return record[key];
  }
  if ('bytes' in record) return record.bytes;

  return Object.fromEntries(
    Object.entries(record).map(([key, entry]) => [key, decodeRaw(entry)])
  );
}

export function decodeScVal(value: unknown): unknown {
  return normalize(decodeRaw(value));
}
