import QtQuick

// Footer of a control-center module: status line + link to the native panel.
Item {
  id: footer
  required property var host
  property string notice: ""
  property bool noticeError: false
  property string linkText: "Advanced settings…"
  signal advancedClicked()

  width: parent ? parent.width : 0
  height: 22

  Text {
    anchors.left: parent.left
    anchors.leftMargin: 4
    anchors.right: advanced.left
    anchors.rightMargin: 8
    anchors.verticalCenter: parent.verticalCenter
    elide: Text.ElideRight
    text: footer.notice
    opacity: footer.notice !== "" ? 1 : 0
    Behavior on opacity { NumberAnimation { duration: 200 } }
    color: footer.noticeError ? "#e5484d" : footer.host.colorMuted
    font.family: footer.host.uiFont
    font.pixelSize: 11
  }
  Text {
    id: advanced
    anchors.right: parent.right
    anchors.rightMargin: 4
    anchors.verticalCenter: parent.verticalCenter
    text: footer.linkText
    color: advMouse.containsMouse ? footer.host.colorText : footer.host.colorMuted
    font.family: footer.host.uiFont
    font.pixelSize: 11
    MouseArea { id: advMouse; anchors.fill: parent; anchors.margins: -4; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: footer.advancedClicked() }
  }
}
