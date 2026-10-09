#!/usr/bin/env bash
# Claude Code status line — affiche : dossier · branche git · modèle · barres
# de progression pour le contexte, le quota 5 h et le quota 7 jours (+ resets).
# Lit le JSON d'état de Claude Code sur stdin. Voir :
#   https://code.claude.com/docs/en/statusline.md

input=$(cat)

# --- Couleurs (séquences littérales, interprétées par `printf %b` à la fin) ---
RESET='\033[0m'
DIM='\033[90m'
CYAN='\033[36m'
MAGENTA='\033[35m'
BOLD='\033[1m'

# Couleur d'une barre selon le taux de remplissage.
color_for() {
  local pct=${1:-0}
  if   (( pct >= 90 )); then printf '%s' '\033[31m'   # rouge
  elif (( pct >= 70 )); then printf '%s' '\033[33m'   # jaune
  else                       printf '%s' '\033[32m'   # vert
  fi
}

# Barre de progression : bar <pourcentage> [largeur]
bar() {
  local pct=${1:-0} width=${2:-8} i
  (( pct < 0 ))   && pct=0
  (( pct > 100 )) && pct=100
  local filled=$(( (pct * width + 50) / 100 ))
  (( filled > width )) && filled=$width
  for ((i = 0; i < filled; i++));        do printf '%s' '█'; done
  for ((i = 0; i < width - filled; i++)); do printf '%s' '░'; done
}

# --- Extraction des champs ---
model=$(jq -r '.model.display_name // "?"'              <<<"$input")
cwd=$(jq   -r '.workspace.current_dir // .cwd // ""'     <<<"$input")
ctx=$(jq   -r '(.context_window.used_percentage // empty) | round'      <<<"$input")
r5=$(jq    -r '(.rate_limits.five_hour.used_percentage // empty) | round' <<<"$input")
r5_at=$(jq -r '.rate_limits.five_hour.resets_at // empty'  <<<"$input")
r7=$(jq    -r '(.rate_limits.seven_day.used_percentage // empty) | round' <<<"$input")
r7_at=$(jq -r '.rate_limits.seven_day.resets_at // empty'  <<<"$input")

# Dossier raccourci (~ pour $HOME).
dir=${cwd/#$HOME/\~}

# Branche git courante (silencieux hors dépôt).
branch=$(git -C "$cwd" rev-parse --abbrev-ref HEAD 2>/dev/null)

# Heures/dates de reset (locale système).
r5_time=""; [ -n "$r5_at" ] && r5_time=$(date -d "@$r5_at" +%H:%M       2>/dev/null)
r7_day="";  [ -n "$r7_at" ] && r7_day=$(date  -d "@$r7_at" +"%a %d/%m"  2>/dev/null)

# --- Assemblage ---
SEP=" ${DIM}│${RESET} "
out="${CYAN}${dir}${RESET}"
[ -n "$branch" ] && out+=" ${DIM}⎇${RESET} ${MAGENTA}${branch}${RESET}"
out+="${SEP}${BOLD}${model}${RESET}"

if [ -n "$ctx" ]; then
  out+="${SEP}${DIM}ctx${RESET} $(color_for "$ctx")$(bar "$ctx")${RESET} ${ctx}%"
fi
if [ -n "$r5" ]; then
  out+="${SEP}${DIM}5h${RESET} $(color_for "$r5")$(bar "$r5")${RESET} ${r5}%"
  [ -n "$r5_time" ] && out+=" ${DIM}${r5_time}${RESET}"
fi
if [ -n "$r7" ]; then
  out+="${SEP}${DIM}7j${RESET} $(color_for "$r7")$(bar "$r7")${RESET} ${r7}%"
  [ -n "$r7_day" ] && out+=" ${DIM}${r7_day}${RESET}"
fi

printf '%b' "$out"
