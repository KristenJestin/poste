import QtQuick
import "WorkspaceGroups.js" as WorkspaceGroups

// This screen's workspaces, numbered 1-10: 1-5 always, beyond them those
// that exist. The one displayed is an accent pill, those that have
// windows are vivid, the empty ones are dimmed. A click shows it on this screen.
// After a thin bar: the workspaces of another screen that Hyprland has put
// here (host.visitingWorkspaces), with their number 1-10, one group per screen
// of origin (WorkspaceGroups.js); they go back to their screen when it returns.
Row {
  id: strip
  required property var host
  required property string screenName
  required property int offset
  spacing: 2

  readonly property int activeId: host.activeWorkspaces[screenName] !== undefined ? host.activeWorkspaces[screenName] : -1
  // Global ids to draw, 0 for the bar before each visiting group.
  readonly property var cells: WorkspaceGroups.cells(host.windowCounts, activeId, offset, host.visitingWorkspaces[screenName])

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
