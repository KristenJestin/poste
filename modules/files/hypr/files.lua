-- Module files: SUPER+D rebuilds the file search index in the background (the command returns
-- at once, the walk takes a fraction of a second and stops; searches read the previous index
-- meanwhile), then opens the island launcher straight on the file search (extension
-- "poste-files"). Loaded by hyprland.lua, installed by `poste sys setup` in
-- ~/.local/share/poste/modules/files/hypr/; when the module is removed, the binding goes with it.
hl.unbind("SUPER + D")
o.bind("SUPER + D", "Find files", "poste files refresh --background; omarchy-shell poste.island launcherIn poste-files")
