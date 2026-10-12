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
- Lignes de module : jamais modifier ni retirer une ligne absente de `installed.json` (`lines`), jamais écrire à travers un lien symbolique, conflit signalé sans rien écrire.
- Orphelins : seulement des liens vers `<dépôt>/config/` à cible disparue, ou des fichiers de module de `installed.json` encore intacts (déjà disparus : seulement oubliés du manifeste) ; jamais un fichier ordinaire, toujours aperçu et confirmation.
- Un module-script n'a pas de `Cargo.toml` et n'est pas dans `members` du `Cargo.toml` racine.
- Config de la machine (`~/.config/poste/config.toml` : `local`, `disabled`, `[[override]]`) : jamais écrite par `setup`, seulement par `local`/`module`/`override` (toml_edit : le reste du fichier est conservé). Un chemin `local` n'est jamais lié, signalé ni touché (ni comme orphelin).
- Fichier généré (`[[override]]`) : jamais écrasé s'il a été modifié à la main (conflit) ; `--take-home` refuse sans rien écrire si la reprise est ambiguë. Le JSON passe par `ojson.rs` (ordre des clés conservé) ; ne pas activer `preserve_order` de serde_json dans le workspace (cela changerait les autres modules).
- Unités systemd : `setup` les active et désactive via `systemctl --user` (jamais `sudo`) ; les tests utilisent un faux `systemctl` (`POSTE_SYS_TEST_SYSTEMCTL` ou `units::Runner::with_program`), jamais le vrai.
