# poste vocaline

Branche la dictée [Vocaline](https://github.com/KristenJestin/vocaline) sur le bureau : la touche
SUPER+Z et la dictée en bas de l'écran actif (le bandeau) : micro et onde, comme voxtype sur
SUPER+A, puis le texte dicté en direct. Vocaline lui-même (démon, micro, moteur, collage) n'est
pas dans poste : ce module ne fait que le relier à l'île et à Hyprland.

## Fonctionnement

- SUPER+Z (`hypr/vocaline.lua`) : `poste vocaline press` à l'appui, `poste vocaline release` au
  relâchement. Maintenir pour parler, ou taper pour démarrer puis taper encore pour arrêter.
- Rien ne s'écrit dans la fenêtre pendant qu'on parle : à l'arrêt, Vocaline colle toute la dictée
  d'un bloc dans la fenêtre de départ.
- L'île lit le flux `watch` directement sur le socket de Vocaline (`$XDG_RUNTIME_DIR/vocaline.sock`,
  `island/VocalineState.qml`, slot `dictation`) :
  - tout s'affiche en bas de l'écran actif, dans le bandeau ;
  - `loading` et `listening` : le micro et l'onde (`level`), et le son d'écoute ; l'onde est la
    même que celle de voxtype : le module ne donne que le niveau courant (`level`, mis à l'échelle
    de voxtype et lissé entre deux évènements, car le démon en envoie 10 par seconde), et le bandeau
    l'échantillonne à 100 par seconde, comme voxtype ;
  - `stopping` : les points de fin, et le son de fin ;
  - après l'onde : les phrases figées (`final`), puis la phrase en cours (`partial`) en gris,
    trois lignes au plus (la fin reste visible) ;
  - `idle` : tout s'efface.
- Démon absent ou flux coupé : l'île regarde toutes les 5 secondes si le socket est revenu, et ne
  s'y reconnecte qu'alors (rien au journal tant qu'il est absent).
- voxtype (SUPER+A) continue de marcher à côté : le bandeau montre celui qui dicte.

## Commandes

`poste vocaline <commande>` passe la commande telle quelle à Vocaline (`press`, `release`,
`status`, `watch`… : voir l'aide de Vocaline).

## Configuration de la machine

Le programme est `vocaline` du PATH. Pour un autre chemin (version d'essai, par exemple), la clé
`bin` de `~/.config/poste/vocaline.toml`, posé par le dépôt de données de la machine :

```toml
bin = "~/chemin/vers/vocaline"
```

La config de Vocaline lui-même (`~/.config/vocaline/`) se range aussi dans le dépôt de données de
la machine, jamais dans poste.

## Désactiver sur une machine

`disabled = ["vocaline"]` dans le `poste.toml` du dépôt de données (ou `poste sys module disable
vocaline`) : ni touche, ni slot de l'île.
