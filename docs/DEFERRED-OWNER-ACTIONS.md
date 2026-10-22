# Deferred Owner Actions

Status: tracked follow-ups, October 2026.

These items are intentionally **not** blocking Phase 2. They either require
owner credentials/approval, a production decision, or a separate engineering
workstream. They are recorded here so they are not lost or silently claimed as
done. Do not move an item to "done" based only on code being present.

## 1. Rotate leaked secrets

**Owner:** repository owner / deployer key holders.

**Why deferred:** requires access to the live Stellar deployer, PUSD/oracle
admin, Arc relayer, Pi node seeds, Cloudflare tunnel and faucet credentials. It
cannot be completed by a code change, and it must be coordinated with a git
history purge.

**Do later:**

1. Follow [`SECRET-ROTATION.md`](./SECRET-ROTATION.md).
2. Revoke the old keys after the new keys are verified.
3. Confirm gitleaks working-tree scans remain clean on fresh checkouts.
4. Do not print or commit any new secret while rotating.

**Current state:** secrets are removed from the current tree; ignored local
`.env` files may still contain live values. Working-tree gitleaks is blocking;
history scanning is report-only.

## 2. Purge git history

**Owner:** repository owner, with force-push approval from all collaborators.

**Why deferred:** destructive force-push and coordination across four repos.
The history contains leaked values and large artifacts.

**Do later:**

1. Rotate secrets first (item 1).
2. Remove `suban-rpc` `pi-rpc` (~98 MB) and `db.test` (~59 MB) from history.
3. Purge the leaked secret blobs from `Suban` history.
4. Force-push all affected branches and require collaborators to re-clone.
5. Flip the gitleaks history scan from report-only to blocking once clean.

**Current state:** working-tree scans are clean on fresh checkouts; history
scanning is report-only and still finds findings.

## 3. On-chain oracle-push integration test

**Owner:** controller/oracle maintainer.

**Why deferred:** needs a funded testnet admin account and a deployed oracle
contract; unit tests and the Soroban `prepare`/`sign`/`send`/`poll` path are
implemented, but no end-to-end transaction is asserted yet.

**Do later:** add a testnet integration test that asserts a transaction hash,
reads back the stored price/confidence, and covers a failed push without moving
the circuit-breaker reference. Until then, docs must not claim production
on-chain oracle readiness.

**Current state:** publisher is opt-in (`ORACLE_PUSH_ENABLED=false` by default)
and refuses non-`HEALTHY` observations.

## 4. Name accountable maintainers

**Owner:** project lead.

**Why deferred:** ownership roles and escalation paths exist, but named humans
are still `TBD` in `docs/OWNERSHIP.md` and `CODEOWNERS`.

**Do later:** assign one named maintainer per module, add them to `CODEOWNERS`,
and record the deployment target and escalation path. Do not claim the
ownership matrix is complete until this is done.

**Current state:** role-level ownership is documented; named ownership is open.

## 5. Triage full/dev dependency audits

**Owner:** maintainers of each repo.

**Why deferred:** production dependency audits are blocking and clean, but
full/dev trees can still report advisories that do not affect runtime. Fixing
them requires per-repo upgrades and compatibility testing.

**Do later:** triage the report-only full/dev findings in each repo, upgrade or
document each exception, then decide whether the check can be made blocking.
Do not suppress findings without a written reason.

**Current state:** production Node audits are blocking; full/dev audits are
report-only; Rust audit reports no vulnerabilities but has unmaintained-crate
warnings.

## 6. Pi RPC preflight host upgrade

**Owner:** `suban-rpc` maintainer.

**Why deferred:** the `internal/preflight` tests target a protocol-25 fixture
while the pinned `soroban-env-host` is version 23. The Go `getLedgerEntries`
runtime path is supported; only the Rust preflight test path is excluded from
the CI gate.

**Do later:** upgrade the host/dependency set, re-enable the preflight tests in
the blocking gate, and remove the documented exception in
`suban-rpc/COMPATIBILITY.md`.

**Current state:** package compiles under `go vet`; preflight tests are excluded
from the gate, not deleted.

## 7. Arc → Stellar mint signer authorization

**Owner:** bridge maintainer.

**Why deferred:** the Arc→Stellar relayer mint path is deliberately disabled
until signer authorization entries are implemented and tested. The Stellar
`mint_pusd` path already requires per-signer `require_auth()` and rejects
duplicate/non-signer entries.

**Do later:** implement signer authorization entries for the relayer, add an
end-to-end test, then re-enable only after a security review. Do not re-enable
the relayer as part of a docs or metrics change.

**Current state:** disabled; documented in `docs/SECURITY.md`.

## 8. Public data-platform deployment

**Owner:** operator / infrastructure owner.

**Why deferred:** Phase 2 delivers the data platform implementation and API
contract. DNS, TLS, public rate-limit policy, a production-sized Horizon
backfill and capacity testing are deployment decisions.

**Do later:**

1. Choose the public hostname (target shown in docs as `data.suban.org`; not yet
   provisioned).
2. Configure `INDEXER_REQUIRE_API_KEY=true` and issue keys with quotas.
3. Put the service behind TLS and a private scrape network for `/metrics`.
4. Run a deliberate backfill using `INDEXER_HORIZON_BOOTSTRAP_LEDGERS` and a
   bounded `INDEXER_MAX_LEDGERS_PER_POLL`.
5. Verify freshness, webhook delivery and quota alerts under load.

**Current state:** local and Docker smoke tests pass; public deployment is not
claimed.

## 9. Deploy enriched financial events and find a production consumer

**Owner:** contract deployer/operator plus product/consumer owner.

**Why deferred:** the Phase 3 projection and risk layers are implemented in
code, but the existing testnet contracts must be redeployed and reindexed before
the enriched payloads are visible. An internal test user exercises the APIs, but
no production application consumes them with measured usage.

**Do later:**

1. Deploy the updated CPMM, stableswap and lending-pool contracts with the
   enriched event payloads.
2. Reindex the deployment window and verify `/financial/pools/:id` returns
   token/reserve/share state, `/financial/pools/:id/lp-positions` derives LP
   shares, and `/financial/lending/accounts/:id` returns position snapshots.
3. If USD valuation is required, configure `INDEXER_PRICE_API_URL`,
   `INDEXER_TOKEN_ASSETS` and `INDEXER_TOKEN_DECIMALS`; verify stale or
   non-HEALTHY prices return `503` instead of a USD value.
4. Configure `DATA_PLATFORM_URL` on the controller and verify
   `/risk/lending/indexed/:account` labels its ledger and source.
5. Replace the internal test user with a production consumer that displays
   freshness/confidence and records real usage.
6. Only then evaluate the Phase 3 exit criterion and the Phase 4 gate.

**Current state:** `tools/test-consumer/consumer.mjs` exercises the APIs and
records freshness/confidence/coverage; it is not production adoption. The
contract and indexer code now supports enriched pool/lending events, derived LP
positions, opt-in USD valuation and indexed risk reads, but live deployment and
production adoption are not claimed. Phase 3 is **not complete**. Phase 4 is
limited to CPMM quote/simulation previews and CPMM route discovery under an
explicit gate override; deployed `get_quote` parity was verified against the
existing Pi testnet pool on 23 October 2026, but the enriched contracts are not
deployed and no execution or capital work exists.

---

## Recording rule

When an item is completed, update this file **and** the relevant canonical doc
(`SECURITY.md`, `OWNERSHIP.md`, `OBSERVABILITY.md`, `DATA-PLATFORM.md`, or
`PLAN-EXECUTION.md`). Do not mark a Phase complete while a deferred item is
described as if it were finished.
