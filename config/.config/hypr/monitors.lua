-- See https://wiki.hypr.land/Configuring/Basics/Monitors/
-- List current monitors and supported resolutions with: hyprctl monitors all

-- 3 écrans Iiyama PL2470H identiques, 1080p, 165 Hz, scale 1.
-- Ordre PHYSIQUE gauche -> droite : DP-3 | DP-1 | DP-2.
-- DP-3 en PORTRAIT (transform 1 = 90°). Les 2 paysages sont centrés
-- verticalement sur le portrait : portrait haut 1920 (centre y=960),
-- paysage haut 1080 -> top à y=420. Si DP-3 est tête-en-bas, passer
-- le transform de 1 à 3 (270°).
hl.env("GDK_SCALE", "1")

hl.monitor({ output = "DP-3", mode = "1920x1080@165", position = "0x0", scale = 1, transform = 1 }) -- gauche, PORTRAIT
hl.monitor({ output = "DP-1", mode = "1920x1080@165", position = "1080x420", scale = 1 }) -- centre
hl.monitor({ output = "DP-2", mode = "1920x1080@165", position = "3000x420", scale = 1 }) -- droite

-- Fallback pour tout autre écran branché (vidéoprojecteur, etc.).
hl.monitor({ output = "", mode = "preferred", position = "auto", scale = 1 })

-- --- Workspaces par écran -------------------------------------------------
-- Chaque écran a ses propres workspaces 1 à 10 (voir bindings.lua) :
-- en interne, gauche = 1-10, centre = 11-20, droite = 21-30.
-- default_name affiche 1 à 10 sur chaque écran au lieu de 11-30.
workspace_monitors = { "DP-3", "DP-1", "DP-2" }

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
