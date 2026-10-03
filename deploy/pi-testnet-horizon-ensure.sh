#!/bin/sh
# Ensures Horizon is running once stellar-core reports catchup complete.
#
# The Pi Node image ships horizon with `autostart=false` and relies on its own
# entrypoint to start it after catchup. When the container is restarted
# independently of that entrypoint (docker restart, host reboot, compose up),
# horizon stays down and testnet.suban.org returns 502.
#
# Usage: horizon-ensure.sh <interval-seconds>
set -u

INTERVAL="${1:-30}"

while true; do
  # Only act once core reports Synced, otherwise horizon would start against an
  # unpopulated database.
  if curl -s --max-time 10 http://localhost:11626/info 2>/dev/null \
      | grep -q '"state"[[:space:]]*:[[:space:]]*"Synced!"'; then
    if ! supervisorctl status horizon 2>/dev/null | grep -q '^horizon[[:space:]]\+RUNNING'; then
      echo "horizon-ensure: core synced and horizon is not running, starting it"
      supervisorctl start horizon 2>&1 || true
    fi
  fi
  sleep "$INTERVAL"
done
