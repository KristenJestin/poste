-- Change the default Omarchy look'n'feel.

-- https://wiki.hypr.land/Configuring/Basics/Variables/#general
-- hl.config({
--   general = {
--     -- No gaps between windows or borders.
--     gaps_in = 0,
--     gaps_out = 0,
--     border_size = 0,
--
--     -- Change to niri-like side-scrolling layout.
--     layout = "scrolling",
--   },
-- })

-- https://wiki.hypr.land/Configuring/Basics/Variables/#decoration
-- hl.config({
--   decoration = {
--     -- Use round window corners.
--     rounding = 8,
--
--     -- Dim unfocused windows (0.0 = no dim, 1.0 = fully dimmed).
--     dim_inactive = true,
--     dim_strength = 0.15,
--   },
-- })

-- https://wiki.hypr.land/Configuring/Basics/Variables/#animations
-- hl.config({
--   animations = {
--     -- Disable all animations.
--     enabled = false,
--   },
-- })

-- https://wiki.hypr.land/Configuring/Basics/Variables/#layout
-- hl.config({
--   layout = {
--     -- Avoid overly wide single-window layouts on wide screens.
--     single_window_aspect_ratio = { 1, 1 },
--   },
-- })

-- https://wiki.hypr.land/Configuring/Layouts/Scrolling-Layout/
-- hl.config({
--   scrolling = {
--     -- See only one column per screen instead of two.
--     column_width = 0.97,
--   },
-- })

-- --- Réglages perso (repris d'Omarchy 3) ----------------------------------
-- Bordures fines et neutres (couleurs : hyprland_active_border / inactive du thème).
hl.config({ general = { border_size = 1 } })

-- Coins arrondis et flou type "Acrylic".
hl.config({
  decoration = {
    rounding = 8,
    blur = {
      enabled = true,
      size = 10,
      passes = 3,
      new_optimizations = true,
      xray = true, -- le flou traverse les fenêtres -> rendu "Acrylic"
      ignore_opacity = true,
      noise = 0.08,
      contrast = 0.9,
      brightness = 0.9,
      vibrancy = 0.2,
    },
  },
})

-- Glissé-fondu entre workspaces.
hl.animation({ leaf = "workspaces", enabled = true, speed = 3.5, bezier = "easeOutQuint", style = "slidefade 15%" })

-- Intensité du voile sombre (dim_around) des menus.
hl.config({ decoration = { dim_around = 0.55 } })

-- Transparence des fenêtres comme sous Omarchy 3 (Quattro : 0.985 / 0.96,
-- quasi opaque, le flou Acrylic ne se voyait plus). Actif / inactif.
o.window({ tag = "default-opacity" }, { opacity = "0.97 0.9" })

-- Bandeau du bas (poste.island/bandeau : notifications, OSD, texte dicté) : fond plein,
-- sans flou ; ses apparitions sont animées dans l'île, pas par le compositeur.
hl.layer_rule({ match = { namespace = "^poste-bandeau$" }, no_anim = true })

-- Menu Omarchy (SUPER+ALT+ESPACE) : voile sombre derrière.
hl.layer_rule({ match = { namespace = "^omarchy-menu$" }, dim_around = true })

-- Panneaux natifs (Réseau, Bluetooth, Audio, VPN…) ouverts depuis l'île :
-- même voile que l'île dépliée et une petite apparition. La surface couvre
-- tout l'écran (la carte est dedans), d'où un popin léger plutôt qu'un slide.
hl.layer_rule({ match = { namespace = "^omarchy-keyboard-panel$" }, no_anim = false, animation = "popin 96%", dim_around = true })
