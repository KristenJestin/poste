import QtQuick

// This screen's workspaces, numbered 1-10 locally: 1-5 always, higher ones
// while they exist. The shown workspace is an accent chip, occupied ones are
// bright, empty ones dim. Click focuses that workspace on this screen.
Row {
  id: strip
  required property var host
  required property string screenName
  required property int offset
  spacing: 2

  readonly property int activeId: host.activeWorkspaces[screenName] !== undefined ? host.activeWorkspaces[screenName] : -1
  readonly property var ids: {
    var list = [1, 2, 3, 4, 5]
    for (var key in host.windowCounts) {
      var local = Number(key) - offset
      if (local > 0 && local <= 10 && list.indexOf(local) === -1) list.push(local)
    }
    if (activeId - offset > 0 && activeId - offset <= 10 && list.indexOf(activeId - offset) === -1) list.push(activeId - offset)
    list.sort(function(a, b) { return a - b })
    return list
  }

  Repeater {
    model: strip.ids
    delegate: Item {
      id: cell
      required property int modelData
      readonly property int globalId: strip.offset + modelData
      readonly property bool active: strip.activeId === globalId
      readonly property bool occupied: (strip.host.windowCounts[globalId] || 0) > 0
      width: 20
      height: 20

      Rectangle {
        anchors.centerIn: parent
        width: 20; height: 20; radius: 10
        color: cell.active ? strip.host.colorAccent : "transparent"
        Behavior on color { ColorAnimation { duration: 160 } }
      }
      Text {
        anchors.centerIn: parent
        text: cell.modelData === 10 ? "0" : String(cell.modelData)
        color: cell.active ? strip.host.colorAccentText
          : cell.occupied ? strip.host.colorText : strip.host.colorMuted
        opacity: cell.active || cell.occupied || mouse.containsMouse ? 1 : 0.6
        font.family: strip.host.uiFont
        font.pixelSize: 12
        font.weight: cell.active ? Font.Bold : Font.DemiBold
        font.features: { "tnum": 1 }
      }
      MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: strip.host.focusWorkspace(cell.globalId)
      }
    }
  }
}
