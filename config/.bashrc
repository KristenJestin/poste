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

# Node géré par Vite+ (met ses shims en tête du PATH). mise gère le reste
# (dotnet, go…) via uwsm, donc on NE fait PAS `mise deactivate`.
[ -f "$HOME/.vite-plus/env" ] && . "$HOME/.vite-plus/env"

# direnv : variables d'environnement par dossier (.envrc)
command -v direnv >/dev/null && eval "$(direnv hook bash)"


# bunx de Vite+ ne passe pas en mode "x" (bun lancé via un wrapper npm) : utiliser bun x.
alias bunx='bun x'
