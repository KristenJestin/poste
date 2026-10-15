-- Module vocaline : touche de dictée Vocaline (chargé par hyprland.lua).
-- Installé par `poste sys setup` dans ~/.local/share/poste/modules/vocaline/hypr/.

-- SUPER+A : appui = `vocaline press`, relâchement = `vocaline release`. Maintenir pour parler,
-- ou taper pour démarrer puis taper encore pour arrêter. Rien ne s'écrit pendant qu'on parle :
-- le texte se voit sous l'île, et Vocaline le colle d'un bloc à l'arrêt.
o.bind("SUPER + A", "Dictée (Vocaline)", "poste vocaline press")
o.bind("SUPER + A", nil, "poste vocaline release", { release = true })
