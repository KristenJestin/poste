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
