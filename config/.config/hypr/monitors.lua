-- See https://wiki.hypr.land/Configuring/Basics/Monitors/
-- List current monitors and supported resolutions with: hyprctl monitors all

-- Exemple générique : chaque machine pose son vrai monitors.lua par son dépôt de
-- données (poste-data, config/.config/hypr/monitors.lua), qui remplace celui-ci.
-- Sans lui : tout écran branché en mode préféré, et les workspaces 1 à 10
-- d'Hyprland, partagés entre les écrans.
hl.env("GDK_SCALE", "1")

-- Un écran par prise (DP-1) ou par sa description EDID (desc:…, début de la
-- description de `hyprctl monitors`), qui le reconnaît quelle que soit la prise :
-- hl.monitor({ output = "DP-1", mode = "1920x1080@144", position = "0x0", scale = 1, transform = 1 }) -- portrait
-- hl.monitor({ output = "desc:Fabricant Modèle Série", mode = "1920x1080@144", position = "1080x420", scale = 1 })

-- Fallback pour tout autre écran branché (vidéoprojecteur, etc.).
hl.monitor({ output = "", mode = "preferred", position = "auto", scale = 1 })

-- --- Workspaces par écran -------------------------------------------------
-- Chaque écran de workspace_monitors a ses propres workspaces 1 à 10 (voir
-- bindings.lua) : en interne, premier écran = 1-10, deuxième = 11-20, etc.
-- default_name affiche 1 à 10 sur chaque écran au lieu de 11-30.
-- Plusieurs jeux d'écrans (portable au bureau, seul…) tiennent dans la même
-- liste : un écran absent garde ses numéros. Débranché, Hyprland pose ses
-- workspaces sur un écran restant (l'île les y montre à part) et les lui rend
-- quand il revient sur la même prise.
-- SUPER+CTRL+ALT+flèche envoie la fenêtre sur l'écran de cette flèche (bindings.lua).
screen_keys = {
  -- { key = "LEFT", monitor = "DP-1", name = "left screen" },
}

workspace_monitors = {} -- { "DP-1", "desc:Fabricant Modèle Série" }

for index, output in ipairs(workspace_monitors) do
  local offset = (index - 1) * 10
  for n = 1, 10 do
    hl.workspace_rule({
      workspace = tostring(offset + n),
      monitor = output,
      default = n == 1,
      default_name = tostring(n),
    })
  end
end
