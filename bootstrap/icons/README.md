# icons : icônes Tabler à la place des glyphes Nerd Font

Le shell Omarchy dessine ses icônes (menu, barre, panneaux, plugins tiers) avec des glyphes
Nerd Font de la famille `monospace`, c'est-à-dire FiraCode Nerd Font. L'île `poste.island`
affiche, elle, des SVG Tabler. Ce dossier donne le même dessin Tabler au reste du shell.

| Fichier | Rôle |
| --- | --- |
| `glyphs.tsv` | la table : point de code Nerd, icône Tabler, variante pleine, nom pour l'île, nom Nerd, où il sert |
| `poste-icons.py` | génère la police, réécrit `Glyphs.js` de l'île, recense les glyphes, fait la planche de contrôle |
| `60-poste-shell-icons.conf` | règle fontconfig qui donne la police au seul shell |

## La police « Poste Nerd Font »

Une copie de chaque graisse de FiraCode Nerd Font (Light à Bold), familles renommées. Les
points de code de `glyphs.tsv` pointent vers les contours Tabler, tirés de la webfont
`@tabler/icons-webfont` 3.49.0 (MIT, sha256 vérifiés, cache dans `~/.cache/poste/`). Le texte,
les autres glyphes et les métriques (head, hhea, OS/2) sont ceux de FiraCode, octet pour
octet : le script le vérifie et s'arrête sinon. Graisse Light : Tabler en trait fin (300).

Une grille Tabler de 24 vaut un em, comme les icônes Material des Nerd Fonts. Centrage comme
le patcher Nerd Font : au milieu de la cellule si l'icône y tient, sinon calée à gauche.

Posée par `poste sys setup` (étape `poste-icons-font`) dans `~/.local/share/fonts/poste/`,
puis `fc-cache`. À la main :

```bash
uv run --no-project --with fonttools==4.66.1 python3 bootstrap/icons/poste-icons.py build
python3 bootstrap/icons/poste-icons.py check   # 0 si à jour
```

Les TTF générées ne sont pas versionnées. Rien ne s'en sert tant que la règle n'est pas posée.

## La règle fontconfig

`60-poste-shell-icons.conf` remplace `monospace` par `Poste Nerd Font` pour le seul processus
`quickshell` (le shell). Les terminaux gardent FiraCode. Elle vit dans
`~/.config/fontconfig/conf.d/`, jamais dans `fonts.conf` que `omarchy-font-set` réécrit.
Étape manuelle `poste-icons-shell` :

```bash
mkdir -p ~/.config/fontconfig/conf.d
ln -s ~/Projects/poste/bootstrap/icons/60-poste-shell-icons.conf ~/.config/fontconfig/conf.d/
omarchy restart shell
```

Pour revenir en arrière : supprimer le lien, puis `omarchy restart shell`.

## Changer une icône

1. Modifier la ligne dans `glyphs.tsv` (noms Tabler : https://tabler.io/icons). `-` garde le
   glyphe Nerd quand Tabler n'a rien de juste (logos absents, glyphes de la police `omarchy`).
2. `python3 bootstrap/icons/poste-icons.py glyphs-js` : réécrit `Glyphs.js` de l'île. Une ligne
   n'y entre que si son SVG est dans `assets/tabler` ; la colonne « île » garde un sens propre
   à l'île (par exemple 󰖂 y veut dire « déconnecter »).
3. `poste sys setup` régénère la police (l'empreinte a changé), puis `omarchy restart shell`.

Après une mise à jour d'Omarchy ou d'un plugin : `python3 bootstrap/icons/poste-icons.py scan`
liste les glyphes utilisés absents de la table. Le sélecteur d'emojis et de glyphes
(`emojis.json`) et la galerie de développement ne sont pas recensés.

Planche de contrôle (original et remplacement côte à côte) :

```bash
uv run --no-project --with fonttools==4.66.1 --with pillow python3 bootstrap/icons/poste-icons.py sheet /tmp/planche.png
```

## Limites

- Le sélecteur de glyphes Nerd du shell (emojis) passe aussi par `monospace` : il montre l'icône
  Tabler pour les points de code de la table.
- Si `omarchy font set` choisit une autre police, le shell garde Poste Nerd Font (FiraCode)
  tant que la règle est posée. `--family` permet de dériver d'une autre famille Nerd Font.
- Le nom affiché par le shell pour la police (`fc-match monospace`) reste FiraCode : fc-match
  n'est pas le processus `quickshell`.
