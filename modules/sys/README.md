# poste sys

Module de gestion du dépôt `poste`. Toujours installé. Le dépôt est repéré par la clé
`repo` de `~/.config/poste/config.toml` (écrite par `./setup`).

## Commandes

- `poste sys setup [--yes] [--dry-run]` : lance `cargo build --release` dans le dépôt, puis
  installe `~/.local/bin/poste` et, pour chaque `modules/*/` contenant un `module.toml`,
  `~/.local/share/poste/modules/<nom>/` (`poste-<nom>`, `module.toml`, `island/`, `hypr/`).
- `poste sys status` : branche, modifications non commitées, commits non poussés ; modules
  installés (empreinte installée / dépôt) ; fichiers posés manquants ou modifiés à la main.
- `poste sys git <args…>` : `git -C <dépôt> <args…>`.
- `poste sys cd` : affiche le chemin du dépôt (`cd "$(poste sys cd)"`).

## Règles de `setup`

- Relançable : ne fait que la différence (comparaison sha256), « Tout est à jour » sinon.
- Affiche le plan (créé / modifié / inchangé) puis demande confirmation, sauf `--yes`.
- `--dry-run` n'écrit rien (la construction produit seulement `target/`).
- Ne remplace jamais un fichier qu'il n'a pas posé : il refuse, sans rien écrire du tout,
  et dit lequel gêne. Un fichier déjà identique à la source est simplement pris en compte.
- Avant de remplacer un fichier posé, copie l'ancien dans
  `~/.local/state/poste/backup/<chemin aplati>.<AAAAMMJJ-HHMMSS>.bak`.
- Ne supprime rien : un fichier retiré du dépôt reste en place.
- Tient `~/.local/state/poste/installed.json` : chemin, sha256, module propriétaire, date.

## À venir

`diff`, `add`, `sync`, `install <module>`, liens de config et morceaux de config des modules.
