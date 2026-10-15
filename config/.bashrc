# If not running interactively, don't do anything (leave this at the top of this file)
[[ $- != *i* ]] && return

# All the default Omarchy aliases and functions
# (don't mess with these directly, just overwrite them here!)
# /etc/omarchy.conf is written by omarchy-dev-link. When absent, force the
# package default instead of preserving a stale inherited dev-link value before
# we decide which rc file to source.
if [[ -f /etc/omarchy.conf ]]; then
  source /etc/omarchy.conf
  export OMARCHY_PATH="${OMARCHY_PATH:-/usr/share/omarchy}"
else
  export OMARCHY_PATH=/usr/share/omarchy
fi
source "$OMARCHY_PATH/default/bash/rc"

# Add your own exports, aliases, and functions here.
#
# Make an alias for invoking commands you use constantly
# alias p='python'

# Node managed by Vite+ (puts its shims at the head of the PATH). mise manages the rest
# (dotnet, go…) through uwsm, so we do NOT run `mise deactivate`.
[ -f "$HOME/.vite-plus/env" ] && . "$HOME/.vite-plus/env"

# direnv: environment variables per folder (.envrc)
command -v direnv >/dev/null && eval "$(direnv hook bash)"


# Vite+'s bunx does not switch to "x" mode (bun started through an npm wrapper): use bun x.
alias bunx='bun x'

# Machine-specific settings (not versioned).
[ -f "$HOME/.bashrc.local" ] && . "$HOME/.bashrc.local"
