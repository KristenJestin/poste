-- See https://wiki.hypr.land/Configuring/Basics/Monitors/
-- List current monitors and supported resolutions with: hyprctl monitors all

-- Generic example: each machine puts its real monitors.lua in place through its data
-- repository (poste-data, config/.config/hypr/monitors.lua), which replaces this one.
-- Without it: every connected screen in preferred mode, and Hyprland's
-- workspaces 1 to 10, shared between the screens.
hl.env("GDK_SCALE", "1")

-- A screen by port (DP-1) or by its EDID description (desc:…, start of the
-- description from `hyprctl monitors`), which recognizes it whatever the port:
-- hl.monitor({ output = "DP-1", mode = "1920x1080@144", position = "0x0", scale = 1, transform = 1 }) -- portrait
-- hl.monitor({ output = "desc:Maker Model Serial", mode = "1920x1080@144", position = "1080x420", scale = 1 })

-- Fallback for any other connected screen (projector, etc.).
hl.monitor({ output = "", mode = "preferred", position = "auto", scale = 1 })

-- --- Workspaces per screen ------------------------------------------------
-- Every screen has its own workspaces 1 to 10 (see bindings.lua). Inside,
-- the screens of workspace_monitors hold fixed blocks in their order: first
-- screen 1-10, second 11-20, etc. default_name shows 1 to 10 on each screen
-- instead of 11-30. Any other screen (projector, dock, virtual output) takes
-- the first free block after the list when it arrives.
-- Several sets of screens (laptop at the office, alone...) fit in the same
-- list: an absent screen keeps its numbers. Unplugged, its workspaces go to a
-- remaining screen (the island shows them apart there) and come back to it
-- when it returns.
-- SUPER+CTRL+ALT+arrow sends the window to the screen in that arrow's direction (bindings.lua).
screen_keys = {
  -- { key = "LEFT", monitor = "DP-1", name = "left screen" },
}

workspace_monitors = {} -- { "DP-1", "desc:Maker Model Serial" }

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
