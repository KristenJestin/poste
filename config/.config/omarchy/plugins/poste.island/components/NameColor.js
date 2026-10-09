// Couleur stable tirée d'un nom : même nom, même couleur, d'une fois sur
// l'autre. Sert à tout ce qui n'a pas de couleur attitrée : un nouveau modèle
// dans la vue Usage, un agent inconnu dans le centre de contrôle.
.pragma library

// Hachage du nom au dixième de degré : Opus, Sonnet, Fable et Haiku tombent
// à plus de 75° les uns des autres.
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
