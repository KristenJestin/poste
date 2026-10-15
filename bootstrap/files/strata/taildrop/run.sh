#!/usr/bin/env bash
# Sends the selection (NUL-separated paths in $STRATA_ACTION_PATHS) to the device
# picked in the submenu ($STRATA_ACTION_CHOICE). Needs the user to be a Tailscale
# operator (`sudo tailscale set --operator=$USER`, once).
set -euo pipefail
if [ -z "${STRATA_ACTION_CHOICE:-}" ]; then
  echo "No device chosen: pick one in the 'Send with Taildrop' submenu." >&2
  exit 1
fi
mapfile -d '' files < "$STRATA_ACTION_PATHS"
if [ "${#files[@]}" -eq 0 ]; then
  echo "Nothing to send." >&2
  exit 1
fi
echo "Sending ${#files[@]} file(s) to $STRATA_ACTION_CHOICE"
tailscale file cp -- "${files[@]}" "$STRATA_ACTION_CHOICE:"
