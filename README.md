# poste

`poste` est mon environnement de travail (Arch Linux, Hyprland, Omarchy), en Rust.
Il a remplacé le dépôt de dotfiles géré par chezmoi : config, paquets et mise en place de la machine
sont décrits ici.

## L'aiguillage

`poste` est un aiguillage mince, comme git : `poste <module> <args…>` exécute le
programme `poste-<module>` avec `<args…>` (remplacement de processus sur Unix, code de
sortie transmis). Il ne connaît que `help`, `--help` et `--version` ; aucun nom de
module n'est codé en dur.

Recherche d'un module, dans l'ordre, le premier trouvé gagne :

1. `~/.local/share/poste/modules/<nom>/poste-<nom>`
2. chaque dossier de la clé `modules` de `~/.config/poste/config.toml`
   (`poste-<nom>` directement, ou `<nom>/poste-<nom>`)
3. le `PATH`

`XDG_DATA_HOME`, `XDG_CONFIG_HOME` et `XDG_STATE_HOME` sont respectés.

`poste help` (ou `poste` seul) liste les modules trouvés à ce moment-là, avec la
description de leur fiche. `poste help <module>` affiche les commandes de la fiche.
Un module inconnu donne une erreur claire, la liste des modules et le code de sortie 127.

### Configuration

`~/.config/poste/config.toml` :

```toml
repo = "/chemin/du/depot"          # écrit par ./setup
modules = ["~/mes-modules"]         # facultatif : dossiers de modules supplémentaires
```

### Fiche d'un module : `module.toml`

À côté du programme du module :

```toml
name = "sys"
description = "Gère le dépôt poste"
commands = [{ name = "status", description = "état du dépôt" }]   # facultatif
```

## Modules

| Module | Rôle |
| --- | --- |
| [`sys`](modules/sys/README.md) | gère le dépôt : construction, installation, état, git. Toujours installé. |
| [`music`](modules/music/README.md) | client Navidrome : connexion, copie locale, recherche, lecture, favoris, playlists, mixes. |
| [`record`](modules/record/README.md) | enregistrement d'écran (gpu-screen-recorder) piloté par l'île : préréglages, fenêtres, écrans, zone. Module-script (Python). |

Un module est une application à part, dans `modules/<nom>/`, avec son `README.md`,
son `AGENTS.md` et sa fiche `module.toml`. Son programme est soit une caisse Rust
(`Cargo.toml`, listée dans `members` du `Cargo.toml` racine, binaire `poste-<nom>` construit en
release), soit un script (`poste-<nom>` exécutable à la racine du module, en Python ou en shell,
copié tel quel : pas de `Cargo.toml`, pas dans `members`). `poste` ne fait pas la différence : il
exécute `~/.local/share/poste/modules/<nom>/poste-<nom>`.

### Ce que les modules apportent à la machine

Un module peut fournir, à côté de son programme, des dossiers facultatifs copiés tels quels par
`poste sys setup` dans `~/.local/share/poste/modules/<nom>/` (l'île et Hyprland les chargent sans
connaître aucun module ; module absent, aucune trace) :

- `island/` : pour l'île Quickshell (`config/.config/omarchy/plugins/poste.island`, composant `ModuleRegistry.qml`).
  - `launcher.json` : entrées, extensions et moteurs du lanceur, même format que
    `~/.config/poste-island/launcher/launcher.json` (fusionné avec lui ; la config de l'utilisateur
    gagne en cas d'id identique). Une extension peut avoir `"highlight": true` : l'île encadre à
    l'écran la cible (`highlight: {x,y,w,h,window}`) de la ligne sélectionnée.
  - `island.json` : `{"slots": [{"slot": "<nom>", "file": "Fichier.qml"}]}`. Slots chargés par l'île :
    `player-extra` (ajouts à la carte du lecteur : `detail`, `actionGlyph`/`actionOn`/`action()`,
    `launcherExt`, voir le commentaire dans `Island.qml`) et `capture` (source d'enregistrement
    de même interface qu'`ObsState.qml`, pour la pastille de capture).
  - Découverte au démarrage, puis rechargée à chaud (inotifywait sur le dossier des modules).
- `hypr/*.lua` : morceaux de config Hyprland. `hyprland.lua` charge chaque
  `modules/*/hypr/*.lua` (`loadfile` + `pcall` : une erreur est isolée et notifiée, le reste de la
  config se charge).
- `bin/` : programmes annexes du module.
- `[[line]]` dans `module.toml` : une ligne à poser dans un fichier sans inclusion (voir
  [`modules/sys/README.md`](modules/sys/README.md), « Lignes de module »).

## Installer sur un PC neuf

Point de départ : une Omarchy fraîche, connectée à Internet. `poste` ne lance jamais `sudo` :
chaque fois qu'il faut un droit root, il affiche la commande exacte à copier.

1. Rust et git (une seule fois, à la main) :
   ```bash
   sudo pacman -S --needed rust git
   ```
2. Cloner le dépôt et lancer le premier setup (`~/.local/bin` doit être dans le `PATH`, c'est le cas sous Omarchy) :
   ```bash
   git clone https://github.com/KristenJestin/poste.git ~/Projects/poste
   cd ~/Projects/poste
   ./setup --dry-run   # aperçu complet, n'écrit rien (hors target/)
   ./setup             # même chose en vrai, avec confirmations
   ```
   `./setup` vérifie `cargo`, enregistre l'emplacement du dépôt (clé `repo`), construit en
   release puis lance `poste sys setup`, qui :
   - installe `poste` et ses modules ;
   - pose la config par liens symboliques (conflits avec des fichiers existants : décidés un par un, voir [`modules/sys/README.md`](modules/sys/README.md)) ;
   - compare `bootstrap/packages.toml` à `pacman -Qq` et affiche la commande `yay -S --needed …` des paquets manquants ;
   - affiche en un bloc copiable les étapes système (root) non faites ;
   - propose les étapes utilisateur (plugins du shell aux versions auditées avec leurs patchs, config de cursor-accent, service et modèle voxtype, Flea par défaut, thème) et les fait après confirmation ;
   - liste les étapes manuelles restantes.
3. Lancer vous-même la commande `yay -S --needed …` affichée, puis les commandes `sudo` du bloc « Étapes système ».
4. Relancer `poste sys setup` : les étapes qui dépendaient des paquets (voxtype, Flea, thème…) se font maintenant. Recommencer jusqu'à « rien à faire ».
5. Faire les étapes manuelles, en les marquant au fur et à mesure (`poste sys done <id>`, `poste sys done` pour la liste ; `--undo` pour annuler). `poste sys status` résume tout ce qui reste.
6. Recharger : `omarchy restart shell` et `hyprctl reload`, ou redémarrer la session.

Ensuite, `poste sys setup` (ou `poste sys sync` après un `git pull`) suffit pour réinstaller après une
modification. `--no-config` ne réinstalle que les binaires et saute toute la partie machine.

Posé par `poste sys setup` :

- `~/.local/bin/poste`
- `~/.local/share/poste/modules/<nom>/` : `poste-<nom>`, `module.toml`, et `island/`, `hypr/`, `bin/` s'ils existent
- les lignes que les modules ajoutent dans des fichiers sans inclusion (par exemple `~/.config/hypr/xdph.conf`)
- la liste de ce qui a été posé : `~/.local/state/poste/installed.json`
- les anciennes versions remplacées : `~/.local/state/poste/backup/`
- les étapes manuelles marquées faites : `~/.local/state/poste/done/<id>`

## Organisation du dépôt

```
config/     la config de ~ (liens posés par poste sys setup)
core/       poste-core : chemins XDG, config, découverte des modules, notifications
poste/      l'aiguillage
modules/    un dossier par module (sys, music, record, …)
bootstrap/  paquets, étapes système/utilisateur/manuelles, patchs de plugins (lus par poste sys setup)
setup       premier lancement
```

Vérifications avant commit : `cargo test` et `cargo clippy --all-targets -- -D warnings`.

## La config

Le dossier `config/` reflète `~` (vrais noms, sans préfixes) et est posé par liens symboliques
par `poste sys setup` (aperçu, sauvegardes, conflits tranchés un par un : voir
[`modules/sys/README.md`](modules/sys/README.md)). `poste sys import-chezmoi` reprend la config
gérée jusqu'ici par chezmoi. `bootstrap/` décrit le reste de la machine (paquets, plugins, étapes) : voir [`bootstrap/README.md`](bootstrap/README.md).

## À venir

Commande de `poste sys` prévue, pas encore là : `install <module>`.
