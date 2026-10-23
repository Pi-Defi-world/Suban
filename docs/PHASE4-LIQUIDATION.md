# Phase 4.4–4.6 — Vault and Liquidation Testnet Evidence

Status: **testnet prototype only (23 October 2026)**. No mainnet deployment,
audit, production keeper or capital allocation is claimed. The contracts and
bot are exerciseable on Pi testnet; they are not approved for real funds.

## Deployed contracts

| Component | Contract ID | Notes |
|---|---|---|
| Oracle | `CATCIFIYSSMIH7AHZXFXTRPIB2MAKZQCJRZNRP3G4ULOQBBAPIW2SL32` | Deployer admin; max price age `10000`; breaker `500` bps |
| Lending pool | `CAKDET6TED6DQWLIPB6ZB2URIVDTFOZKL62ALN5DDUTDN7LDELEKGKRC` | Oracle-aware; PUSD debt, wPi collateral |
| Strategy vault | `CD4RHSQM5IWBIXO6MEN6U4DHEANH6HFSD5FETB3HBMSLGLPXLLW6VMDP` | PUSD; 1,000 PUSD deposit cap |
| CPMM pool | `CBN77D4T6EPPVHLCNU2XCAEWMRPGTDH67RSGVKKJSIPHT3W23KB6QNCM` | Enriched events; 1,000/1,000 initial liquidity |
| Stableswap pool | `CD4HZJPLPHMKHMHBZ5VTP4VTISJGXIP7CMCEUG2GZOHCR2EQK5DGM6MC` | Enriched events |
| wPi token | `CCODZXYOZMKBOKCCMOKSVW3FIGPJRILM7GH4MKFOS4IWZ56EBX5MVONV` | 7 decimals |
| PUSD token | `CAPDFYOFXSQTVCZ7KPUACHVNMOO3TWLSLHTPQ3H64EBJVRFMAWIBEUAY` | 7 decimals |

The earlier enriched lending deployment `CCTSJAKLI42YMDXUI6BGWJTDDB27LCSLBUR4XWB7R5LKVXIAG7C5HPPK`
was superseded by the oracle-aware `CAKDET...` contract. Do not use the older
address for new tests.

## 4.5 — Oracle-aware lending

Implemented in `contracts/lending-pool`:

- Distinct collateral and debt tokens (`set_collateral_token`) instead of
  assuming one asset for both sides.
- Optional price oracle (`set_price_oracle`) and explicit
  `deposit_collateral` / `withdraw_collateral` paths.
- Health factor remains in basis points with a `10_000` liquidation threshold.
- New errors: `OracleUnavailable = 11`, `InvalidOraclePrice = 12`.
- `read_oracle_price` decodes `Result<Price, HubError>` and rejects missing,
  stale or invalid prices.
- Liquidation repays debt and seizes collateral, capped at the borrower's
  available collateral.

### Testnet exercise

1. Initialized the oracle with the deployer as admin, max age `10000` and
   breaker `500`.
2. Set wPi at `2.0` and PUSD at `1.0` (7 decimals).
3. Deposited `1,000` PUSD, deposited `100` wPi collateral and borrowed `100`
   PUSD. Health factor: `17000` bps.
4. Changed wPi to `0.5`. Health factor: `4250` bps.
5. The liquidation bot detected the position as liquidatable and executed
   testnet transaction
   `ee6ae5c8c0a74ef9e3c0a1fae229bde61f75c26fe7156a707fbacbd51e5d4a36`:
   - repay `952380952` raw PUSD,
   - estimated collateral `999999999` raw wPi,
   - minimum collateral `989999999` raw wPi.
6. Post-liquidation position: `borrow=47619048`, `collateral=1`,
   `deposit=10000000000`, health `0` bps.

### Known dust limitation

The contract caps seizure at the borrower's collateral. After the liquidation,
one raw unit of wPi collateral and `47,619,048` raw PUSD debt remain. The
position is economically dust but still reports `liquidatable` through the
indexed risk route because the health factor is effectively zero. A future
version should define a bad-debt/dust write-off or minimum-position policy.
Until then, do not describe liquidation as fully clearing every position.

## 4.4 — Strategy vault

Implemented in `contracts/strategy-vault`:

- Single-strategy framework with an explicit deposit cap.
- Emergency pause blocks deposits but permits withdrawals.
- Strategy isolation: a non-empty vault cannot switch strategies; a second
  `set_strategy` call fails with `Error(Contract, #6)`.
- Admin-only cap and pause controls; non-admin controls are tested.
- Share accounting and zero/insufficient-share tests.

### Testnet exercise

Using PUSD and a `1,000` PUSD cap:

- Deposited `10` PUSD.
- Withdrew `5` PUSD.
- Paused the vault; a deposit failed with `Error(Contract, #2)`.
- Withdrawal while paused succeeded.
- Unpaused and set the strategy.
- A second strategy change failed with `Error(Contract, #6)`.

The vault does **not** allocate capital to a live strategy. It is a testnet
framework and must not be described as a yield product.

## 4.6 — Liquidation bot prototype

`services/liquidation-bot` is a testnet-only TypeScript prototype:

- Dry-run is the default; `LIQUIDATION_MODE=execute` is required to sign.
- Keys come only from the process environment; the repository contains no
  tracked secret. Local values live in the gitignored
  `services/liquidation-bot/.env.local`.
- Candidate discovery from the data platform is an address hint only. Health
  and positions are re-read from the contract before a decision.
- Repayment is bounded by the borrower's collateral:
  `collateralRaw * 10000 / (10000 + bonusBps)`.
- `LIQUIDATION_MAX_REPAY_RAW`, `LIQUIDATION_MIN_HEALTH_BPS`,
  `LIQUIDATION_SLIPPAGE_BPS` and `LIQUIDATION_MAX_CANDIDATES` bound exposure.
- No flash loan or capital source; the liquidator must already hold the debt
  asset.

Verification: 18 Vitest tests pass on Node 22; typecheck and build pass. A live
dry run detected the health-`4250` position, and the execute run produced the
transaction above. The bot is not a production keeper, has no uptime guarantee,
and has no mainnet authorization.

## Limits

- No independent audit and no bug bounty.
- No mainnet deployment or mainnet capital.
- No vault capital allocation.
- No production keeper, alerting, or incident automation.
- The liquidation dust/bad-debt policy is unresolved.
- The oracle price is testnet-only and was set by the deployer; it is not a
  production price feed.
- The bot's key is a testnet key and must be rotated if it was ever used
  anywhere else.

## Reproduction

The contract tests can be run without the deployed network:

```bash
docker run --rm --user root --entrypoint cargo \
  -v "$PWD:/work" -w /work \
  -e CARGO_TARGET_DIR=/tmp/target \
  stellar/stellar-cli:latest \
  test -p cpmm-pool -p stableswap -p lending-pool -p strategy-vault
```

The bot checks:

```bash
docker run --rm -v "$PWD:/work" -w /work/services/liquidation-bot \
  node:22-bookworm-slim \
  sh -c "npm ci --no-audit --no-fund && npm run typecheck && npm test && npm run build"
```

Live testnet commands require the local env files and must remain dry-run
unless a reviewed testnet liquidation is intended.
