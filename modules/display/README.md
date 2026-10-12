# poste display

Profils d'écrans pour Hyprland (0.56, config Lua) : maison, boulot, nomade. Le démon
(`poste-display watch`, lancé par `poste-display.service`, activé par `poste sys setup`) écoute les
branchements d'écrans et applique le profil qui correspond. Il remplace `display-profile` et
`display-profile-watch` de l'ancien portable pro (Omarchy 3, profils work / nomad / home faits à la va-vite).
Prévu pour le portable ; à désactiver ailleurs (`poste sys module disable display`).

## Commandes

- `poste display status` : écrans branchés (nom et description EDID), profil courant, profil que choisirait `auto`.
- `poste display list` : profils déclarés.
- `poste display apply <profil> [--dry-run]` : applique un profil. Choisi à la main, il n'est pas défait
  par `auto` tant que les écrans branchés ne changent pas. `--dry-run` montre les commandes sans rien appliquer.
- `poste display auto [--dry-run]` : choisit selon les écrans branchés et applique (sans effet si les écrans n'ont pas changé).
- `poste display items` : entrées du lanceur (JSON).
- `poste display watch` : le démon.

L'île : l'extension « Écrans » du lanceur (mots-clés `ecrans`, `display`) liste « Écrans : Maison / Boulot /
Nomade / automatique » (`island/launcher.json`, entrées lues dans `display.toml`).
Quand le démon change de profil tout seul : notification discrète (urgence basse).

## Profils : `~/.config/poste/display.toml`

Local à la machine, non versionné. Modèle commenté : [`display.toml.example`](display.toml.example)
(`cp` puis ajuster). Profils déduits des anciens scripts du pro ; l'exemple dit ce qui est sûr et ce qui
ne l'est pas :

- nomade (portable seul, 2880x1800@120 échelle 1.6, rapatrie les fenêtres de `mid` et `left` vers 9 et 10) : sûr ;
- boulot (HDMI-A-1 au milieu au-dessus, DP-1 portrait à gauche) : disposition sûre, détection par prise
  (`connector:`) PAS fiable : les descriptions EDID du bureau ne sont pas connues, à remplacer ;
- maison : hypothèse (les trois Iiyama PL2470H du PC fixe), à confirmer ou supprimer.

Un profil : `detect` (tous présents), `detect_any` (au moins un) parmi les écrans EXTERNES, `internal`
(activé ou non, mode, position, échelle), `[[profile.monitor]]` (`match`, `mode`, `position`, `scale`,
`transform`), `[[profile.workspace]]` (règles d'espaces de travail, `1-10` accepté, `internal` désigne l'écran interne),
`[[profile.rescue]]` (fenêtres d'un espace nommé ramenées vers un autre). Un critère est un morceau de description EDID
(sans casse) ou `connector:NOM`. Le plus précis gagne ; sans `detect` ni `detect_any`, le profil
ne vaut que sans écran externe ; aucun profil ne correspond : rien n'est changé.

## Comment c'est appliqué (Hyprland 0.56, Lua)

Pour chaque règle, d'abord `hyprctl eval 'hl.monitor({ output = …, mode = …, position = …, scale = … })'`
(de même `hl.workspace_rule`, `hl.dispatch(hl.dsp.window.move / workspace.move …)`), puis, si Hyprland ne
répond pas `ok`, la forme historique (`hyprctl keyword monitor …`, `dispatch moveworkspacetomonitor …`).
`--dry-run` affiche les deux. Ordre : fenêtres à rapatrier, écrans (l'interne d'abord s'il reste actif, après
les autres s'il est désactivé : jamais zéro écran), règles d'espaces, déplacement des espaces existants.

Vérifié sur le pro le 2026-10-12 : `hl.monitor`, `window.move` avec `window = "address:…"`, et `workspace.move`
qui prend `workspace` (avec `id`, Hyprland répond `ok` mais ne déplace rien). Reste à valider : `disabled = true`
de `hl.monitor`. Le démon réapplique le profil après un `hyprctl reload` (qui remet les règles de `monitors.lua`).

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
