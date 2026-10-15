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

-- --- Personal settings (taken from Omarchy 3) ----------------------------
-- Thin, neutral borders (colors: the theme's hyprland_active_border / inactive).
hl.config({ general = { border_size = 1 } })

-- Rounded corners and "Acrylic"-type blur.
hl.config({
  decoration = {
    rounding = 8,
    blur = {
      enabled = true,
      size = 10,
      passes = 3,
      new_optimizations = true,
      xray = true, -- the blur goes through the windows -> "Acrylic" look
      ignore_opacity = true,
      noise = 0.08,
      contrast = 0.9,
      brightness = 0.9,
      vibrancy = 0.2,
    },
  },
})

-- Slide-fade between workspaces.
hl.animation({ leaf = "workspaces", enabled = true, speed = 3.5, bezier = "easeOutQuint", style = "slidefade 15%" })

-- Strength of the dark veil (dim_around) of the menus.
hl.config({ decoration = { dim_around = 0.55 } })

-- Window transparency as under Omarchy 3 (Quattro: 0.985 / 0.96,
-- almost opaque, the Acrylic blur was no longer visible). Active / inactive.
o.window({ tag = "default-opacity" }, { opacity = "0.97 0.9" })

-- Banner (poste.island/bandeau: notifications, OSD, dictated text under the island, dictation
-- pill at the bottom): dark glass, blur of the windows really behind (xray off), only
-- under the pieces (ignore_alpha > alpha of the shadow 0.38, < alpha of the glass 0.86). Its
-- appearances are animated in the island, not by the compositor.
hl.layer_rule({ match = { namespace = "^poste-bandeau$" }, no_anim = true, blur = true, ignore_alpha = 0.45, xray = false })

-- Omarchy menu (SUPER+ALT+SPACE): dark veil behind.
hl.layer_rule({ match = { namespace = "^omarchy-menu$" }, dim_around = true })

-- Native panels (Network, Bluetooth, Audio, VPN…) opened from the island:
-- same veil as the unfolded island and a small appearance. The surface covers
-- the whole screen (the card is inside), hence a light popin rather than a slide.
hl.layer_rule({ match = { namespace = "^omarchy-keyboard-panel$" }, no_anim = false, animation = "popin 96%", dim_around = true })
