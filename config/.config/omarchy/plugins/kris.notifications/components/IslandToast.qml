import QtQuick
import QtQuick.Effects
import Quickshell
import qs.Commons
import "../NotificationLogic.js" as NotificationLogic
import "AgentBrand.js" as AgentBrand

// A floating notification, macOS-style frosted glass: a translucent grey pill
// lighter than the windows behind it (Hyprland blurs what is under it, see
// the "omarchy-notifications" layer rule in ~/.config/hypr/looknfeel.lua),
// a faint light inner rim and a soft wide shadow. Critical ones get a thin
// red rim and a red dot; the thin bar at the bottom is the remaining lifetime.
Item {
  id: toast

  property string app: ""
  property string appIcon: ""
  property string summary: ""
  property string body: ""
  property string image: ""
  property string glyph: ""
  property int urgency: 1
  property real progress: 1
  property bool showProgress: false
  readonly property bool hovered: hover.hovered

  signal closeRequested()
  signal cardClicked()

  readonly property bool critical: urgency === 2
  readonly property var brand: AgentBrand.brand(summary, app, appIcon)
  readonly property string bodyText: NotificationLogic.sanitizeBody(body, app, appIcon)
  readonly property string iconSource: brand ? "" : (image.length > 0 ? fileUrl(image) : themedIcon(appIcon))
  // Glass: background tinted 11% toward the foreground, 76% opaque. The
  // shadow stays under Hyprland's ignore_alpha (0.45) so only the pill blurs.
  readonly property color glass: {
    var c = Qt.tint(Color.background, Qt.rgba(Color.foreground.r, Color.foreground.g, Color.foreground.b, 0.11))
    return Qt.rgba(c.r, c.g, c.b, 0.76)
  }
  readonly property color text: Qt.tint(Color.foreground, Qt.rgba(1, 1, 1, 0.35))
  readonly property color bodyColor: Color.foreground

  function fileUrl(value) {
    if (value.indexOf("file://") === 0 || value.indexOf("image://") === 0) return value
    if (value.charAt(0) === "/") return Util.fileUrl(value)
    return value
  }
  function themedIcon(value) {
    value = String(value || "")
    if (value === "") return ""
    if (value.indexOf("file://") === 0 || value.indexOf("image://") === 0 || value.charAt(0) === "/") return fileUrl(value)
    return Quickshell.iconPath(value, true)
  }

  implicitWidth: 420
  implicitHeight: Math.max(64, content.implicitHeight + 28)

  HoverHandler { id: hover }

  // Soft shadow under the pill.
  RectangularShadow {
    anchors.fill: pill
    radius: pill.radius
    blur: 40
    spread: 2
    offset.y: 10
    color: Qt.rgba(0, 0, 0, 0.4)
  }

  Rectangle {
    id: pill
    anchors.fill: parent
    radius: Math.min(height / 2, 26)
    color: toast.glass
    border.width: 1
    border.color: toast.critical ? Qt.rgba(Color.urgent.r, Color.urgent.g, Color.urgent.b, 0.8)
      : Qt.rgba(1, 1, 1, 0.1)
    clip: true

    MouseArea {
      anchors.fill: parent
      acceptedButtons: Qt.LeftButton | Qt.RightButton
      cursorShape: Qt.PointingHandCursor
      onClicked: function(mouse) {
        if (mouse.button === Qt.RightButton) toast.closeRequested()
        else toast.cardClicked()
      }
    }

    // Remaining lifetime.
    Rectangle {
      visible: toast.showProgress
      anchors.bottom: parent.bottom
      anchors.left: parent.left
      anchors.leftMargin: pill.radius
      anchors.bottomMargin: 1
      height: 2
      radius: 1
      width: (parent.width - pill.radius * 2) * Math.max(0, Math.min(1, toast.progress))
      color: Qt.rgba(1, 1, 1, 0.22)
    }
  }

  Row {
    id: content
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.leftMargin: 12
    anchors.rightMargin: 40
    anchors.verticalCenter: parent.verticalCenter
    spacing: 12

    Item {
      id: tile
      width: 40; height: 40
      anchors.verticalCenter: parent.verticalCenter
      Rectangle {
        id: tileBg
        anchors.fill: parent
        radius: 12
        color: toast.brand ? toast.brand.tile : Qt.rgba(1, 1, 1, 0.1)
        visible: toast.brand || tileImage.status !== Image.Ready
      }
      Image {
        id: tileImage
        anchors.fill: parent
        source: toast.iconSource
        sourceSize.width: 80
        sourceSize.height: 80
        fillMode: toast.image.length > 0 ? Image.PreserveAspectCrop : Image.PreserveAspectFit
        asynchronous: true
        visible: false
      }
      MultiEffect {
        anchors.fill: parent
        source: tileImage
        visible: !toast.brand && tileImage.status === Image.Ready
        maskEnabled: true
        maskSource: tileMask
      }
      Item {
        id: tileMask
        anchors.fill: parent
        layer.enabled: true
        visible: false
        Rectangle { anchors.fill: parent; radius: 12 }
      }
      Text {
        anchors.centerIn: parent
        visible: toast.brand || tileImage.status !== Image.Ready
        text: toast.brand ? toast.brand.glyph : (toast.glyph || "󰂚")
        color: toast.brand ? toast.brand.ink : toast.text
        font.family: "JetBrainsMono Nerd Font"
        font.pixelSize: toast.brand ? 24 : 18
      }
    }

    Column {
      anchors.verticalCenter: parent.verticalCenter
      width: content.width - tile.width - content.spacing
      spacing: 2
      Row {
        width: parent.width
        spacing: 6
        Rectangle {
          visible: toast.critical
          anchors.verticalCenter: parent.verticalCenter
          width: 7; height: 7; radius: 3.5
          color: Color.urgent
        }
        Text {
          width: parent.width - (toast.critical ? 13 : 0)
          text: toast.summary || toast.app || "Notification"
          textFormat: Text.PlainText
          elide: Text.ElideRight
          color: toast.text
          font.family: "Adwaita Sans"
          font.pixelSize: 14
          font.weight: Font.DemiBold
          font.letterSpacing: -0.2
        }
      }
      Text {
        width: parent.width
        visible: text !== ""
        text: toast.bodyText
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        maximumLineCount: 2
        elide: Text.ElideRight
        color: toast.bodyColor
        font.family: "Adwaita Sans"
        font.pixelSize: 13
        lineHeight: 1.1
      }
    }
  }

  // Close button, shown on hover.
  Rectangle {
    anchors.right: parent.right
    anchors.rightMargin: 12
    anchors.verticalCenter: parent.verticalCenter
    width: 22; height: 22; radius: 11
    color: closeMouse.containsMouse ? Qt.rgba(toast.text.r, toast.text.g, toast.text.b, 0.16) : Qt.rgba(toast.text.r, toast.text.g, toast.text.b, 0.07)
    opacity: toast.hovered ? 1 : 0
    Behavior on opacity { NumberAnimation { duration: 150 } }
    Text {
      anchors.centerIn: parent
      text: "󰅖"
      color: toast.text
      font.family: "JetBrainsMono Nerd Font"
      font.pixelSize: 12
    }
    MouseArea {
      id: closeMouse
      anchors.fill: parent
      hoverEnabled: true
      cursorShape: Qt.PointingHandCursor
      onClicked: toast.closeRequested()
    }
  }
}
