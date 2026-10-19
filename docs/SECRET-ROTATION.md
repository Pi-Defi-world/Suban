# Secret Rotation and Git History Purge

**Status:** required before any public launch or mainnet deployment.

Secrets were committed to this repository (and copied into deploy configs and
docs). Removing them from the current tree does **not** remove them from git
history, existing clones, forks, CI caches, or any machine that fetched the
repo. Treat every secret listed here as compromised.

## 1. Rotate first

Rotation must happen before (or independently of) the history rewrite. A
rewrite does not make a leaked key safe.

| Secret | Where it was exposed | Rotation |
|---|---|---|
| Stellar deployer secret | `docs/PROJECT-REPORT.md`, deploy scripts | Generate a new keypair, fund it, update the deploy environment, retire the old account. |
| PUSD admin / oracle admin secret | `docs/PROJECT-REPORT.md`, bridge and controller configs | Rotate the admin on-chain (or redeploy), then update `ORACLE_ADMIN_SECRET` and the PUSD admin secret. |
| Arc relayer private key | `contracts/arc-bridge/.env` (history), docs | Generate a new relayer key, update the Arc bridge allowlist, drain the old address. |
| Pi node seeds (`NODE_SEED`) | deploy core configs | Generate new node keypairs, update `pi-core.cfg` / `suban-core*.cfg`, restart nodes, update peers. |
| Cloudflare tunnel token | deploy configs | Revoke the tunnel token in Cloudflare Zero Trust and create a new tunnel/token. |
| Faucet secret (`FAUCET_SECRET`) | `fund-deployer.*` | Rotate the faucet account key and update the secret store. |

Store new values only in a secret manager or the CI secret store. Never put
them in a tracked file. `.env` files are gitignored; confirm with
`git check-ignore -v <file>` before adding one.

After rotation, update the redacted placeholders in `docs/PROJECT-REPORT.md`
and the deploy examples so operators know which variable names to set.

## 2. Purge history

> **Destructive.** This rewrites `main` and requires a force-push. It breaks
> existing clones and open pull requests. Coordinate with every collaborator.

1. Freeze merges and announce the window.
2. Back up the repository (`git clone --mirror`).
3. Install [`git-filter-repo`](https://github.com/newren/git-filter-repo).
4. Prefer `--replace-text` when the file should remain but the value must go:
   ```
   # replacements.txt, one rule per line: literal:==>REDACTED
   git filter-repo --replace-text replacements.txt
   ```
   Use `--path <file> --invert-paths` only when the whole file should be removed.
5. Force-push all branches and tags (`git push --force --all` and `--tags`).
6. Ask every collaborator to **re-clone**; do not pull.
7. Invalidate forks and CI caches, and rotate any credential that was ever
   stored in CI.
8. Verify with a full-history scan:
   ```
   docker run --rm -v "$PWD:/repo" -w /repo ghcr.io/gitleaks/gitleaks:latest \
     detect --source=/repo --config=/repo/.gitleaks.toml --redact --verbose
   ```
   It must report no leaks.

## 3. Preventing recurrence

- CI scans the working tree on every push and fails on findings
  (`.github/workflows/ci.yml`). The history scan is report-only until the purge
  is complete, then it should be made blocking.
- Add a pre-commit hook if contributors work outside CI:
  `gitleaks protect --staged --config .gitleaks.toml`.
- Never paste secrets into issues, docs, screenshots, chat, or logs.
- Prefer short-lived credentials and per-environment keys.

## 4. Known repository-hygiene issue

`suban-rpc` currently tracks two build artifacts:

- `pi-rpc` (~98 MB)
- `db.test` (~59 MB)

They bloat every clone and CI checkout. Remove them from git, add them to
`.gitignore`, and include them in the same history rewrite.
