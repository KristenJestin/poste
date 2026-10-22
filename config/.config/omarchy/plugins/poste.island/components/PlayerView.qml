import QtQuick
import QtQuick.Layouts
import Quickshell

// Now-playing view, opened from the media pill's cover or wave: one full card
// per player with a track (PlayerCard), in the order they appeared, so that
// each can be paused on its own. Keys, on the island's player: Space play/pause,
// ←/→ seek 10 s; Esc closes.
Item {
  id: view
  required property var host
  property bool active: false
  signal closeRequested()

  implicitHeight: column.implicitHeight

  onActiveChanged: if (active) Qt.callLater(function() { view.forceActiveFocus() })

  Keys.onPressed: function(event) {
    var target = null
    for (var i = 0; i < cards.count; i++) {
      var it = cards.itemAt(i)
      if (it && (!target || it.modelData === view.host.player)) target = it
    }
    if (event.key === Qt.Key_Escape) { view.closeRequested(); event.accepted = true }
    else if (!target) return
    else if (event.key === Qt.Key_Space) { target.action("playPause"); event.accepted = true }
    else if (event.key === Qt.Key_Left) { target.seekTo(target.position - 10); event.accepted = true }
    else if (event.key === Qt.Key_Right) { target.seekTo(target.position + 10); event.accepted = true }
  }

  ColumnLayout {
    id: column
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.top: parent.top
    spacing: 0

    Repeater {
      id: cards
      // ScriptModel keeps the cards of the players still there (no rebuild on each change).
      model: ScriptModel { values: view.host.shownPlayers }
      delegate: ColumnLayout {
        id: entry
        required property var modelData
        required property int index
        Layout.fillWidth: true
        spacing: 0
        readonly property alias position: playerCard.position
        function action(name) { playerCard.action(name) }
        function seekTo(seconds) { playerCard.seekTo(seconds) }

        Rectangle {
          Layout.fillWidth: true
          Layout.topMargin: 18
          Layout.bottomMargin: 18
          visible: entry.index > 0
          implicitHeight: 1
          color: view.host.withAlpha(view.host.colorText, 0.08)
        }
        PlayerCard {
          id: playerCard
          Layout.fillWidth: true
          host: view.host
          mpris: entry.modelData
          active: view.active
          onCloseRequested: view.closeRequested()
        }
      }
    }
  }
}
