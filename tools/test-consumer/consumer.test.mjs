import assert from 'node:assert/strict';
import test from 'node:test';
import { CONSUMER_ID, runConsumer } from './consumer.mjs';

function json(body, status = 200) {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'content-type': 'application/json' },
  });
}

function healthyMock() {
  const seenHeaders = [];
  const fetchImpl = async (url, options = {}) => {
    seenHeaders.push({ url: String(url), headers: options.headers || {} });
    const text = String(url);
    if (text.includes('/api/v1/price')) {
      return json({
        asset: 'PI',
        price_usd: 0.31,
        confidence_score: 0.91,
        is_fresh: true,
        age_ms: 2400,
        trust: { state: 'HEALTHY' },
        aggregation_method: 'weighted_average_v1',
      });
    }
    if (text.includes('/api/v1/statistics')) {
      return json({
        method_version: 'twap_volatility_v1',
        statistics: {
          twap_usd: 0.3,
          annualized_volatility: 0.2,
          sample_count: 100,
          coverage_ratio: 1,
          warnings: [],
        },
      });
    }
    if (text.includes('/api/v1/risk/lending')) {
      return json({
        method_version: 'lending_risk_v1',
        data_quality: { status: 'ok' },
        risk: {
          status: 'healthy',
          health_factor: 1.6,
          collateral_value_usd: 31,
          debt_value_usd: 5,
        },
      });
    }
    if (text.includes('/financial/summary')) {
      return json({
        counts: { swap: 4, add_liquidity: 2, remove_liquidity: 1 },
        coverage: { usd_volume: false, lp_positions: true },
        last_event_at: '2026-10-21T00:00:00.000Z',
      });
    }
    if (text.includes('/financial/pools')) {
      return json({
        data: [{ contract_id: 'CPOOL', pool_kind: 'cpmm' }],
        coverage: { usd_volume: false, lp_positions: true },
      });
    }
    return json({ error: 'unexpected route' }, 404);
  };
  return { fetchImpl, seenHeaders };
}

test('a healthy test user sees provenance, risk and financial coverage', async () => {
  const { fetchImpl, seenHeaders } = healthyMock();
  const report = await runConsumer({
    controllerUrl: 'http://controller',
    dataPlatformUrl: 'http://data',
    fetchImpl,
    now: (() => {
      const first = new Date('2026-10-21T00:00:00.000Z');
      let call = 0;
      return () => new Date(first.getTime() + (call += 25));
    })(),
  });

  assert.equal(report.status, 'ok');
  assert.equal(report.consumer.id, CONSUMER_ID);
  assert.equal(report.facts.price.confidence_score, 0.91);
  assert.equal(report.facts.price.is_fresh, true);
  assert.equal(report.facts.risk.health_factor, 1.6);
  assert.equal(report.facts.financial.coverage.lp_positions, true);
  assert.equal(report.facts.financial.coverage.usd_volume, false);
  assert.equal(report.checks.length, 5);
  assert.ok(report.checks.every((entry) => entry.ok));
  assert.ok(
    seenHeaders.every((entry) => entry.headers['x-suban-consumer'] === CONSUMER_ID)
  );
});

test('missing history and unavailable prices degrade without invented values', async () => {
  const { fetchImpl } = healthyMock();
  const degradedFetch = async (url, options = {}) => {
    const text = String(url);
    if (text.includes('/api/v1/statistics')) {
      return json({ error: 'Insufficient price history', method_version: 'twap_volatility_v1' }, 503);
    }
    if (text.includes('/api/v1/risk/lending')) {
      return json({ error: 'Price is stale', asset: 'PI' }, 503);
    }
    return fetchImpl(url, options);
  };

  const report = await runConsumer({
    fetchImpl: degradedFetch,
    controllerUrl: 'http://controller',
    dataPlatformUrl: 'http://data',
  });

  assert.equal(report.status, 'degraded');
  assert.equal(report.facts.statistics.available, false);
  assert.equal(report.facts.risk.available, false);
  assert.ok(report.warnings.includes('statistics unavailable'));
  assert.ok(report.warnings.includes('risk unavailable'));
  assert.equal(report.facts.price.price_usd, 0.31);
});

test('a malformed price response fails the consumer instead of being accepted', async () => {
  const { fetchImpl } = healthyMock();
  const malformedFetch = async (url, options = {}) => {
    if (String(url).includes('/api/v1/price')) {
      return json({ asset: 'PI', price_usd: 0.31, is_fresh: true });
    }
    return fetchImpl(url, options);
  };

  const report = await runConsumer({
    fetchImpl: malformedFetch,
    controllerUrl: 'http://controller',
    dataPlatformUrl: 'http://data',
  });

  assert.equal(report.status, 'failed');
  const priceCheck = report.checks.find((entry) => entry.name === 'price.provenance');
  assert.equal(priceCheck.ok, false);
  assert.match(priceCheck.detail, /missing provenance/);
});
