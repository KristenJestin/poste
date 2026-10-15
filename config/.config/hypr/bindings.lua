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

-- --- Workspaces per screen ------------------------------------------------
-- SUPER+N goes to workspace N of the active screen (screens defined in
-- workspace_monitors, monitors.lua). Replaces Omarchy's SUPER+1..0.

-- Does an entry of workspace_monitors designate this screen? Port name
-- (DP-1) or EDID description (desc:AOC 24G2W1G3- …), compared with the start of the
-- description like Hyprland does.
local function screen_matches(entry, monitor)
  if entry:sub(1, 5) == "desc:" then
    local want = entry:sub(6):match("^%s*(.-)%s*$")
    return (monitor.description or ""):sub(1, #want) == want
  end
  return monitor.name == entry
end

local function monitor_workspace(n)
  local monitor = hl.get_active_monitor()
  local slot = #workspace_monitors
  for index, entry in ipairs(workspace_monitors) do
    if monitor and screen_matches(entry, monitor) then
      slot = index - 1
      break
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

-- Next/previous workspace: stay on the active screen (m) instead of
-- going through all the screens (e).
hl.unbind("SUPER + TAB")
hl.unbind("SUPER + SHIFT + TAB")
hl.unbind("SUPER + mouse_down")
hl.unbind("SUPER + mouse_up")
o.bind("SUPER + TAB", "Next workspace on this monitor", hl.dsp.focus({ workspace = "m+1" }))
o.bind("SUPER + SHIFT + TAB", "Previous workspace on this monitor", hl.dsp.focus({ workspace = "m-1" }))
o.bind("SUPER + mouse_down", "Scroll active workspace forward", hl.dsp.focus({ workspace = "m+1" }))
o.bind("SUPER + mouse_up", "Scroll active workspace backward", hl.dsp.focus({ workspace = "m-1" }))

-- --- Personal shortcuts (taken from Omarchy 3) -----------------------------
-- The other old personal shortcuts (terminal, tmux, browser, editor,
-- docker, obsidian, music...) are now Omarchy defaults.

-- SUPER+SPACE: launcher of the poste.island island (was: Vicinae, removed, before that the Omarchy menu).
hl.unbind("SUPER + SPACE")
o.bind("SUPER + SPACE", "Launcher", "omarchy-shell poste.island launcher")


-- SUPER+ALT+SPACE: Omarchy menu in the poste.island island (was: native Omarchy menu).
hl.unbind("SUPER + ALT + SPACE")
o.bind("SUPER + ALT + SPACE", "Omarchy menu", "omarchy-shell poste.island menu")

-- SUPER+CTRL+ALT+SPACE: native Omarchy menu (fallback).
o.bind("SUPER + CTRL + ALT + SPACE", "Omarchy menu (native)", "omarchy-menu toggle")

-- SUPER+L: lock (was: workspace layout toggle).
-- SUPER+L goes through the island: its immediate 'lock' sound, then omarchy-system-lock.
-- (Auto lock and unlock: sounds played by the island.)
hl.unbind("SUPER + L")
o.bind("SUPER + L", "Lock screen", "omarchy-shell poste.island lock || omarchy-system-lock")
-- PRINT: Omarchy capture + 'snap' sound when a file is created.
hl.unbind("PRINT")
o.bind("PRINT", "Screenshot", os.getenv("HOME") .. "/.config/omarchy/plugins/poste.island/bin/poste-screenshot")
-- Screen recording (SHIFT+PRINT, ALT+PRINT removed): provided by the record module
-- (~/.local/share/poste/modules/record/hypr/record.lua, loaded by hyprland.lua).



-- SUPER+E: Flea on the active terminal's folder (Nautilus stays installed).
o.bind("SUPER + E", "File manager (cwd)", 'flea --gui "$(omarchy-cmd-terminal-cwd)"')
-- SUPER+SHIFT+M: Omamail mail (was: Spotify, uninstalled).
hl.unbind("SUPER + SHIFT + M")
o.bind("SUPER + SHIFT + M", "Mail (Omamail)", "omarchy-shell shell toggle omamail '{}'")
o.bind("SUPER + SHIFT + code:60", "Passwords", { launch = "proton-pass" })

-- Dictation (SUPER+A): provided by the dictation add-on the machine chooses (vocaline or voxtype,
-- ~/.local/share/poste/modules/<add-on>/hypr/, loaded by hyprland.lua).

-- Send the window to a given screen (SUPER+CTRL+ALT+arrow). The screens
-- of each arrow are specific to the machine: screen_keys, in monitors.lua.
for _, screen in ipairs(screen_keys or {}) do
  o.bind("SUPER + CTRL + ALT + " .. screen.key, "Send window to " .. screen.name, hl.dsp.window.move({ monitor = screen.monitor }))
end

-- Volume keys: same command as Omarchy, but through poste.island's shim
-- which skips the volume OSD when the island is the active bar (the island's banner
-- shows the volume read from PipeWire). Without the island, the native OSD shows as before.
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

-- SUPER+SHIFT+SPACE: disabled (Omarchy default 'show/hide the bar', pointless with the island).
hl.unbind("SUPER + SHIFT + SPACE")


-- Omarchy default shortcuts to uninstalled apps: removed.
-- SUPER+SHIFT+W (Omawrite), SUPER+SHIFT+ALT+M (cliamp),
-- SUPER+SHIFT+SLASH (1Password; Proton Pass is on SUPER+SHIFT+.), SUPER+SHIFT+G (Signal),
-- SUPER+CTRL+RETURN and SUPER+CTRL+K (Herdr): removed on request.
for _, key in ipairs({ "SUPER + SHIFT + W", "SUPER + SHIFT + ALT + M", "SUPER + SHIFT + SLASH",
                        "SUPER + SHIFT + G", "SUPER + CTRL + RETURN", "SUPER + CTRL + K" }) do
  hl.unbind(key)
end

-- SUPER+SHIFT+C: T3 Code (in place of Calendar HEY), brings the window back if it is open.
hl.unbind("SUPER + SHIFT + C")
-- A launcher ~/.local/bin/t3code (specific to the machine, e.g. credentials of the work
-- PC) takes precedence over the package's command.
local t3code = os.getenv("HOME") .. "/.local/bin/t3code"
local t3code_wrapper = io.open(t3code)
if t3code_wrapper then t3code_wrapper:close() else t3code = "t3code-nightly" end
o.bind("SUPER + SHIFT + C", "T3 Code", o.launch_sole("com.t3tools.T3Code", t3code))

-- Omarchy shortcuts to websites (webapps): removed on request.
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

-- Network / Audio / Bluetooth panels: views built into the island on the focused
-- screen (2nd press = close). The native panel stays reachable through
-- 'Advanced settings…' in each view.
for key, spec in pairs({
  ["SUPER + CTRL + W"] = { "Wi-Fi (island)", "wifi" },
  ["SUPER + CTRL + A"] = { "Sound (island)", "audio" },
  ["SUPER + CTRL + B"] = { "Bluetooth (island)", "bluetooth" },
}) do
  hl.unbind(key)
  o.bind(key, spec[1], "omarchy-shell poste.island " .. spec[2])
end

-- SUPER+D: Flea's file shelf (Shelf), opened under the island.
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

-- SUPER+SHIFT+F and SUPER+ALT+SHIFT+F: removed, SUPER+E is enough (Flea on the
-- terminal's folder). The 'flea --default' block above is kept: without
-- it, `flea --default` (bootstrap) would rewrite it.
hl.unbind("SUPER + SHIFT + F")
hl.unbind("SUPER + ALT + SHIFT + F")
