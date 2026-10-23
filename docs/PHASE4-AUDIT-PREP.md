# Phase 4.7 — Audit Preparation Package

Status: **preparation complete (23 October 2026)**. No auditor has been
engaged, no audit report exists, and nothing here is a security approval. This
package is the material a prospective auditor would need to scope the work.

## 1. Purpose and non-claims

This package covers the Phase 4 testnet contracts and the supporting off-chain
services. It is not a substitute for an independent review and does not
authorize mainnet deployment, vault capital, or production liquidation.

Explicitly not claimed:

- an auditor, audit firm, or audit engagement;
- a completed audit, penetration test, or formal verification;
- mainnet readiness or mainnet capital safety;
- production adoption or production keeper operation;
- resolution of the liquidation dust/bad-debt limitation.

## 2. Scope

### In scope

| Area | Code | Why it matters |
|---|---|---|
| CPMM math and quotes | `contracts/cpmm-pool`, `sdk/src/quote.ts`, controller `quote`/`route` services | Integer rounding, fee handling, price impact and route discovery |
| Stableswap events and quotes | `contracts/stableswap` | Amplification math and enriched event payloads |
| Lending and liquidation | `contracts/lending-pool` | Oracle prices, distinct collateral/debt, health factor and seizure cap |
| Strategy vault | `contracts/strategy-vault` | Caps, pause, share accounting and strategy isolation |
| Oracle integration | `contracts/oracle`, lending `read_oracle_price`, controller price/risk APIs | Staleness, invalid prices, breaker state and provenance |
| Financial projections | `services/event-indexer/src/financial.ts`, `src/scval.ts` | Event decoding, LP/lending positions, USD valuation refusal |
| Liquidation bot | `services/liquidation-bot` | Key handling, dry-run default, repayment bound and execution path |
| Quote/risk APIs | `suban-controller/src/services/onChainQuote.ts`, `route.ts`, risk routes | Read-only simulation, route source ledgers and `503` behavior |

### Out of scope

- Bridge contracts and the Arc→Stellar mint path (deliberately disabled).
- Escrow, identity, node-staking and other non-Phase-4 primitives.
- Mainnet deployment, production infrastructure, DNS/TLS and operator runbooks.
- Vault capital allocation, yield strategies and investment performance.
- A production liquidation keeper with capital, uptime or MEV guarantees.

## 3. Threat model

| ID | Threat | Current control | Residual risk |
|---|---|---|---|
| T1 | Stale or manipulated oracle price inflates collateral or hides insolvency | Lending reads `Result<Price, HubError>`; rejects missing/stale/invalid prices; oracle has max age and breaker; controller labels non-`HEALTHY` prices degraded | Testnet oracle is deployer-controlled; no production price-source diversity or on-chain push integration test |
| T2 | Liquidation seizes more collateral than the borrower has, or leaves bad debt | Repayment is collateral-bounded in the bot; contract caps seizure; `min_collateral` applies slippage | Dust remains after seizure (`1` raw wPi and `47,619,048` raw PUSD in the test run); no write-off policy |
| T3 | Vault share inflation, strategy drain, or strategy replacement while assets are deployed | Single-strategy vault; cap; pause blocks deposits only; non-empty strategy cannot be replaced; admin controls tested | No independent review; no live strategy; no capital allocation |
| T4 | CPMM rounding or fee error creates value leakage | Shared integer formula; contract unit vector; SDK/HTTP/deployed `get_quote` parity | Point-in-time parity only; no execution-time slippage or MEV protection |
| T5 | Unauthorized admin, oracle, vault or lending actions | Soroban `require_auth`; admin checks; non-admin tests; controller control routes require an admin key | Deployer/admin keys are testnet keys; production key management is an owner action |
| T6 | Indexer fabricates state or serves stale data as fresh | Event-only projections; unenriched state is `null`; route discovery excludes stableswap/unenriched pools; freshness and source ledgers exposed; stale/missing valuation returns `503` | Indexed state can lag the chain; account scans are bounded; no production backfill/capacity test |
| T7 | Bot signs with a leaked key or executes unexpectedly | Dry-run default; env-only secret; no tracked secret; explicit execute opt-in; repayment cap; contract re-checks health | Testnet key handling; no HSM/KMS; no production incident automation |
| T8 | Event decoding misclassifies vault events as lending events | Lending projections filter `contract_name LIKE '%lending%'`; test covers a vault `deposit` event | New contract names must follow the naming convention or be added explicitly |
| T9 | Bridge mint path is re-enabled without signer authorization | Arc→Stellar mint remains disabled; Stellar `mint_pusd` requires per-signer `require_auth` | Owner/bridge maintainer must implement and review before re-enabling |

## 4. Invariants to review

1. **CPMM:** `amount_out` matches the integer contract formula; `k` does not
   decrease after fees; `get_quote` and the SDK preview agree for the same
   reserves and fee.
2. **Lending:** health factor is in basis points; liquidation is allowed only
   below `10_000`; seizure is capped at available collateral; debt and
   collateral cannot go negative.
3. **Oracle:** a price older than `max_age` or outside the configured bounds is
   rejected; an invalid price cannot be stored as a healthy observation.
4. **Vault:** deposits cannot exceed the cap; paused deposits fail; withdrawals
   remain available while paused; a non-empty vault cannot switch strategy;
   shares are not inflated by direct transfers.
5. **Indexer:** an event without enriched token/reserve/fee fields yields
   `state: null`, not guessed values; USD valuation refuses stale or
   non-`HEALTHY` prices; route discovery returns `404` rather than a synthetic
   route.
6. **Authorization:** admin/oracle/vault/lending state changes require the
   expected address; the controller control routes return `503` without an
   admin key; oracle publishing remains opt-in.
7. **Bot:** `LIQUIDATION_MODE` defaults to dry-run; execution requires an
   explicit opt-in and a secret; repayment is bounded by collateral.

## 5. Test and verification evidence

| Check | Command | Result |
|---|---|---|
| Rust contracts | `cargo test -p cpmm-pool -p stableswap -p lending-pool -p strategy-vault` | 43 passed: 10 CPMM, 10 stableswap, 15 lending, 8 vault |
| Event indexer | Node 22 Docker, `npm run typecheck && npm test` | 22 Vitest tests passed, including `scval` decoding and vault-event exclusion |
| Controller | Node 22 Docker, Jest | 56 tests passed, including quote, route and data-platform client tests |
| SDK | Node 22, `npm run typecheck && npm test` | 20 Vitest tests passed |
| Test consumer | `node --test tools/test-consumer/consumer.test.mjs` | 3 tests passed |
| Liquidation bot | Node 22 Docker, `npm run typecheck && npm test && npm run build` | 18 Vitest tests passed; typecheck and build passed |
| Working-tree secret scan | gitleaks Docker `--no-git` on tracked files | No leaks found in `Suban` or `suban-controller` tracked files |
| Deployed quote parity | `sdk/scripts/quote-parity.mjs` against enriched CPMM `CBN77...` | Both directions matched at ledger `27319628` |
| Controller route/quote | Live controller against indexer | Route `96783046`, indexed quote `96783046`, on-chain quote `96783046`, `parity.matches=true` |
| Test consumer live run | Controller + indexer | `status: ok`, 5/5 checks |
| Liquidation execution | Bot on Pi testnet | Transaction `ee6ae5c8...` confirmed; post-state recorded |

### Deployed parity detail

Enriched CPMM `CBN77D4T6EPPVHLCNU2XCAEWMRPGTDH67RSGVKKJSIPHT3W23KB6QNCM`,
reserves `10100000000 / 9901284197`, fee `30` bps, amount in `100000000`:

- wPi → PUSD: preview `96783046`, on-chain `96783046` (ledger `27319628`).
- PUSD → wPi: preview `100687090`, on-chain `100687090` (same ledger).

The controller's indexed route used ledger `27319581`; the read-only on-chain
simulation used ledger `27319714`. Both returned `96783046` and
`parity.matches=true`.

## 6. Known limitations and accepted risks

- **Dust/bad debt:** the liquidation leaves `1` raw wPi collateral and
  `47,619,048` raw PUSD debt. The position is economically dust but remains
  `liquidatable` in the indexed risk view. This is documented, not fixed.
- **Oracle:** the testnet price was set by the deployer. There is no production
  price-source diversity, no on-chain push integration test, and the breaker is
  a testnet configuration.
- **Indexed state:** route/quote and indexed risk read events, not live contract
  storage. They expose source ledgers and lag warnings, but can still lag.
- **Bounded scans:** account activity scans at most 5,000 recent financial
  events; lending-account lookup scans at most 10,000 recent lending events.
- **Vault:** single-strategy, no capital allocation, no live strategy, and no
  performance/fee model.
- **Bot:** no flash loan, no capital source, no production uptime, no MEV
  strategy, and no KMS/HSM.
- **History:** gitleaks history scanning still reports historical findings
  until the owner rotates and purges. Working-tree scans are clean on fresh
  checkouts. This is an owner action, not an audit finding.
- **No audit:** this package has not been reviewed by an independent auditor.

## 7. Auditor questions

1. Does the lending liquidation path correctly cap seizure at available
   collateral in all rounding and partial-liquidation cases?
2. Is the oracle price validation sufficient against stale, malformed, and
   boundary prices, including the `max_age` and breaker interactions?
3. Can the vault share accounting be manipulated through direct token
   transfers, strategy changes, or pause transitions?
4. Does the CPMM integer formula preserve the intended invariant for extreme
   reserves and fees, and does the route discovery ever overstate available
   liquidity?
5. Are all admin/oracle/vault/lending state changes protected by the intended
   authorization, including the controller's read-only simulation path?
6. Can the indexer's event decoding be tricked into treating a non-lending or
   unenriched event as a valid position/reserve snapshot?
7. Does the liquidation bot's dry-run/execute boundary prevent accidental
   signing when configuration is incomplete?
8. What is the recommended bad-debt/dust policy for a future lending version?

## 8. Audit readiness checklist

- [x] Scope and out-of-scope list.
- [x] Threat model with controls and residual risks.
- [x] Invariants and test evidence.
- [x] Deployment inventory and transaction evidence.
- [x] Known limitations and accepted risks.
- [x] Reproduction commands.
- [ ] Independent auditor engaged.
- [ ] Audit report and finding triage.
- [ ] Critical/high findings fixed and re-tested.
- [ ] Mainnet deployment and capital authorization.

The unchecked items are owner/auditor actions. Do not mark Phase 4 complete on
the basis of this package alone.
