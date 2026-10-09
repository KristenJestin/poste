// Adapted from Island (https://github.com/Guilhermerisu/island), MIT License,
// Copyright (c) 2026 Guilherme Pimenta. See ../LICENSE-island.
import QtQuick
import Quickshell.Widgets

// The island becomes a wide slider that fills with the accent from the left.
ClippingRectangle {
  id: slider
  required property var host
  required property bool shown
  required property Item shape
  readonly property real level: host.muted ? 0 : Math.max(0, Math.min(1, host.volume))
  property real shownLevel: level
  Behavior on shownLevel { NumberAnimation { duration: 220 * slider.host.motionScale; easing.type: Easing.OutCubic } }
  radius: shape.radius
  color: "transparent"
  opacity: shown ? 1 : 0
  visible: opacity > 0.01
  Behavior on opacity { NumberAnimation { duration: opacity > 0.5 ? 70 : 150 * slider.host.motionScale; easing.type: Easing.InOutQuad } }

  Rectangle { anchors.fill: parent; color: Qt.lighter(slider.host.colorIsland, 2.2) }
  Rectangle {
    id: fill
    anchors.left: parent.left
    anchors.top: parent.top
    anchors.bottom: parent.bottom
    width: parent.width * slider.shownLevel
    color: slider.host.colorAccent
  }
  Glyph {
    anchors.left: parent.left
    anchors.leftMargin: 16
    anchors.verticalCenter: parent.verticalCenter
    readonly property bool onFill: fill.width > x + width / 2
    glyph: slider.level <= 0 ? "󰖁" : slider.level < 0.34 ? "󰕿" : slider.level < 0.67 ? "󰖀" : "󰕾"
    color: onFill ? slider.host.colorAccentText : slider.host.colorText
    fontFamily: slider.host.fontFamily
    pixelSize: 22
    Behavior on color { ColorAnimation { duration: 120 } }
  }
  Text {
    anchors.right: parent.right
    anchors.rightMargin: 16
    anchors.verticalCenter: parent.verticalCenter
    readonly property bool onFill: fill.width > x + width / 2
    text: Math.round(slider.level * 100)
    color: onFill ? slider.host.colorAccentText : slider.host.colorText
    font.family: slider.host.uiFont
    font.pixelSize: 13
    font.weight: Font.DemiBold
    font.features: { "tnum": 1 }
  }
}
