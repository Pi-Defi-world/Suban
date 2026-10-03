#!/bin/bash
# Staging entrypoint for the Pi testnet node.
#
# Three things need fixing that the stock image cannot do on its own:
#
#  1. `--testnet2` hardcodes validator keys GDFDD/GDOJP/GAOBN, which sit on a
#     different fork at ledger ~11.1M. The live testnet validators are
#     GC6R2/GDRIZZC5/GAKI7. Core config is staged from /etc/suban.
#  2. Horizon spawns its own captive core using a second copy of the same stale
#     validators, so it replays the wrong chain.
#  3. The image ships horizon with `autostart=false` and relies on its own
#     entrypoint to start it after catchup, so a container restart leaves
#     testnet.suban.org returning 502.
#
# The stock /start runs `sed -ri` over the core config to inject the postgres
# password, so configs are copied into the writable volume rather than mounted
# read-only over the target path.
set -e

SUPHOME=/opt/stellar/supervisor/etc
SUBAN_CFG=/etc/suban

stage() {
  src="$1"
  dst="$2"
  if [ -f "$src" ]; then
    mkdir -p "$(dirname "$dst")"
    cp "$src" "$dst"
    echo "suban-entrypoint: staged $(basename "$dst")"
  fi
}

# Core: correct validators + live history archive.
stage "$SUBAN_CFG/pi-testnet-core.cfg" /opt/stellar/core/etc/stellar-core.cfg

# Horizon's captive core: same validators and archive.
stage "$SUBAN_CFG/pi-testnet-horizon-core.yml" /opt/stellar/horizon/etc/stellar-core-captive.yml

# Horizon env: live history archive.
stage "$SUBAN_CFG/pi-testnet-horizon.env" /opt/stellar/horizon/etc/horizon.env

# Watchdog that starts Horizon once core reports Synced.
if [ -f "$SUBAN_CFG/horizon-ensure.sh" ]; then
  cp "$SUBAN_CFG/horizon-ensure.sh" /usr/local/bin/horizon-ensure.sh
  chmod +x /usr/local/bin/horizon-ensure.sh
  if ! grep -q 'program:horizon-ensure' "$SUPHOME/supervisord.conf" 2>/dev/null; then
    printf '\n[program:horizon-ensure]\ncommand=/usr/local/bin/horizon-ensure.sh 30\nautostart=true\npriority=5\nredirect_stderr=true\nstdout_logfile=/var/log/supervisor/horizon-ensure.log\n' >> "$SUPHOME/supervisord.conf"
    echo "suban-entrypoint: registered horizon-ensure watchdog"
  fi
fi

exec /start "$@"
