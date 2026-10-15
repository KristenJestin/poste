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
-- Every screen has its own workspaces 1 to 10, held in its own block of ten
-- (its slot): the first screen of workspace_monitors (monitors.lua) 1-10, the
-- second 11-20, and so on. A screen outside that list (a projector, a dock, a
-- virtual output) takes, when it arrives, the first block that no plugged
-- screen holds and keeps it while plugged; it gets it back when it returns if
-- no other screen took it meanwhile. SUPER+N goes to workspace N of the active
-- screen. Replaces Omarchy's SUPER+1..0.

-- Does an entry of workspace_monitors name this screen? A port name (DP-1) or
-- an EDID description (desc:AOC 24G2W1G3- ...), matched against the start of
-- the description as Hyprland does.
local function screen_matches(entry, monitor)
  if entry:sub(1, 5) == "desc:" then
    local want = entry:sub(6):match("^%s*(.-)%s*$")
    return (monitor.description or ""):sub(1, #want) == want
  end
  return monitor.name == entry
end

-- Slot of a screen of workspace_monitors, nil for any other.
local function listed_slot(monitor)
  for index, entry in ipairs(workspace_monitors) do
    if screen_matches(entry, monitor) then return index - 1 end
  end
end

-- Slots of the screens outside the list, one "<name> <slot>" per line. The
-- island (poste.island) and the bar (poste.workspaces) read it too.
local slots_dir = (os.getenv("XDG_RUNTIME_DIR") or "/tmp") .. "/poste"
local slots_path = slots_dir .. "/screen-slots"

-- Read once per config load and kept here: SUPER+N reads no file. A slot
-- inside the list (the list grew since) is dropped.
local slots = {}
do
  local file = io.open(slots_path, "r")
  if file then
    for line in file:lines() do
      local name, slot = line:match("^(%S+)%s+(%d+)$")
      if name and tonumber(slot) >= #workspace_monitors then slots[name] = tonumber(slot) end
    end
    file:close()
  end
end

local slots_dir_made = false
local function write_slots()
  if not slots_dir_made then
    os.execute("mkdir -p '" .. slots_dir:gsub("'", "'\\''") .. "'")
    slots_dir_made = true
  end
  local names = {}
  for name in pairs(slots) do names[#names + 1] = name end
  table.sort(names)
  local file = io.open(slots_path, "w")
  if not file then return end
  for _, name in ipairs(names) do file:write(name, " ", slots[name], "\n") end
  file:close()
end

local function screen_slot(monitor)
  return listed_slot(monitor) or slots[monitor.name] or #workspace_monitors
end

-- Gives a screen outside the list its slot: the one it had, unless a plugged
-- screen holds it now, else the first block that no plugged screen holds and
-- that no workspace uses (those of an unplugged screen wait for it there).
local function claim_slot(monitor)
  local listed = listed_slot(monitor)
  if listed then return listed end
  local held = {}
  for _, other in ipairs(hl.get_monitors()) do
    if other.name ~= monitor.name then
      local slot = listed_slot(other) or slots[other.name]
      if slot then held[slot] = true end
    end
  end
  local slot = slots[monitor.name]
  if slot == nil or held[slot] then
    local used = {}
    local shown = monitor.active_workspace
    for _, workspace in ipairs(hl.get_workspaces()) do
      if workspace.id > 0 and not (shown and workspace.id == shown.id) then
        used[(workspace.id - 1) // 10] = true
      end
    end
    slot = #workspace_monitors
    while held[slot] or used[slot] do slot = slot + 1 end
  end
  local changed = slots[monitor.name] ~= slot
  for name, other in pairs(slots) do
    if other == slot and name ~= monitor.name then
      slots[name] = nil
      changed = true
    end
  end
  if changed then
    slots[monitor.name] = slot
    write_slots()
  end
  return slot
end

-- A screen that arrives shows one of its own workspaces. Hyprland gave a
-- screen no rule names the first free number: that new, empty workspace takes
-- the first number of its block, or, when the block still holds workspaces
-- (the screen comes back), those come to it and the first one is shown.
local function settle(monitor)
  local first = claim_slot(monitor) * 10 + 1
  local shown = monitor.active_workspace
  if shown and shown.id >= first and shown.id < first + 10 then return end
  local own = {}
  for _, workspace in ipairs(hl.get_workspaces()) do
    if workspace.id >= first and workspace.id < first + 10 then own[#own + 1] = workspace.id end
  end
  if #own == 0 then
    if shown and shown.id > 0 and shown.windows == 0 then
      hl.dispatch(hl.dsp.workspace.change_id({ workspace = tostring(shown.id), id = first }))
    end
    return
  end
  table.sort(own)
  for _, id in ipairs(own) do
    hl.dispatch(hl.dsp.workspace.move({ workspace = tostring(id), monitor = monitor.name }))
  end
  monitor:set_workspace({ workspace = tostring(own[1]) })
end

hl.on("monitor.added", settle)
-- On a config reload, the plugged screens outside the list keep their slots,
-- or get one if they have none yet.
for _, monitor in ipairs(hl.get_monitors()) do claim_slot(monitor) end

local function monitor_workspace(n)
  local monitor = hl.get_active_monitor()
  local slot = monitor and screen_slot(monitor) or #workspace_monitors
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
