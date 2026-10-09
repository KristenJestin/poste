import QtQuick

// One row of a control-center module list: glyph, title, subtitle, trailing
// glyph; grey selection with the orange marker. Extra content (password
// field, Ctrl+B actions) goes below the row via `extraHeight` + children.
Item {
  id: row
  required property var host
  property real rowHeight: 46
  property string glyph: ""
  property string title: ""
  property string subtitle: ""
  property bool subtitleAccent: false
  property string trailing: ""
  property bool strong: false
  property bool isSelected: false
  property real extraHeight: 0
  default property alias extra: extraArea.data
  signal clicked(int button)
  signal hovered()

  width: parent ? parent.width : 0
  height: rowHeight + extraHeight
  Behavior on height { NumberAnimation { duration: 220; easing.type: Easing.OutQuint } }
  clip: true

  Rectangle {
    anchors.fill: parent
    radius: 14
    color: row.isSelected ? row.host.withAlpha(row.host.colorText, 0.09) : "transparent"
    Behavior on color { ColorAnimation { duration: 140 } }
    Rectangle {
      anchors.left: parent.left
      anchors.leftMargin: 6
      y: (row.rowHeight - height) / 2
      width: 3; height: 18; radius: 1.5
      color: row.host.colorAccent
      opacity: row.isSelected ? 1 : 0
      Behavior on opacity { NumberAnimation { duration: 140 } }
    }
  }
  MouseArea {
    width: parent.width
    height: row.rowHeight
    hoverEnabled: true
    cursorShape: Qt.PointingHandCursor
    acceptedButtons: Qt.LeftButton | Qt.RightButton
    onEntered: row.hovered()
    onClicked: function(mouse) { row.clicked(mouse.button) }
  }
  Glyph {
    id: glyphText
    x: 18
    width: 18
    horizontalAlignment: Text.AlignHCenter
    y: (row.rowHeight - height) / 2
    glyph: row.glyph
    color: row.strong ? row.host.colorText : row.host.withAlpha(row.host.colorText, 0.7)
    fontFamily: row.host.fontFamily
    pixelSize: 16
  }
  Column {
    anchors.left: glyphText.right
    anchors.leftMargin: 12
    anchors.right: trailingText.left
    anchors.rightMargin: 8
    y: (row.rowHeight - height) / 2
    spacing: 1
    Text {
      width: parent.width
      elide: Text.ElideRight
      text: row.title
      color: row.host.colorText
      font.family: row.host.uiFont
      font.pixelSize: 13
      font.weight: row.strong ? Font.DemiBold : Font.Normal
    }
    Text {
      visible: text !== ""
      width: parent.width
      elide: Text.ElideRight
      text: row.subtitle
      color: row.subtitleAccent ? row.host.colorAccent : row.host.colorMuted
      font.family: row.host.uiFont
      font.pixelSize: 11
    }
  }
  Glyph {
    id: trailingText
    anchors.right: parent.right
    anchors.rightMargin: 14
    y: (row.rowHeight - height) / 2
    glyph: row.trailing
    color: row.host.colorMuted
    fontFamily: row.host.fontFamily
    pixelSize: 12
  }
  Item {
    id: extraArea
    x: 12
    y: row.rowHeight
    width: parent.width - 24
    height: row.extraHeight
  }
}
