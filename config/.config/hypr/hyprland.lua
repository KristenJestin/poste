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

-- Morceaux de config des modules poste : chaque module installé peut fournir
-- hypr/*.lua (installés par `poste sys setup` dans ~/.local/share/poste/modules/<nom>/hypr/).
-- Hyprland ne connaît aucun module : un module absent ne laisse aucune trace. Une erreur dans
-- un morceau est isolée (pcall) et signalée par une notification, le reste se charge.
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
        local message = "poste : " .. file:gsub("^" .. dir:gsub("%p", "%%%0") .. "/", "") .. " : " .. tostring(err)
        io.stderr:write(message .. "\n")
        pcall(hl.exec_cmd, "notify-send -u critical 'Hyprland : module poste' " .. "'" .. message:gsub("'", "'\\''") .. "'")
      end
    end
    handle:close()
  end
end

-- Toggle config flags dynamically.
require("default.hypr.toggles")

-- Add any other personal Hyprland configuration below.
-- o.window("qemu", { workspace = "5" })

-- Applis Qt : sélecteur de fichiers via le portail (donc Flea), au lieu de la
-- boîte GTK qu'elles dessinent elles-mêmes avec le thème gtk3 d'Omarchy.
hl.env("QT_QPA_PLATFORMTHEME", "xdgdesktopportal")
