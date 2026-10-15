#!/bin/bash
# Opens the address of an Internet shortcut (.url, what Taildrop receives when a phone shares
# a link) rather than the file itself. Only http(s) addresses are opened; the file is kept.
set -euo pipefail

file="$1"
url=$(sed -n 's/^URL=//p' "$file" | tr -d '\r' | head -n 1)

case "$url" in
http://* | https://*) exec xdg-open "$url" ;;
*)
  notify-send "No web address in ${file##*/}"
  exit 1
  ;;
esac
