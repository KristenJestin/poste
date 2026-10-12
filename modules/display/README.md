# poste display

Profils d'écrans pour Hyprland (0.56, config Lua) : où sont les écrans selon ceux qui sont branchés
(au bureau, portable seul…). Le démon
(`poste-display watch`, lancé par `poste-display.service`, activé par `poste sys setup`) écoute les
branchements d'écrans et applique le profil qui correspond. Il remplace `display-profile` et
`display-profile-watch` de l'ancien portable pro (Omarchy 3, profils work / nomad / home faits à la va-vite).
Prévu pour le portable ; à désactiver ailleurs (`poste sys module disable display`).

Un profil règle seulement la disposition des écrans (mode, position, échelle, rotation) et, au besoin,
range des fenêtres. Les espaces de travail de chaque écran viennent de `~/.config/hypr/monitors.lua`
(`workspace_monitors`) et de lui seul : un profil qui en déclare (`[[profile.workspace]]`) est refusé
avec un message qui le dit. Avant le 2026-10-12, les profils du pro posaient leurs propres règles
d'espaces et le démon les remettait à chaque rechargement d'Hyprland, par-dessus `monitors.lua`.

## Commandes

- `poste display status` : écrans branchés (nom et description EDID), profil courant, profil que choisirait `auto`.
- `poste display list` : profils déclarés.
- `poste display apply <profil> [--dry-run]` : applique un profil. Choisi à la main, il n'est pas défait
  par `auto` tant que les écrans branchés ne changent pas. `--dry-run` montre les commandes sans rien appliquer.
- `poste display auto [--force] [--dry-run]` : choisit selon les écrans branchés et applique (sans effet si les
  écrans n'ont pas changé, sauf avec `--force`, qui choisit à nouveau même après un `apply` à la main).
- `poste display items` : entrées du lanceur (JSON).
- `poste display watch` : le démon.

L'île : l'extension « Écrans » du lanceur (mots-clés `ecrans`, `display`) a une seule entrée, « Écrans :
réappliquer » (`poste display auto --force`), avec le profil en cours en sous-titre (`island/launcher.json`).
Le démon choisit déjà à chaque branchement : la liste des profils n'y est pas, `apply` reste pour la main.
Quand le démon change de profil tout seul : notification discrète (urgence basse).

## Profils : `~/.config/poste/display.toml`

Local à la machine, non versionné. Modèle commenté : [`display.toml.example`](display.toml.example)
(`cp` puis ajuster), celui du portable pro, vérifié au bureau le 2026-10-12 :

- nomade (portable seul) : range les fenêtres des espaces des écrans du bureau (1-10 et 11-20) sur ceux du
  portable (21-30), les seuls que la barre de l'île y montre ;
- boulot : les deux AOC 24G2W1G3 du bureau, reconnus par leur numéro de série (HDMI-A-1 au milieu au-dessus
  du portable, DP-1 en portrait à gauche) ; un seul des deux suffit.

Un profil : `detect` (tous présents), `detect_any` (au moins un) parmi les écrans EXTERNES, `internal`
(activé ou non, mode, position, échelle), `[[profile.monitor]]` (`match`, `mode`, `position`, `scale`,
`transform`), `[[profile.rescue]]` (fenêtres à ranger : `from` est un numéro d'espace, une plage `1-10` ou un
nom d'espace, `to` un numéro ou une plage de même longueur). Un critère est un morceau de description EDID
(sans casse) ou `connector:NOM`. Le plus précis gagne ; sans `detect` ni `detect_any`, le profil
ne vaut que sans écran externe ; aucun profil ne correspond : rien n'est changé.

## Comment c'est appliqué (Hyprland 0.56, Lua)

Pour chaque règle, d'abord `hyprctl eval 'hl.monitor({ output = …, mode = …, position = …, scale = … })'`
(de même `hl.dispatch(hl.dsp.window.move …)`), puis, si Hyprland ne répond pas `ok`, la forme historique
(`hyprctl keyword monitor …`, `dispatch movetoworkspacesilent …`). `--dry-run` affiche les deux. Ordre :
fenêtres à ranger, puis écrans (l'interne d'abord s'il reste actif, après les autres s'il est désactivé :
jamais zéro écran).

Vérifié sur le pro le 2026-10-12 : `hl.monitor`, `window.move` avec `window = "address:…"` (et, pour mémoire,
`hl.dsp.workspace.move` prend `workspace`, pas `id` : avec `id`, Hyprland répond `ok` sans rien déplacer).
Reste à valider : `disabled = true` de `hl.monitor`. Le démon réapplique le profil après un `hyprctl reload`,
qui remet les écrans de `monitors.lua`.

Attention : Omarchy 4 gère aussi l'écran interne (`omarchy-hyprland-monitor-clamshell`, à la fermeture
du capot). Désactiver l'écran interne par un profil peut être contredit par cette bascule.

## Le démon

Un seul socket (`$XDG_RUNTIME_DIR/hypr/<instance>/.socket2.sock`, instance trouvée même sans
`HYPRLAND_INSTANCE_SIGNATURE`), lecture bloquante avec un délai de 100 ms, aucun sondage. Un événement
`monitoradded` ou `monitorremoved` lance `auto` après 700 ms (`debounce_ms`, Hyprland stabilise ses
écrans ; plusieurs événements = une application). `auto` ne fait rien si les écrans (noms et descriptions) n'ont pas changé.
Reconnexion automatique. État (profil courant, empreinte des écrans) : `$XDG_RUNTIME_DIR/poste-display/state.json`.

## Développement

Variables RÉSERVÉES AUX TESTS : `POSTE_DISPLAY_HYPRCTL` (faux `hyprctl`), `POSTE_DISPLAY_NOTIFY`. Tests
(`cargo test -p poste-display`) : choix du profil selon de faux JSON `hyprctl monitors all -j` (`src/model.rs`),
génération des commandes (`src/plan.rs`), configuration et exemple (`src/config.rs`), binaire complet
avec faux `hyprctl` et faux socket d'événements (`tests/cli.rs`).
