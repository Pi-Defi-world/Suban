# Suban Security

Status: canonical security model (Phase 2-4, October 2026). For the current build and deployment inventory, see [PROJECT-STATUS.md](./PROJECT-STATUS.md).

Suban handles market data, chain access and value-movement code. This document
states the threat model, the controls that exist, and the gaps that are still
open. It is deliberately conservative: a capability is not "secure" because it
compiles.

> Suban is currently **testnet-first**. No mainnet capital product should be
> considered safe until the gates in `ROADMAP.md` and the audit requirements in
> `AUDIT-PROGRAM.md` are met.

## 1. Reporting a vulnerability

Do **not** open a public issue for a vulnerability that could move funds, leak
keys or forge oracle data.

Use GitHub's private **Security Advisories** feature on the affected repository
(`Pi-Defi-world/Suban`, `Pi-Defi-world/suban-rpc`,
`Pi-Defi-world/Suban-api`, `Pi-Defi-world/suban-controller`). Include:

- affected repository, commit and deployment;
- reproduction steps or a proof of concept;
- impact (funds, oracle integrity, availability, data);
- whether the issue is already public.

If the advisory feature is unavailable, contact the repository maintainers
through the private channel listed in the repository profile rather than a public
issue. Do not test against third-party funds or accounts.

## 2. Scope and security objectives

| Asset | Objective |
|---|---|
| Oracle prices | Integrity: no fabricated, stale or single-source price reaches consumers or the chain |
| Chain data | Correctness and freshness; a derived view must be replayable |
| Bridge value | No mint without authorized burn and required signatures |
| Privileged keys | Confidentiality, least privilege, rotation, isolated signing |
| Service availability | Public read APIs stay up; control plane stays private |
| Historical data | Integrity and retention; observations are append-only |
| User/operator trust | No secret in source, logs, images or public docs |

## 3. Trust boundaries

```text
Internet ──► public read APIs (controller, API, RPC, indexer)
                  │  rate limits, validation, no writes
                  ▼
            control plane (controller admin, oracle signer, relayer)
                  │  shared secret / signing keys / isolated network
                  ▼
            Pi Horizon / Soroban RPC / Arc RPC
```

- Public reads and privileged operations are logically separated by
  authentication and must be separated by network policy in deployment.
- The controller admin key, oracle admin secret and relayer keys never belong in
  a frontend, public API process or container image.
- The relayer is a worker, not a public API. Its only listener is the
  metrics/health port.

## 4. Threat model and controls

| Threat | Impact | Controls | Status |
|---|---|---|---|
| Malicious or compromised price source | Wrong oracle price | Multiple sources, weights, outlier filter, freshness, confidence, per-asset breaker | Implemented |
| Source outage or stale data | Silent degradation | Per-source age/status, trust state, `503` for unconfigured assets | Implemented |
| Single bad observation | Bad on-chain price | Only `HEALTHY` observations are pushed; confidence threshold; breaker | Implemented |
| Forged bridge signer list | Unauthorized mint | Every signer must be configured, unique and `require_auth()`; tests cover non-signer/duplicate/replay | Implemented |
| Arc→Stellar mint abuse | Unauthorized mint | Path disabled until signer authorization entries exist | Mitigated by disablement |
| Unauthenticated control route | Unauthorized price/breaker change | `CONTROLLER_ADMIN_API_KEY`, constant-time compare, per-IP limit, `503` if unset | Implemented |
| Leaked secrets in git | Key compromise | Secrets removed from tree, gitleaks working-tree gate, rotation runbook | Partial: rotation/history purge pending |
| Key compromise | Funds/oracle control | Isolated env, rotation, least privilege; multisig/HSM is a target | Partial |
| RPC/Horizon abuse | DoS, quota exhaustion | Rate limits, caching, upstream timeouts | Partial |
| Supply-chain compromise | Malicious dependency/build | Lockfiles, pinned `ed25519-dalek`, `--locked`, `npm ci`, CI build | Partial: production Node audits block high/critical; Rust audit is enabled; dev-only findings are triaged separately |
| Indexer data loss/corruption | Wrong analytics | SQLite checkpoints, replayable derived view, backups | Partial: backup drill not documented |
| Public data API key abuse | Quota exhaustion, cost/DoS | Per-key minute/day quotas, hashed keys, `429` + `Retry-After`, usage endpoint | Implemented |
| Webhook spoofing or replay | False event delivery | HMAC-SHA256 signature over raw body, stable `Idempotency-Key`, retries | Implemented; secret storage is DB-protected |
| Stale indexed data presented as live | Wrong analytics/decisions | `freshness` on every response, `/health` `503` when stale, lag alerts | Implemented |
| Public data admin route exposed | Contract/webhook tampering | `INDEXER_ADMIN_API_KEY`, constant-time compare, `503` if unset, private network | Implemented |
| Oracle push failure | Stale on-chain price | Push only after success; failed push does not move breaker reference; metrics | Implemented |
| Pause-registry not wired | Inability to stop contracts | Pause registry exists but is not called by value-moving contracts | **Open gap** |
| Contract bug | Loss of funds | Tests, workspace CI, audit program; Phase 4.7 prep package | **Open gap: no independent audit** |

## 5. Implemented controls (evidence)

### 5.1 Controller

- Control routes require `CONTROLLER_ADMIN_API_KEY` via `x-api-key` or
  `Authorization: Bearer`; comparison is constant-time; unset key returns `503`,
  never falls open; per-IP rate limiting.
- Oracle publication is opt-in (`ORACLE_PUSH_ENABLED=false` by default) and
  requires `ORACLE_CONTRACT_ID`, `ORACLE_ASSET*`, `ORACLE_ADMIN_SECRET`.
- Only `HEALTHY` observations are published. Confidence is sent as a 0–100
  integer, matching the contract.
- Every price carries `schema_version`, `method_version`, `age_ms`,
  `is_fresh`, source provenance and a trust snapshot.
- Per-asset circuit breakers prevent a single asset's failure from affecting
  another.
- SQLite persistence is fail-closed at startup if the store cannot be opened.

### 5.2 Bridge contracts and relayer

- `mint_pusd` does not trust a caller-supplied signature list. Every listed
  signer must be configured, unique and pass Soroban `require_auth()`.
- Threshold, non-signer, duplicate, replay and missing-authorization cases are
  covered by tests.
- Arc→Stellar mint is explicitly disabled until signer authorization entries are
  implemented; the relayer cannot silently enable it.
- The relayer's only network listener is `/metrics` + `/health` on `9464`.

### 5.3 Data platform (`event-indexer`)

- Public data routes support per-key quotas; keys are stored as SHA-256 hashes,
  usage is persisted per key, and exhausted keys receive `429` with
  `Retry-After: 60`.
- Admin routes require `INDEXER_ADMIN_API_KEY`; unset returns `503`, never falls
  open. Admin routes are intended for a private network.
- Indexed queries read SQLite only. They never proxy live Horizon/RPC calls, so
  a `404` means "not indexed", not "not on chain".
- Every data response carries `freshness`; `/health` returns `503` when stale.
- Webhooks use a durable outbox, unique `(subscription, event)` delivery,
  HMAC-SHA256 signatures, a stable idempotency key and bounded exponential
  backoff. Secret values are omitted from list responses.
- The SQLite database contains webhook signing secrets and API-key usage; treat
  the file as a secret and restrict filesystem access.

### 5.4 Repositories and CI

- GitHub Actions in all four repositories: Rust workspace build/test, Go
  build/vet/test with a gated preflight package, SDK typecheck/test, service
  builds, secret scanning.
- Gitleaks scans the working tree as a blocking check and full history as a
  report-only check.
- `suban-rpc` pins `ed25519-dalek = "=2.2.0"`, commits `Cargo.lock` and builds
  with `--locked` to prevent an unreviewed dependency graph.
- Production Node dependencies are audited on every CI run (`npm audit
  --omit=dev --audit-level=high`, `pnpm audit --prod --audit-level=high`), and
  Rust dependencies use `rustsec/audit-check`. Full/dev trees are reported, not
  silently ignored. The controller, API, SDK, indexer, relayer and escrow viewer
  are clean at production level; the controller was upgraded to
  `@stellar/stellar-sdk@17.2.1` with patched `axios`, `proxy-addr`, `form-data`
  and `path-to-regexp`.
- Controller uses Node 22+ for `better-sqlite3` prebuilt compatibility.

### 5.5 Operations

- Caddy/Cloudflare provide TLS at the edge; internal container traffic is HTTP
  and must remain on private networks.
- Incident severity, contacts and procedures: `INCIDENT-RUNBOOK.md`.
- Secret rotation and history purge: `SECRET-ROTATION.md`.
- Metrics and dashboards: `OBSERVABILITY.md`.

## 6. Open gaps and accepted risks

| Gap | Risk | Required action | Owner |
|---|---|---|---|
| Secret rotation not executed | Compromised keys remain valid | Rotate per `SECRET-ROTATION.md`; revoke old keys | Security lead / owners |
| Git history not purged | Secrets remain in clones/forks | Approve destructive rewrite, bundle large-binary removal, flip history scan to blocking | Repository owner |
| No independent audit | Contract bugs may be unknown | Send `PHASE4-AUDIT-PREP.md` to an auditor; do not deploy mainnet capital | Contracts owner |
| Pause registry unwired | Cannot globally pause value-moving contracts | Wire pause checks into contracts that need it; test | Contracts owner |
| Arc→Stellar mint disabled | Bridge direction unavailable | Implement signer authorization entries and tests | Bridge owner |
| Dependency audit pending | Known-vulnerable dependency may ship | Production Node audits and Rust audit are in CI; triage report-only full/dev findings | Platform owner |
| Data-platform public deployment not provisioned | No live public endpoint/backfill | DNS/TLS, key policy, private metrics, production backfill and load test | SRE owner |
| Webhook signing secrets in SQLite | DB read exposes webhook secrets | Restrict file permissions, backup encryption, rotate per subscription | Indexing owner |
| `suban-rpc` preflight protocol mismatch | Contract-state preflight tests not in the gate | Upgrade host to protocol 25 or keep the documented compatibility limit | RPC owner |
| Single-sig relayer keys | One key compromise moves funds | Multisig/HSM design before mainnet value | Bridge owner |
| No backup/restore drill | Data loss after disk failure | Document and test SQLite backups | SRE owner |
| Controller control plane in same process | Larger attack surface | Network isolation now; separate control listener/process later | Controller owner |
| oUSD feed unconfigured | Cannot price oUSD | Configure a reviewed `OUSD_PRICE_URL` and source validation | Oracle owner |

Accepted for now because Suban is testnet-first: internal HTTP between
containers, single-region deployment, and log-only history scanning. These are
not acceptable for mainnet capital.

## 7. Key management rules

1. One key, one purpose. Never reuse an oracle admin key as a relayer key.
2. Keys live in the deployment secret store or an isolated `.env` with strict
   permissions; never in git, images, CI logs or chat.
3. The controller admin secret is not an oracle signing secret and must not be
   distributed to API consumers.
4. Rotate on personnel change, suspected exposure or incident. Rotation is not
   complete until the old key is revoked and monitored.
5. Prefer hardware-backed or multisig signing for any mainnet value movement.
6. CI must never print secrets; use masked repository secrets.

## 8. Contract security requirements

Before any mainnet capital:

- Independent audit with published scope and remediation.
- All value-moving contracts in the root workspace and CI.
- Pause/stop path wired and tested.
- Authorization tests for every privileged entry point.
- Replay/idempotency tests for bridge and settlement paths.
- Upgrade/admin keys behind multisig or timelock.
- Deterministic risk formulas with test vectors (Phase 3).
- A staged rollout with caps and monitoring.

See `AUDIT-PROGRAM.md` for the audit scope and `PRODUCTION-ROADMAP.md` for the
bridge production requirements.

## 9. Secure development checklist

- [ ] No secret in the diff; gitleaks passes.
- [ ] New privileged route has auth, rate limit, tests and a `503` fail-closed
      path when unconfigured.
- [ ] New external data source has timeout, freshness, error handling and a
      metric.
- [ ] New financial number has source, timestamp, freshness, confidence and
      method version.
- [ ] New contract entry point has authorization and replay tests.
- [ ] New dependency is pinned/locked and reviewed.
- [ ] Logs redact keys, signatures and full request bodies containing secrets.
- [ ] Docs updated in the same change as behavior.

## 10. Incident response

Use `INCIDENT-RUNBOOK.md`. Severity definitions:

| Severity | Meaning | Target response |
|---|---|---|
| P0 — Critical | Active fund loss, key compromise, forged oracle/bridge mint | Immediate; page owners |
| P1 — High | Service down, control-plane exposure, data corruption | < 1 hour |
| P2 — Medium | Degraded price quality, indexer lag, partial outage | < 4 hours |
| P3 — Low | Minor bug or cosmetic issue | < 24 hours |

During an incident: stop the bleeding (disable push/relayer/pause), preserve
evidence, rotate exposed keys, notify owners, then write a blameless postmortem
with concrete follow-ups.

## 11. Related documents

- [ARCHITECTURE.md](./ARCHITECTURE.md) — trust boundaries and data flows.
- [DATA-PLATFORM.md](./DATA-PLATFORM.md) — indexed data security/freshness contract.
- [SECRET-ROTATION.md](./SECRET-ROTATION.md) — rotation and history purge.
- [DEFERRED-OWNER-ACTIONS.md](./DEFERRED-OWNER-ACTIONS.md) — owner-blocked follow-ups.
- [INCIDENT-RUNBOOK.md](./INCIDENT-RUNBOOK.md) — procedures and severity.
- [AUDIT-PROGRAM.md](./AUDIT-PROGRAM.md) — audit scope.
- [PRODUCTION-ROADMAP.md](./PRODUCTION-ROADMAP.md) — bridge production gates.
- [OWNERSHIP.md](./OWNERSHIP.md) — accountable owners.
- [PLAN-EXECUTION.md](../PLAN-EXECUTION.md) — open defects and progress.
