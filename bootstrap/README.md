# dotfiles — Omarchy 4 (« Quattro ») de kris

Dotfiles gérés avec [chezmoi](https://chezmoi.io) pour une Arch + Omarchy 4 sur trois
écrans (DP-3 à gauche en portrait, DP-1 au centre, DP-2 à droite).

## Ce que contient le dépôt

| Brique | Où | Rôle |
|---|---|---|
| **kris.island** | `~/.config/omarchy/plugins/kris.island` | Remplace la barre Omarchy : une « île » par écran (workspaces de l'écran, heure, musique, point de notifications, cadenas quand le VPN est connecté), qui se déplie en lanceur, menu Omarchy, control center, liste des notifications, lecteur. Retour volume, dictée voxtype (onde + bandeau de sous-titres), masquage en plein écran, voile sombre derrière les panneaux. |
| ↳ control center | clic sur l'heure | Calendrier, **Usage IA** (Claude Code / Codex : session 5 h et hebdo, via `omarchy-agent-usage-update` toutes les 5 min), tuiles Réseau / Bluetooth / Audio, **Mail** (Omamail, badge non-lus) et **VPN** (OmaProton, seulement si `proton-vpn-cli` est installé), tray. |
| **kris.notifications** | `~/.config/omarchy/plugins/kris.notifications` | Clone du service de notifications d'Omarchy (même historique, DND, actions) avec des toasts en verre dépoli centrés sous l'île. Chaque toast reste sur l'écran où elle est apparue (minuteur continu) ; les nouvelles suivent l'écran actif. |
| **Plugins tiers** (installés par `bootstrap.sh`, figés) | `~/.config/omarchy/plugins/` | crmne.mpris, jankeesvw.notification-center, localsend, **omamail** (client mail, SUPER+SHIFT+M), **OmaProton VPN** (panneau Proton VPN, monté une fois sur l'écran central), **cursor-accent** (curseur à l'accent du thème, patch local `stroke_scale` dans `bootstrap-patches/`). |
| **kris.workspaces** | `~/.config/omarchy/plugins/kris.workspaces` | Widget de workspaces par écran (utilisé avec la barre native). |
| **kris-theme** | `~/.config/omarchy/themes/kris-theme` | Palette sombre façon Linear, accent orange `#ff8226`, fond d'écran « winding road ». |
| **Modèles de thème** | `~/.config/omarchy/themed/*.tpl` | Générés à chaque `omarchy theme set` : T3 Code, GTK4 (Nautilus), et `shell.toml` (remplace celui d'Omarchy : panneaux natifs Réseau/Bluetooth/Audio/VPN au look de l'île, fond quasi noir, bordure 1 px discrète). |
| **Hyprland** | `~/.config/hypr/*.lua` | Écrans, workspaces par écran (1–10 à gauche, 11–20 au centre, 21–30 à droite), raccourcis (ceux des applis désinstallées et des webapps retirés), look (flou, animations, opacité), flou des toasts / bandeau de dictée, voile + popin des panneaux natifs. |
| **Lanceur** | `~/.config/kris-island/` | `launcher.json` (éléments, dont Proton VPN et Mail, et extensions) + `sources/` (Projets Unity) ; la musique (mot-clé `play`) vient de `poste music`. |
| **voxtype** | `~/.config/voxtype/config.toml`, `~/.local/bin/voxtype-*`, drop-in systemd | Dictée : collage d'un coup (attend que SUPER soit relâché avant de coller), 600 s max, retour du focus, mode eager, OSD natif désactivé (l'île l'affiche). |
| **Veille** | `~/.config/omarchy/shell.json` (`idle`) | Économiseur d'écran à 45 min, verrouillage à 1 h 30. |
| Divers | `~/.config/foot`, `gtk-4.0`, `git`, `.bashrc`, `.claude/settings.json`… | Terminal (flou, copier/coller), GTK4, etc. |

## Réinstallation

1. Installer Omarchy 4.
2. Récupérer les dotfiles :
   ```sh
   chezmoi init --apply git@github.com:KristenJestin/dotfiles.git
   # ou https://github.com/KristenJestin/dotfiles.git
   ```
3. Lancer le script (d'abord en simulation) :
   ```sh
   bash ~/.local/share/chezmoi/bootstrap.sh --dry-run
   bash ~/.local/share/chezmoi/bootstrap.sh
   ```
   Il installe les paquets manquants (yay, dont `proton-vpn-cli`, `proton-vpn-daemon`,
   `protonmail-bridge`), les plugins shell tiers aux versions auditées
   (crmne.mpris `85bb4ff`, jankeesvw.notification-center `e4c4568`,
   io.github.honeyspoons.localsend `eef3289`, omamail `v0.10.8` = `a7388fa`,
   io.github.grichard99.omaproton-vpn `6d68681`, io.github.smoothpixels.cursor-accent
   `97e001c` + patch `stroke_scale` et sa config), la règle polkit systemd-creds pour Unity,
   la ligne fstab du SSD NTFS (si le disque est présent), le service voxtype (modèle
   « small », GPU), désactive `omarchy-crash-watch.service`, puis applique kris-theme et
   relance le shell. Il est idempotent.
4. Faire les étapes manuelles ci-dessous, puis redémarrer la session.

`bootstrap.sh`, `bootstrap-patches/` et ce README sont ignorés par `chezmoi apply` (voir `.chezmoiignore`).

## Étapes manuelles

- **GitHub** : `gh auth login` puis `gh auth setup-git`.
- **Zen** : choisir le thème interne sombre dans ses réglages.
- **Musique** : installer `poste` (dépôt `KristenJestin/poste`, `./setup`), puis `poste music login`
  pour se connecter au serveur Navidrome.
- **Unity Hub** : se connecter (et `unity auth login` pour la CLI si besoin).
- **Proton Mail Bridge** : le lancer, se connecter ; Réglages : *Open on startup* **ON**,
  *Automatic updates* **OFF**. Noter le mot de passe Bridge (≠ mot de passe Proton).
- **Omamail** (`SUPER+SHIFT+M` ou tuile Mail) : cliquer **Install backend** dans la fenêtre
  (binaire de la release GitHub, vérifié par `SHA256SUMS` ; pas de `curl | sh`), puis
  ajouter le compte Proton avec le mot de passe Bridge (préréglage Proton : IMAP
  `127.0.0.1:1143`, SMTP `127.0.0.1:1025`, STARTTLS).
- **Proton VPN** : se connecter depuis le panneau OmaProton (tuile **VPN** du control
  center, visible dès que `proton-vpn-cli` est installé) ou `protonvpn signin`.
- *(Optionnel)* **IP fixe** (non versionnée) :
  `nmcli con mod "<connexion>" ipv4.method manual ipv4.addresses 192.0.2.10/24 ipv4.gateway 192.0.2.1 ipv4.dns "1.1.1.1 1.0.0.1" ipv4.ignore-auto-dns yes ipv6.dns "2606:4700:4700::1111 2606:4700:4700::1001" ipv6.ignore-auto-dns yes && nmcli con up "<connexion>"` (DNS Cloudflare)
- **Non versionné** : secrets (`~/.ssh`, `.env`, `~/.npmrc`, identifiants), `~/.local/state`
  (historique du lanceur, notifications…), caches.

## Raccourcis principaux

| Raccourci | Action |
|---|---|
| `SUPER+ESPACE` | Lanceur de l'île (applis, calcul, web, presse-papier avec `Tab`, extensions ; `Ctrl+B` = actions) |
| `SUPER+ALT+ESPACE` | Menu Omarchy dans l'île |
| `SUPER+CTRL+ALT+ESPACE` | Menu Omarchy natif |
| `SUPER+1…0` / `SUPER+SHIFT+1…0` | Workspace N de l'écran courant / y envoyer la fenêtre |
| `SUPER+TAB` / `SUPER+SHIFT+TAB` | Workspace suivant / précédent sur l'écran |
| `SUPER+CTRL+ALT+←/↑/→` | Envoyer la fenêtre sur l'écran gauche / centre / droite |
| `SUPER+A` | Dictée voxtype (appui = bascule, maintien = push-to-talk ; le collage attend le relâchement de SUPER) |
| `SUPER+L` | Verrouiller |
| `SUPER+E` | Fichiers (dossier courant) |
| `SUPER+SHIFT+M` | Mail (Omamail) |
| `SUPER+SHIFT+.` | Proton Pass |
| `SUPER+CTRL+W` / `SUPER+CTRL+A` / `SUPER+CTRL+B` | Vues Wi-Fi / Sound / Bluetooth dans l’île (2e appui ferme ; « Advanced settings… » → panneau natif) |

Retirés : `SUPER+SHIFT+SPACE` (barre), les raccourcis vers des applis désinstallées
(`SUPER+SHIFT+C`, `SUPER+SHIFT+W`, `SUPER+SHIFT+ALT+M`, `SUPER+SHIFT+/`, `SUPER+SHIFT+G`,
`SUPER+CTRL+ENTRÉE`, `SUPER+CTRL+K`) et ceux des webapps (ChatGPT, Grok, HEY, YouTube,
WhatsApp, Google Messages/Photos/Maps, X).

## Ajouter une source au lanceur

`~/.config/kris-island/launcher.json` (rechargé à chaud) :

```json
{
  "items": [
    { "name": "GitHub", "icon": "🐙", "keywords": ["git"], "type": "url", "url": "https://github.com" },
    { "name": "Moniteur", "icon": "utilities-system-monitor", "type": "terminal", "command": "btop", "cwd": "~" }
  ],
  "extensions": [
    { "id": "unity", "name": "Projets Unity", "icon": "unityhub", "ttl": 300,
      "source": { "type": "script", "command": "~/.config/kris-island/sources/unity-projects" } }
  ]
}
```

- Types d'action : `url`, `command` (détaché, `cwd` optionnel), `terminal`, `app` (`id` = desktop id, `args`), `copy` (`text`).
- `icon` : nom d'icône système, chemin de fichier ou emoji. `"enabled": false` désactive une entrée.
- Une extension apparaît avec un chevron ; `→`/Entrée y entre, `←`/Échap revient. Son script
  (exécutable) imprime un JSON :
  ```json
  [{ "id": "…", "name": "…", "subtitle": "…", "icon": "…", "keywords": ["…"], "path": "…",
     "action": { "type": "command", "command": "…" },
     "actions": [{ "label": "…", "icon": "󰉋", "action": { "type": "copy", "text": "…" } }] }]
  ```
  Le résultat est mis en cache `ttl` secondes. Les éléments sont aussi cherchables depuis la racine.
- Classement : épinglés (`Ctrl+B` → Épingler), puis « frecency » (usages récents), puis A→Z.
  État dans `~/.local/state/kris-island/launcher.json`.

## Revenir à la barre native

```sh
jq 'del(.bar.id)' ~/.config/omarchy/shell.json > /tmp/shell.json && mv /tmp/shell.json ~/.config/omarchy/shell.json
omarchy plugin disable kris.notifications && omarchy plugin enable omarchy.notifications
omarchy restart shell
```

Les raccourcis de l'île (`SUPER+ESPACE`, `SUPER+ALT+ESPACE`, volume) sont dans
`~/.config/hypr/bindings.lua` : les remettre sur les commandes natives
(`omarchy-menu toggle`, `omarchy-audio-output-volume`) si besoin.
