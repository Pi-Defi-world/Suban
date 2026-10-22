/**
 * Test user for the Suban financial data and oracle APIs.
 *
 * This is a deterministic internal consumer, not a production customer. It
 * exercises the public routes that a financial application would use and
 * records whether freshness, confidence and coverage metadata are visible.
 *
 * Usage:
 *   CONTROLLER_URL=http://localhost:3000 \
 *   DATA_PLATFORM_URL=http://localhost:3002 \
 *   node tools/test-consumer/consumer.mjs
 *
 * Optional:
 *   SUBAN_API_KEY=...            sent as x-api-key
 *   CONSUMER_REPORT_PATH=...     write the JSON report to this file
 *   CONSUMER_STRICT=1            exit non-zero when the run is degraded
 */
import fs from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

export const CONSUMER_ID = 'test-user-v1';
export const CONSUMER_SCHEMA_VERSION = 'consumer_report_v1';

function joinUrl(base, suffix) {
  return `${String(base).replace(/\/+$/, '')}${suffix}`;
}

function finiteNumber(value) {
  return typeof value === 'number' && Number.isFinite(value);
}

async function requestJson(fetchImpl, url, options, timeoutMs) {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const response = await fetchImpl(url, {
      ...options,
      signal: controller.signal,
      headers: {
        accept: 'application/json',
        'x-suban-consumer': CONSUMER_ID,
        ...(options?.headers || {}),
      },
    });
    const text = await response.text();
    let body = null;
    try {
      body = text ? JSON.parse(text) : null;
    } catch {
      body = { raw: text };
    }
    return { status: response.status, ok: response.ok, body };
  } finally {
    clearTimeout(timer);
  }
}

function check(name, ok, detail, status) {
  return { name, ok, status: status ?? null, detail };
}

export async function runConsumer({
  controllerUrl = 'http://localhost:3000',
  dataPlatformUrl = 'http://localhost:3002',
  apiKey = '',
  fetchImpl = globalThis.fetch,
  timeoutMs = 10_000,
  now = () => new Date(),
} = {}) {
  if (typeof fetchImpl !== 'function') {
    throw new Error('A fetch implementation is required');
  }

  const started = now();
  const checks = [];
  const warnings = [];
  const facts = {};

  const headers = apiKey ? { 'x-api-key': apiKey } : {};

  // 1. Price: a consumer must be able to see price, confidence and freshness.
  try {
    const price = await requestJson(
      fetchImpl,
      joinUrl(controllerUrl, '/api/v1/price?asset=PI'),
      { headers },
      timeoutMs
    );
    const body = price.body || {};
    const trust = body.trust || {};
    const valid =
      price.status === 200 &&
      finiteNumber(body.price_usd) &&
      body.price_usd > 0 &&
      finiteNumber(body.confidence_score) &&
      typeof body.is_fresh === 'boolean' &&
      typeof trust.state === 'string';
    checks.push(
      check(
        'price.provenance',
        valid,
        valid
          ? 'price, confidence, freshness and trust are visible'
          : 'price response is unavailable or missing provenance fields',
        price.status
      )
    );
    facts.price = {
      asset: body.asset ?? 'PI',
      price_usd: body.price_usd ?? null,
      confidence_score: body.confidence_score ?? null,
      is_fresh: body.is_fresh ?? null,
      trust_state: trust.state ?? null,
      age_ms: body.age_ms ?? null,
      aggregation_method: body.aggregation_method ?? null,
    };
    if (!valid) warnings.push('price provenance check did not pass');
  } catch (error) {
    checks.push(check('price.provenance', false, `request failed: ${error.message}`));
    warnings.push('price request failed');
  }

  // 2. TWAP/volatility: partial or missing history is reported, not invented.
  try {
    const stats = await requestJson(
      fetchImpl,
      joinUrl(controllerUrl, '/api/v1/statistics?asset=PI&window=24h'),
      { headers },
      timeoutMs
    );
    const body = stats.body || {};
    const statistics = body.statistics || {};
    if (stats.status === 200) {
      const valid =
        body.method_version === 'twap_volatility_v1' &&
        finiteNumber(statistics.twap_usd) &&
        finiteNumber(statistics.sample_count) &&
        finiteNumber(statistics.coverage_ratio);
      checks.push(
        check(
          'statistics.twap',
          valid,
          valid
            ? 'TWAP, coverage and method version are visible'
            : 'statistics response is missing required fields',
          stats.status
        )
      );
      facts.statistics = {
        method_version: body.method_version ?? null,
        twap_usd: statistics.twap_usd ?? null,
        annualized_volatility: statistics.annualized_volatility ?? null,
        sample_count: statistics.sample_count ?? null,
        coverage_ratio: statistics.coverage_ratio ?? null,
        warnings: statistics.warnings ?? [],
      };
    } else {
      checks.push(
        check(
          'statistics.twap',
          false,
          'statistics unavailable; consumer received an explicit non-200 instead of a fabricated value',
          stats.status
        )
      );
      warnings.push('statistics unavailable');
      facts.statistics = { available: false, status: stats.status, error: body.error ?? null };
    }
  } catch (error) {
    checks.push(check('statistics.twap', false, `request failed: ${error.message}`));
    warnings.push('statistics request failed');
  }

  // 3. Risk: the consumer supplies test positions; missing prices are not filled in.
  try {
    const risk = await requestJson(
      fetchImpl,
      joinUrl(controllerUrl, '/api/v1/risk/lending'),
      {
        method: 'POST',
        headers: { ...headers, 'content-type': 'application/json' },
        body: JSON.stringify({
          collateral: [{ asset: 'PI', amount: 100 }],
          debt: [{ asset: 'USDC', amount: 5 }],
        }),
      },
      timeoutMs
    );
    const body = risk.body || {};
    if (risk.status === 200) {
      const value = body.risk || {};
      const valid = typeof value.status === 'string' && finiteNumber(value.collateral_value_usd);
      checks.push(
        check(
          'risk.lending',
          valid,
          valid ? 'risk calculator returned a versioned result' : 'risk response is missing required fields',
          risk.status
        )
      );
      facts.risk = {
        method_version: body.method_version ?? null,
        status: value.status ?? null,
        health_factor: value.health_factor ?? null,
        collateral_value_usd: value.collateral_value_usd ?? null,
        debt_value_usd: value.debt_value_usd ?? null,
        data_quality: body.data_quality?.status ?? null,
      };
      if (body.data_quality?.status === 'degraded') warnings.push('risk result is degraded');
    } else {
      checks.push(
        check(
          'risk.lending',
          false,
          'risk unavailable; consumer received an explicit non-200 instead of a fabricated result',
          risk.status
        )
      );
      warnings.push('risk unavailable');
      facts.risk = { available: false, status: risk.status, error: body.error ?? null };
    }
  } catch (error) {
    checks.push(check('risk.lending', false, `request failed: ${error.message}`));
    warnings.push('risk request failed');
  }

  // 4. Financial summary and pools: coverage flags must be explicit.
  try {
    const summary = await requestJson(
      fetchImpl,
      joinUrl(dataPlatformUrl, '/financial/summary'),
      { headers },
      timeoutMs
    );
    const body = summary.body || {};
    const valid =
      summary.status === 200 &&
      body.counts &&
      finiteNumber(body.counts.swap) &&
      finiteNumber(body.counts.add_liquidity) &&
      finiteNumber(body.counts.remove_liquidity) &&
      body.coverage?.lp_positions === true &&
      body.coverage?.route_discovery === true &&
      typeof body.coverage?.usd_volume === 'boolean';
    checks.push(
      check(
        'financial.summary',
        valid,
        valid
          ? 'financial counts and explicit coverage flags are visible'
          : 'financial summary is unavailable or hides its coverage limits',
        summary.status
      )
    );
    facts.financial = {
      counts: body.counts ?? null,
      coverage: body.coverage ?? null,
      last_event_at: body.last_event_at ?? null,
    };
    if (!valid) warnings.push('financial summary check did not pass');
  } catch (error) {
    checks.push(check('financial.summary', false, `request failed: ${error.message}`));
    warnings.push('financial summary request failed');
  }

  try {
    const pools = await requestJson(
      fetchImpl,
      joinUrl(dataPlatformUrl, '/financial/pools'),
      { headers },
      timeoutMs
    );
    const body = pools.body || {};
    const valid =
      pools.status === 200 &&
      Array.isArray(body.data) &&
      typeof body.coverage?.usd_volume === 'boolean' &&
      body.coverage?.lp_positions === true;
    checks.push(
      check(
        'financial.pools',
        valid,
        valid
          ? 'pool list and explicit coverage flags are visible'
          : 'pool list is unavailable or missing coverage flags',
        pools.status
      )
    );
    if (body.data && facts.financial) {
      facts.financial.pool_count = body.data.length;
      facts.financial.first_pool = body.data[0] ?? null;
    }
  } catch (error) {
    checks.push(check('financial.pools', false, `request failed: ${error.message}`));
    warnings.push('financial pools request failed');
  }

  const failed = checks.some((entry) => !entry.ok && entry.name !== 'statistics.twap' && entry.name !== 'risk.lending');
  const degraded =
    !failed &&
    (checks.some((entry) => !entry.ok) ||
      warnings.includes('statistics unavailable') ||
      warnings.includes('risk unavailable') ||
      warnings.includes('risk result is degraded'));
  const finished = now();

  return {
    schema_version: CONSUMER_SCHEMA_VERSION,
    consumer: {
      id: CONSUMER_ID,
      kind: 'test-user',
      started_at: started.toISOString(),
      finished_at: finished.toISOString(),
      duration_ms: finished.getTime() - started.getTime(),
    },
    targets: {
      controller: controllerUrl,
      data_platform: dataPlatformUrl,
      api_key_used: Boolean(apiKey),
    },
    status: failed ? 'failed' : degraded ? 'degraded' : 'ok',
    checks,
    facts,
    warnings,
  };
}

export async function writeReport(report, targetPath) {
  if (!targetPath) return;
  const resolved = path.resolve(targetPath);
  await fs.mkdir(path.dirname(resolved), { recursive: true });
  await fs.writeFile(resolved, `${JSON.stringify(report, null, 2)}\n`, 'utf8');
}

async function main() {
  const strict = process.env.CONSUMER_STRICT === '1';
  const report = await runConsumer({
    controllerUrl: process.env.CONTROLLER_URL || 'http://localhost:3000',
    dataPlatformUrl: process.env.DATA_PLATFORM_URL || 'http://localhost:3002',
    apiKey: process.env.SUBAN_API_KEY || '',
    timeoutMs: parseInt(process.env.CONSUMER_TIMEOUT_MS || '10000', 10),
  });
  await writeReport(report, process.env.CONSUMER_REPORT_PATH);
  process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
  if (report.status === 'failed' || (strict && report.status === 'degraded')) {
    process.exitCode = 1;
  }
}

const entryPoint = process.argv[1] ? pathToFileURL(process.argv[1]).href : '';
if (import.meta.url === entryPoint) {
  main().catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
}
