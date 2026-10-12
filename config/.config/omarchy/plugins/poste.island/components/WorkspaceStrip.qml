import QtQuick

// This screen's workspaces, numbered 1-10 locally: 1-5 always, higher ones
// while they exist. The shown workspace is an accent chip, occupied ones are
// bright, empty ones dim. Click focuses that workspace on this screen.
// After a thin bar: workspaces of an unplugged screen that Hyprland parked
// here (host.visitingWorkspaces), with their own 1-10 number; they go back
// to their screen when it is plugged in again.
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
  readonly property var visiting: host.visitingWorkspaces[screenName] || []
  // Global ids to draw, 0 standing for the bar before the visiting ones.
  readonly property var cells: {
    var list = ids.map(function(n) { return offset + n })
    return visiting.length > 0 ? list.concat([0], visiting) : list
  }

  Repeater {
    model: strip.cells
    delegate: Item {
      id: cell
      required property int modelData
      readonly property bool divider: modelData === 0
      readonly property int local: ((modelData - 1) % 10) + 1
      readonly property bool active: strip.activeId === modelData
      readonly property bool occupied: (strip.host.windowCounts[modelData] || 0) > 0
      width: divider ? 9 : 20
      height: 20

      Rectangle {
        visible: cell.divider
        anchors.centerIn: parent
        width: 1; height: 12
        color: strip.host.withAlpha(strip.host.colorText, 0.12)
      }
      Rectangle {
        visible: !cell.divider
        anchors.centerIn: parent
        width: 20; height: 20; radius: 10
        color: cell.active ? strip.host.colorAccent : "transparent"
        Behavior on color { ColorAnimation { duration: 160 } }
      }
      Text {
        visible: !cell.divider
        anchors.centerIn: parent
        text: cell.local === 10 ? "0" : String(cell.local)
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
        visible: !cell.divider
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: strip.host.focusWorkspace(cell.modelData)
      }
    }
  }
}
