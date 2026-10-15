// Adapted from Island (https://github.com/Guilhermerisu/island), MIT License,
// Copyright (c) 2026 Guilherme Pimenta. See ../LICENSE-island.
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import Quickshell.Widgets

// Now-playing view, opened from the media pill's cover or wave: cover, title
// and artist, the live sound wave, a seekable progress bar with elapsed and
// remaining time, and transport controls. Keys: Space play/pause, ←/→ seek
// 10 s, Esc close.
Item {
  id: player
  required property var host
  property bool active: false
  signal closeRequested()

  readonly property var mpris: host.player
  readonly property bool playing: !!(mpris && mpris.isPlaying)
  readonly property real length: mpris && mpris.lengthSupported ? mpris.length : 0
  readonly property bool canSeek: !!(mpris && mpris.canSeek && length > 0)
  property real position: 0
  property bool dragging: false
  // Volume de l'application : le curseur remplace les commandes après un clic
  // sur le haut-parleur, et leur rend la place après 2,5 s sans interaction.
  property bool volumeOpen: false
  readonly property bool volumeAvailable: host.playerVolume >= 0
  onVolumeAvailableChanged: if (!volumeAvailable) volumeOpen = false
  Timer { id: volumeIdle; interval: 2500; onTriggered: player.volumeOpen = false }
  WheelHandler { onWheel: function(event) { player.host.wheelPlayerVolume(event.angleDelta.y) } }

  implicitHeight: layout.implicitHeight

  onActiveChanged: {
    if (!active) { volumeOpen = false; return }
    position = mpris ? mpris.position : 0
    Qt.callLater(function() { player.forceActiveFocus() })
  }
  // Mpris only reports position on seeks; re-read it every frame while open
  // and playing so the bar and times glide.
  FrameAnimation {
    running: player.active && player.playing && !player.dragging
    onTriggered: player.position = player.mpris ? player.mpris.position : 0
  }
  Connections {
    target: player.mpris
    function onPositionChanged() { if (!player.dragging) player.position = player.mpris.position }
  }

  function action(name) { host.mediaAction(name) }
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

  Keys.onPressed: function(event) {
    if (event.key === Qt.Key_Space) { action("playPause"); event.accepted = true }
    else if (event.key === Qt.Key_Left) { seekTo(position - 10); event.accepted = true }
    else if (event.key === Qt.Key_Right) { seekTo(position + 10); event.accepted = true }
    else if (event.key === Qt.Key_Escape) { player.closeRequested(); event.accepted = true }
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
        color: player.host.withAlpha(player.host.colorAccent, 0.3)
        Image {
          id: cover
          anchors.fill: parent
          source: player.host.mediaArt
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
          color: player.host.colorAccentText
          fontFamily: player.host.fontFamily
          pixelSize: 30
        }
      }
      ColumnLayout {
        Layout.fillWidth: true
        spacing: 2
        Text {
          Layout.fillWidth: true
          text: player.mpris ? String(player.mpris.trackTitle || "Unknown track") : "Nothing playing"
          textFormat: Text.PlainText
          elide: Text.ElideRight
          color: player.host.colorText
          font.family: player.host.uiFont
          font.pixelSize: 17
          font.weight: Font.DemiBold
        }
        Text {
          Layout.fillWidth: true
          visible: text !== ""
          text: player.mpris ? String(player.mpris.trackArtist || player.mpris.identity || "") : ""
          textFormat: Text.PlainText
          elide: Text.ElideRight
          color: player.host.withAlpha(player.host.colorText, 0.55)
          font.family: player.host.uiFont
          font.pixelSize: 15
        }
        // Detail line declared by a module (`player` in its island.json), small and discreet.
        Text {
          Layout.fillWidth: true
          visible: text !== ""
          text: player.host.playerDetail
          textFormat: Text.PlainText
          elide: Text.ElideRight
          color: player.host.withAlpha(player.host.colorText, 0.4)
          font.family: player.host.uiFont
          font.pixelSize: 11
          font.features: { "tnum": 1 }
        }
        // D'où vient le son : l'appli (icône et nom). Clic : sa fenêtre passe
        // au premier plan (celle de l'onglet qui joue pour un navigateur).
        // A player without a window, declared by a module (`player` in its island.json): the click
        // opens the launcher on the extension it names (which closes this view).
        Item {
          id: sourceChip
          readonly property var entry: player.mpris && player.mpris.desktopEntry
            ? DesktopEntries.heuristicLookup(String(player.mpris.desktopEntry)) : null
          readonly property string name: entry ? String(entry.name)
            : (player.mpris ? String(player.mpris.identity || "") : "")
          Layout.topMargin: 4
          visible: name !== ""
          implicitWidth: sourceRow.implicitWidth + 12
          implicitHeight: 22
          Rectangle {
            anchors.fill: parent
            radius: height / 2
            color: player.host.withAlpha(player.host.colorText, sourceMouse.containsMouse ? 0.14 : 0.07)
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
              color: player.host.withAlpha(player.host.colorText, 0.7)
              font.family: player.host.uiFont
              font.pixelSize: 12
            }
          }
          MouseArea {
            id: sourceMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: {
              if (player.host.playerSourceExt !== "") {
                player.host.openLauncherExtOn(player.host.focusedScreen, player.host.playerSourceExt)
                return
              }
              var m = player.mpris
              focusProc.command = [Quickshell.env("HOME") + "/.config/omarchy/plugins/poste.island/bin/poste-focus-player",
                String(m.desktopEntry || m.identity || ""), String(m.trackTitle || ""),
                String((m.metadata || {})["xesam:url"] || "")]
              focusProc.running = true
              player.closeRequested()
            }
          }
          // Pas de fenêtre (réduite dans la barre des tâches, par exemple un lecteur de bureau) :
          // on demande au lecteur de se réafficher, sinon on relance l'appli.
          Process {
            id: focusProc
            onExited: function(code) {
              if (code !== 3) return
              var m = player.mpris
              if (m && m.canRaise) m.raise()
              else if (sourceChip.entry) sourceChip.entry.execute()
            }
          }
        }
      }
      // Action button declared by a module (the heart, `player` in its island.json).
      Glyph {
        id: heart
        visible: player.host.playerHasAction
        Layout.alignment: Qt.AlignTop
        Layout.topMargin: 6
        glyph: player.host.playerActionGlyph
        color: player.host.playerActionOn ? player.host.colorAccent : player.host.withAlpha(player.host.colorText, 0.6)
        opacity: heartMouse.pressed ? 0.6 : 1
        fontFamily: player.host.fontFamily
        pixelSize: 22
        stroke: 1.75
        scale: heartMouse.pressed ? 0.9 : 1
        Behavior on scale { NumberAnimation { duration: 120 * player.host.motionScale; easing.type: Easing.OutCubic } }
        MouseArea {
          id: heartMouse
          anchors.fill: parent
          anchors.margins: -8
          cursorShape: Qt.PointingHandCursor
          onClicked: player.host.playerAction()
        }
      }
      SoundWave {
        Layout.alignment: Qt.AlignTop
        Layout.topMargin: 10
        color: player.host.mediaTint
        playing: player.active && player.playing
        barWidth: 3.5
        maxHeight: 24
      }
    }

    // ---------- Progress ----------

    RowLayout {
      Layout.fillWidth: true
      Layout.topMargin: 22
      spacing: 12
      visible: player.length > 0

      Text {
        text: player.clock(player.position)
        color: player.host.withAlpha(player.host.colorText, 0.55)
        font.family: player.host.uiFont
        font.pixelSize: 12
        font.weight: Font.Medium
        font.features: { "tnum": 1 }
      }
      Item {
        Layout.fillWidth: true
        Layout.preferredHeight: 18
        Rectangle {
          id: track
          anchors.verticalCenter: parent.verticalCenter
          width: parent.width
          height: 6
          radius: 3
          color: player.host.withAlpha(player.host.colorText, 0.16)
          Rectangle {
            height: parent.height
            radius: parent.radius
            width: player.length > 0 ? parent.width * Math.max(0, Math.min(1, player.position / player.length)) : 0
            color: player.host.colorAccent
          }
        }
        MouseArea {
          anchors.fill: parent
          enabled: player.canSeek
          cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
          function at(x) { return Math.max(0, Math.min(1, x / width)) * player.length }
          onPressed: function(e) { player.dragging = true; player.position = at(e.x) }
          onPositionChanged: function(e) { if (pressed) player.position = at(e.x) }
          onReleased: function(e) { player.dragging = false; player.seekTo(at(e.x)) }
        }
      }
      Text {
        text: "−" + player.clock(player.length - player.position)
        color: player.host.withAlpha(player.host.colorText, 0.55)
        font.family: player.host.uiFont
        font.pixelSize: 12
        font.weight: Font.Medium
        font.features: { "tnum": 1 }
      }
    }

    // ---------- Controls ----------

    Item {
      Layout.fillWidth: true
      Layout.topMargin: player.length > 0 ? 16 : 22
      Layout.preferredHeight: 48

      component Control: Glyph {
        id: control
        property string name: ""
        property bool available: true
        signal activated()
        color: player.host.colorText
        opacity: available ? (controlMouse.pressed ? 0.6 : 1) : 0.35
        fontFamily: player.host.fontFamily
        filled: true
        stroke: 1.5
        scale: controlMouse.pressed ? 0.9 : 1
        Behavior on scale { NumberAnimation { duration: 120 * player.host.motionScale; easing.type: Easing.OutCubic } }
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
        opacity: player.volumeOpen ? 0 : 1
        visible: opacity > 0
        Behavior on opacity { NumberAnimation { duration: 160 * player.host.motionScale } }
        Control {
          anchors.verticalCenter: parent.verticalCenter
          glyph: "󰑟"
          pixelSize: 34
          available: !!(player.mpris && player.mpris.canGoPrevious)
          onActivated: player.action("previous")
        }
        Control {
          anchors.verticalCenter: parent.verticalCenter
          glyph: player.playing ? "󰏤" : "󰐊"
          pixelSize: 42
          available: !!player.mpris
          onActivated: player.action("playPause")
        }
        Control {
          anchors.verticalCenter: parent.verticalCenter
          glyph: "󰈑"
          pixelSize: 34
          available: !!(player.mpris && player.mpris.canGoNext)
          onActivated: player.action("next")
        }
      }

      // Haut-parleur discret, à droite des commandes : clic pour le curseur.
      Glyph {
        id: speaker
        visible: player.volumeAvailable && !player.volumeOpen
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        glyph: player.host.playerMuted || player.host.playerVolume <= 0 ? "󰝟" : "󰕾"
        color: player.host.colorMuted
        fontFamily: player.host.fontFamily
        pixelSize: 16
        MouseArea {
          anchors.fill: parent
          anchors.margins: -10
          cursorShape: Qt.PointingHandCursor
          onClicked: { player.volumeOpen = true; volumeIdle.restart() }
        }
      }
      // Curseur compact, à la place des commandes.
      Item {
        anchors.fill: parent
        opacity: player.volumeOpen ? 1 : 0
        visible: opacity > 0
        Behavior on opacity { NumberAnimation { duration: 160 * player.host.motionScale } }
        // Le glyphe du curseur (clic) ramène les commandes.
        LevelSlider {
          anchors.verticalCenter: parent.verticalCenter
          width: parent.width
          host: player.host
          value: Math.max(0, player.host.playerVolume)
          muted: player.host.playerMuted || player.host.playerVolume <= 0
          onMoved: function(v) { player.host.setPlayerVolume(v); volumeIdle.restart() }
          onMuteClicked: player.volumeOpen = false
        }
      }
    }
  }
}
