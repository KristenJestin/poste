-- Add-on voxtype: dictation key (loaded by hyprland.lua).
-- Installed by `poste sys setup` in ~/.local/share/poste/modules/voxtype/hypr/.

-- SUPER+A: a short tap toggles, holding is push-to-talk (poste voxtype press / release).
-- voxtype pastes with Ctrl+V: if it does while SUPER is still held, that is SUPER+Ctrl+V and
-- nothing is pasted. The press marks "SUPER held", and Hyprland (which sees the keyboard) removes
-- the mark as soon as SUPER is released; `poste voxtype focus restore` waits for that.
local super_held_file = (os.getenv("XDG_RUNTIME_DIR") or "/tmp") .. "/voxtype-super-held"
local super_watch = nil
local function super_down()
  return hl.is_key_down(133) or hl.is_key_down(134) -- Super_L / Super_R (X11 keycodes)
end
o.bind("SUPER + A", "Dictation (voxtype: tap to toggle, hold to talk)", function()
  local f = io.open(super_held_file, "w")
  if f then f:close() end
  if super_watch then super_watch:set_enabled(false) end
  super_watch = hl.timer(function()
    if not super_down() then
      os.remove(super_held_file)
      super_watch:set_enabled(false)
    end
  end, { timeout = 100, type = "repeat" })
  hl.exec_cmd("poste voxtype press")
end)
o.bind("SUPER + A", nil, "poste voxtype release", { release = true })
