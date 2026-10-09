-- Module record : raccourcis d'enregistrement d'écran (chargé par hyprland.lua).
-- Installé par `poste sys setup` dans ~/.local/share/poste/modules/record/hypr/.

-- SHIFT+PRINT : arrête l'enregistrement en cours ; sinon ouvre le lanceur de l'île sur « Record »
-- (préréglages, fenêtres, écrans, zone). gpu-screen-recorder, à la place d'OBS.
o.bind("SHIFT + PRINT", "Screen recording", "poste record hotkey")

-- ALT+PRINT : menu d'enregistrement d'écran d'Omarchy retiré.
hl.unbind("ALT + PRINT")
