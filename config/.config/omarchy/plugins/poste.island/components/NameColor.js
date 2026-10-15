// Stable color drawn from a name: same name, same color, from one time to
// the next. Used for everything that has no assigned color: a new model
// in the Usage view, an unknown agent in the control center.
.pragma library

// Hashing of the name to a tenth of a degree: Opus, Sonnet, Fable and Haiku land
// more than 75° apart.
function hue(name) {
  name = String(name || "")
  var h = 0
  for (var i = 0; i < name.length; i++) h = (h * 67 + name.charCodeAt(i)) % 3600
  return ((Math.floor(h / 10) + 40) % 360) / 360
}

function color(name, saturation, lightness) {
  return Qt.hsla(hue(name), saturation === undefined ? 0.6 : saturation, lightness === undefined ? 0.62 : lightness, 1)
}

function initial(name) {
  var m = String(name || "").match(/[A-Za-zÀ-ÿ0-9]/)
  return m ? m[0].toUpperCase() : "?"
}
