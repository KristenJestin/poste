import QtQuick
import QtQuick.Effects
import Quickshell
import "AgentBrand.js" as AgentBrand

// Recent notifications, opened from the unread dot: newest first, relative
// time, app tile; hover shows a cross to drop one, "Tout effacer" drops all,
// clicking one opens its picture or focuses the app that sent it.
Item {
  id: list
  required property var host
  property bool active: false
  signal closeRequested()

  readonly property int maxListHeight: 440

  // A ListModel kept in step with host.notificationList by key, so arrivals
  // are insertions (animated) rather than a reset of the whole view.
  ListModel { id: rows }
  readonly property var source: host.notificationList
  onSourceChanged: sync()
  Component.onCompleted: sync()
  function sync() {
    var wanted = source || []
    var keys = {}
    for (var i = 0; i < wanted.length; i++) keys[wanted[i].key] = true
    for (var j = rows.count - 1; j >= 0; j--) if (!keys[rows.get(j).key]) rows.remove(j)
    for (var k = 0; k < wanted.length; k++) {
      var n = wanted[k]
      var entry = {
        key: String(n.key), app: String(n.app || ""), appIcon: String(n.appIcon || ""),
        summary: String(n.summary || ""), body: String(n.body || ""), image: String(n.image || ""),
        preview: String(n.preview || ""), file: String(n.file || ""), glyph: String(n.glyph || ""),
        urgency: Number(n.urgency || 0), timestamp: Number(n.timestamp || 0)
      }
      if (k < rows.count && rows.get(k).key === entry.key) continue
      var found = -1
      for (var m = k + 1; m < rows.count; m++) if (rows.get(m).key === entry.key) { found = m; break }
      if (found !== -1) rows.move(found, k, 1)
      else rows.insert(k, entry)
    }
    while (rows.count > wanted.length) rows.remove(rows.count - 1)
  }
  implicitHeight: header.height + 10 + (rows.count ? Math.min(maxListHeight, view.contentHeight) : empty.height)

  onActiveChanged: {
    if (!active) return
    host.markAllRead()
    host.refreshStore()
    view.positionViewAtBeginning()
    Qt.callLater(function() { list.forceActiveFocus() })
  }
  Keys.onEscapePressed: list.closeRequested()

  function iconSource(n) {
    var value = String(n.preview || n.image || n.appIcon || "")
    if (value === "") return ""
    if (value.indexOf("file://") === 0 || value.indexOf("image://") === 0) return value
    if (value.charAt(0) === "/") return "file://" + value
    return Quickshell.iconPath(value, true)
  }

  Item {
    id: header
    width: parent.width
    height: 26
    Text {
      anchors.left: parent.left
      anchors.leftMargin: 4
      anchors.verticalCenter: parent.verticalCenter
      text: "Notifications"
      color: list.host.colorText
      font.family: list.host.uiFont
      font.pixelSize: 15
      font.weight: Font.DemiBold
    }
    Text {
      anchors.right: parent.right
      anchors.rightMargin: 4
      anchors.verticalCenter: parent.verticalCenter
      visible: rows.count > 0
      text: "Clear all"
      color: clearMouse.containsMouse ? list.host.colorText : list.host.colorMuted
      font.family: list.host.uiFont
      font.pixelSize: 12
      font.weight: Font.DemiBold
      MouseArea {
        id: clearMouse
        anchors.fill: parent
        anchors.margins: -6
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: list.host.clearNotifications()
      }
    }
  }

  Text {
    id: empty
    anchors.top: header.bottom
    anchors.topMargin: 10
    anchors.horizontalCenter: parent.horizontalCenter
    visible: rows.count === 0
    height: 44
    verticalAlignment: Text.AlignVCenter
    text: "Nothing new"
    color: list.host.colorMuted
    font.family: list.host.uiFont
    font.pixelSize: 13
  }

  ListView {
    WheelScroll {}
    id: view
    anchors.top: header.bottom
    anchors.topMargin: 10
    anchors.left: parent.left
    anchors.right: parent.right
    height: Math.min(list.maxListHeight, contentHeight)
    clip: true
    spacing: 6
    boundsBehavior: Flickable.StopAtBounds
    model: rows

    // New notifications slide in at the top; the rest move down.
    add: Transition {
      ParallelAnimation {
        NumberAnimation { property: "opacity"; from: 0; to: 1; duration: 280; easing.type: Easing.OutCubic }
        NumberAnimation { property: "y"; from: -24; duration: 380; easing.type: Easing.OutQuint }
      }
    }
    populate: Transition {}

    remove: Transition {
      ParallelAnimation {
        NumberAnimation { property: "opacity"; to: 0; duration: 200; easing.type: Easing.InCubic }
        NumberAnimation { property: "x"; to: 40; duration: 240; easing.type: Easing.InCubic }
      }
    }
    displaced: Transition {
      NumberAnimation { properties: "y"; duration: 340; easing.type: Easing.OutQuint }
      NumberAnimation { property: "opacity"; to: 1; duration: 200 }
    }

    delegate: Item {
      id: row
      required property int index
      required property string key
      required property string app
      required property string appIcon
      required property string summary
      required property string body
      required property string image
      required property string preview
      required property string file
      required property string glyph
      required property int urgency
      required property double timestamp
      readonly property var modelData: ({ key: key, app: app, appIcon: appIcon, summary: summary, body: body,
        image: image, preview: preview, file: file, glyph: glyph, urgency: urgency, timestamp: timestamp })
      readonly property var brand: AgentBrand.brand(modelData.summary, modelData.app, modelData.appIcon)
      readonly property string source: brand ? "" : list.iconSource(modelData)
      width: view.width
      height: Math.max(58, texts.implicitHeight + 22)

      HoverHandler { id: rowHover }

      Rectangle {
        anchors.fill: parent
        radius: 16
        color: rowHover.hovered ? list.host.withAlpha(list.host.colorText, 0.08) : list.host.withAlpha(list.host.colorText, 0.04)
        border.width: row.modelData.urgency === 2 ? 1 : 0
        border.color: "#d9534f"
        Behavior on color { ColorAnimation { duration: 120 } }
      }
      MouseArea {
        anchors.fill: parent
        cursorShape: Qt.PointingHandCursor
        onClicked: { list.host.activateStoredNotification(row.modelData); list.closeRequested() }
      }

      Item {
        id: tile
        anchors.left: parent.left
        anchors.leftMargin: 10
        anchors.verticalCenter: parent.verticalCenter
        width: 36; height: 36
        Rectangle {
          anchors.fill: parent
          radius: 10
          visible: row.brand || img.status !== Image.Ready
          color: row.brand ? row.brand.tile : list.host.withAlpha(list.host.colorText, 0.08)
        }
        Image {
          id: img
          anchors.fill: parent
          source: row.source
          sourceSize.width: 72
          sourceSize.height: 72
          fillMode: Image.PreserveAspectCrop
          asynchronous: true
          visible: false
        }
        MultiEffect {
          anchors.fill: parent
          source: img
          visible: !row.brand && img.status === Image.Ready
          maskEnabled: true
          maskSource: mask
        }
        Item {
          id: mask
          anchors.fill: parent
          layer.enabled: true
          visible: false
          Rectangle { anchors.fill: parent; radius: 10 }
        }
        // Logos Claude / Codex : glyphes de police, gardés tels quels.
        Text {
          anchors.centerIn: parent
          visible: !!row.brand
          text: row.brand ? row.brand.glyph : ""
          color: row.brand ? row.brand.ink : list.host.colorText
          font.family: list.host.fontFamily
          font.pixelSize: 20
        }
        Glyph {
          anchors.centerIn: parent
          visible: !row.brand && img.status !== Image.Ready
          glyph: row.modelData.glyph || "󰂚"
          color: list.host.colorText
          fontFamily: list.host.fontFamily
          pixelSize: 15
        }
      }

      Column {
        id: texts
        anchors.left: tile.right
        anchors.leftMargin: 12
        anchors.right: parent.right
        anchors.rightMargin: 38
        anchors.verticalCenter: parent.verticalCenter
        spacing: 1
        Row {
          width: parent.width
          spacing: 8
          Text {
            id: title
            width: parent.width - age.width - 8
            text: row.modelData.summary || row.modelData.app || "Notification"
            textFormat: Text.PlainText
            elide: Text.ElideRight
            color: list.host.colorText
            font.family: list.host.uiFont
            font.pixelSize: 13
            font.weight: Font.DemiBold
          }
          Text {
            id: age
            anchors.baseline: title.baseline
            text: list.host.relativeTime(row.modelData.timestamp)
            color: list.host.colorMuted
            font.family: list.host.uiFont
            font.pixelSize: 11
          }
        }
        Text {
          width: parent.width
          visible: text !== ""
          text: String(row.modelData.body || "")
          textFormat: Text.PlainText
          wrapMode: Text.Wrap
          maximumLineCount: 2
          elide: Text.ElideRight
          color: list.host.withAlpha(list.host.colorText, 0.65)
          font.family: list.host.uiFont
          font.pixelSize: 12
        }
      }

      Rectangle {
        anchors.right: parent.right
        anchors.rightMargin: 10
        anchors.verticalCenter: parent.verticalCenter
        width: 22; height: 22; radius: 11
        opacity: rowHover.hovered ? 1 : 0
        Behavior on opacity { NumberAnimation { duration: 140 } }
        color: xMouse.containsMouse ? list.host.withAlpha(list.host.colorText, 0.18) : list.host.withAlpha(list.host.colorText, 0.08)
        Glyph {
          anchors.centerIn: parent
          glyph: "󰅖"
          color: list.host.colorText
          fontFamily: list.host.fontFamily
          pixelSize: 11
        }
        MouseArea {
          id: xMouse
          anchors.fill: parent
          hoverEnabled: true
          cursorShape: Qt.PointingHandCursor
          onClicked: list.host.removeNotification(row.modelData.key)
        }
      }
    }
  }
}
