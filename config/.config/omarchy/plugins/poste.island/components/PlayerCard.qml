// Adapted from Island (https://github.com/Guilhermerisu/island), MIT License,
// Copyright (c) 2026 Guilherme Pimenta. See ../LICENSE-island.
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import Quickshell.Widgets

// One player of the player view: cover, title and artist, the live sound wave,
// a seekable progress bar with elapsed and remaining time, and transport controls.
Item {
  id: card
  required property var host
  required property var mpris
  property bool active: false
  signal closeRequested()

  readonly property bool playing: !!(mpris && mpris.isPlaying)
  readonly property real length: mpris && mpris.lengthSupported ? mpris.length : 0
  readonly property bool canSeek: !!(mpris && mpris.canSeek && length > 0)
  readonly property var extras: host.extrasOf(mpris)
  readonly property string art: host.artOf(mpris)
  readonly property real volume: host.volumeOf(mpris)
  readonly property bool muted: host.mutedOf(mpris)
  property real position: 0
  property bool dragging: false
  // Application volume: the slider replaces the controls after a click
  // on the speaker, and gives them their room back after 2.5 s without interaction.
  property bool volumeOpen: false
  readonly property bool volumeAvailable: volume >= 0
  onVolumeAvailableChanged: if (!volumeAvailable) volumeOpen = false
  Timer { id: volumeIdle; interval: 2500; onTriggered: card.volumeOpen = false }
  WheelHandler { onWheel: function(event) { card.host.wheelVolumeOf(card.mpris, event.angleDelta.y) } }

  implicitHeight: layout.implicitHeight

  onActiveChanged: {
    if (!active) { volumeOpen = false; return }
    position = mpris ? mpris.position : 0
  }
  onMprisChanged: position = mpris ? mpris.position : 0
  // Mpris only reports position on seeks; re-read it every frame while open
  // and playing so the bar and times glide.
  FrameAnimation {
    running: card.active && card.playing && !card.dragging
    onTriggered: card.position = card.mpris ? card.mpris.position : 0
  }
  Connections {
    target: card.mpris
    function onPositionChanged() { if (!card.dragging) card.position = card.mpris.position }
  }

  // The wave takes this cover's most vivid color.
  ColorQuantizer {
    id: coverColors
    // Local images only: ColorQuantizer cannot load HTTP(S) covers and then logs the
    // full URL, Subsonic authentication token included.
    source: card.mpris !== card.host.player && /^(file:|\/)/.test(card.art) ? card.art : ""
    depth: 2
    rescaleSize: 64
  }
  readonly property color tint: mpris === host.player ? host.mediaTint : host.tintOf(coverColors.colors)

  function action(name) { host.actionOf(mpris, name) }
  function seekTo(seconds) {
    if (!canSeek) return
    var target = Math.max(0, Math.min(length, seconds))
    mpris.position = target
    position = target
  }
  function clock(seconds) {
    var s = Math.max(0, Math.floor(seconds || 0))
    var m = Math.floor(s / 60), r = s % 60
    return m + ":" + (r < 10 ? "0" : "") + r
  }

  ColumnLayout {
    id: layout
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.top: parent.top
    spacing: 0

    // ---------- Cover, title, wave ----------

    RowLayout {
      Layout.fillWidth: true
      spacing: 16

      ClippingRectangle {
        Layout.preferredWidth: 72
        Layout.preferredHeight: 72
        radius: 16
        color: card.host.withAlpha(card.host.colorAccent, 0.3)
        Image {
          id: cover
          anchors.fill: parent
          source: card.art
          sourceSize.width: 144
          sourceSize.height: 144
          fillMode: Image.PreserveAspectCrop
          asynchronous: true
          cache: false
          visible: status === Image.Ready
        }
        Glyph {
          anchors.centerIn: parent
          visible: cover.status !== Image.Ready
          glyph: "󰝚"
          color: card.host.colorAccentText
          fontFamily: card.host.fontFamily
          pixelSize: 30
        }
      }
      ColumnLayout {
        Layout.fillWidth: true
        spacing: 2
        Text {
          Layout.fillWidth: true
          text: card.mpris ? String(card.mpris.trackTitle || "Unknown track") : "Nothing playing"
          textFormat: Text.PlainText
          elide: Text.ElideRight
          color: card.host.colorText
          font.family: card.host.uiFont
          font.pixelSize: 17
          font.weight: Font.DemiBold
        }
        Text {
          Layout.fillWidth: true
          visible: text !== ""
          text: card.mpris ? String(card.mpris.trackArtist || card.mpris.identity || "") : ""
          textFormat: Text.PlainText
          elide: Text.ElideRight
          color: card.host.withAlpha(card.host.colorText, 0.55)
          font.family: card.host.uiFont
          font.pixelSize: 15
        }
        // Detail line declared by a module (`player` in its island.json), small and discreet.
        Text {
          Layout.fillWidth: true
          visible: text !== ""
          text: card.extras.detail
          textFormat: Text.PlainText
          elide: Text.ElideRight
          color: card.host.withAlpha(card.host.colorText, 0.4)
          font.family: card.host.uiFont
          font.pixelSize: 11
          font.features: { "tnum": 1 }
        }
        // Where the sound comes from: the app (icon and name). Click: its window comes
        // to the front (that of the playing tab for a browser).
        // A module can name the player's app (`app` in the `player` block of its island.json) when
        // its MPRIS has no DesktopEntry, or a launcher extension (`launcher`) for a player without a
        // window: the click then opens the launcher there (which closes this view).
        Item {
          id: sourceChip
          readonly property string app: card.extras.app !== "" ? card.extras.app
            : (card.mpris ? String(card.mpris.desktopEntry || "") : "")
          readonly property var entry: app !== "" ? DesktopEntries.heuristicLookup(app) : null
          readonly property string name: entry ? String(entry.name)
            : (card.mpris ? String(card.mpris.identity || "") : "")
          Layout.topMargin: 4
          visible: name !== ""
          implicitWidth: sourceRow.implicitWidth + 12
          implicitHeight: 22
          Rectangle {
            anchors.fill: parent
            radius: height / 2
            color: card.host.withAlpha(card.host.colorText, sourceMouse.containsMouse ? 0.14 : 0.07)
          }
          Row {
            id: sourceRow
            anchors.centerIn: parent
            spacing: 5
            IconImage {
              anchors.verticalCenter: parent.verticalCenter
              implicitSize: 13
              source: sourceChip.entry && sourceChip.entry.icon ? Quickshell.iconPath(sourceChip.entry.icon, true) : ""
              visible: sourceChip.entry !== null
            }
            Text {
              anchors.verticalCenter: parent.verticalCenter
              text: sourceChip.name + "  󰁔"
              textFormat: Text.PlainText
              color: card.host.withAlpha(card.host.colorText, 0.7)
              font.family: card.host.uiFont
              font.pixelSize: 12
            }
          }
          MouseArea {
            id: sourceMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: {
              if (sourceChip.app === "" && card.extras.launcher !== "") {
                card.host.openLauncherExtOn(card.host.focusedScreen, card.extras.launcher)
                return
              }
              var m = card.mpris
              focusProc.command = [Quickshell.env("HOME") + "/.config/omarchy/plugins/poste.island/bin/poste-focus-player",
                String(sourceChip.app || m.identity || ""), String(m.trackTitle || ""),
                String((m.metadata || {})["xesam:url"] || "")]
              focusProc.running = true
              card.closeRequested()
            }
          }
          // No window (minimized to the task bar, for example a desktop player):
          // the player is asked to show itself again, otherwise the app is relaunched.
          Process {
            id: focusProc
            onExited: function(code) {
              if (code !== 3) return
              var m = card.mpris
              if (m && m.canRaise) m.raise()
              else if (sourceChip.entry) sourceChip.entry.execute()
            }
          }
        }
      }
      // Action button declared by a module (the heart, `player` in its island.json).
      Glyph {
        id: heart
        visible: card.extras.hasAction
        Layout.alignment: Qt.AlignTop
        Layout.topMargin: 6
        glyph: card.host.actionGlyph(card.extras)
        color: card.extras.actionOn ? card.host.colorAccent : card.host.withAlpha(card.host.colorText, 0.6)
        opacity: heartMouse.pressed ? 0.6 : 1
        fontFamily: card.host.fontFamily
        pixelSize: 22
        stroke: 1.75
        scale: heartMouse.pressed ? 0.9 : 1
        Behavior on scale { NumberAnimation { duration: 120 * card.host.motionScale; easing.type: Easing.OutCubic } }
        MouseArea {
          id: heartMouse
          anchors.fill: parent
          anchors.margins: -8
          cursorShape: Qt.PointingHandCursor
          onClicked: card.host.playerActionOf(card.mpris)
        }
      }
      SoundWave {
        Layout.alignment: Qt.AlignTop
        Layout.topMargin: 10
        color: card.tint
        playing: card.active && card.playing
        barWidth: 3.5
        maxHeight: 24
      }
    }

    // ---------- Progress ----------

    RowLayout {
      Layout.fillWidth: true
      Layout.topMargin: 22
      spacing: 12
      visible: card.length > 0

      Text {
        text: card.clock(card.position)
        color: card.host.withAlpha(card.host.colorText, 0.55)
        font.family: card.host.uiFont
        font.pixelSize: 12
        font.weight: Font.Medium
        font.features: { "tnum": 1 }
      }
      Item {
        Layout.fillWidth: true
        Layout.preferredHeight: 18
        Rectangle {
          anchors.verticalCenter: parent.verticalCenter
          width: parent.width
          height: 6
          radius: 3
          color: card.host.withAlpha(card.host.colorText, 0.16)
          Rectangle {
            height: parent.height
            radius: parent.radius
            width: card.length > 0 ? parent.width * Math.max(0, Math.min(1, card.position / card.length)) : 0
            color: card.host.colorAccent
          }
        }
        MouseArea {
          anchors.fill: parent
          enabled: card.canSeek
          cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
          function at(x) { return Math.max(0, Math.min(1, x / width)) * card.length }
          onPressed: function(e) { card.dragging = true; card.position = at(e.x) }
          onPositionChanged: function(e) { if (pressed) card.position = at(e.x) }
          onReleased: function(e) { card.dragging = false; card.seekTo(at(e.x)) }
        }
      }
      Text {
        text: "−" + card.clock(card.length - card.position)
        color: card.host.withAlpha(card.host.colorText, 0.55)
        font.family: card.host.uiFont
        font.pixelSize: 12
        font.weight: Font.Medium
        font.features: { "tnum": 1 }
      }
    }

    // ---------- Controls ----------

    Item {
      Layout.fillWidth: true
      Layout.topMargin: card.length > 0 ? 16 : 22
      Layout.preferredHeight: 48

      component Control: Glyph {
        id: control
        property string name: ""
        property bool available: true
        signal activated()
        color: card.host.colorText
        opacity: available ? (controlMouse.pressed ? 0.6 : 1) : 0.35
        fontFamily: card.host.fontFamily
        filled: true
        stroke: 1.5
        scale: controlMouse.pressed ? 0.9 : 1
        Behavior on scale { NumberAnimation { duration: 120 * card.host.motionScale; easing.type: Easing.OutCubic } }
        MouseArea {
          id: controlMouse
          anchors.fill: parent
          anchors.margins: -10
          enabled: control.available
          cursorShape: Qt.PointingHandCursor
          onClicked: control.activated()
        }
      }

      Row {
        anchors.centerIn: parent
        spacing: 46
        opacity: card.volumeOpen ? 0 : 1
        visible: opacity > 0
        Behavior on opacity { NumberAnimation { duration: 160 * card.host.motionScale } }
        Control {
          anchors.verticalCenter: parent.verticalCenter
          glyph: "󰑟"
          pixelSize: 34
          available: !!(card.mpris && card.mpris.canGoPrevious)
          onActivated: card.action("previous")
        }
        Control {
          anchors.verticalCenter: parent.verticalCenter
          glyph: card.playing ? "󰏤" : "󰐊"
          pixelSize: 42
          available: !!card.mpris
          onActivated: card.action("playPause")
        }
        Control {
          anchors.verticalCenter: parent.verticalCenter
          glyph: "󰈑"
          pixelSize: 34
          available: !!(card.mpris && card.mpris.canGoNext)
          onActivated: card.action("next")
        }
      }

      // Discreet speaker, to the right of the controls: click for the slider.
      Glyph {
        visible: card.volumeAvailable && !card.volumeOpen
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        glyph: card.muted || card.volume <= 0 ? "󰝟" : "󰕾"
        color: card.host.colorMuted
        fontFamily: card.host.fontFamily
        pixelSize: 16
        MouseArea {
          anchors.fill: parent
          anchors.margins: -10
          cursorShape: Qt.PointingHandCursor
          onClicked: { card.volumeOpen = true; volumeIdle.restart() }
        }
      }
      // Compact slider, in place of the controls.
      Item {
        anchors.fill: parent
        opacity: card.volumeOpen ? 1 : 0
        visible: opacity > 0
        Behavior on opacity { NumberAnimation { duration: 160 * card.host.motionScale } }
        // The slider's glyph (click) brings the controls back.
        LevelSlider {
          anchors.verticalCenter: parent.verticalCenter
          width: parent.width
          host: card.host
          value: Math.max(0, card.volume)
          muted: card.muted || card.volume <= 0
          onMoved: function(v) { card.host.setVolumeOf(card.mpris, v); volumeIdle.restart() }
          onMuteClicked: card.volumeOpen = false
        }
      }
    }
  }
}
