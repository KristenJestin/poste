import QtQuick

// Thin volume slider in the island's style (0..1), with a mute button.
Item {
  id: slider
  required property var host
  property real value: 0
  property bool muted: false
  property string glyph: "󰕾"
  property string mutedGlyph: "󰝟"
  signal moved(real v)
  signal muteClicked()

  width: parent ? parent.width : 0
  height: 30

  Rectangle {
    id: muteButton
    anchors.left: parent.left
    anchors.verticalCenter: parent.verticalCenter
    width: 30; height: 30; radius: 15
    color: muteMouse.containsMouse ? slider.host.withAlpha(slider.host.colorText, 0.1) : "transparent"
    Glyph {
      anchors.centerIn: parent
      glyph: slider.muted ? slider.mutedGlyph : slider.glyph
      color: slider.muted ? slider.host.colorMuted : slider.host.colorText
      fontFamily: slider.host.fontFamily
      pixelSize: 15
    }
    MouseArea { id: muteMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: slider.muteClicked() }
  }
  Text {
    id: pct
    anchors.right: parent.right
    anchors.verticalCenter: parent.verticalCenter
    width: 36
    horizontalAlignment: Text.AlignRight
    text: Math.round(slider.value * 100) + " %"
    color: slider.host.colorMuted
    font.family: slider.host.uiFont
    font.pixelSize: 11
    font.features: { "tnum": 1 }
  }
  Item {
    id: track
    anchors.left: muteButton.right
    anchors.leftMargin: 10
    anchors.right: pct.left
    anchors.rightMargin: 10
    anchors.verticalCenter: parent.verticalCenter
    height: 20
    readonly property real shown: Math.max(0, Math.min(1, slider.value))
    Rectangle {
      anchors.verticalCenter: parent.verticalCenter
      width: parent.width; height: 6; radius: 3
      color: slider.host.withAlpha(slider.host.colorText, 0.12)
    }
    Rectangle {
      anchors.verticalCenter: parent.verticalCenter
      width: Math.max(6, parent.width * track.shown); height: 6; radius: 3
      color: slider.muted ? slider.host.withAlpha(slider.host.colorText, 0.35) : slider.host.colorText
      Behavior on width { enabled: !drag.pressed; NumberAnimation { duration: 120 } }
    }
    MouseArea {
      id: drag
      anchors.fill: parent
      anchors.topMargin: -6
      anchors.bottomMargin: -6
      cursorShape: Qt.PointingHandCursor
      function set(mx) { slider.moved(Math.max(0, Math.min(1, mx / track.width))) }
      onPressed: function(mouse) { set(mouse.x) }
      onPositionChanged: function(mouse) { if (pressed) set(mouse.x) }
      onWheel: function(wheel) { slider.moved(Math.max(0, Math.min(1, slider.value + (wheel.angleDelta.y > 0 ? 0.02 : -0.02)))) }
    }
  }
}
