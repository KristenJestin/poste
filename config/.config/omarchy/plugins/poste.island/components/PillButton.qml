import QtQuick

// Small rounded action button (Ctrl+B actions, module buttons).
Rectangle {
  id: sb
  required property var host
  property string glyph: ""
  property string label: ""
  property bool highlighted: false
  property bool danger: false
  property bool accent: false
  signal clicked()
  height: 28
  width: sbRow.implicitWidth + 20
  radius: 14
  color: sb.danger ? sb.host.withAlpha("#e5484d", sbMouse.containsMouse || sb.highlighted ? 0.32 : 0.2)
    : sb.accent ? sb.host.withAlpha(sb.host.colorAccent, sbMouse.containsMouse || sb.highlighted ? 0.9 : 0.75)
    : sb.host.withAlpha(sb.host.colorText, sbMouse.containsMouse || sb.highlighted ? 0.14 : 0.06)
  Behavior on color { ColorAnimation { duration: 120 } }
  Row {
    id: sbRow
    anchors.centerIn: parent
    spacing: 6
    Glyph { glyph: sb.glyph; visible: glyph !== ""; color: sb.accent ? sb.host.colorAccentText : sb.host.colorText; fontFamily: sb.host.fontFamily; pixelSize: 12; anchors.verticalCenter: parent.verticalCenter }
    Text { text: sb.label; color: sb.accent ? sb.host.colorAccentText : sb.host.colorText; font.family: sb.host.uiFont; font.pixelSize: 12; font.weight: sb.accent ? Font.DemiBold : Font.Normal; anchors.verticalCenter: parent.verticalCenter }
  }
  MouseArea { id: sbMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: sb.clicked() }
}
