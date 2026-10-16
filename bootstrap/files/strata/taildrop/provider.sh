#!/usr/bin/env bash
# Strata file provider (protocol 1, docs/file-providers.md of Strata): sending the selected
# files with Taildrop from the context menu. One device online: 'Send to <device>'; several: a
# 'Send with Taildrop' submenu of them. Strata sends one JSON request per line on stdin and
# waits for the reply on stdout, eight seconds at most: the send itself runs detached and ends
# in a notification. Needs the user to be a Tailscale operator
# (`sudo tailscale set --operator=$USER`, once).
set -uo pipefail

# The online Taildrop targets, one name per line. Columns of `tailscale file cp --targets`: IP,
# name, then "offline" when it is.
targets() {
  # Bounded: a request has eight seconds, and a stuck tailscaled must not cost the provider.
  timeout 4 tailscale file cp --targets 2>/dev/null | awk -F'\t' '$2 != "" && $3 !~ /offline/ { print $2 }'
}

# Detached send: `provider.sh --send <device> <file>...`.
if [ "${1:-}" = --send ]; then
  device=$2
  shift 2
  if err=$(tailscale file cp -- "$@" "$device:" 2>&1); then
    notify-send -a Taildrop "Taildrop" "Sent $# file(s) to $device."
  else
    notify-send -a Taildrop -u critical "Taildrop: sending to $device failed" "${err:0:300}"
  fi
  exit 0
fi

while IFS= read -r req; do
  id=$(jq -r '.id' <<<"$req" 2>/dev/null) || continue
  [[ $id =~ ^[0-9]+$ ]] || continue
  method=$(jq -r '.method' <<<"$req")
  # Decorations are asked for every visible row: answered before looking at the paths.
  if [ "$method" = query ]; then
    jq -nc --argjson id "$id" '{version: 1, id: $id, decorations: []}'
    continue
  fi
  mapfile -d '' paths < <(jq -j '.paths[] | ., "\u0000"' <<<"$req")
  # Taildrop sends regular files only; a background menu (the folder itself) gets nothing.
  files=1
  [ "$(jq -r '.background' <<<"$req")" = false ] && [ "${#paths[@]}" -gt 0 ] || files=0
  for p in "${paths[@]}"; do [ -f "$p" ] || files=0; done
  case $method in
    menu)
      devices=()
      [ "$files" = 1 ] && mapfile -t devices < <(targets | head -n 63)
      printf '%s\n' "${devices[@]}" | jq -Rnc --argjson id "$id" '
        [inputs | select(. != "")] as $d
        | {version: 1, id: $id, actions: (
            if ($d | length) == 0 then []
            elif ($d | length) == 1 then [{id: "send", label: ("Send to " + $d[0]), context: $d[0]}]
            else [{id: "send", label: "Send with Taildrop",
                   children: [$d | to_entries[] | {id: ("d\(.key)"), label: .value, context: .value}]}]
            end)}'
      ;;
    activate)
      device=$(jq -r '.context // ""' <<<"$req")
      n=${#paths[@]}
      # The menu is no authorization: the files and the device are checked again.
      if [ "$files" = 0 ]; then
        why="Taildrop sends files only, and one of them is gone or is not a file."
      elif [ -z "$device" ]; then
        why="No device chosen: open the menu again."
      elif ! grep -qxF -- "$device" <<<"$(targets)"; then
        why="$device is no longer online."
      else
        why=
      fi
      if [ -n "$why" ]; then
        jq -nc --argjson id "$id" --arg m "$why" --argjson n "$n" \
          '{version: 1, id: $id, message: $m, outcome: {status: "rejected", accepted: 0, total: $n}}'
        continue
      fi
      # Its own session: Strata ends the provider's process group when it stops it.
      setsid -f "$0" --send "$device" "${paths[@]}" </dev/null >/dev/null 2>&1
      jq -nc --argjson id "$id" --arg m "Sending $n file(s) to $device." --argjson n "$n" \
        '{version: 1, id: $id, message: $m, outcome: {status: "accepted", accepted: $n, total: $n}, feedback: "notice"}'
      ;;
    *)
      jq -nc --argjson id "$id" '{version: 1, id: $id, error: "unsupported-method"}'
      ;;
  esac
done
