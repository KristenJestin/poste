// Ce que la barre d'un écran affiche : ses workspaces 1-10, puis, après un
// séparateur, ceux d'autres écrans que Hyprland a posés ici (écran débranché
// ou pris par un autre outil), un groupe par écran d'origine. L'écran
// d'origine d'un workspace est son bloc de dizaines (workspace_monitors dans
// monitors.lua) : 1-10 le premier écran, 11-20 le deuxième, etc.
.pragma library

function numeric(a, b) { return a - b }

// Bloc de dizaines d'un workspace : 0 pour 1-10, 1 pour 11-20…
function originSlot(id) {
  return Math.floor((id - 1) / 10)
}

// Workspaces propres à l'écran (ids globaux) : 1-5 toujours, au-delà ceux qui
// existent (fenêtres) ou celui qui est affiché.
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

// Cases à dessiner : ids globaux, 0 pour un séparateur. Chaque groupe de
// workspaces de passage commence par un séparateur.
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
