#!/bin/bash
# Staging entrypoint for the Pi node images (mainnet and testnet).
#
# The stock image ships Horizon with `autostart=false` and relies on its own
# entrypoint to start it once stellar-core reports catchup-complete. When the
# container is restarted outside that entrypoint (docker restart, host reboot,
# `compose up`), Horizon never starts and the Horizon subdomain returns 502.
#
# This script stages any corrected configs supplied under /etc/suban and then
# runs a watchdog that starts Horizon as soon as core reports Synced.
#
# Horizon's directory is created by /start itself and also holds the horizon
# binary, so creating it early would make /start skip populating it. Horizon
# configs are therefore applied by a detached helper that waits for /start.
set -e

SUBAN_CFG=/etc/suban

# --- Core config: optional, staged now. ---
# Safe to stage early: this directory holds only configuration, and /start
# patches the postgres password into it in place with sed.
if [ -f "$SUBAN_CFG/pi-testnet-core.cfg" ] || [ -f "$SUBAN_CFG/pi-mainnet-core.cfg" ]; then
  for f in pi-mainnet-core.cfg pi-testnet-core.cfg; do
    if [ -f "$SUBAN_CFG/$f" ]; then
      mkdir -p /opt/stellar/core/etc
      cp "$SUBAN_CFG/$f" /opt/stellar/core/etc/stellar-core.cfg
      echo "suban-entrypoint: staged stellar-core.cfg from $f"
    fi
  done
fi

# --- Watchdog ---
if [ -f "$SUBAN_CFG/horizon-ensure.sh" ]; then
  cp "$SUBAN_CFG/horizon-ensure.sh" /usr/local/bin/horizon-ensure.sh
  chmod +x /usr/local/bin/horizon-ensure.sh
fi

mkdir -p /opt/stellar/horizon-ensure
cat > /opt/stellar/horizon-ensure/apply.sh <<'APPLY'
#!/bin/bash
CFG=/etc/suban
HETC=/opt/stellar/horizon/etc
SUP=/opt/stellar/supervisor/etc/supervisord.conf

for _ in $(seq 1 180); do
  [ -d "$HETC" ] && [ -f "$SUP" ] && break
  sleep 1
done

if [ -d "$HETC" ]; then
  for f in pi-testnet-horizon-core.yml pi-mainnet-horizon-core.yml; do
    [ -f "$CFG/$f" ] && cp "$CFG/$f" "$HETC/stellar-core-captive.yml" && echo "suban-apply: staged $f"
  done
  for f in pi-testnet-horizon.env pi-mainnet-horizon.env; do
    [ -f "$CFG/$f" ] && cp "$CFG/$f" "$HETC/horizon.env" && echo "suban-apply: staged $f"
  done
fi

# Remove any stale watchdog [program] block from an earlier run. Registering the
# watchdog as a supervisord program would persist in the volume's config, and a
# later run of the stock entrypoint (which does not copy the script) would then
# fail to spawn it and take the node down.
if [ -f "$SUP" ]; then
  sed -i '/^\[program:horizon-ensure\]/,$d' "$SUP"
fi

for _ in $(seq 1 180); do
  supervisorctl status >/dev/null 2>&1 && break
  sleep 1
done

if [ -x /usr/local/bin/horizon-ensure.sh ]; then
  setsid /usr/local/bin/horizon-ensure.sh 30 >/tmp/horizon-ensure.log 2>&1 &
  echo "suban-apply: horizon watchdog started"
fi
exit 0
APPLY
chmod +x /opt/stellar/horizon-ensure/apply.sh
setsid /opt/stellar/horizon-ensure/apply.sh >/tmp/suban-horizon-apply.log 2>&1 &
echo "suban-entrypoint: horizon apply scheduled"

exec /start "$@"
