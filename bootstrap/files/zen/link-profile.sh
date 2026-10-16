#!/bin/sh
# Links, in Zen's default profile, chrome/userChrome.css to the zen.css Omarchy renders from
# config/.config/omarchy/themed/zen.css.tpl, and user.js to <user.js>, the first argument
# (bootstrap/files/zen/user.js, given by the step).
# The profile is the one installs.ini names for the installed Zen (its folder name is random);
# with no profile yet (Zen never started) or several installs naming different ones, it waits.
# A link poste made (same file name at the end of its target) is moved to the new target; any
# other file is the user's: left as is, and counted as done so setup does not ask again.
# `--check <user.js>`: changes nothing, code 0 when there is nothing left to do.

check=0
if [ "$1" = --check ]; then
  check=1
  shift
fi
prefs="$1"
theme="$HOME/.local/state/omarchy/current/theme/zen.css"

say() {
  [ "$check" = 1 ] || echo "$*" >&2
}

zen="${XDG_CONFIG_HOME:-$HOME/.config}/zen"
[ -d "$zen" ] || [ ! -d "$HOME/.zen" ] || zen="$HOME/.zen"

defaults=$(sed -n 's/^Default=//p' "$zen/installs.ini" 2>/dev/null | sort -u)
if [ -z "$defaults" ]; then
  say "no default Zen profile in $zen/installs.ini yet: start Zen once, then run poste sys setup again"
  [ "$check" = 1 ] && exit 1
  exit 0
fi
if [ "$(printf '%s\n' "$defaults" | wc -l)" -gt 1 ]; then
  say "several Zen installs in $zen/installs.ini, with different default profiles: nothing linked"
  [ "$check" = 1 ] && exit 1
  exit 0
fi
case "$defaults" in
  /*) profile="$defaults" ;;
  *) profile="$zen/$defaults" ;;
esac
if [ ! -d "$profile" ]; then
  say "Zen profile $profile not found"
  [ "$check" = 1 ] && exit 1
  exit 0
fi

# link <path> <target>: code 0 when there is nothing left to do for <path>.
link() {
  if [ -L "$1" ]; then
    current=$(readlink "$1")
    [ "$current" = "$2" ] && return 0
    if [ "${current##*/}" = "${2##*/}" ]; then
      [ "$check" = 1 ] && return 1
      ln -sfn "$2" "$1"
      return
    fi
  fi
  if [ -e "$1" ] || [ -L "$1" ]; then
    say "$1 exists and was not put in place by poste: left as is"
    return 0
  fi
  [ "$check" = 1 ] && return 1
  mkdir -p "$(dirname "$1")" && ln -s "$2" "$1"
}

status=0
link "$profile/chrome/userChrome.css" "$theme" || status=1
link "$profile/user.js" "$prefs" || status=1
exit "$status"
