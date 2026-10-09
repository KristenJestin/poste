import QtQuick

// Header of a control-center module (Wi-Fi, Bluetooth, Audio, VPN):
// back button, title, optional status text and on/off switch.
Item {
  id: header
  required property var host
  property string title: ""
  property string status: ""
  property bool showSwitch: false
  property bool switchOn: false
  signal backClicked()
  signal switchClicked()

  width: parent ? parent.width : 0
  height: 30

  Rectangle {
    id: backButton
    anchors.left: parent.left
    anchors.verticalCenter: parent.verticalCenter
    width: 28; height: 28; radius: 14
    color: backMouse.containsMouse ? header.host.withAlpha(header.host.colorText, 0.1) : "transparent"
    Behavior on color { ColorAnimation { duration: 120 } }
    Glyph { anchors.centerIn: parent; glyph: "󰅁"; color: header.host.colorText; fontFamily: header.host.fontFamily; pixelSize: 17 }
    MouseArea { id: backMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: header.backClicked() }
  }
  Text {
    anchors.left: backButton.right
    anchors.leftMargin: 8
    anchors.verticalCenter: parent.verticalCenter
    text: header.title
    color: header.host.colorText
    font.family: header.host.uiFont
    font.pixelSize: 15
    font.weight: Font.DemiBold
  }
  Text {
    anchors.right: header.showSwitch ? toggle.left : parent.right
    anchors.rightMargin: header.showSwitch ? 10 : 4
    anchors.verticalCenter: parent.verticalCenter
    opacity: header.status !== "" ? 1 : 0
    Behavior on opacity { NumberAnimation { duration: 200 } }
    text: header.status
    color: header.host.colorMuted
    font.family: header.host.uiFont
    font.pixelSize: 11
  }
  Rectangle {
    id: toggle
    visible: header.showSwitch
    anchors.right: parent.right
    anchors.verticalCenter: parent.verticalCenter
    width: 38; height: 22; radius: 11
    color: header.switchOn ? header.host.colorAccent : header.host.withAlpha(header.host.colorText, 0.16)
    Behavior on color { ColorAnimation { duration: 180 } }
    Rectangle {
      width: 18; height: 18; radius: 9
      y: 2
      x: header.switchOn ? parent.width - width - 2 : 2
      Behavior on x { NumberAnimation { duration: 200; easing.type: Easing.OutCubic } }
      color: "#ffffff"
    }
    MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: header.switchClicked() }
  }
}
