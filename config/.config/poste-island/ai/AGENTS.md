# Bureau poste : commandes utiles

Ce fichier (AGENTS.md) n'est chargé que par l'IA de l'île (poste-ask la lance depuis ce
dossier) et par la skill Claude Code `poste-desktop`. Arch + Omarchy 4,
Hyprland 0.56, shell Quickshell avec l'île perso `poste.island`.

## Île (`omarchy-shell poste.island <commande>`)
- `timer "10m pâtes"` : minuteur (formats : `25`, `5m30`, `1h30`, `90s`, `12:30`, libellé après). `timerStop` : arrête tous les minuteurs de poste.
- Les rappels d'Omarchy (`omarchy-reminder`, SUPER+CTRL+R) s'affichent aussi comme des minuteurs (lus dans systemd toutes les 5 s, jamais écrits dans `timers.json`) : sans pause ni +1 min, ils disparaissent à l'heure (la notification d'Omarchy est l'alerte). `timerCancel "omarchy:<unité>"` en annule un ; `timerStop` ne touche que les minuteurs de poste.
- `ask "question"` : ouvre la vue IA (vide = rouvre la conversation en cours). `askPin` : range la conversation dans la pastille de droite, ou l en retire.
- `askBg "question"` (ou `askBgWith "question" codex`) : lance une IA en arrière-plan, dans une pastille épinglée, sans ouvrir la vue. `askUnpinAll` : retire toutes les pastilles IA au repos.
- `devices` : batteries des périphériques sans fil en JSON (clavier, souris, casque). `deviceSim "mouse 12"` / `batterySim "charging 75"` : simulations (`off` pour arrêter).
- `usage` / `usageOf codex` : détail de la consommation de Claude Code ou Codex (sessions, agents, modèles ; script `bin/poste-agent-usage [session|week] [claude|codex]` en JSON).
- `lock` : son puis verrouillage. `launcher`, `shelf` (vue Fichiers), `menu`, `notifications`, `player`, `controls`, `close`.
- `module wifi|bluetooth|audio|vpn` : ouvre ce panneau. `unread` : nombre de notifications non lues.

## Sons
- `~/.config/omarchy/plugins/poste.island/bin/poste-sfx <événement>` : timer-done, dictation-start/stop/paste, screenshot, lock, unlock, ai-done, notification. Réglages : `~/.config/poste-island/sounds/sounds.json`.

## Fenêtres et applis (Hyprland 0.56, dispatch en Lua)
- Lister : `hyprctl clients -j`. Focus : `hyprctl dispatch 'hl.dsp.focus({ window = "address:0x…" })'`.
- Lancer une appli graphique détachée : `setsid -f uwsm-app -- <appli> >/dev/null 2>&1`.
- Workspaces : 1-10 sur le premier écran de `workspace_monitors` (monitors.lua), 11-20 sur le deuxième, etc.

## Musique (`poste music`, serveur Navidrome)
Lecteur de fond (démon, MPRIS « Musique »), lu aussi par l'île. Les commandes de lecture agissent sur ce que l'utilisateur écoute : n'en lance une que s'il le demande.
- Chercher : `poste music search daft punk --json` (artistes, albums, morceaux, playlists ; `id` de la forme `kind:id`). `poste music playlists`, `poste music mixes`.
- Lire : `poste music play playlist|album|artist|track <id> [--shuffle]` (remplace la file). Ajouter à la file : `poste music queue album <id>`.
- Mixes : `poste music mix me` (perso), `mix rediscover`, `mix artist "Daft Punk"`, `mix genre rock` ; `--print` pour voir la liste sans jouer.
- Contrôles : `poste music toggle|pause|resume|next|prev|stop`, `poste music volume +5` (ou `30`).
- État : `poste music now --json` (titre, artiste, position, qualité, `starred`).
- Favoris et playlists : `poste music like` (morceau en cours, `--toggle` pour inverser), `unlike`, `poste music playlist add <playlistId> current`, `playlist create "Nom" current`.
- Connexion : `poste music login` (terminal interactif, à lancer par l'utilisateur), `poste music status`.
- Dans le lanceur de l'île : mot-clé `play` ou `music`.

## Fichiers
- `flea --gui <dossier>` : gestionnaire de fichiers. `flea shelf add <chemin>` : ajoute à l'étagère (SUPER+D).

## Quotas des agents IA
- `poste usage` : quotas de Claude Code (session de 5 h, semaine), en pourcentage, avec l'heure de remise à zéro. `poste usage codex`, `poste usage --all`, `--refresh` pour refaire le relevé, `--json` pour le brut.
- La source est le collecteur d'Omarchy (`~/.local/state/omarchy/agents/usage/<agent>.json`, la même que le centre de contrôle), jamais `~/.claude` : les journaux de Claude Code ne disent rien des quotas.
- Toute question de consommation (pourquoi le quota de 5 h part plus vite, quel modèle ou quel effort coûte, sous-agents, sessions en parallèle) : ces commandes, jamais une relecture à la main des journaux (doublons, sous-agents, raisonnement : déjà traités). Toutes ont `--json` (lisible avec `jq`) et `--agent claude|codex` ; dates : ISO ou ce que comprend `date -d`.
  - `poste usage history [--since "3 days ago"]` : l'historique des quotas (relevé toutes les 5 min, une ligne quand ça change).
  - `poste usage calls --since … [--until …]` : un appel par ligne, doublons retirés (modèle, effort, sous-agent, tokens d'entrée, de cache lu et écrit, de sortie, de raisonnement).
  - `poste usage windows [--since …]` : chaque fenêtre de 5 h (pic, temps jusqu'au plafond, appels, sessions en parallèle, part de chaque effort et modèle, tokens par point de quota) ; `estimated` : fenêtre devinée d'après les appels, sans relevé.
  - `poste usage compare "14 days ago..7 days ago" "7 days ago.."` : les mêmes indicateurs pour deux périodes, côte à côte.

## Règles
- Jamais de sudo : donner la commande à l'utilisateur.
- Ne rien supprimer, écraser ni pousser sans demande claire.
- Réglages du lanceur : `~/.config/poste-island/launcher/launcher.json`. Code de l'île : `~/.config/omarchy/plugins/poste.island/` (lien vers le dépôt `~/Projects/poste` : une modif est déjà dans le dépôt, il reste `poste sys git commit` puis `push`).
