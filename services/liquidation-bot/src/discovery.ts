/**
 * Candidate discovery from the public data platform.
 *
 * The bot never trusts indexed snapshots as execution state: discovery only
 * supplies account addresses. Every health factor and position is re-read from
 * the contract immediately before a decision.
 */

const ACCOUNT_ID = /^G[A-Z2-7]{55}$/;

interface LendingEvent {
  accounts?: unknown;
  position?: { borrow?: unknown } | null;
}

interface LendingEventsResponse {
  data?: unknown;
}

function asRaw(value: unknown): string | null {
  if (typeof value === 'string' && /^\d+$/.test(value)) return value;
  if (typeof value === 'number' && Number.isInteger(value) && value >= 0) {
    return String(value);
  }
  return null;
}

export function accountsFromEvents(
  events: unknown,
  maxCandidates: number
): string[] {
  if (!Array.isArray(events)) return [];
  const found = new Set<string>();
  for (const event of events as LendingEvent[]) {
    if (!event || !Array.isArray(event.accounts)) continue;
    const borrow = asRaw(event.position?.borrow);
    if (borrow === null || BigInt(borrow) <= 0n) continue;
    for (const account of event.accounts) {
      if (typeof account === 'string' && ACCOUNT_ID.test(account)) {
        found.add(account);
      }
    }
    if (found.size >= maxCandidates) break;
  }
  return [...found].slice(0, maxCandidates);
}

export async function discoverAccounts(
  dataPlatformUrl: string,
  maxCandidates: number,
  fetchImpl: typeof fetch = fetch,
  timeoutMs = 10_000
): Promise<string[]> {
  const url = `${dataPlatformUrl.replace(/\/+$/, '')}/financial/lending/events?limit=1000&offset=0`;
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const response = await fetchImpl(url, {
      signal: controller.signal,
      headers: { accept: 'application/json' },
    });
    if (!response.ok) {
      throw new Error(`Data platform returned ${response.status}`);
    }
    const body = (await response.json()) as LendingEventsResponse;
    return accountsFromEvents(body.data, maxCandidates);
  } finally {
    clearTimeout(timer);
  }
}
