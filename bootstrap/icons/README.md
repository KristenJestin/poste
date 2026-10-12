# icons : icônes Tabler à la place des glyphes Nerd Font

Le shell Omarchy dessine ses icônes (menu, barre, panneaux, plugins tiers) avec des glyphes
Nerd Font de la famille `monospace`, c'est-à-dire FiraCode Nerd Font. L'île `poste.island`
affiche, elle, des SVG Tabler. La police « Poste Nerd Font » donne le même dessin Tabler au
reste du shell.

| Fichier | Rôle |
| --- | --- |
| `glyphs.tsv` | la table : point de code Nerd, icône Tabler, variante pleine, nom pour l'île, nom Nerd, où il sert |
| `config/.local/share/fonts/poste/` | la police, versionnée telle quelle (six graisses de FiraCode Nerd Font 3.5.1) |
| `config/.config/fontconfig/conf.d/60-poste-shell-icons.conf` | règle fontconfig qui donne la police au seul shell |

Les deux sont liés par `poste sys setup` comme le reste de `config/` : rien à générer ni à
installer sur la machine.

## La police

Une copie de chaque graisse de FiraCode Nerd Font (Light à Bold), familles renommées. Les
points de code de `glyphs.tsv` pointent vers les contours de la webfont `@tabler/icons-webfont`
3.49.0 (MIT, licence à côté des TTF). Le texte, les autres glyphes et les métriques sont ceux
de FiraCode (SIL Open Font License 1.1, sans nom réservé : `LICENSE-firacode` à côté des TTF). Graisse Light : Tabler en trait fin. Une grille Tabler de 24 vaut un em, centrée
dans la cellule comme le fait le patcher Nerd Font.

## La règle fontconfig

Elle remplace `monospace` par `Poste Nerd Font` pour le seul processus `quickshell` (le shell).
Les terminaux gardent FiraCode. Elle vit dans `conf.d`, jamais dans `fonts.conf` que
`omarchy-font-set` réécrit. Pour revenir en arrière : supprimer le lien
`~/.config/fontconfig/conf.d/60-poste-shell-icons.conf`, puis `omarchy restart shell`.

## Changer une icône

La police est un fichier figé : changer une icône veut dire la regénérer hors des PC, puis
commiter les TTF. `glyphs.tsv` garde la correspondance, et `Glyphs.js` de l'île suit la même
table (à changer aux deux endroits).

## Limites

- Le sélecteur de glyphes Nerd du shell (emojis) passe aussi par `monospace` : il montre l'icône
  Tabler pour les points de code de la table.
- Si `omarchy font set` choisit une autre police, le shell garde Poste Nerd Font (FiraCode)
  tant que la règle est posée.
- Un glyphe ajouté par une mise à jour d'Omarchy garde son dessin Nerd tant que la police n'est
  pas regénérée.
