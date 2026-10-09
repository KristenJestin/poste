-- Keep only your personal keybinding overrides here. Add new bindings or
-- unbind defaults before replacing them.

-- See current bindings and descriptions:
--   omarchy menu keybindings --print

-- To disable every Omarchy default binding, set this in
-- ~/.config/hypr/hyprland.lua before require("default.hypr.omarchy"), then add
-- only the bindings you want below:
--   omarchy_default_bindings = false

-- To disable all preinstalled app/webapp bindings, set:
--   omarchy_preinstalled_bindings = false

-- Add a new binding.
-- o.bind("SUPER + SHIFT + R", "SSH", "xdg-terminal-exec ssh your-server")

-- Change an existing binding by unbinding it first, then binding the key again.
-- This example changes SUPER+SPACE from the launcher to the Omarchy root menu.
-- hl.unbind("SUPER + SPACE")
-- o.bind("SUPER + SPACE", "Omarchy menu", "omarchy-menu toggle root")

-- Disable a default binding without replacing it.
-- hl.unbind("SUPER + SHIFT + B")

-- Logitech MX Keys examples:
-- o.bind("SUPER + SHIFT + S", nil, "omarchy-capture-screenshot")
-- o.bind("SUPER + H", nil, "voxtype record toggle")
-- o.bind("SUPER + PERIOD", nil, "omarchy-shell shell toggle omarchy.emojis")

-- --- Workspaces par écran -------------------------------------------------
-- SUPER+N va au workspace N de l'écran actif (écrans définis dans
-- workspace_monitors, monitors.lua). Remplace les SUPER+1..0 d'Omarchy.
local function monitor_workspace(n)
  local monitor = hl.get_active_monitor()
  local slot = #workspace_monitors
  for index, output in ipairs(workspace_monitors) do
    if monitor and monitor.name == output then
      slot = index - 1
    end
  end
  return tostring(slot * 10 + n)
end

for n = 1, 10 do
  local key = "code:" .. tostring(n + 9)

  hl.unbind("SUPER + " .. key)
  hl.unbind("SUPER + SHIFT + " .. key)
  hl.unbind("SUPER + SHIFT + ALT + " .. key)

  o.bind("SUPER + " .. key, "Switch to workspace " .. n .. " of this monitor", function()
    hl.dispatch(hl.dsp.focus({ workspace = monitor_workspace(n) }))
  end)
  o.bind("SUPER + SHIFT + " .. key, "Move window to workspace " .. n .. " of this monitor", function()
    hl.dispatch(hl.dsp.window.move({ workspace = monitor_workspace(n) }))
  end)
  o.bind("SUPER + SHIFT + ALT + " .. key, "Move window silently to workspace " .. n .. " of this monitor", function()
    hl.dispatch(hl.dsp.window.move({ workspace = monitor_workspace(n), follow = false }))
  end)
end

-- Workspace suivant/précédent : rester sur l'écran actif (m) au lieu de
-- parcourir tous les écrans (e).
hl.unbind("SUPER + TAB")
hl.unbind("SUPER + SHIFT + TAB")
hl.unbind("SUPER + mouse_down")
hl.unbind("SUPER + mouse_up")
o.bind("SUPER + TAB", "Next workspace on this monitor", hl.dsp.focus({ workspace = "m+1" }))
o.bind("SUPER + SHIFT + TAB", "Previous workspace on this monitor", hl.dsp.focus({ workspace = "m-1" }))
o.bind("SUPER + mouse_down", "Scroll active workspace forward", hl.dsp.focus({ workspace = "m+1" }))
o.bind("SUPER + mouse_up", "Scroll active workspace backward", hl.dsp.focus({ workspace = "m-1" }))

-- --- Raccourcis perso (repris d'Omarchy 3) ---------------------------------
-- Les autres anciens raccourcis perso (terminal, tmux, navigateur, éditeur,
-- docker, obsidian, musique...) sont désormais des défauts Omarchy.

-- SUPER+SPACE : lanceur de l'île poste.island (était : Vicinae — supprimé —, avant ça le menu Omarchy).
hl.unbind("SUPER + SPACE")
o.bind("SUPER + SPACE", "Launcher", "omarchy-shell poste.island launcher")


-- SUPER+ALT+SPACE : menu Omarchy dans l'île poste.island (était : menu Omarchy natif).
hl.unbind("SUPER + ALT + SPACE")
o.bind("SUPER + ALT + SPACE", "Omarchy menu", "omarchy-shell poste.island menu")

-- SUPER+CTRL+ALT+SPACE : menu Omarchy natif (repli).
o.bind("SUPER + CTRL + ALT + SPACE", "Omarchy menu (native)", "omarchy-menu toggle")

-- SUPER+L : verrouiller (était : bascule de layout du workspace).
-- SUPER+L passe par l'île : son « lock » immédiat, puis omarchy-system-lock.
-- (Verrouillage auto et déverrouillage : sons joués par l'île.)
hl.unbind("SUPER + L")
o.bind("SUPER + L", "Lock screen", "omarchy-shell poste.island lock || omarchy-system-lock")
-- PRINT : capture Omarchy + son « snap » quand un fichier est créé.
hl.unbind("PRINT")
o.bind("PRINT", "Screenshot", os.getenv("HOME") .. "/.config/omarchy/plugins/poste.island/bin/poste-screenshot")
-- Enregistrement d'écran (SHIFT+PRINT, ALT+PRINT retiré) : fourni par le module record
-- (~/.local/share/poste/modules/record/hypr/record.lua, chargé par hyprland.lua).



-- SUPER+E : Flea sur le dossier du terminal actif (Nautilus reste installé).
o.bind("SUPER + E", "File manager (cwd)", 'flea --gui "$(omarchy-cmd-terminal-cwd)"')
-- SUPER+SHIFT+M : mail Omamail (était : Spotify, désinstallé).
hl.unbind("SUPER + SHIFT + M")
o.bind("SUPER + SHIFT + M", "Mail (Omamail)", "omarchy-shell shell toggle omamail '{}'")
o.bind("SUPER + SHIFT + code:60", "Passwords", { launch = "proton-pass" })

-- Dictée : tap = toggle, maintien = push-to-talk.
-- Voxtype colle avec Ctrl+V : s'il le fait pendant que SUPER est encore tenu, ça
-- donne SUPER+Ctrl+V et rien ne se colle. On marque « SUPER tenu » à l'appui et
-- Hyprland (qui voit le clavier) retire la marque dès que SUPER est relâché ;
-- voxtype-focus restore attend cette disparition avant de coller.
local super_held_file = (os.getenv("XDG_RUNTIME_DIR") or "/tmp") .. "/voxtype-super-held"
local super_watch = nil
local function super_down()
  return hl.is_key_down(133) or hl.is_key_down(134) -- Super_L / Super_R (keycodes X11)
end
o.bind("SUPER + A", "Dictée (tap=toggle / maintien=PTT)", function()
  local f = io.open(super_held_file, "w")
  if f then f:close() end
  if super_watch then super_watch:set_enabled(false) end
  super_watch = hl.timer(function()
    if not super_down() then
      os.remove(super_held_file)
      super_watch:set_enabled(false)
    end
  end, { timeout = 100, type = "repeat" })
  hl.exec_cmd("voxtype-hold-toggle press")
end)
o.bind("SUPER + A", nil, "voxtype-hold-toggle release", { release = true })

-- Envoyer la fenêtre sur un écran précis.
o.bind("SUPER + CTRL + ALT + LEFT", "Send window to left screen", hl.dsp.window.move({ monitor = "DP-3" }))
o.bind("SUPER + CTRL + ALT + UP", "Send window to center screen", hl.dsp.window.move({ monitor = "DP-1" }))
o.bind("SUPER + CTRL + ALT + RIGHT", "Send window to right screen", hl.dsp.window.move({ monitor = "DP-2" }))

-- Touches de volume : même commande qu'Omarchy, mais via le shim de poste.island
-- qui saute l'OSD de volume quand l'île est la barre active (l'île affiche le
-- volume elle-même). Sans l'île, l'OSD natif s'affiche comme avant.
local island_volume = 'PATH="$HOME/.config/omarchy/plugins/poste.island/bin:$PATH" omarchy-audio-output-volume '
for key, spec in pairs({
  ["XF86AudioRaiseVolume"] = { "Volume up", "raise", true },
  ["XF86AudioLowerVolume"] = { "Volume down", "lower", true },
  ["XF86AudioMute"] = { "Mute", "mute-toggle", false },
  ["ALT + XF86AudioRaiseVolume"] = { "Volume up precise", "+1", true },
  ["ALT + XF86AudioLowerVolume"] = { "Volume down precise", "-1", true },
}) do
  hl.unbind(key)
  o.bind(key, spec[1], island_volume .. spec[2], { locked = true, repeating = spec[3] })
end

-- SUPER+SHIFT+SPACE : désactivé (défaut Omarchy « afficher/masquer la barre », sans objet avec l'île).
hl.unbind("SUPER + SHIFT + SPACE")


-- Raccourcis Omarchy par défaut vers des applis désinstallées : retirés.
-- SUPER+SHIFT+W (Omawrite), SUPER+SHIFT+ALT+M (cliamp),
-- SUPER+SHIFT+SLASH (1Password ; Proton Pass est sur SUPER+SHIFT+.), SUPER+SHIFT+G (Signal),
-- SUPER+CTRL+RETURN et SUPER+CTRL+K (Herdr) : retirés à la demande.
for _, key in ipairs({ "SUPER + SHIFT + W", "SUPER + SHIFT + ALT + M", "SUPER + SHIFT + SLASH",
                        "SUPER + SHIFT + G", "SUPER + CTRL + RETURN", "SUPER + CTRL + K" }) do
  hl.unbind(key)
end

-- SUPER+SHIFT+C : T3 Code (à la place de Calendar HEY), ramène la fenêtre si elle est ouverte.
hl.unbind("SUPER + SHIFT + C")
o.bind("SUPER + SHIFT + C", "T3 Code", o.launch_sole("com.t3tools.T3Code", "/opt/t3code-nightly-bin/t3code"))

-- Raccourcis Omarchy vers des sites web (webapps) : retirés à la demande.
for _, key in ipairs({
  "SUPER + SHIFT + A", "SUPER + SHIFT + ALT + A",                         -- ChatGPT, Grok
  "SUPER + SHIFT + E", "SUPER + SHIFT + ALT + E", "SUPER + CTRL + ALT + D", -- HEY Email, New email, Calendar
  "SUPER + SHIFT + Y",                                                     -- YouTube
  "SUPER + SHIFT + ALT + G", "SUPER + SHIFT + CTRL + G",                  -- WhatsApp, Google Messages
  "SUPER + SHIFT + P", "SUPER + SHIFT + S",                                -- Google Photos, Maps
  "SUPER + SHIFT + X", "SUPER + SHIFT + ALT + X",                          -- X, X Post
}) do
  hl.unbind(key)
end

-- Panneaux Réseau / Audio / Bluetooth : vues intégrées de l'île sur l'écran
-- focus (2e appui = fermer). Le panneau natif reste accessible via
-- « Advanced settings… » dans chaque vue.
for key, spec in pairs({
  ["SUPER + CTRL + W"] = { "Wi-Fi (island)", "wifi" },
  ["SUPER + CTRL + A"] = { "Sound (island)", "audio" },
  ["SUPER + CTRL + B"] = { "Bluetooth (island)", "bluetooth" },
}) do
  hl.unbind(key)
  o.bind(key, spec[1], "omarchy-shell poste.island " .. spec[2])
end

-- SUPER+D : bac de fichiers de Flea (Shelf), ouvert sous l'île.
o.bind("SUPER + D", "File shelf (Flea)", "flea shelf toggle")

-- flea --default: begin. Written by `flea --default`; `flea --default off` removes the block whole.
hl.unbind("SUPER + SHIFT + F")
o.bind("SUPER + SHIFT + F", "File manager", { launch = 'flea --gui' })
hl.unbind("SUPER + ALT + SHIFT + F")
o.bind("SUPER + ALT + SHIFT + F", "File manager (cwd)", { launch = 'flea --gui "$(omarchy-cmd-terminal-cwd)"' })
-- flea --default: end.

-- flea --picker: begin. Written by `flea --picker`; `flea --picker off` removes the block whole.
o.window("com.thisisgm.flea.picker", { tag = "+floating-window" })
-- flea --picker: end.

-- SUPER+SHIFT+F et SUPER+ALT+SHIFT+F : retirés, SUPER+E suffit (Flea sur le
-- dossier du terminal). Le bloc « flea --default » ci-dessus est gardé : sans
-- lui, `flea --default` (bootstrap) le réécrirait.
hl.unbind("SUPER + SHIFT + F")
hl.unbind("SUPER + ALT + SHIFT + F")
