#!/usr/bin/env bash
# Réinstallation de l'environnement « kris » sur une Omarchy 4 fraîchement installée.
#
# À lancer APRÈS :
#   chezmoi init --apply git@github.com:KristenJestin/dotfiles.git
#   (ou https://github.com/KristenJestin/dotfiles.git)
#
# Usage : bash ~/.local/share/chezmoi/bootstrap.sh [--dry-run]
#   --dry-run  affiche ce qui serait fait, sans rien exécuter.
#
# Idempotent : chaque étape vérifie l'état avant d'agir, on peut le relancer.
# sudo est demandé seulement pour les étapes système (polkit, fstab, GPU voxtype).
set -Eeuo pipefail

DRY_RUN=0
[[ ${1:-} == "--dry-run" ]] && DRY_RUN=1

bold() { printf '\n\033[1m==> %s\033[0m\n' "$*"; }
info() { printf '    %s\n' "$*"; }
run() {
  if ((DRY_RUN)); then
    printf '    [dry-run] %s\n' "$*"
  else
    printf '    $ %s\n' "$*"
    "$@"
  fi
}
# Écrit $2 dans le fichier root $1 (via sudo tee), si le contenu diffère.
write_root_file() {
  local target=$1 content=$2
  if [[ -f $target ]] && [[ $(cat "$target") == "$content" ]]; then
    info "déjà à jour : $target"
    return
  fi
  if ((DRY_RUN)); then
    printf '    [dry-run] écrire %s :\n' "$target"
    sed 's/^/        /' <<<"$content"
  else
    printf '%s\n' "$content" | sudo tee "$target" >/dev/null
    info "écrit : $target"
  fi
}
need_sudo() { ((DRY_RUN)) || sudo -v; }

USER_NAME=$(id -un)
USER_UID=$(id -u)
USER_GID=$(id -g)
MANUAL=()

# ---------------------------------------------------------------- a) paquets
bold "Paquets (yay)"
# Seulement ce qui n'est pas déjà dans l'installation de base d'Omarchy 4
# (/usr/share/omarchy/install/omarchy-*.packages) et dont ce setup dépend.
PACKAGES=(
  chezmoi              # ces dotfiles
  github-cli           # gh (auth git HTTPS, credential helper dans git/config)
  direnv               # hook dans .bashrc
  ttf-fira-code        # police des terminaux (foot / alacritty / ghostty)
  ttf-firacode-nerd    # glyphes Nerd Font de la police
  libqalculate         # qalc : calculatrice du lanceur de l'île
  python               # sources du lanceur (unity-projects)
  libnotify            # notify-send (sources du lanceur)
  voxtype-bin          # dictée (config ~/.config/voxtype, île kris.island)
  zen-browser-bin      # navigateur
  t3code-nightly-bin   # T3 Code (thème généré par ~/.config/omarchy/themed/t3code.json.tpl)
  proton-pass-bin      # mots de passe (SUPER+SHIFT+.)
  unityhub             # Unity Hub
  unity-cli-bin        # CLI « unity » (source « Projets Unity » du lanceur)
  proton-vpn-cli       # Proton VPN (piloté par le plugin OmaProton, tuile VPN de l'île)
  proton-vpn-daemon    #   son démon
  protonmail-bridge    # Proton Mail Bridge (IMAP/SMTP local pour Omamail)
  flea                 # gestionnaire de fichiers par défaut (dépôt Omarchy) + Shelf / vue Fichiers de l'île (SUPER+D)
  fd                   #   recherche de la vue Fichiers
  zoxide               #   dossiers fréquentés (classement de la recherche)
  headsetcontrol       #   batterie du casque sans fil (île : DevicesState)
  zip                  #   LocalSend d'un dossier (zippé d'abord)
  # Optionnels, installés sur la machine d'origine :
  # opencode-bin dotnet-runtime-9.0
)
missing=()
for pkg in "${PACKAGES[@]}"; do
  pacman -Qq "$pkg" &>/dev/null || missing+=("$pkg")
done
if ((${#missing[@]})); then
  info "à installer : ${missing[*]}"
  run yay -S --needed --noconfirm "${missing[@]}"
else
  info "tous présents"
fi

# ---------------------------------------------------------------- b) plugins shell tiers
bold "Plugins du shell Omarchy (versions auditées)"
# kris.island, kris.notifications et kris.workspaces viennent de chezmoi.
PLUGINS_DIR="$HOME/.config/omarchy/plugins"
declare -A PLUGINS=(
  [crmne.mpris]="https://github.com/crmne/omarchy-mpris 85bb4ff"
  [jankeesvw.notification-center]="https://github.com/jankeesvw/omarchy-notification-center e4c4568"
  [io.github.honeyspoons.localsend]="https://github.com/HoneySpoons/omarchy-localsend.git eef3289"
  [omamail]="https://github.com/huacnlee/omamail a7388fa"                     # tag v0.10.8
  [io.github.grichard99.omaproton-vpn]="https://github.com/grichard99/omaproton-vpn 6d68681"
  [io.github.smoothpixels.cursor-accent]="https://github.com/SmoothPixels/cursor-accent 97e001c"
)
for id in "${!PLUGINS[@]}"; do
  read -r url commit <<<"${PLUGINS[$id]}"
  dir="$PLUGINS_DIR/$id"
  if [[ -d $dir/.git ]]; then
    current=$(git -C "$dir" rev-parse --short HEAD)
    if [[ $current == "$commit"* ]]; then
      info "$id déjà à $commit"
      continue
    fi
  else
    run omarchy plugin add "$url" --yes
  fi
  run git -C "$dir" fetch --quiet --tags origin
  run git -C "$dir" -c advice.detachedHead=false checkout --quiet "$commit"
done
# Cursor Accent : patch local de recolor.py (option stroke_scale, contour
# plus fin), réappliqué s'il manque. `git apply --reverse --check` réussit
# seulement si le patch est déjà en place.
CA_DIR="$PLUGINS_DIR/io.github.smoothpixels.cursor-accent"
CA_PATCH="$(dirname "$(readlink -f "$0")")/bootstrap-patches/cursor-accent-stroke-scale.patch"
if [[ -d $CA_DIR/.git ]] && git -C "$CA_DIR" apply --reverse --check "$CA_PATCH" &>/dev/null; then
  info "cursor-accent : patch stroke_scale déjà appliqué"
else
  run git -C "$CA_DIR" apply "$CA_PATCH" \
    || MANUAL+=("cursor-accent : le patch stroke_scale ne s'applique plus (plugin mis à jour ?) : $CA_PATCH")
fi
# Omamail : patch local du tri des mails (dates texte ou nombre comptaient 0 et
# gardaient l'ordre du serveur, plus ancien d'abord), réappliqué s'il manque.
OM_DIR="$PLUGINS_DIR/omamail"
OM_PATCH="$(dirname "$(readlink -f "$0")")/bootstrap-patches/omamail-local.patch"
if [[ -d $OM_DIR/.git ]] && git -C "$OM_DIR" apply --reverse --check "$OM_PATCH" &>/dev/null; then
  info "omamail : patchs locaux (tri par date, touche Suppr, IA et agenda masqués, attente au démarrage, style de l'île) déjà appliqué"
else
  run git -C "$OM_DIR" apply "$OM_PATCH" \
    || MANUAL+=("omamail : le patch local ne s'applique plus (plugin mis à jour ?) : $OM_PATCH")
fi
CA_CONFIG="$HOME/.local/state/cursor-accent/config.json"
CA_CONTENT='{
  "mode": "theme",
  "shape": "modern",
  "fixed_color": "#F25623",
  "stroke_scale": 0.55
}'
if [[ -f $CA_CONFIG ]] && [[ $(jq -S . "$CA_CONFIG" 2>/dev/null) == $(jq -S . <<<"$CA_CONTENT") ]]; then
  info "cursor-accent : config déjà à jour"
elif ((DRY_RUN)); then
  printf '    [dry-run] écrire %s :\n' "$CA_CONFIG"
  sed 's/^/        /' <<<"$CA_CONTENT"
else
  mkdir -p "$(dirname "$CA_CONFIG")"
  printf '%s\n' "$CA_CONTENT" >"$CA_CONFIG"
  info "écrit : $CA_CONFIG"
fi
if ((DRY_RUN)); then
  info "[dry-run] python3 $CA_DIR/recolor.py <thème courant>"
else
  run python3 "$CA_DIR/recolor.py" "$(omarchy-theme-current 2>/dev/null || echo unknown)"
fi

# `omarchy plugin add` peut toucher shell.json : on remet la version versionnée
# (barre = kris.island, service de notifications = kris.notifications).
run chezmoi apply --force "$HOME/.config/omarchy/shell.json"

# ---------------------------------------------------------------- c) polkit systemd-creds (Unity)
bold "Règle polkit systemd-creds (Unity Hub / CLI)"
POLKIT_RULE="/etc/polkit-1/rules.d/49-systemd-creds-$USER_NAME.rules"
POLKIT_CONTENT="// Unity Hub / CLI chiffrent leurs identifiants avec systemd-creds : autorisé
// sans mot de passe pour $USER_NAME en session locale active.
polkit.addRule(function(action, subject) {
  if ((action.id == \"io.systemd.credentials.encrypt\" ||
       action.id == \"io.systemd.credentials.decrypt\") &&
      subject.user == \"$USER_NAME\" && subject.local && subject.active) {
    return polkit.Result.YES;
  }
});"
# rules.d n'est lisible que par root : la vérification passe par sudo.
polkit_ok() {
  local existing
  existing=$(sudo cat /etc/polkit-1/rules.d/*.rules 2>/dev/null || true)
  [[ $existing == *io.systemd.credentials.encrypt* && $existing == *io.systemd.credentials.decrypt* \
    && $existing == *"subject.user == \"$USER_NAME\""* ]]
}
if ((DRY_RUN)); then
  info "[dry-run] vérifierait avec sudo /etc/polkit-1/rules.d/ ; si aucune règle ne couvre"
  info "io.systemd.credentials.encrypt/decrypt pour $USER_NAME, écrirait $POLKIT_RULE :"
  sed 's/^/        /' <<<"$POLKIT_CONTENT"
else
  need_sudo
  if polkit_ok; then
    info "règle déjà présente dans /etc/polkit-1/rules.d/"
  else
    write_root_file "$POLKIT_RULE" "$POLKIT_CONTENT"
  fi
fi

# ---------------------------------------------------------------- d) SSD NTFS dans fstab
bold "SSD NTFS (montage automatique)"
SSD_UUID=0123456789ABCDEF
SSD_MOUNT="/run/media/$USER_NAME/SSD"
SSD_LINE="UUID=$SSD_UUID  $SSD_MOUNT  ntfs3  uid=$USER_UID,gid=$USER_GID,windows_names,nofail,x-gvfs-show,x-systemd.device-timeout=10s  0 0"
if ! blkid -U "$SSD_UUID" &>/dev/null && [[ ! -e /dev/disk/by-uuid/$SSD_UUID ]]; then
  info "disque UUID=$SSD_UUID absent : étape ignorée"
elif grep -q "UUID=$SSD_UUID" /etc/fstab; then
  info "déjà dans /etc/fstab"
else
  need_sudo
  if ((DRY_RUN)); then
    printf '    [dry-run] ajouter à /etc/fstab : %s\n' "$SSD_LINE"
  else
    printf '%s\n' "$SSD_LINE" | sudo tee -a /etc/fstab >/dev/null
    info "ajouté à /etc/fstab"
  fi
  run sudo systemctl daemon-reload
  MANUAL+=("Monter le SSD maintenant (ou redémarrer) : sudo mount $SSD_MOUNT")
fi

# ---------------------------------------------------------------- e) services et rechargements
bold "voxtype (service utilisateur, modèle, GPU)"
if [[ ! -f $HOME/.config/systemd/user/voxtype.service ]]; then
  run voxtype setup systemd
else
  info "service déjà installé"
fi
if ! ls "$HOME/.local/share/voxtype/models/"ggml-small.bin &>/dev/null; then
  run voxtype setup --download --model small --quiet
else
  info "modèle « small » présent"
fi
if lspci 2>/dev/null | grep -qiE 'vga.*(nvidia|amd|radeon)'; then
  gpu_status=$(voxtype setup gpu --status 2>/dev/null || true)
  if [[ $gpu_status == *"Active backend: GPU"* ]]; then
    info "GPU déjà actif"
  else
    need_sudo
    run sudo voxtype setup gpu --enable
  fi
fi
run systemctl --user daemon-reload
run systemctl --user enable voxtype.service
run systemctl --user restart voxtype.service

bold "Services Omarchy désactivés"
if systemctl --user is-enabled --quiet omarchy-crash-watch.service 2>/dev/null \
  || systemctl --user is-active --quiet omarchy-crash-watch.service 2>/dev/null; then
  run systemctl --user disable --now omarchy-crash-watch.service
else
  info "omarchy-crash-watch.service déjà désactivé"
fi

bold "Flea par défaut (dossiers, « Afficher dans le dossier », sélecteur de fichiers)"
# Idempotent : mimeapps.list, service D-Bus FileManager1, portals.conf. Le bloc
# de raccourcis qu'il écrit est déjà dans bindings.lua (chezmoi). Nautilus reste
# installé ; « flea --default off » revient en arrière.
run flea --default
run systemctl --user restart xdg-desktop-portal

bold "Thème, shell et Hyprland"
run omarchy theme set kris-theme
run omarchy restart shell
run hyprctl reload

# ---------------------------------------------------------------- f) étapes manuelles
bold "Étapes manuelles restantes"
MANUAL+=(
  "gh auth login  (puis : gh auth setup-git)"
  "Zen : Réglages → Apparence : choisir le thème interne sombre voulu"
  "Musique : installer poste (KristenJestin/poste, ./setup) puis « poste music login » (serveur Navidrome)"
  "Unity Hub : se connecter (puis unity auth login pour la CLI si besoin)"
  "Proton Mail Bridge : lancer, se connecter ; Réglages : « Open on startup » ON, « Automatic updates » OFF"
  "Omamail (SUPER+SHIFT+M) : cliquer « Install backend » dans la fenêtre (release GitHub vérifiée par SHA256SUMS), puis ajouter le compte Proton avec le mot de passe Bridge (IMAP 127.0.0.1:1143 / SMTP 1025, STARTTLS : préréglage Proton)"
  "Proton VPN : se connecter depuis le panneau OmaProton (tuile VPN du control center de l'île) ou « protonvpn signin » ; la tuile apparaît dès que proton-vpn-cli est installé"
  "Secrets non versionnés à restaurer : ~/.ssh, fichiers .env, ~/.npmrc…"
  "Redémarrer la session pour tout valider"
)
for step in "${MANUAL[@]}"; do info "- $step"; done
((DRY_RUN)) && printf '\n(dry-run : rien n’a été exécuté)\n'
exit 0
