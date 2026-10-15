import QtQuick
import QtQuick.Effects

// The background of a banner piece: the banner's dark glass (blurred by Hyprland), a discreet
// outline and a soft shadow (under Hyprland's blur threshold, ignore_alpha). Rounded into a
// pill on one line, the banner's `blockRadius` on several.
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
