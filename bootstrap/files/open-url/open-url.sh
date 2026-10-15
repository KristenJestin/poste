#!/bin/bash
# Opens the address of an Internet shortcut (.url, what Taildrop receives when a phone shares
# a link) rather than the file itself. Only http(s) addresses are opened; the file is kept.
# Never fails silently: it runs from a click (notification, file manager), so a notification
# says what went wrong.
set -uo pipefail

file="${1:-}"
if [[ ! -f $file || ! -r $file ]]; then
  notify-send "Cannot read the shortcut ${file:-(none)}"
  exit 1
fi

# The URL key of the [InternetShortcut] section (CRLF line ends, spaces around the =).
url=$(awk '
  { sub(/\r$/, "") }
  /^[[:space:]]*\[/ { in_section = tolower($0) ~ /^[[:space:]]*\[internetshortcut\]/; next }
  in_section && tolower($0) ~ /^[[:space:]]*url[[:space:]]*=/ {
    sub(/^[^=]*=[[:space:]]*/, ""); sub(/[[:space:]]+$/, ""); print; exit
  }
' "$file")

case "${url,,}" in
http://* | https://*) exec xdg-open "$url" ;;
*)
  notify-send "No web address in ${file##*/}"
  exit 1
  ;;
esac
