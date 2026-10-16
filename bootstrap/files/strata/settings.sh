#!/bin/sh
# Sets top-level keys of Strata's settings.toml (`key=value`, the value as TOML), leaving
# everything else as Strata wrote it (folder colors, device labels…: paths and UUIDs of this
# machine, never in the repository). `--check`: succeeds when the keys already have those values.
# Strata keeps the file in memory and writes it back: never changed while it runs.
set -eu
f="${XDG_CONFIG_HOME:-$HOME/.config}/strata/settings.toml"
check=0
if [ "${1:-}" = --check ]; then check=1; shift; fi

# A table header starts the tables; indented or nested `[` of a multi-line array does not.
export STRATA_HEADER='^\[\[?[A-Za-z0-9_."-]+\]\]?[[:space:]]*(#.*)?$'

# The value of a top-level key (before the first [table]), empty when absent.
value() {
  [ -f "$f" ] || return 0
  awk -v k="$1" '
    $0 ~ ENVIRON["STRATA_HEADER"] { exit }
    { line = $0; sub(/^[[:space:]]+/, "", line) }
    index(line, k) == 1 {
      rest = substr(line, length(k) + 1)
      if (rest ~ /^[[:space:]]*=/) { sub(/^[[:space:]]*=[[:space:]]*/, "", rest); print rest; exit }
    }' "$f"
}

if [ "$check" = 1 ]; then
  for kv in "$@"; do
    [ "$(value "${kv%%=*}")" = "${kv#*=}" ] || exit 1
  done
  exit 0
fi

if pgrep -x strata >/dev/null; then
  # Also its D-Bus helpers (file chooser, FileManager1), which can stay after a dialog.
  echo "Strata is running (a window, or its file chooser in the background, see \`pgrep -ax strata\`): close it, then run this again." >&2
  exit 1
fi
mkdir -p "$(dirname "$f")"
if [ -f "$f" ]; then
  # Not put in place by poste: a copy before any change.
  backup="${XDG_STATE_HOME:-$HOME/.local/state}/poste/backup"
  mkdir -p "$backup"
  cp -p "$f" "$backup/strata-settings.toml.$(date +%Y%m%d-%H%M%S)"
else
  : > "$f"
fi
tmp="$f.poste.new"
for kv in "$@"; do
  # Replaces the key where it is; absent, adds it before the first [table].
  k="${kv%%=*}"
  # A value that can span lines (array, inline table, multi-line string) is not replaced.
  case "$(value "$k")" in
    \[*|\{*|\"\"\"*|"'''"*)
      echo "$k: its value spans lines, not changed by this script." >&2
      exit 1 ;;
  esac
  awk -v k="$k" -v v="${kv#*=}" '
    BEGIN { done = 0; tables = 0 }
    $0 ~ ENVIRON["STRATA_HEADER"] && !tables { tables = 1; if (!done) { print k " = " v; done = 1 } }
    !tables {
      line = $0; sub(/^[[:space:]]+/, "", line)
      if (index(line, k) == 1 && substr(line, length(k) + 1) ~ /^[[:space:]]*=/) {
        if (!done) print k " = " v
        done = 1
        next
      }
    }
    { print }
    END { if (!done) print k " = " v }' "$f" > "$tmp"
  # In place: the file keeps its permissions.
  cat "$tmp" > "$f"
  rm -f "$tmp"
done
