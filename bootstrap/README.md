# bootstrap : la machine décrite en données

Lu par `poste sys setup` (aperçu), `poste sys status` et `poste sys done`. Rien ici n'est un script.

| Fichier | Contenu |
| --- | --- |
| `packages.toml` | paquets attendus (`nom = "à quoi il sert"`), comparés à `pacman -Qq` |
| `steps.toml` | plugins tiers, fichiers, étapes utilisateur, étapes système, étapes manuelles |
| `system/` | contenus des fichiers root (`@USER@`, `@UID@`, `@GID@`, `@HOME@` y sont remplacés) |
| `files/` | contenus de fichiers utilisateur posés par poste (config de cursor-accent) |
| `patches/` | patchs locaux de plugins tiers, appliqués par `git apply` |
| `icons/` | icônes Tabler à la place des glyphes Nerd Font dans le shell : table des correspondances (voir [`icons/README.md`](icons/README.md)) |

Familles d'étapes (détail et commentaires dans `steps.toml`) :

- `[[plugin]]` : plugin du shell Omarchy épinglé à un commit (`omarchy plugin add`, `git checkout`),
  patch facultatif (déjà appliqué = `git apply --reverse --check` réussit). Un plugin à une autre
  version avec des fichiers modifiés, ou un patch qui ne s'applique plus, n'est pas touché : une
  note l'indique.
- `[[file]]` : fichier utilisateur posé s'il est absent ; s'il existe et diffère, il est laissé et signalé.
- `[[user]]` : sans sudo. `check` (code 0 = déjà fait), sinon `run`, lancé par poste après confirmation.
  `requires` = binaire nécessaire ; `trigger` = lancé seulement si l'une des étapes citées vient d'être faite.
- `[[system]]` : avec sudo. Poste vérifie (contenu du fichier, ligne présente, commande `check`) et
  AFFICHE les commandes dans un bloc copiable. Il ne lance jamais `sudo`. Un fichier illisible sans
  root (polkit) reste « à vérifier » jusqu'à `poste sys done <id>`.
- `[[manual]]` : à la main ; `check` facultatif, sinon `poste sys done <id>` (`--undo` pour annuler).
