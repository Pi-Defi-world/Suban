#!/usr/bin/env bash
set -euo pipefail

CONF="${STELLAR_CORE_CONF:-/stellar-core.cfg}"

# Initialize core DB on first start (harmless if already initialized).
if [[ "${SKIP_NEW_DB:-}" != "1" ]]; then
  stellar-core new-db --conf "$CONF" 2>/dev/null || true
fi

exec stellar-core run --conf "$CONF" "$@"
