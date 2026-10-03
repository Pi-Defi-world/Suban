#!/bin/bash
# Staging entrypoint for the Pi testnet node.
#
# Three things need fixing that the stock image cannot do on its own:
#
#  1. `--testnet2` hardcodes validator keys GDFDD/GDOJP/GAOBN, which sit on a
#     different fork at ledger ~11.1M. The live testnet validators are
#     GC6R2/GDRIZZC5/GAKI7.
#  2. Horizon spawns its own captive core from a second copy of the same stale
#     validators, so it replays the wrong chain.
#  3. The image ships horizon with `autostart=false` and relies on its own
#     entrypoint to start it after catchup, so a container restart leaves
#     testnet.suban.org returning 502.
#
# The stock /start runs `sed -ri` over the core config to inject the postgres
# password, so the core config is copied into the writable volume rather than
# mounted read-only over the target path.
#
# Horizon's directory is created by /start itself and also holds the horizon
# binary. Creating it early would make /start skip populating it, so the
# horizon configs are applied by a detached helper once that directory exists.
set -e

SUBAN_CFG=/etc/suban

# --- Core: correct validators + live history archive. ---
# Safe to stage now: this directory only holds configuration, and /start
# patches the postgres password into it in place.
if [ -f "$SUBAN_CFG/pi-testnet-core.cfg" ]; then
  mkdir -p /opt/stellar/core/etc
  cp "$SUBAN_CFG/pi-testnet-core.cfg" /opt/stellar/core/etc/stellar-core.cfg
  echo "suban-entrypoint: staged stellar-core.cfg"
fi

# --- Horizon + watchdog: deferred until /start has created the directory. ---
mkdir -p /opt/stellar/horizon-ensure
if [ -f "$SUBAN_CFG/horizon-ensure.sh" ]; then
  cp "$SUBAN_CFG/horizon-ensure.sh" /usr/local/bin/horizon-ensure.sh
  chmod +x /usr/local/bin/horizon-ensure.sh
fi

# The watchdog runs as a detached process rather than a supervisord program.
# Registering it as a [program] block would persist in the volume's
# supervisord.conf, and a later run of the stock entrypoint (which does not
# copy the script) would then fail to spawn it and take down the node.
cat > /opt/stellar/horizon-ensure/apply.sh <<'APPLY'
#!/bin/bash
CFG=/etc/suban
HETC=/opt/stellar/horizon/etc
SUP=/opt/stellar/supervisor/etc/supervisord.conf

# Wait for /start to populate the horizon directory and supervisor config.
for _ in $(seq 1 180); do
  [ -d "$HETC" ] && [ -f "$SUP" ] && break
  sleep 1
done

if [ -d "$HETC" ]; then
  [ -f "$CFG/pi-testnet-horizon-core.yml" ] && cp "$CFG/pi-testnet-horizon-core.yml" "$HETC/stellar-core-captive.yml"
  [ -f "$CFG/pi-testnet-horizon.env" ]      && cp "$CFG/pi-testnet-horizon.env"      "$HETC/horizon.env"
  echo "suban-apply: horizon configs applied"
fi

# Drop any stale watchdog block from an earlier run before starting the
# watchdog directly.
if [ -f "$SUP" ]; then
  sed -i '/^\[program:horizon-ensure\]/,$d' "$SUP"
fi

# Wait for supervisord to accept connections, then supervise horizon ourselves.
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
