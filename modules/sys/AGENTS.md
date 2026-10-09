# Règles du module sys

- Toujours installé : `poste sys setup` doit rester utilisable seul, sans autre module.
- Ne jamais remplacer un fichier absent de `installed.json` s'il diffère de la source ; ne jamais rien supprimer.
- Tout fichier remplacé est d'abord copié en `.bak` horodaté dans `~/.local/state/poste/backup/`.
- `--dry-run` n'écrit rien (hors `target/` pour la construction) ; le plan est toujours affiché avant d'écrire.
- Écritures atomiques (fichier temporaire puis renommage) : `poste-sys` se remplace lui-même.
- Le nom du dossier d'un module du dépôt doit être égal à son `name` dans `module.toml`.
- Pas de `sudo`, pas de push, pas de secret dans les sorties.
- Tests unitaires pour toute logique de plan, de classement ou de manifeste.
- Config : `setup` ne remplace jamais un fichier ordinaire de `~` qui diffère du dépôt sans décision explicite (`--take-repo`, `--take-home`) ; un fichier identique n'est remplacé qu'après sauvegarde. Un conflit n'arrête pas le reste mais donne le code de sortie 2.
- `add` refuse tout chemin qui correspond à `config/.poste/ignore` ou aux règles de secrets intégrées (`.ssh`, `.env*`, `.npmrc`, `.claude/.credentials.json`…).
- Les tests de la config et de l'import travaillent dans un HOME temporaire, jamais dans le vrai `~`.

- Bootstrap : `setup` ne lance JAMAIS `sudo` ; les étapes système sont vérifiées puis affichées. Les étapes utilisateur passent par un aperçu et une confirmation, sont idempotentes, et ne touchent jamais ce qui est ambigu (note affichée). Le contenu des fichiers root vit dans `bootstrap/system/`, pas dans le code.
- Les tests de `provision` n'exécutent aucune étape réelle (pas de systemctl, voxtype, omarchy) ; le faux `pacman -Qq` passe par `POSTE_TEST_PACMAN_QQ`.
