// What a screen's bar shows: its workspaces 1-10, then, after a
// separator, those of other screens that Hyprland has put here (screen unplugged
// or taken by another tool), one group per screen of origin. The screen
// of origin of a workspace is its block of tens (workspace_monitors in
// monitors.lua): 1-10 the first screen, 11-20 the second, etc.
.pragma library

function numeric(a, b) { return a - b }

// Block of tens of a workspace: 0 for 1-10, 1 for 11-20…
function originSlot(id) {
  return Math.floor((id - 1) / 10)
}

// Slots of the screens outside workspace_monitors, from the screen-slots file
// that Hyprland writes (bindings.lua): one "<name> <slot>" per line.
function parseSlots(text) {
  var slots = {}
  var lines = String(text || "").split("\n")
  for (var i = 0; i < lines.length; i++) {
    var m = /^(\S+)\s+(\d+)$/.exec(lines[i])
    if (m) slots[m[1]] = Number(m[2])
  }
  return slots
}

// Does an entry of workspace_monitors name this screen? A port name (DP-1) or
// "desc:" and the start of the screen's EDID description, as Hyprland matches.
function screenMatches(entry, name, description) {
  if (entry.indexOf("desc:") === 0)
    return String(description || "").indexOf(entry.slice(5).trim()) === 0
  return entry === name
}

// Block of ten of a screen: its place in workspace_monitors, else the slot
// Hyprland gave it when it arrived, else the first block after the list.
function screenSlot(entries, slots, name, description) {
  for (var i = 0; i < entries.length; i++)
    if (screenMatches(entries[i], name, description)) return i
  if (slots && slots[name] !== undefined) return slots[name]
  return entries.length
}

// Workspaces of the screen itself (global ids): 1-5 always, beyond them those that
// exist (windows) or the one displayed.
function ownIds(windowCounts, activeId, offset) {
  var list = [1, 2, 3, 4, 5]
  for (var key in windowCounts) {
    var local = Number(key) - offset
    if (local > 0 && local <= 10 && list.indexOf(local) === -1) list.push(local)
  }
  var shown = activeId - offset
  if (shown > 0 && shown <= 10 && list.indexOf(shown) === -1) list.push(shown)
  list.sort(numeric)
  return list.map(function(n) { return offset + n })
}

// Cells to draw: global ids, 0 for a separator. Each group of
// visiting workspaces starts with a separator.
function cells(windowCounts, activeId, offset, visiting) {
  var list = ownIds(windowCounts, activeId, offset)
  var guests = (visiting || []).slice().sort(numeric)
  var group = null
  for (var i = 0; i < guests.length; i++) {
    var slot = originSlot(guests[i])
    if (slot !== group) { list.push(0); group = slot }
    list.push(guests[i])
  }
  return list
}
