# poste

`poste` est mon environnement de travail (Arch Linux, Hyprland, Omarchy), en Rust.
À terme il remplacera le dépôt de dotfiles géré par chezmoi. À cette étape (étape 1),
il ne gère que ses propres fichiers : l'aiguillage et le module `sys`.

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
| [`music`](modules/music/README.md) | client Navidrome : connexion, copie locale de la bibliothèque, recherche. |

Un module est une application à part, dans `modules/<nom>/`, avec son `README.md`,
son `AGENTS.md` et sa fiche `module.toml`. Il peut fournir des dossiers facultatifs
`island/` et `hypr/`, copiés tels quels à l'installation.

## Installation

Prérequis : Rust (`cargo`) et `~/.local/bin` dans le `PATH`.

```bash
git clone <url> ~/Projects/poste
cd ~/Projects/poste
./setup            # ou ./setup --dry-run, ./setup --yes
```

`./setup` enregistre l'emplacement du dépôt (clé `repo`), construit en release et lance
`poste-sys setup`. Ensuite, `poste sys setup` suffit pour réinstaller après une modification.

Posé par `poste sys setup` :

- `~/.local/bin/poste`
- `~/.local/share/poste/modules/<nom>/` : `poste-<nom>`, `module.toml`, et `island/`, `hypr/` s'ils existent
- la liste de ce qui a été posé : `~/.local/state/poste/installed.json`
- les anciennes versions remplacées : `~/.local/state/poste/backup/`

## Organisation du dépôt

```
core/       poste-core : chemins XDG, config, découverte des modules, notifications
poste/      l'aiguillage
modules/    un dossier par module (sys, music, …)
setup       premier lancement
```

Vérifications avant commit : `cargo test` et `cargo clippy --all-targets -- -D warnings`.

## À venir

Commandes de `poste sys` prévues, pas encore là : `diff`, `add`, `sync`,
`install <module>`, gestion des liens de config et des morceaux de config des modules
(dossiers `.d`, lignes ajoutées dans des fichiers sans inclusion).
