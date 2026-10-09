import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import qs.Commons
import qs.Ui

// Clone de omarchy.workspaces, version "workspaces par écran" : chaque écran
// a ses propres 1 à 10, et chaque barre affiche / cible ceux de son écran.
// Même correspondance que workspace_monitors dans ~/.config/hypr/monitors.lua
// (gauche = 1-10, centre = 11-20, droite = 21-30, autre écran = 31-40).
BarWidget {
  id: root
  moduleName: "kris.workspaces"

  readonly property var workspaceMonitors: ["DP-3", "DP-1", "DP-2"]

  // Écran de cette barre. Lu après installation du widget dans sa barre : un
  // binding direct gardait l'écran du premier rendu pour toutes les barres.
  property string screenName: ""
  function updateScreen() {
    var window = QsWindow.window
    var name = window && window.screen ? String(window.screen.name || "") : ""
    if (name !== screenName) screenName = name
  }

  readonly property int offset: {
    var slot = workspaceMonitors.indexOf(screenName)
    return (slot === -1 ? workspaceMonitors.length : slot) * 10
  }

  // État lu directement dans hyprctl (les objets Quickshell.Hyprland ne
  // suivaient pas de façon fiable le workspace actif par écran).
  property int activeWorkspaceId: -1
  property var windowCounts: ({})
  property bool refreshPending: false

  function refresh() {
    if (stateProc.running) {
      refreshPending = true
      return
    }
    refreshPending = false
    stateProc.running = true
  }

  Component.onCompleted: {
    Qt.callLater(updateScreen)
    refresh()
  }
  onParentChanged: Qt.callLater(updateScreen)
  onVisibleChanged: Qt.callLater(updateScreen)
  onScreenNameChanged: refresh()

  Connections {
    target: Hyprland
    function onRawEvent(event) {
      if (/workspace|window|monitor|focus|openlayer|closelayer/.test(event.name)) root.refresh()
    }
  }

  Process {
    id: stateProc
    command: ["sh", "-c", "printf '{\"monitors\":'; hyprctl -j monitors; printf ',\"workspaces\":'; hyprctl -j workspaces; printf '}'"]
    onRunningChanged: if (!running && root.refreshPending) root.refresh()
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        var state
        try {
          state = JSON.parse(text)
        } catch (e) {
          return
        }

        var active = -1
        for (var i = 0; i < state.monitors.length; i++) {
          if (state.monitors[i].name === root.screenName) active = state.monitors[i].activeWorkspace.id
        }

        var counts = {}
        for (var j = 0; j < state.workspaces.length; j++) {
          counts[state.workspaces[j].id] = state.workspaces[j].windows
        }

        root.activeWorkspaceId = active
        root.windowCounts = counts
      }
    }
  }

  // Numéros locaux (1-10) à afficher pour cet écran : 1 à 5, plus les
  // workspaces au-delà qui existent.
  function workspaceIds() {
    var ids = [1, 2, 3, 4, 5]

    for (var key in windowCounts) {
      var local = Number(key) - root.offset
      if (local > 0 && local <= 10 && ids.indexOf(local) === -1) ids.push(local)
    }

    ids.sort(function(left, right) { return left - right })
    return ids
  }

  function focusWorkspace(id) {
    if (!root.bar) return
    root.bar.run("hyprctl dispatch " + Util.shellQuote("hl.dsp.focus({ workspace = \"" + id + "\" })"))
  }

  readonly property real trailingGap: root.vertical ? 0 : Style.spaceReal(1.5)

  implicitWidth: grid.implicitWidth + trailingGap
  implicitHeight: grid.implicitHeight

  GridLayout {
    id: grid
    anchors.fill: parent
    anchors.rightMargin: root.trailingGap
    columns: root.vertical ? 1 : root.workspaceIds().length
    columnSpacing: root.vertical ? 0 : Style.space(1)
    rowSpacing: root.vertical ? Style.space(2) : 0

    Repeater {
      model: root.workspaceIds()

      WidgetButton {
        required property int modelData

        readonly property int globalId: root.offset + modelData
        readonly property bool occupied: (root.windowCounts[globalId] || 0) > 0
        // Workspace affiché sur CET écran (pas le workspace focus global).
        readonly property bool focused: root.activeWorkspaceId === globalId

        bar: root.bar
        text: focused ? "󱓻" : (modelData === 10 ? "0" : String(modelData))
        opacity: occupied || focused ? 1 : 0.5
        horizontalMargin: 6
        verticalPadding: 6
        fixedWidth: root.vertical ? root.barSize : Style.space(20)
        fixedHeight: root.barSize
        onPressed: function() { root.focusWorkspace(globalId) }
      }
    }
  }
}
