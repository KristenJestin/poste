# poste vocaline

Branche la dictée [Vocaline](https://github.com/KristenJestin/vocaline) sur le bureau : la touche
SUPER+A, la pastille de dictée en bas de l'écran actif (micro et onde)
et le texte dicté en direct juste sous l'île. Vocaline lui-même (démon, micro, moteur, collage)
n'est pas dans poste : ce module ne fait que le relier à l'île et à Hyprland.

## Fonctionnement

- SUPER+A (`hypr/vocaline.lua`) : `poste vocaline press` à l'appui, `poste vocaline release` au
  relâchement. Maintenir pour parler, ou taper pour démarrer puis taper encore pour arrêter.
- Rien ne s'écrit dans la fenêtre pendant qu'on parle : à l'arrêt, Vocaline colle toute la dictée
  d'un bloc dans la fenêtre de départ.
- L'île lit le flux `watch` directement sur le socket de Vocaline (`$XDG_RUNTIME_DIR/vocaline.sock`,
  `island/VocalineState.qml`, slot `dictation`) :
  - tout s'affiche dans le bandeau, sur l'écran actif : la pastille en bas, le texte sous l'île ;
  - `loading` et `listening` : la pastille avec le micro et l'onde (`level`), et le son d'écoute ;
    le module ne donne que le niveau courant (`level`, mis à l'échelle de l'onde de l'île
    et lissé entre deux évènements, car le démon en envoie 10 par seconde),
    et le bandeau l'échantillonne à 100 par seconde ;
  - `stopping` : les points de fin, et le son de fin ;
  - sous l'île : les phrases figées (`final`), puis la phrase en cours (`partial`) en gris,
    trois lignes au plus (la fin reste visible) ;
  - `idle` : tout s'efface.
- Démon absent ou flux coupé : l'île regarde toutes les 5 secondes si le socket est revenu, et ne
  s'y reconnecte qu'alors (rien au journal tant qu'il est absent).

## Commandes

`poste vocaline <commande>` passe la commande telle quelle à Vocaline (`press`, `release`,
`status`, `watch`… : voir l'aide de Vocaline).

## Configuration de la machine

The program is `vocaline` from the `PATH` (or `~/.local/bin`): the add-on is plugged in only
when it is there. To try another build, put it first in the `PATH` under the name `vocaline`.

La config de Vocaline lui-même (`~/.config/vocaline/`) se range aussi dans le dépôt de données de
la machine, jamais dans poste.

## Choose it on a machine

This is an add-on: it is installed only on a machine that lists it in the `poste.toml` of its
data repository (`addons = ["vocaline"]`), and plugged in once `vocaline` is in the `PATH`
(`requires`). The `vocaline` package itself is installed by hand (see the Vocaline repository).
Removing it from the list removes the key and the island part.
