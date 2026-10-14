import QtQuick
import QtQuick.Effects

// Le fond d'une pièce du bandeau : le verre sombre du bandeau (flouté par Hyprland), un liseré
// discret et une ombre douce (sous le seuil de flou de Hyprland, ignore_alpha). Arrondi en
// pilule sur une ligne, `blockRadius` du bandeau sur plusieurs.
Item {
  id: surface
  required property var bandeau
  property real radius: height / 2

  RectangularShadow {
    anchors.fill: fill
    radius: fill.radius
    blur: 26
    spread: 0
    offset.y: 6
    color: Qt.rgba(0, 0, 0, 0.38)
  }
  Rectangle {
    id: fill
    anchors.fill: parent
    radius: Math.min(surface.radius, height / 2)
    color: surface.bandeau.withAlpha(surface.bandeau.colorSurface, surface.bandeau.surfaceOpacity)
    border.width: 1
    border.color: surface.bandeau.withAlpha(surface.bandeau.colorText, 0.14)
  }
}
