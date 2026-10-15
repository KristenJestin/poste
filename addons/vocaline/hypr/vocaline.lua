-- Vocaline module: Vocaline dictation key (loaded by hyprland.lua).
-- Installed by `poste sys setup` in ~/.local/share/poste/modules/vocaline/hypr/.

-- SUPER+A: press = `vocaline press`, release = `vocaline release`. Hold to talk,
-- or tap to start then tap again to stop. Nothing is typed while you talk:
-- the text shows under the island, and Vocaline pastes it in one block at the stop.
o.bind("SUPER + A", "Dictation (Vocaline)", "poste vocaline press")
o.bind("SUPER + A", nil, "poste vocaline release", { release = true })
