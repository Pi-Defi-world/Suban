/**
 * Minimal Horizon client used by the indexer.
 *
 * Only read endpoints are used. Responses are fetched from the configured
 * Horizon base URL, including pagination links, so a malicious upstream cannot
 * redirect the indexer to another host.
 */
export class HorizonError extends Error {
  status: number;

  constructor(message: string, status = 0) {
    super(message);
    this.name = 'HorizonError';
    this.status = status;
  }
}

export interface HorizonLedger {
  sequence: number;
  hash: string;
  prev_hash?: string;
  closed_at: string;
  successful_transaction_count?: number;
  failed_transaction_count?: number;
  operation_count?: number;
  tx_set_operation_count?: number;
  total_coins?: string;
  fee_pool?: string;
  base_fee_in_stroops?: number;
  base_reserve_in_stroops?: number;
  max_tx_set_size?: number;
  protocol_version?: number;
  header_xdr?: string;
}

export interface HorizonTransaction {
  hash: string;
  ledger: number;
  successful: boolean;
  source_account: string;
  source_account_sequence?: string;
  fee_account?: string;
  fee_charged?: string;
  max_fee?: string;
  operation_count?: number;
  created_at: string;
  memo_type?: string;
  memo?: string;
  envelope_xdr?: string;
  result_xdr?: string;
  result_meta_xdr?: string;
}

export interface HorizonOperation {
  id: string;
  paging_token?: string;
  transaction_hash: string;
  source_account?: string;
  type: string;
  type_i?: number;
  created_at: string;
  transaction_successful?: boolean;
  [key: string]: unknown;
}

export interface HorizonAccount {
  account_id: string;
  sequence?: string;
  sequence_ledger?: number;
  last_modified_ledger?: number;
  subentry_count?: number;
  balances?: unknown[];
  signers?: unknown[];
  thresholds?: unknown;
  flags?: unknown;
  home_domain?: string;
  num_sponsoring?: number;
  num_sponsored?: number;
}

interface HorizonCollection<T> {
  _embedded?: { records?: T[] };
  _links?: { next?: { href?: string } };
}

export class HorizonClient {
  private readonly baseUrl: string;

  constructor(
    baseUrl: string,
    private readonly fetchImpl: typeof fetch = fetch,
    private readonly timeoutMs = 15_000
  ) {
    this.baseUrl = baseUrl.replace(/\/+$/, '');
  }

  private async request(pathOrUrl: string, attempt = 0): Promise<any> {
    const url = pathOrUrl.startsWith('http')
      ? new URL(new URL(pathOrUrl).pathname + new URL(pathOrUrl).search, this.baseUrl)
      : new URL(pathOrUrl, `${this.baseUrl}/`);

    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), this.timeoutMs);
    try {
      const response = await this.fetchImpl(url.toString(), {
        signal: controller.signal,
        headers: { accept: 'application/json' },
      });
      if (!response.ok) {
        const body = await response.text().catch(() => '');
        if (
          attempt < 3 &&
          (response.status === 429 || response.status === 502 || response.status === 503 || response.status === 504)
        ) {
          const retryAfter = response.headers.get('retry-after');
          const retrySeconds = retryAfter ? parseInt(retryAfter, 10) : NaN;
          const delayMs = Number.isFinite(retrySeconds)
            ? retrySeconds * 1000
            : 250 * 2 ** attempt;
          await new Promise((resolve) => setTimeout(resolve, Math.min(delayMs, 10_000)));
          return this.request(pathOrUrl, attempt + 1);
        }
        throw new HorizonError(
          `Horizon ${response.status} for ${url.pathname}${body ? `: ${body.slice(0, 200)}` : ''}`,
          response.status
        );
      }
      return await response.json();
    } catch (error: any) {
      if (error instanceof HorizonError) throw error;
      if (error?.name === 'AbortError') {
        throw new HorizonError(`Horizon request timed out: ${url.pathname}`);
      }
      throw new HorizonError(`Horizon request failed: ${error?.message || error}`);
    } finally {
      clearTimeout(timer);
    }
  }

  private async collection<T>(path: string, maxPages = 50): Promise<T[]> {
    const records: T[] = [];
    let next: string | null = path;
    let pages = 0;

    while (next && pages < maxPages) {
      const page = (await this.request(next)) as HorizonCollection<T>;
      records.push(...(page._embedded?.records || []));
      next = page._links?.next?.href || null;
      pages++;
    }
    return records;
  }

  async latestLedger(): Promise<HorizonLedger> {
    const page = (await this.request('/ledgers?order=desc&limit=1')) as HorizonCollection<HorizonLedger>;
    const ledger = page._embedded?.records?.[0];
    if (!ledger) throw new HorizonError('Horizon returned no ledgers');
    return ledger;
  }

  async ledgersFrom(sequence: number, limit: number): Promise<HorizonLedger[]> {
    const cursor = (BigInt(sequence) * 4_294_967_296n).toString();
    return this.collection<HorizonLedger>(
      `/ledgers?order=asc&limit=${Math.max(1, Math.min(limit, 200))}&cursor=${cursor}`
    );
  }

  async transactionsForLedger(sequence: number): Promise<HorizonTransaction[]> {
    return this.collection<HorizonTransaction>(
      `/ledgers/${sequence}/transactions?order=asc&limit=200`
    );
  }

  async operationsForLedger(sequence: number): Promise<HorizonOperation[]> {
    return this.collection<HorizonOperation>(
      `/ledgers/${sequence}/operations?order=asc&limit=200`
    );
  }

  async operationsForTransaction(hash: string): Promise<HorizonOperation[]> {
    return this.collection<HorizonOperation>(
      `/transactions/${encodeURIComponent(hash)}/operations?order=asc&limit=200`
    );
  }

  async account(accountId: string): Promise<HorizonAccount> {
    return (await this.request(`/accounts/${encodeURIComponent(accountId)}`)) as HorizonAccount;
  }
}
