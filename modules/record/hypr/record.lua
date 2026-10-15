-- Record module: screen recording shortcuts (loaded by hyprland.lua).
-- Installed by `poste sys setup` in ~/.local/share/poste/modules/record/hypr/.

-- SHIFT+PRINT: stops the recording in progress; otherwise opens the island's launcher on "Record"
-- (presets, windows, screens, region). gpu-screen-recorder, in place of OBS.
o.bind("SHIFT + PRINT", "Screen recording", "poste record hotkey")

-- ALT+PRINT: Omarchy's screen recording menu removed.
hl.unbind("ALT + PRINT")
