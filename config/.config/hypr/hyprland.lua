-- Learn how to configure Hyprland: https://wiki.hypr.land/Configuring/Start/

-- Omarchy's bootstrap keeps path setup out of this user config.
dofile((os.getenv("OMARCHY_PATH") or "/usr/share/omarchy") .. "/default/hypr/bootstrap.lua")

-- Disable all Omarchy default bindings. Add your own in hypr/bindings.lua.
-- omarchy_default_bindings = false
--
-- Or disable only bindings for Omarchy's preinstalled apps/web apps while
-- keeping core window-manager bindings:
-- omarchy_preinstalled_bindings = false

-- Load Omarchy defaults.
require("default.hypr.omarchy")

-- Put your personal overrides in these files. They're loaded after Omarchy's
-- defaults so package updates can improve the defaults without rewriting your
-- ~/.config/hypr files.
require("hypr.monitors")
require("hypr.input")
require("hypr.bindings")
require("hypr.looknfeel")
require("hypr.autostart")

-- Machine-specific settings, put in place by its poste-data repository (optional).
if package.searchpath("hypr.machine", package.path) then require("hypr.machine") end

-- Config pieces of the poste modules: each installed module may provide
-- hypr/*.lua (installed by `poste sys setup` in ~/.local/share/poste/modules/<name>/hypr/).
-- Hyprland knows no module: an absent module leaves no trace. An error in
-- a piece is isolated (pcall) and reported by a notification, the rest loads.
do
  local data = os.getenv("XDG_DATA_HOME")
  if not data or data == "" then data = os.getenv("HOME") .. "/.local/share" end
  local dir = data .. "/poste/modules"
  local handle = io.popen("find '" .. dir:gsub("'", "'\\''") .. "' -mindepth 3 -maxdepth 3 -path '*/hypr/*.lua' -type f 2>/dev/null | sort")
  if handle then
    for file in handle:lines() do
      local chunk, err = loadfile(file)
      local ok = chunk ~= nil
      if ok then ok, err = pcall(chunk) end
      if not ok then
        local message = "poste: " .. file:gsub("^" .. dir:gsub("%p", "%%%0") .. "/", "") .. ": " .. tostring(err)
        io.stderr:write(message .. "\n")
        pcall(hl.exec_cmd, "notify-send -u critical 'Hyprland: poste module' " .. "'" .. message:gsub("'", "'\\''") .. "'")
      end
    end
    handle:close()
  end
end

-- Toggle config flags dynamically.
require("default.hypr.toggles")

-- Add any other personal Hyprland configuration below.
-- o.window("qemu", { workspace = "5" })

-- Qt apps: file picker through the portal (so Flea), instead of the
-- GTK box they draw themselves with Omarchy's gtk3 theme.
hl.env("QT_QPA_PLATFORMTHEME", "xdgdesktopportal")
