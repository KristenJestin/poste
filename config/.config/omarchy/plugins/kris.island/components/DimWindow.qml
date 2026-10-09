import QtQuick
import Quickshell
import Quickshell.Wayland

// Full-screen dim behind an unfolded island (launcher, notifications,
// control center, player), like Vicinae's dim_around. It stays mapped at all
// times in the Top layer (above windows, below the island and the toasts,
// which are Overlay) and only takes clicks while shown: a click on it
// closes the island.
PanelWindow {
  id: dim
  property bool shown: false
  signal dismissed()

  color: "transparent"
  exclusionMode: ExclusionMode.Ignore
  anchors { top: true; bottom: true; left: true; right: true }
  WlrLayershell.namespace: "kris-island-dim"
  WlrLayershell.layer: WlrLayer.Top
  WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
  // Deux zones fixes plutôt qu'une Region dont l'item change : celle-ci
  // n'était pas recalculée, le voile s'affichait sans jamais prendre le clic.
  Region { id: veilRegion; item: veil }
  Region { id: noRegion }
  mask: dim.shown ? veilRegion : noRegion

  Rectangle {
    id: veil
    anchors.fill: parent
    color: "black"
    opacity: dim.shown ? 0.5 : 0
    Behavior on opacity { NumberAnimation { duration: dim.shown ? 220 : 180; easing.type: Easing.OutCubic } }
    MouseArea {
      anchors.fill: parent
      enabled: dim.shown
      acceptedButtons: Qt.AllButtons
      onClicked: dim.dismissed()
    }
  }
}
