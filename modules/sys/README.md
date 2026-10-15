# poste sys

Module de gestion du dépôt `poste`. Toujours installé. Le clone du dépôt est repéré par la clé
`repo` de `~/.config/poste/config.toml` (écrite par `./setup`) ; ce que `setup` pose se lit dans la
racine `~/.local/share/poste/current`, qui désigne le clone (mode dev) ou une version installée
(mode release) : voir « Versions et racine ».

## Commandes

- `poste sys setup [--yes] [--dry-run] [--no-config] [--take-repo <chemin>]… [--take-home <chemin>]…` :
  pose `current` s'il est absent (dev : vers le clone ; release : vers la version installée la plus
  récente), lance `cargo build --release` si la racine est le clone (une version installée arrive
  construite), installe `~/.local/bin/poste` et, pour chaque
  `modules/*/` contenant un `module.toml`, `~/.local/share/poste/modules/<nom>/`
  (`poste-<nom>`, `module.toml`, `island/`, `hypr/`, `bin/`), puis pose la config, les lignes de
  module et retire les orphelins (voir plus bas). Le programme d'un module est le binaire construit
  s'il a un `Cargo.toml`, sinon le script `poste-<nom>` du dossier du module.
  Code de sortie 2 s'il reste des conflits de config. Sauf avec `--no-config`, enchaîne la partie
  « machine » (voir plus bas).
- `poste sys done [<id>] [--undo]` : marque une étape manuelle (ou système non vérifiable) faite ;
  sans id, les liste. État dans `~/.local/state/poste/done/<id>`.
- `poste sys status` : source (dev ou release) et racine lue ; branche du clone, modifications non commitées, commits non poussés ; modules
  installés (empreinte installée / dépôt) ; fichiers posés manquants ou modifiés à la main ;
  config (liés, manquants, liens cassés, liens remplacés par un fichier ordinaire, conflits) ;
  lignes de module (en place, conflits) ; orphelins ;
  paquets manquants, étapes système, utilisateur et manuelles en attente.
- `poste sys diff [<chemin>]` : écarts entre `config/` et `~` (diff unifié).
- `poste sys add <chemin> [--dry-run]` : met un fichier (ou un dossier) de `~` sous gestion :
  copié dans `config/` (après sauvegarde), remplacé par un lien. Refuse ce qui correspond à
  `config/.poste/ignore` (secrets : `.ssh/**`, `.claude/.credentials.json`, `**/.env*`, `.npmrc`…).
- `poste sys forget <chemin>` : remet un fichier ordinaire à la place du lien ; le fichier reste
  dans le dépôt jusqu'à votre `git rm` et votre commit.
- `poste sys sync [--yes] [--dry-run]` : en dev, `git pull --ff-only` dans le clone et `current` remis sur le clone ; en release, la dernière version publiée, téléchargée et vérifiée si elle n'est pas déjà installée, et `current` dessus (voir « Versions et racine »). Dans les deux modes, `git pull --ff-only` dans le dépôt de données de la machine (`data`, voir le README racine), puis `setup` (aperçu compris), puis l'élagage des anciennes versions (release). Ensuite, selon les fichiers changés par le pull, par le changement de version (les deux racines comparées) ou posés par `setup`, `omarchy restart shell` (île : `config/.config/omarchy/`, `poste-island/`, `fontconfig/`, `island/` des modules) et `hyprctl reload` (`config/.config/hypr/`, `hypr/` et `module.toml` des modules), après confirmation sauf avec `--yes`. L'île n'est pas redémarrée si `gpu-screen-recorder` tourne (le message dit de le faire plus tard).
- `poste sys import-chezmoi [--source <dossier>] [--dry-run]` : voir « Import depuis chezmoi ».
- `poste sys local [list|add <chemin>|remove <chemin>]` : fichiers de `config/` que CETTE machine garde
  à elle (voir « Réglages propres à la machine »).
- `poste sys module [list|enable <nom>|disable <nom>]` : modules du dépôt non installés ici.
- `poste sys skip [list|add <id>…|remove <id>…]` : paquets et étapes du bootstrap ignorés ici.
- `poste sys override [list|set <chemin> <clé> <valeur>|unset <chemin> <clé>]` : surcharges JSON par machine.
- `poste sys use dev|release|<commit>` `[--yes] [--dry-run]` : bascule la machine (voir « Versions et racine »).
- `poste sys versions` : source, racine (`current`) et versions installées (`*` : celle en place).
- `poste sys git <args…>` : `git -C <clone> <args…>`.
- `poste sys cd` : affiche le chemin du clone (`cd "$(poste sys cd)"`).

## Versions et racine

- Racine : le lien `~/.local/share/poste/current`. `config/`, `modules/`, `bootstrap/` et les binaires
  (`target/release/`) s'y lisent ; les liens de config de `~` visent `current/config/<chemin>`, si
  bien que changer `current` change la config sans reposer les liens. Une version a la disposition
  du dépôt plus un fichier `VERSION` (`commit=…`, `date=…`) et `ARCHIVE_SHA256` (somme de l'archive
  d'où elle vient) ; elle n'a pas de `Cargo.toml`.
- Clé `source` de `config.toml` : `dev` (défaut quand elle manque) ou `release`. Écrite seulement par
  `poste sys use` (toml_edit, le reste du fichier est gardé), jamais par `setup` ; propre à la
  machine (ignorée dans le `poste.toml` du dépôt de données).
- `poste sys use dev` : `source = "dev"`, `current` vers le clone, `setup` (qui construit).
  `poste sys use release` : `source = "release"`, `current` vers la version installée la plus
  récente (téléchargée s'il n'y en a aucune), `setup`. `poste sys use <début de commit>` : une
  version installée précise (retour arrière ; le prochain `sync` revient à la dernière). Ensuite,
  comme `sync` : rechargement de l'île et de Hyprland selon les fichiers qui diffèrent entre
  l'ancienne et la nouvelle racine, après confirmation sauf `--yes`, jamais l'île pendant un enregistrement d'écran.
  `--dry-run` montre la bascule et le rechargement prévu sans rien écrire ni lancer `setup`.
- Téléchargement (`sync` en release, `use release` sans version installée) : `curl` lit d'abord
  `poste-x86_64-linux.tar.gz.sha256` à l'adresse publiée (`POSTE_SYS_RELEASE_URL` la remplace, par
  exemple pour un fork) ; si une version installée vient de la même archive, rien n'est téléchargé.
  Sinon l'archive va dans `~/.cache/poste/download/`, sa somme est vérifiée AVANT l'extraction
  (différente : refusée, rien n'est installé), puis elle est extraite par `tar` dans un dossier
  temporaire de `versions/`, renommé en `versions/<commit>/`. `--dry-run` ne lit que la somme.
- Élagage : après un `sync` en release, seules les 3 versions les plus récentes (date de
  construction) restent, plus celle de `current` si elle est plus ancienne ; jamais celle de
  `current`.
- Relais (`sync` et `use`, sauf `--dry-run`) : une fois `current` posé, si le `poste-sys` de la
  nouvelle racine (`target/release/poste-sys`, construit d'abord pour le clone) diffère de celui
  qui tourne, celui-ci lui passe la main (`exec`) par la commande cachée `poste-sys finish`
  (`--yes`, `--prune`, `--reload-shell`, `--reload-hypr`) : `setup`, élagage et rechargement sont
  faits par le nouveau programme, qui connaît les nouveautés de sa version (par exemple les
  minuteurs systemd). Le programme relayé ne repasse jamais la main (`POSTE_SYS_RELAYED`). Les
  options de `finish` ne changent pas d'une version à l'autre : on en ajoute seulement.
- Lecture seule en release : `add` et `setup --take-home` écrivent dans le dépôt, ils sont refusés
  tant que `current` désigne une version (passer en dev). `import-chezmoi`, `git` et `cd` travaillent
  toujours sur le clone.
- Migration d'une machine existante : un lien de `~` vers `config/<chemin>` d'une autre racine de
  poste (le clone, une version, `current` lui-même) au lieu de `current/config/<chemin>` est
  « à relier ». `setup` le repose vers `current` (ancien lien sauvegardé dans `backup/`) seulement s'il
  est noté dans `installed.json` ; sinon c'est un conflit comme un autre lien étranger, jamais touché.

## Règles de `setup`

- Relançable : ne fait que la différence (comparaison sha256), « Tout est à jour » sinon.
- Affiche le plan (créé / modifié / inchangé, puis la config) puis demande confirmation, sauf `--yes`.
- `--dry-run` n'écrit rien (la construction produit seulement `target/`).
- Ne remplace jamais un fichier qu'il n'a pas posé : il refuse, sans rien écrire du tout,
  et dit lequel gêne. Un fichier déjà identique à la source est simplement pris en compte.
- Avant de remplacer un fichier posé, copie l'ancien dans
  `~/.local/state/poste/backup/<chemin aplati>.<AAAAMMJJ-HHMMSS>.bak`.
- Ne supprime que les orphelins (voir plus bas), après aperçu et confirmation : un fichier retiré du dépôt reste en place tant que ce n'est pas un lien ou un fichier de module posé par nous et intact.
- Tient `~/.local/state/poste/installed.json` : chemin, sha256, module propriétaire (`config` pour la config), date.

## Réglages propres à la machine

Le dépôt est le même sur tous les PC (pas de profils par machine). Ce qui diffère d'une machine à
l'autre vit dans `~/.config/poste/config.toml`, LOCAL à la machine et jamais dans le dépôt. Quatre clés :

```toml
local = [".config/hypr/monitors.lua", ".config/hypr/input.lua"]   # fichiers à cette machine
disabled = ["record", "power"]                                      # modules non installés ici
skip = ["gimp", "io.github.grichard99.omaproton-vpn"]               # paquets / étapes du bootstrap ignorés ici

[[override]]                                                        # JSON : dépôt + valeurs locales
path = ".config/omarchy/shell.json"
set = { "idle.lock" = 3600 }
```

Les quatre s'éditent sans ouvrir le fichier (commentaires et autres clés conservés).

### `local` : fichiers à la machine

- `setup` ne lie pas ces chemins, ne les signale pas en conflit, ne les touche jamais (ni liaison,
  ni remplacement, ni droits, ni retrait comme orphelin). Un chemin peut être un dossier de `config/`
  (tout ce qu'il contient est local) ; un fichier dans un dossier lié d'un bloc (`link-dirs`) se
  déclare par ce dossier.
- `status` les liste dans « Fichiers locaux » avec leur état face au dépôt (absent, identique,
  différent, encore lié) ; `diff <chemin>` montre quand même l'écart (`diff` sans argument les saute).
- `local add <chemin>` refuse un chemin que `config/` ne connaît pas. S'il est encore un lien vers le
  dépôt, il est laissé tel quel (l'éditer modifierait le dépôt) : `poste sys forget <chemin>` en fait
  un fichier ordinaire. `local remove` remet le chemin sous gestion (conflit à trancher s'il diffère).

### `skip` : paquets et étapes du bootstrap ignorés

- Un identifiant est un nom de paquet de `bootstrap/packages.toml` ou l'`id` d'une étape de
  `bootstrap/steps.toml` (`[[plugin]]`, `[[file]]`, `[[user]]`, `[[system]]`, `[[manual]]`).
- Ce qui est ignoré n'est ni exigé, ni proposé, ni compté comme manquant dans `setup`,
  `setup --dry-run` et `status`, qui ajoutent une ligne
  `ignorés sur cette machine (config.toml, skip) : …`.
- Dépendances : une étape utilisateur dont un `trigger` est ignoré l'est aussi (par ex.
  `cursor-accent-recolor` quand `io.github.smoothpixels.cursor-accent` ou `cursor-accent-config`
  est dans `skip`), en cascade. `requires` désigne un binaire, pas une étape : non concerné.
- Un identifiant inconnu (faute de frappe) donne un avertissement, pas une erreur ; `skip add` l'ajoute
  quand même. `skip remove` échoue si un identifiant n'y est pas (les autres sont retirés).
- Rien n'est désinstallé : poste n'a jamais lancé pacman, et un plugin ou un fichier déjà posé reste.
  `poste sys done` reste utilisable sur une étape ignorée.

### `disabled` : modules non installés

- `setup` n'installe pas ces modules ; s'ils l'étaient, il les retire proprement par la mécanique
  des orphelins, avec aperçu et confirmation : fichiers (`poste-<nom>`, `module.toml`, `island/`,
  `hypr/`, `bin/`, `systemd/`), lignes posées dans des fichiers sans inclusion, unités systemd
  (désactivées d'abord). Un fichier modifié à la main est gardé. `sys` ne se désactive pas.
- `module enable` réactive : le prochain `setup` réinstalle. Un nom inconnu du dépôt est signalé par `module list`.

### `[[override]]` : JSON par machine

Pour un fichier JSON de `config/` dont le reste doit suivre le dépôt (cas réel : `idle.lock` du
`shell.json` d'Omarchy, la barre et les plugins venant du dépôt). `set` : clés pointées
(`"idle.lock" = 3600`, ou en table imbriquée `[override.set.idle] lock = 3600`, aplatie ; pas
d'indices de tableaux). Une valeur est un nombre, booléen, chaîne ou liste.

- Un fichier ayant des surcharges n'est PAS lié : `setup` écrit un fichier ordinaire égal au contenu du
  dépôt plus les surcharges, JSON réécrit proprement (indentation 2, ordre des clés du dépôt conservé).
  Il est noté dans `installed.json` comme les autres entrées de config (empreinte du texte écrit) et
  régénéré quand le dépôt ou les surcharges changent (« régénéré » dans l'aperçu, ancien sauvegardé).
- Un ancien lien vers le dépôt, ou un fichier qui contient déjà les mêmes valeurs (même JSON, autre
  mise en forme, cas du PC pro réglé à la main), est remplacé sans conflit (sauvegarde faite).
- Modifié à la main depuis notre écriture : CONFLIT (code 2, rien n'est touché ; `status` : « généré,
  modifié à la main » ; `diff <chemin>` compare à ce que `setup` écrirait).
  - `--take-repo <chemin>` régénère (l'ancien est sauvegardé).
  - `--take-home <chemin>` recopie le fichier de `~` dans le dépôt EN REMETTANT, pour chaque clé
    surchargée, la valeur du dépôt (ou en retirant la clé si le dépôt ne l'avait pas), puis régénère. Le
    JSON du dépôt est réécrit avec l'indentation 2. Choix : la reprise est REFUSÉE, sans rien écrire, si une valeur
    surchargée a été changée à la main dans `~` (ou a disparu) ou si le fichier n'est pas du JSON valide : on ne peut pas deviner
    si l'on voulait changer la surcharge ou le dépôt. Corriger la surcharge (`override set`) ou utiliser `--take-repo`.
- `override set <chemin> <clé> <valeur>` : la valeur est du JSON (`3600`, `true`, `"texte"`, `[1,2]`),
  sinon une chaîne ; refusé si la clé traverse une valeur qui n'est pas un objet. `override unset`
  retire la clé ; la dernière retirée supprime l'entrée et le fichier redevient un lien au prochain
  `setup` (un fichier généré qui diffère alors du dépôt est un conflit à trancher). `override` seul liste.
- Un chemin à la fois `local` et surchargé reste local.

## Unités systemd utilisateur des modules

Un module déclare `units = ["poste-<nom>.service"]` dans `module.toml` et range le fichier d'unité dans
son dossier `systemd/` (copié avec le reste dans `~/.local/share/poste/modules/<nom>/systemd/`).

- `setup` l'active avec `systemctl --user enable --now <chemin du fichier installé>` (systemd crée
  lui-même le lien dans `~/.config/systemd/user/` ; rien n'est copié ailleurs) et la note dans
  `installed.json` (`units`). Quand les fichiers du module sont mis à jour par ce `setup`, l'unité est
  rechargée et redémarrée (`daemon-reload`, `try-restart`).
- Module retiré, désactivé (`disabled`) ou unité retirée de `module.toml` : `systemctl --user disable
  --now` AVANT le retrait des fichiers, puis l'entrée est oubliée.
- Minuteur : `units = ["poste-<nom>.timer"]`, et le service qu'il déclenche (`poste-<nom>.service`,
  même nom) à côté dans `systemd/` sans être déclaré. `setup` lie d'abord le service
  (`systemctl --user link`, pour que systemd le trouve), puis active le minuteur ; au retrait, il
  désactive le minuteur puis délie le service. Un lien du service qui n'est pas à nous est un conflit.
- Une unité du même nom que nous n'avons pas posée (fichier ordinaire, lien ailleurs) est un conflit (code 2),
  jamais touchée. `--dry-run` n'active rien (seules des lectures `is-enabled`).
- Test : `POSTE_SYS_TEST_SYSTEMCTL` (RÉSERVÉE AUX TESTS) remplace `systemctl`.

## Lignes de module (fichiers sans inclusion)

Certains fichiers n'ont pas de mécanisme d'inclusion (cas réel : `~/.config/hypr/xdph.conf`,
où le module `record` pose `screencopy:custom_picker_binary = …`). Un module déclare dans son
`module.toml` :

```toml
[[line]]
file = "~/.config/hypr/xdph.conf"            # sous ~ seulement
line = "screencopy:custom_picker_binary = {modules}/record/bin/poste-share-picker"
```

`{home}` (dossier personnel) et `{modules}` (`~/.local/share/poste/modules`) sont remplacés.
`config/.poste/lines.toml` (même format, propriétaire `config`) porte les lignes de base du dépôt.

- `setup` ajoute la ligne à la fin du fichier si elle manque (fichier créé s'il n'existe pas) et
  note `{file, line, module}` dans `installed.json` (`lines`). Le fichier existant est d'abord sauvegardé.
- Si le module ne la déclare plus ou disparaît, la ligne est retirée ; si elle avait déjà disparu du fichier,
  l'entrée est seulement oubliée. Si la valeur change, la ligne qu'on avait posée est remplacée.
- Jamais touchée : une ligne qu'on n'a pas posée (la même ligne déjà présente est laissée, non suivie).
- Conflit (rien n'est écrit, code de sortie 2) : la clé (ce qui précède le premier `=`) est demandée par
  deux modules, ou existe déjà dans le fichier avec une autre valeur non posée par nous.
- Un fichier qui est un lien symbolique n'est jamais modifié (on écrirait dans sa cible).

## Orphelins

`setup` et `status` repèrent ce que poste avait posé et dont la source a disparu :

- un lien de `~` qui pointe vers `config/…` d'une racine de poste (`current`, le clone, une version
  installée, le dépôt de données) dont la cible n'existe plus (fichier renommé ou retiré du dépôt) :
  seuls les liens, jamais un fichier ordinaire, jamais un lien étranger ;
- un fichier de module noté dans `installed.json` que le dépôt ne produit plus (module supprimé, fichier
  renommé) et resté tel qu'installé (un fichier modifié à la main est gardé). Il est sauvegardé avant
  retrait, et les dossiers de module devenus vides sont retirés.

`setup` montre l'aperçu (avec les lignes de module) et demande une confirmation (sauf `--yes`) ;
`--dry-run` ne retire rien.

## La config

Le dossier `config/` du dépôt reflète `~` avec les vrais noms (`config/.config/hypr/bindings.lua`
pour `~/.config/hypr/bindings.lua`), sans préfixes. `setup` pose chaque entrée en lien
symbolique vers la racine (`current/config/…`) : en dev, modifier le fichier de `~` modifie le
clone ; en release, la version installée est en lecture seule (à ne pas modifier : le prochain
`sync` la remplace).

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
| lien vers le même chemin dans une autre racine de poste (clone, version), noté dans `installed.json` | sauvegarde dans `backup/`, puis lien vers `current` |
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

`install <module>`.
