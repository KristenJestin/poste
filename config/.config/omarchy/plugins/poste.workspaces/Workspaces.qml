import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import qs.Commons
import qs.Ui

// Clone of omarchy.workspaces, 'workspaces per screen' version: each screen
// has its own 1 to 10, and each bar shows / targets those of its screen.
// Same correspondence as workspace_monitors in ~/.config/hypr/monitors.lua
// (left = 1-10, center = 11-20, right = 21-30, other screen = 31-40).
BarWidget {
  id: root
  moduleName: "poste.workspaces"

  // Read from workspace_monitors of monitors.lua: each machine keeps its list.
  property var workspaceMonitors: []
  function parseWorkspaceMonitors(text) {
    var m = /^\s*workspace_monitors\s*=\s*\{([^}]*)\}/m.exec(text || "")
    if (!m) return []
    var out = []
    var re = /["']([^"']+)["']/g
    var s
    while ((s = re.exec(m[1])) !== null) out.push(s[1])
    return out
  }
  FileView {
    id: monitorsFile
    path: Quickshell.env("HOME") + "/.config/hypr/monitors.lua"
    watchChanges: true
    printErrors: false
    onFileChanged: reload()
    onLoaded: root.workspaceMonitors = root.parseWorkspaceMonitors(text())
    onLoadFailed: root.workspaceMonitors = []
  }

  // Screen of this bar. Read after the widget is installed in its bar: a
  // direct binding kept the screen of the first render for all the bars.
  property string screenName: ""
  function updateScreen() {
    var window = QsWindow.window
    var name = window && window.screen ? String(window.screen.name || "") : ""
    if (name !== screenName) screenName = name
  }

  // An entry of workspace_monitors: port name (DP-1) or 'desc:' followed by the
  // start of the screen's EDID description, compared as Hyprland does.
  property string screenDescription: ""
  readonly property int offset: {
    for (var i = 0; i < workspaceMonitors.length; i++) {
      var entry = workspaceMonitors[i]
      var hit = entry.indexOf("desc:") === 0
        ? screenDescription.indexOf(entry.slice(5).trim()) === 0
        : entry === screenName
      if (hit) return i * 10
    }
    return workspaceMonitors.length * 10
  }

  // State read directly from hyprctl (the Quickshell.Hyprland objects did not
  // reliably follow the active workspace per screen).
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
      // A re-wired monitors.lua (link to another file) does not trigger
      // watchChanges: it is read again when Hyprland reloads its config.
      if (event.name === "configreloaded") monitorsFile.reload()
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
          if (state.monitors[i].name === root.screenName) {
            active = state.monitors[i].activeWorkspace.id
            if (root.screenDescription !== (state.monitors[i].description || ""))
              root.screenDescription = state.monitors[i].description || ""
          }
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

  // Local numbers (1-10) to show for this screen: 1 to 5, plus the
  // workspaces beyond that exist.
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
        // Workspace shown on THIS screen (not the global focus workspace).
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
