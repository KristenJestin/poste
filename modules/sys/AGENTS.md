# Règles du module sys

- Toujours installé : `poste sys setup` doit rester utilisable seul, sans autre module.
- Ne jamais remplacer un fichier absent de `installed.json` s'il diffère de la source ; ne jamais rien supprimer.
- Tout fichier remplacé est d'abord copié en `.bak` horodaté dans `~/.local/state/poste/backup/`.
- `--dry-run` n'écrit rien (hors `target/` pour la construction) ; le plan est toujours affiché avant d'écrire.
- Écritures atomiques (fichier temporaire puis renommage) : `poste-sys` se remplace lui-même.
- Le nom du dossier d'un module du dépôt doit être égal à son `name` dans `module.toml`.
- Pas de `sudo`, pas de push, pas de secret dans les sorties.
- Tests unitaires pour toute logique de plan, de classement ou de manifeste.
