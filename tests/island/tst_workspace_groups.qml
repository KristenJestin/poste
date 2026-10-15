import QtQuick
import QtTest
import "../../config/.config/omarchy/plugins/poste.island/components"
import "../../config/.config/omarchy/plugins/poste.island/components/WorkspaceGroups.js" as WorkspaceGroups

// Groups of the island's bar with a fake Hyprland: three screens, 1-5 on the
// left, 11-13 in the middle, 21-25 on the right.
TestCase {
  name: "WorkspaceGroups"
  when: windowShown

  // Fake host: what WorkspaceStrip reads from the island.
  QtObject {
    id: host
    property var activeWorkspaces: ({ "DP-1": 1, "DP-2": 11, "DP-3": 21 })
    property var windowCounts: ({})
    property var visitingWorkspaces: ({})
    property color colorText: "white"
    property color colorMuted: "gray"
    property color colorAccent: "orange"
    property color colorAccentText: "black"
    property string uiFont: "sans"
    function withAlpha(c, a) { return Qt.rgba(c.r, c.g, c.b, a) }
    function focusWorkspace(id) {}
  }

  WorkspaceStrip {
    id: left
    host: host
    screenName: "DP-1"
    offset: 0
  }

  function threeScreens() {
    host.activeWorkspaces = { "DP-1": 1, "DP-2": 11, "DP-3": 21 }
    host.windowCounts = { 1: 1, 2: 1, 11: 1, 12: 1, 13: 1, 25: 1 }
    host.visitingWorkspaces = {}
  }

  // The middle and the right gone: Hyprland puts 11, 12, 13 and 25 on the left.
  function leftOnly() {
    host.activeWorkspaces = { "DP-1": 1 }
    host.windowCounts = { 1: 1, 2: 1, 11: 1, 12: 1, 13: 1, 25: 1 }
    host.visitingWorkspaces = { "DP-1": [25, 11, 13, 12] }
  }

  // What the bar shows, '|' for a separator.
  function shown(cells) {
    return cells.map(function(id) { return id === 0 ? "|" : String(((id - 1) % 10) + 1) }).join(" ")
  }

  function test_origin_slot() {
    compare(WorkspaceGroups.originSlot(1), 0)
    compare(WorkspaceGroups.originSlot(10), 0)
    compare(WorkspaceGroups.originSlot(11), 1)
    compare(WorkspaceGroups.originSlot(25), 2)
  }

  function test_no_visitors() {
    compare(shown(WorkspaceGroups.cells({ 1: 1, 7: 2 }, 1, 0, undefined)), "1 2 3 4 5 7")
    compare(shown(WorkspaceGroups.cells({}, 13, 10, [])), "1 2 3 4 5")
  }

  function test_one_group_per_origin_screen() {
    compare(shown(WorkspaceGroups.cells({}, 1, 0, [25, 11, 13, 12])), "1 2 3 4 5 | 1 2 3 | 5")
    compare(WorkspaceGroups.cells({}, 1, 0, [25, 11, 13, 12]), [1, 2, 3, 4, 5, 0, 11, 12, 13, 0, 25])
  }

  function test_visitors_from_a_lower_screen() {
    // Middle screen left alone: those of the left and of the right arrive there.
    compare(shown(WorkspaceGroups.cells({}, 11, 10, [21, 2, 1])), "1 2 3 4 5 | 1 2 | 1")
  }

  function test_strip_follows_screens() {
    threeScreens()
    var before = left.cells
    compare(shown(before), "1 2 3 4 5")

    leftOnly()
    compare(shown(left.cells), "1 2 3 4 5 | 1 2 3 | 5")

    threeScreens()
    compare(left.cells, before)
  }

  function test_strip_draws_one_bar_per_group() {
    leftOnly()
    waitForRendering(left)
    var bars = 0
    var chips = 0
    for (var i = 0; i < left.children.length; i++) {
      var cell = left.children[i]
      if (cell.divider === undefined) continue
      if (cell.divider) bars++
      else chips++
    }
    compare(bars, 2)
    compare(chips, 9)
  }
}
