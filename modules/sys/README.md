# poste sys

Module de gestion du dépôt `poste`. Toujours installé. Le dépôt est repéré par la clé
`repo` de `~/.config/poste/config.toml` (écrite par `./setup`).

## Commandes

- `poste sys setup [--yes] [--dry-run] [--no-config] [--take-repo <chemin>]… [--take-home <chemin>]…` :
  lance `cargo build --release` dans le dépôt, installe `~/.local/bin/poste` et, pour chaque
  `modules/*/` contenant un `module.toml`, `~/.local/share/poste/modules/<nom>/`
  (`poste-<nom>`, `module.toml`, `island/`, `hypr/`), puis pose la config (voir plus bas).
  Code de sortie 2 s'il reste des conflits de config. Sauf avec `--no-config`, enchaîne la partie
  « machine » (voir plus bas).
- `poste sys done [<id>] [--undo]` : marque une étape manuelle (ou système non vérifiable) faite ;
  sans id, les liste. État dans `~/.local/state/poste/done/<id>`.
- `poste sys status` : branche, modifications non commitées, commits non poussés ; modules
  installés (empreinte installée / dépôt) ; fichiers posés manquants ou modifiés à la main ;
  config (liés, manquants, liens cassés, liens remplacés par un fichier ordinaire, conflits) ;
  paquets manquants, étapes système, utilisateur et manuelles en attente.
- `poste sys diff [<chemin>]` : écarts entre `config/` et `~` (diff unifié).
- `poste sys add <chemin> [--dry-run]` : met un fichier (ou un dossier) de `~` sous gestion :
  copié dans `config/` (après sauvegarde), remplacé par un lien. Refuse ce qui correspond à
  `config/.poste/ignore` (secrets : `.ssh/**`, `.claude/.credentials.json`, `**/.env*`, `.npmrc`…).
- `poste sys forget <chemin>` : remet un fichier ordinaire à la place du lien ; le fichier reste
  dans le dépôt jusqu'à votre `git rm` et votre commit.
- `poste sys sync [--yes] [--dry-run]` : `git pull --ff-only`, puis `setup` (aperçu compris).
- `poste sys import-chezmoi [--source <dossier>] [--dry-run]` : voir « Import depuis chezmoi ».
- `poste sys git <args…>` : `git -C <dépôt> <args…>`.
- `poste sys cd` : affiche le chemin du dépôt (`cd "$(poste sys cd)"`).

## Règles de `setup`

- Relançable : ne fait que la différence (comparaison sha256), « Tout est à jour » sinon.
- Affiche le plan (créé / modifié / inchangé, puis la config) puis demande confirmation, sauf `--yes`.
- `--dry-run` n'écrit rien (la construction produit seulement `target/`).
- Ne remplace jamais un fichier qu'il n'a pas posé : il refuse, sans rien écrire du tout,
  et dit lequel gêne. Un fichier déjà identique à la source est simplement pris en compte.
- Avant de remplacer un fichier posé, copie l'ancien dans
  `~/.local/state/poste/backup/<chemin aplati>.<AAAAMMJJ-HHMMSS>.bak`.
- Ne supprime rien : un fichier retiré du dépôt reste en place.
- Tient `~/.local/state/poste/installed.json` : chemin, sha256, module propriétaire (`config` pour la config), date.

## La config

Le dossier `config/` du dépôt reflète `~` avec les vrais noms (`config/.config/hypr/bindings.lua`
pour `~/.config/hypr/bindings.lua`), sans préfixes. `setup` pose chaque entrée en lien
symbolique vers le dépôt : modifier le fichier de `~` modifie le dépôt.

- Un lien symbolique de `config/` est recréé tel quel (même cible) dans `~`.
- Le bit exécutable est porté par git. Les droits restreints (0600, 0700) sont listés dans
  `config/.poste/modes`, une ligne `chemin mode` (par exemple `.config/foot/foot.ini 0600`), et
  réappliqués à la cible réelle du lien (le fichier du dépôt ; pour un simple dossier, le dossier de `~`).
- Granularité : un lien par fichier. Un dossier listé dans `config/.poste/link-dirs` (un chemin par
  ligne) est lié d'un bloc ; à réserver aux dossiers dont tout le contenu appartient au dépôt et où
  aucun outil n'écrit de fichiers propres. `__pycache__` est ignoré dans les comparaisons et dans git.
- `config/.poste/ignore` : motifs (relatifs à `~`) que `add` refuse d'ajouter.

État de chaque cible et action de `setup` :

| État de la cible | Action |
| --- | --- |
| déjà liée comme prévu | rien |
| absente | crée le lien |
| fichier ordinaire identique au dépôt | sauvegarde dans `backup/`, puis lien |
| fichier ordinaire différent | CONFLIT : rien n'est fait ; `poste sys diff <chemin>` pour voir |
| lien vers ailleurs, dossier à la place d'un fichier ou l'inverse, parent qui n'est pas un dossier | CONFLIT |

Les conflits n'empêchent pas le reste ; ils sont listés en fin d'aperçu et le code de sortie est 2.
Pour trancher un par un : `--take-repo <chemin>` (le dépôt gagne, l'ancien est sauvegardé) ou
`--take-home <chemin>` (le fichier réel est recopié dans le dépôt, sauvegardé, puis lié ; pour un
fichier aux droits restreints, le mode est noté dans `modes`). Un dossier réel remplacé par un lien
est déplacé tel quel dans `backup/`.

## La machine : paquets et étapes (`bootstrap/`)

Après les binaires et la config, `setup` lit `bootstrap/packages.toml` et `bootstrap/steps.toml`
(format : [`bootstrap/README.md`](../../bootstrap/README.md)) :

1. Paquets : comparés à `pacman -Qq`. S'il en manque, la commande `yay -S --needed …` est AFFICHÉE.
2. Étapes système (root) : vérifiées, puis les commandes sont AFFICHÉES dans un bloc copiable.
   `setup` ne lance jamais `sudo`.
3. Étapes utilisateur (plugins du shell épinglés + patchs, fichiers, services, thème…) : aperçu des
   commandes, puis confirmation (sauf `--yes`) ; `--dry-run` n'en lance aucune. Idempotentes : un
   état déjà bon n'apparaît pas. Ce qui est ambigu ou risqué (plugin à une autre version et modifié,
   patch qui ne s'applique plus, fichier utilisateur différent) n'est jamais touché, une note le dit.
   Si une étape a été faite, la config est replanifiée (`omarchy plugin add` peut réécrire
   `shell.json`) pour remettre les liens du dépôt.
4. Étapes manuelles non faites, listées en fin de `setup`.

Test : la variable `POSTE_TEST_PACMAN_QQ` (réservée aux tests) remplace `pacman -Qq` par le
programme indiqué (sans argument).

## Import depuis chezmoi

`poste sys import-chezmoi` liste les entrées de `chezmoi managed` et copie dans `config/` le
contenu RÉEL de la machine (pas celui de la source chezmoi), avec bit exécutable, droits restreints
et liens. Il n'écrit que dans le dépôt poste et dans `~/.local/state/poste/import-chezmoi.md`
(rapport : écarts avec la source, fichiers absents, entrées `DA`, dossiers candidats au lien de
dossier, secrets refusés sans recopier leur valeur). `README.md`, `bootstrap.sh` et
`bootstrap-patches/` de chezmoi iraient dans `bootstrap/` : l'import est fait une fois pour toutes et ces
fichiers ont été repris dans `bootstrap/*.toml` ; ne pas relancer l'import (il les recréerait). Un fichier qui contient un secret évident n'est pas importé. Relançable.

## À venir

`install <module>` et les morceaux de config des modules (dossiers `.d`, lignes ajoutées dans des
fichiers sans inclusion).
