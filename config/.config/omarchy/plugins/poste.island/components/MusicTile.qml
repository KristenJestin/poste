import QtQuick
import Quickshell
import Quickshell.Widgets

// One player in the music card unfolded under the island (ActivityCard): cover,
// title, artist, previous / play / next, progress. The card stacks one per player.
Item {
  id: tile
  required property var host
  required property var mpris
  property bool shown: false
  signal openPlayer()

  readonly property real length: mpris && mpris.lengthSupported ? mpris.length : 0
  readonly property var extras: host.extrasOf(mpris)
  readonly property string art: host.artOf(mpris)
  property real position: 0
  // Music without a known duration (YouTube…): no progress bar, shorter tile.
  implicitHeight: length > 0 ? 132 : 100

  // Wheel on the tile: its player's volume, 5 % per notch, the level shown over the cover.
  WheelHandler { onWheel: function(event) { if (tile.host.wheelVolumeOf(tile.mpris, event.angleDelta.y)) volumeHint.restart() } }
  Timer { id: volumeHint; interval: 1200 }

  // MPRIS only reports the position at jumps: it is read again as long as the tile is there.
  Timer {
    interval: 500
    repeat: true
    triggeredOnStart: true
    running: tile.shown
    onTriggered: tile.position = tile.mpris ? tile.mpris.position : 0
  }
  function clock(sec) { sec = Math.max(0, Math.floor(sec || 0)); return Math.floor(sec / 60) + ":" + String(sec % 60).padStart(2, "0") }

  ColorQuantizer {
    id: coverColors
    // Local images only (see PlayerCard.qml).
    source: tile.mpris !== tile.host.player && /^(file:|\/)/.test(tile.art) ? tile.art : ""
    depth: 2
    rescaleSize: 64
  }

  ClippingRectangle {
    id: bigCover
    x: 18
    y: 18
    width: 64; height: 64; radius: 14
    color: tile.host.withAlpha(tile.host.colorAccent, 0.3)
    Image {
      id: bigArt
      anchors.fill: parent
      source: tile.art
      sourceSize.width: 128
      sourceSize.height: 128
      fillMode: Image.PreserveAspectCrop
      asynchronous: true
      cache: false
      visible: status === Image.Ready
    }
    Rectangle {
      anchors.fill: parent
      color: Qt.rgba(0, 0, 0, 0.55)
      opacity: volumeHint.running ? 1 : 0
      Behavior on opacity { NumberAnimation { duration: 120 } }
      Text {
        anchors.centerIn: parent
        text: Math.round(Math.max(0, tile.host.volumeOf(tile.mpris)) * 100) + " %"
        color: "#ffffff"
        font.family: tile.host.uiFont
        font.pixelSize: 14
        font.weight: Font.DemiBold
        font.features: { "tnum": 1 }
      }
    }
    Glyph { anchors.centerIn: parent; visible: bigArt.status !== Image.Ready; glyph: "󰝚"; color: tile.host.colorAccentText; fontFamily: tile.host.fontFamily; pixelSize: 24 }
    MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: tile.openPlayer() }
  }
  Column {
    anchors.left: bigCover.right
    anchors.leftMargin: 14
    anchors.right: parent.right
    anchors.rightMargin: 18
    y: 22
    spacing: 2
    Text {
      width: parent.width - (tile.extras.hasAction ? 28 : 0)
      elide: Text.ElideRight
      text: tile.mpris ? String(tile.mpris.trackTitle || "Unknown track") : ""
      color: tile.host.colorText
      font.family: tile.host.uiFont
      font.pixelSize: 15
      font.weight: Font.DemiBold
    }
    Text {
      width: parent.width
      elide: Text.ElideRight
      text: tile.mpris ? String(tile.mpris.trackArtist || tile.mpris.identity || "") : ""
      color: tile.host.colorMuted
      font.family: tile.host.uiFont
      font.pixelSize: 13
    }
    // Detail line declared by a module (`player` in its island.json), in the width the controls leave.
    Text {
      width: parent.width - 104
      visible: text !== ""
      elide: Text.ElideRight
      text: tile.extras.detail
      textFormat: Text.PlainText
      color: tile.host.withAlpha(tile.host.colorText, 0.4)
      font.family: tile.host.uiFont
      font.pixelSize: 11
    }
  }
  // Action button of a module (favorites heart), at the top right: the title leaves it room.
  Glyph {
    visible: tile.extras.hasAction
    x: parent.width - 18 - width
    y: 20
    glyph: tile.host.actionGlyph(tile.extras)
    color: tile.extras.actionOn ? tile.host.colorAccent : tile.host.withAlpha(tile.host.colorText, 0.6)
    fontFamily: tile.host.fontFamily
    pixelSize: 18
    stroke: 1.75
    MouseArea { anchors.fill: parent; anchors.margins: -8; cursorShape: Qt.PointingHandCursor; onClicked: tile.host.playerActionOf(tile.mpris) }
  }
  // Controls, on the right under the title.
  Row {
    anchors.right: parent.right
    anchors.rightMargin: 14
    y: 58
    spacing: 4
    Repeater {
      model: [{ g: "󰑟", a: "previous" }, { g: tile.mpris && tile.mpris.isPlaying ? "󰏤" : "󰐊", a: "playPause" }, { g: "󰈑", a: "next" }]
      delegate: Rectangle {
        required property var modelData
        width: 32; height: 32; radius: 16
        color: ctlMouse.containsMouse ? tile.host.withAlpha(tile.host.colorText, 0.12) : "transparent"
        Glyph { anchors.centerIn: parent; glyph: modelData.g; color: tile.host.colorText; fontFamily: tile.host.fontFamily; pixelSize: modelData.a === "playPause" ? 20 : 16; stroke: 1.5 }
        MouseArea { id: ctlMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: tile.host.actionOf(tile.mpris, modelData.a) }
      }
    }
  }
  // Progression
  Item {
    x: 18
    y: 98
    width: parent.width - 36
    height: 18
    visible: tile.length > 0
    Text { id: elapsed; anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter; text: tile.clock(tile.position); color: tile.host.colorMuted; font.family: tile.host.uiFont; font.pixelSize: 11; font.features: { "tnum": 1 } }
    Text { id: remaining; anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter; text: "−" + tile.clock(tile.length - tile.position); color: tile.host.colorMuted; font.family: tile.host.uiFont; font.pixelSize: 11; font.features: { "tnum": 1 } }
    Rectangle {
      anchors.left: elapsed.right
      anchors.leftMargin: 10
      anchors.right: remaining.left
      anchors.rightMargin: 10
      anchors.verticalCenter: parent.verticalCenter
      height: 4
      radius: 2
      color: tile.host.withAlpha(tile.host.colorText, 0.14)
      Rectangle {
        width: parent.width * Math.max(0, Math.min(1, tile.length > 0 ? tile.position / tile.length : 0))
        height: parent.height
        radius: 2
        color: tile.mpris === tile.host.player ? tile.host.mediaTint : tile.host.tintOf(coverColors.colors)
        Behavior on width { NumberAnimation { duration: 450 } }
      }
    }
  }
}
