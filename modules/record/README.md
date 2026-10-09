# poste record

Enregistrement d'écran avec `gpu-screen-recorder`, piloté par l'île. C'est un module-script
(Python, `poste-record`) : pas de Rust ici, `poste sys setup` copie le script tel quel dans
`~/.local/share/poste/modules/record/poste-record`, et `poste record …` l'exécute.

## Commandes

- `poste record start --preset ID` : un préréglage (voir plus bas).
- `poste record start --window ADDR [--audio A]` : une fenêtre (adresse Hyprland).
- `poste record start --monitor NOM [--audio A]` : un écran entier.
- `poste record start --region [--audio A]` : une zone choisie à la souris.
- `poste record stop | pause | resume | toggle` : arrêter (le fichier est terminé proprement),
  mettre en pause, reprendre, ou basculer pause/reprise.
- `poste record status` : état en JSON (`{"t": "status", "recording": false}` au repos).
- `poste record watch` : une ligne JSON par changement d'état (lu par l'île).
- `poste record items` : la liste de l'extension « Record » du lanceur (JSON).
- `poste record hotkey` : SHIFT+PRINT. Arrête l'enregistrement en cours, sinon ouvre le lanceur
  de l'île sur « Record ».
- `poste record open-dir | open-last [--preset ID]` : le dossier, ou le dernier enregistrement.

Son (`--audio`) : `none`, `app` (le son de l'appli de la fenêtre), `desktop`, `mic`,
`desktop+mic`, `app+mic`, ou des sources `gpu-screen-recorder` brutes séparées par `|`.

## Préréglages (config utilisateur)

`~/.config/poste/record/presets.json` (`XDG_CONFIG_HOME` respecté), versionné dans
`config/.config/poste/record/presets.json` et posé par lien. Il était avant dans
`~/.config/poste-island/record/presets.json` : déplacé sans changement de contenu ; l'ancien lien
est repéré comme orphelin et retiré par `poste sys setup`. Le format est décrit dans le champ
`"//"` du fichier (`defaults`, `presets`, `audioSource`, `audioAppName`, `cursor`, `dir`…).

## Ce que le module apporte à la machine

| Où | Fichier du module | Rôle |
| --- | --- | --- |
| Hyprland | `hypr/record.lua` | SHIFT+PRINT vers `poste record hotkey`, `hl.unbind("ALT + PRINT")` |
| Lanceur de l'île | `island/launcher.json` | extension `record` (mots-clés `rec`, `record`), avec `"highlight": true` : l'île encadre à l'écran la cible sélectionnée |
| Île (slot `capture`) | `island/island.json`, `island/RecordState.qml` | état de l'enregistrement pour la pastille de capture et sa carte (partagées avec OBS, qui restent dans l'île) |
| `xdph.conf` | ligne de `module.toml` (`[[line]]`) | `screencopy:custom_picker_binary` vers `bin/poste-share-picker` |
| Programme annexe | `bin/poste-share-picker` | sélecteur de partage d'écran de xdg-desktop-portal-hyprland : répond seul avec la fenêtre demandée par `poste record` (fichier `pick.json` de moins de 20 s dans `$XDG_RUNTIME_DIR/poste-record`), sinon passe la main à `hyprland-preview-share-picker` |

Sans le module, aucune de ces traces n'existe (ni raccourci, ni entrée de lanceur, ni ligne `xdph.conf`).
`xdg-desktop-portal-hyprland` ne relit sa config qu'au démarrage : après avoir posé ou retiré la
ligne, redémarrer le portail (`systemctl --user restart xdg-desktop-portal-hyprland`).

## Fichiers

- `$XDG_RUNTIME_DIR/poste-record/` : `state.json` (enregistrement en cours), `pick.json`, `last.log`.
- Notifications : « Recording saved » (un clic ouvre le dossier) ou l'erreur.
