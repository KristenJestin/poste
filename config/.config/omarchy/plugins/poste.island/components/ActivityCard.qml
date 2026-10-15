import QtQuick
import Quickshell.Widgets

// An island activity unfolded, under its bubble (hover or click):
//   timer: end time, big time left, pause / cancel, +1 min, and
//     the progress that goes around the edge;
//   AI: the question, where it stands (or the start of the answer), Open and
//     stop / unpin;
//   battery: the gauge, the percentage and the estimated time;
//   music: cover, title, artist, progress, previous / play / next.
Rectangle {
  id: card
  required property var host
  required property var activity
  readonly property string kind: activity ? activity.kind : ""
  readonly property var timer: activity ? activity.timer || null : null
  readonly property bool isReminder: !!timer && timer.omarchy === true   // rappel d'Omarchy (Island.qml)
  readonly property var conv: activity ? activity.conv || null : null
  readonly property var battery: host.battery
  readonly property var device: activity ? activity.device || null : null
  readonly property var obs: host.obs
  // Source of the 'battery' block: the laptop's battery or a device.
  readonly property var batSrc: kind === "device" && device
    ? { percent: device.percent, charging: device.charging, color: device.color, glyph: device.glyph, label: device.name }
    : { percent: battery.percent, charging: battery.charging, color: battery.color, glyph: battery.glyph, label: battery.timeText() || (battery.charging ? "Charging" : "") }
  readonly property bool hovered: hover.hovered
  signal openConversation(var conv)
  signal openPlayer()
  signal done()

  width: kind === "ai" ? 360 : kind === "music" ? 360 : kind === "obs" ? 330 : 320
  // Music without a known duration (YouTube…): no progress bar, shorter card.
  height: kind === "obs" ? 132 : kind === "timer" ? 132 : kind === "battery" || kind === "device" ? 64 : kind === "music" ? (musicCard.length > 0 ? 132 : 100) : aiCol.implicitHeight + 28
  radius: kind === "battery" || kind === "device" ? height / 2 : 28
  color: host.colorIsland
  border.width: kind === "timer" ? 0 : 1
  border.color: host.withAlpha(host.colorText, 0.08)
  HoverHandler { id: hover }

  component RoundButton: Rectangle {
    id: rb
    property string glyph: ""
    property color ink: card.host.colorText
    property color fill: card.host.withAlpha(card.host.colorText, 0.12)
    signal activated()
    width: 46; height: 46; radius: 23
    color: rbMouse.containsMouse ? Qt.lighter(fill, 1.35) : fill
    Glyph { anchors.centerIn: parent; glyph: rb.glyph; color: rb.ink; fontFamily: card.host.fontFamily; pixelSize: 20 }
    MouseArea { id: rbMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: rb.activated() }
  }
  component Chip: Rectangle {
    id: chip
    property string label: ""
    property bool accent: false
    signal activated()
    height: 30
    width: chipText.implicitWidth + 26
    radius: 15
    color: accent ? (chipMouse.containsMouse ? Qt.lighter(card.host.colorAccent, 1.1) : card.host.colorAccent)
      : card.host.withAlpha(card.host.colorText, chipMouse.containsMouse ? 0.16 : 0.09)
    Text {
      id: chipText
      anchors.centerIn: parent
      text: chip.label
      color: chip.accent ? card.host.colorAccentText : card.host.colorText
      font.family: card.host.uiFont
      font.pixelSize: 12
      font.weight: Font.DemiBold
    }
    MouseArea { id: chipMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: chip.activated() }
  }

  // ================================================ minuteur
  Item {
    anchors.fill: parent
    visible: card.kind === "timer"
    readonly property real remaining: card.timer ? card.host.timerLeft(card.timer) : 0
    BorderProgress {
      anchors.fill: parent
      progress: card.timer ? (card.timer.done ? 1 : card.timer.duration > 0 ? parent.remaining / card.timer.duration : 0) : 0
      radius: card.radius
      color: card.timer && card.timer.paused ? card.host.colorMuted : card.host.colorAccent
      track: card.host.withAlpha(card.host.colorText, 0.14)
      lineWidth: 5
    }
    RoundButton {
      anchors.left: parent.left
      anchors.leftMargin: 22
      anchors.verticalCenter: parent.verticalCenter
      visible: !card.isReminder   // an Omarchy reminder cannot be paused
      glyph: card.timer && card.timer.done ? "󰑐" : card.timer && card.timer.paused ? "󰐊" : "󰏤"
      onActivated: {
        if (card.timer.done) card.host.addToTimer(card.timer.id, 60000)
        else card.host.togglePause(card.timer.id)
      }
    }
    Column {
      anchors.centerIn: parent
      spacing: 0
      Text {
        anchors.horizontalCenter: parent.horizontalCenter
        text: {
          if (!card.timer) return ""
          if (card.timer.done) return "󰂞  Time's up"
          if (card.timer.paused) return "󰏤  Paused"
          var d = new Date(card.timer.end)
          return "󰂞  " + String(d.getHours()).padStart(2, "0") + ":" + String(d.getMinutes()).padStart(2, "0")
        }
        color: card.host.colorMuted
        font.family: card.host.uiFont
        font.pixelSize: 12
        font.weight: Font.DemiBold
      }
      Text {
        anchors.horizontalCenter: parent.horizontalCenter
        text: card.timer ? (card.timer.done ? "0:00" : card.host.formatLeft(parent.parent.remaining)) : ""
        color: card.host.colorText
        font.family: card.host.uiFont
        font.pixelSize: 40
        font.weight: Font.Bold
        font.features: { "tnum": 1 }
      }
      Text {
        anchors.horizontalCenter: parent.horizontalCenter
        width: Math.min(implicitWidth, 150)
        elide: Text.ElideRight
        text: card.timer && card.timer.label ? card.timer.label : "Timer"
        color: card.host.colorMuted
        font.family: card.host.uiFont
        font.pixelSize: 13
      }
    }
    Column {
      anchors.right: parent.right
      anchors.rightMargin: 22
      anchors.verticalCenter: parent.verticalCenter
      spacing: 8
      RoundButton {
        glyph: "󰅖"
        ink: "#ff453a"
        fill: Qt.rgba(1, 0.27, 0.23, 0.22)
        onActivated: { card.host.cancelTimer(card.timer.id); card.done() }
      }
    }
    // +1 min, discreet under the left button
    Text {
      anchors.left: parent.left
      anchors.leftMargin: 26
      anchors.bottom: parent.bottom
      anchors.bottomMargin: 12
      visible: card.timer && !card.timer.done && !card.isReminder
      text: "+1 min"
      color: plusMouse.containsMouse ? card.host.colorText : card.host.colorMuted
      font.family: card.host.uiFont
      font.pixelSize: 11
      font.weight: Font.DemiBold
      MouseArea { id: plusMouse; anchors.fill: parent; anchors.margins: -6; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: card.host.addToTimer(card.timer.id, 60000) }
    }
  }

  // ================================================ IA
  Column {
    id: aiCol
    visible: card.kind === "ai"
    x: 16
    y: 14
    width: parent.width - 32
    spacing: 8
    Row {
      spacing: 8
      Glyph {
        anchors.verticalCenter: parent.verticalCenter
        glyph: "󰚩"
        color: card.conv && (card.conv.busy || card.conv.unseen) ? card.host.colorAccent : card.host.colorMuted
        fontFamily: card.host.fontFamily
        pixelSize: 15
      }
      Text {
        anchors.verticalCenter: parent.verticalCenter
        text: card.conv ? card.conv.providerName : ""
        color: card.host.colorText
        font.family: card.host.uiFont
        font.pixelSize: 13
        font.weight: Font.DemiBold
      }
      Text {
        anchors.verticalCenter: parent.verticalCenter
        text: !card.conv ? "" : card.conv.busy ? "" : card.conv.unseen ? (card.conv.lastError ? "· failed" : "· answer ready") : (card.conv.pinned ? "· pinned" : "")
        color: card.host.colorMuted
        font.family: card.host.uiFont
        font.pixelSize: 12
      }
    }
    Text {
      width: parent.width
      text: card.conv ? card.conv.question : ""
      wrapMode: Text.Wrap
      maximumLineCount: 2
      elide: Text.ElideRight
      color: card.host.colorText
      font.family: card.host.uiFont
      font.pixelSize: 13
    }
    Shimmer {
      width: parent.width
      visible: !!card.conv && card.conv.busy
      host: card.host
      label: card.conv ? card.host.ai.toolLabel(card.conv.currentTool) : ""
      lines: [0.9, 0.6]
      lineHeight: 7
      spacing: 7
    }
    Text {
      width: parent.width
      visible: !!card.conv && !card.conv.busy && text !== ""
      // Start of the answer, without the Markdown formatting.
      text: card.conv ? String(card.conv.answerText || "").replace(/[*_`#>\[\]]/g, "").replace(/\(https?:[^)]*\)/g, "").trim() : ""
      wrapMode: Text.Wrap
      maximumLineCount: 3
      elide: Text.ElideRight
      color: card.host.colorMuted
      font.family: card.host.uiFont
      font.pixelSize: 12
      lineHeight: 1.15
    }
    Row {
      spacing: 8
      Chip { label: "Open"; accent: true; onActivated: { card.openConversation(card.conv); card.done() } }
      Chip {
        visible: !!card.conv && (card.conv.busy || card.conv.pinned || card.conv.unseen)
        label: card.conv && card.conv.busy ? "Stop" : "Unpin"
        onActivated: {
          if (card.conv.busy) card.conv.stop()
          else { card.conv.pinned = false; card.conv.unseen = false; card.done() }
        }
      }
    }
  }

  // ================================================ OBS
  Item {
    anchors.fill: parent
    visible: card.kind === "obs"
    Column {
      anchors.centerIn: parent
      spacing: 2
      Row {
        anchors.horizontalCenter: parent.horizontalCenter
        spacing: 6
        Rectangle { anchors.verticalCenter: parent.verticalCenter; width: 8; height: 8; radius: 4; color: card.obs.paused ? "#ffd60a" : "#ff453a" }
        Text {
          text: card.obs.recording ? (card.obs.paused ? "Paused" : "Recording") + (card.obs.streaming ? " · Live" : "") : "Live"
          color: card.host.colorMuted
          font.family: card.host.uiFont
          font.pixelSize: 12
          font.weight: Font.DemiBold
        }
      }
      Text {
        anchors.horizontalCenter: parent.horizontalCenter
        text: card.host.formatLeft(card.obs.recording ? card.obs.recordElapsed : card.obs.streamElapsed)
        color: card.host.colorText
        font.family: card.host.uiFont
        font.pixelSize: 38
        font.weight: Font.Bold
        font.features: { "tnum": 1 }
      }
      Text {
        anchors.horizontalCenter: parent.horizontalCenter
        width: Math.min(implicitWidth, 160)
        elide: Text.ElideRight
        visible: text !== ""
        text: card.obs.scene
        color: card.host.colorMuted
        font.family: card.host.uiFont
        font.pixelSize: 12
      }
    }
    // Pause / resume (recording) on the left, stop on the right.
    RoundButton {
      anchors.left: parent.left
      anchors.leftMargin: 20
      anchors.verticalCenter: parent.verticalCenter
      visible: card.obs.recording
      glyph: card.obs.paused ? "󰐊" : "󰏤"
      onActivated: card.obs.request(card.obs.paused ? "ResumeRecord" : "PauseRecord")
    }
    RoundButton {
      anchors.right: parent.right
      anchors.rightMargin: 20
      anchors.verticalCenter: parent.verticalCenter
      glyph: "󰓛"
      ink: "#ff453a"
      fill: Qt.rgba(1, 0.27, 0.23, 0.22)
      onActivated: { card.obs.request(card.obs.recording ? "StopRecord" : "StopStream"); card.done() }
    }
    // Replay buffer active: save the last seconds.
    Text {
      anchors.horizontalCenter: parent.horizontalCenter
      anchors.bottom: parent.bottom
      anchors.bottomMargin: 10
      visible: card.obs.replay
      text: "Save replay"
      color: replayMouse.containsMouse ? card.host.colorText : card.host.colorMuted
      font.family: card.host.uiFont
      font.pixelSize: 11
      font.weight: Font.DemiBold
      MouseArea { id: replayMouse; anchors.fill: parent; anchors.margins: -6; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: card.obs.request("SaveReplayBuffer") }
    }
  }

  // ================================================ musique
  Item {
    id: musicCard
    anchors.fill: parent
    visible: card.kind === "music"
    readonly property var mpris: card.host.player
    // Wheel on the card: player volume, 5 % per notch.
    WheelHandler { onWheel: function(event) { card.host.wheelPlayerVolume(event.angleDelta.y) } }
    readonly property real length: mpris && mpris.lengthSupported ? mpris.length : 0
    property real position: 0
    // MPRIS only reports the position at jumps: it is read again as long as the card is there.
    Timer {
      interval: 500
      repeat: true
      triggeredOnStart: true
      running: musicCard.visible && card.visible
      onTriggered: musicCard.position = musicCard.mpris ? musicCard.mpris.position : 0
    }
    function clock(sec) { sec = Math.max(0, Math.floor(sec || 0)); return Math.floor(sec / 60) + ":" + String(sec % 60).padStart(2, "0") }
    ClippingRectangle {
      id: bigCover
      x: 18
      y: 18
      width: 64; height: 64; radius: 14
      color: card.host.withAlpha(card.host.colorAccent, 0.3)
      Image {
        id: bigArt
        anchors.fill: parent
        source: card.kind === "music" ? card.host.mediaArt : ""
        sourceSize.width: 128
        sourceSize.height: 128
        fillMode: Image.PreserveAspectCrop
        asynchronous: true
        cache: false
        visible: status === Image.Ready
      }
      // Level after a wheel notch, over the cover.
      Rectangle {
        anchors.fill: parent
        color: Qt.rgba(0, 0, 0, 0.55)
        opacity: card.host.playerVolumeHintShown ? 1 : 0
        Behavior on opacity { NumberAnimation { duration: 120 } }
        Text {
          anchors.centerIn: parent
          text: Math.round(Math.max(0, card.host.playerVolume) * 100) + " %"
          color: "#ffffff"
          font.family: card.host.uiFont
          font.pixelSize: 14
          font.weight: Font.DemiBold
          font.features: { "tnum": 1 }
        }
      }
      Glyph { anchors.centerIn: parent; visible: bigArt.status !== Image.Ready; glyph: "󰝚"; color: card.host.colorAccentText; fontFamily: card.host.fontFamily; pixelSize: 24 }
      MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: { card.openPlayer(); card.done() } }
    }
    Column {
      anchors.left: bigCover.right
      anchors.leftMargin: 14
      anchors.right: parent.right
      anchors.rightMargin: 18
      y: 22
      spacing: 2
      Text {
        width: parent.width - (card.host.playerHasAction ? 28 : 0)
        elide: Text.ElideRight
        text: musicCard.mpris ? String(musicCard.mpris.trackTitle || "Unknown track") : ""
        color: card.host.colorText
        font.family: card.host.uiFont
        font.pixelSize: 15
        font.weight: Font.DemiBold
      }
      Text {
        width: parent.width
        elide: Text.ElideRight
        text: musicCard.mpris ? String(musicCard.mpris.trackArtist || musicCard.mpris.identity || "") : ""
        color: card.host.colorMuted
        font.family: card.host.uiFont
        font.pixelSize: 13
      }
      // Detail line declared by a module (`player` in its island.json), in the width the controls leave.
      Text {
        width: parent.width - 104
        visible: text !== ""
        elide: Text.ElideRight
        text: card.host.playerDetail
        textFormat: Text.PlainText
        color: card.host.withAlpha(card.host.colorText, 0.4)
        font.family: card.host.uiFont
        font.pixelSize: 11
      }
    }
    // Action button of a module (favorites heart), at the top right: the title leaves it room.
    Glyph {
      id: cardHeart
      visible: card.host.playerHasAction
      x: parent.width - 18 - width
      y: 20
      glyph: card.host.playerActionGlyph
      color: card.host.playerActionOn ? card.host.colorAccent : card.host.withAlpha(card.host.colorText, 0.6)
      fontFamily: card.host.fontFamily
      pixelSize: 18
      stroke: 1.75
      MouseArea { anchors.fill: parent; anchors.margins: -8; cursorShape: Qt.PointingHandCursor; onClicked: card.host.playerAction() }
    }
    // Controls, on the right under the title.
    Row {
      anchors.right: parent.right
      anchors.rightMargin: 14
      y: 58
      spacing: 4
      Repeater {
        model: [{ g: "󰑟", a: "previous" }, { g: card.host.mediaPlaying ? "󰏤" : "󰐊", a: "playPause" }, { g: "󰈑", a: "next" }]
        delegate: Rectangle {
          required property var modelData
          width: 32; height: 32; radius: 16
          color: ctlMouse.containsMouse ? card.host.withAlpha(card.host.colorText, 0.12) : "transparent"
          Glyph { anchors.centerIn: parent; glyph: modelData.g; color: card.host.colorText; fontFamily: card.host.fontFamily; pixelSize: modelData.a === "playPause" ? 20 : 16; stroke: 1.5 }
          MouseArea { id: ctlMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: card.host.mediaAction(modelData.a) }
        }
      }
    }
    // Progression
    Item {
      x: 18
      y: 98
      width: parent.width - 36
      height: 18
      visible: musicCard.length > 0
      Text { id: elapsed; anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter; text: musicCard.clock(musicCard.position); color: card.host.colorMuted; font.family: card.host.uiFont; font.pixelSize: 11; font.features: { "tnum": 1 } }
      Text { id: remaining; anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter; text: "−" + musicCard.clock(musicCard.length - musicCard.position); color: card.host.colorMuted; font.family: card.host.uiFont; font.pixelSize: 11; font.features: { "tnum": 1 } }
      Rectangle {
        anchors.left: elapsed.right
        anchors.leftMargin: 10
        anchors.right: remaining.left
        anchors.rightMargin: 10
        anchors.verticalCenter: parent.verticalCenter
        height: 4
        radius: 2
        color: card.host.withAlpha(card.host.colorText, 0.14)
        Rectangle {
          width: parent.width * Math.max(0, Math.min(1, musicCard.length > 0 ? musicCard.position / musicCard.length : 0))
          height: parent.height
          radius: 2
          color: card.host.mediaTint
          Behavior on width { NumberAnimation { duration: 450 } }
        }
      }
    }
  }

  // ================================================ batterie
  Item {
    anchors.fill: parent
    visible: card.kind === "battery" || card.kind === "device"
    readonly property real p: Math.max(0, Math.min(1, card.batSrc.percent))
    Rectangle {
      id: track
      anchors.left: parent.left
      anchors.leftMargin: 26
      anchors.right: timeLabel.left
      anchors.rightMargin: 16
      anchors.verticalCenter: parent.verticalCenter
      height: 10
      radius: 5
      color: card.host.withAlpha(card.host.colorText, 0.12)
      Rectangle {
        width: Math.max(knob.width / 2, parent.width * parent.parent.p)
        height: parent.height
        radius: 5
        color: card.batSrc.color
      }
      // Level pill, with a halo.
      Rectangle {
        id: knob
        x: Math.max(0, Math.min(parent.width - width, parent.width * parent.parent.p - width / 2))
        anchors.verticalCenter: parent.verticalCenter
        width: knobText.implicitWidth + 18
        height: 26
        radius: 13
        color: card.batSrc.color
        Rectangle {
          anchors.centerIn: parent
          width: parent.width + 14; height: parent.height + 14; radius: height / 2
          color: card.batSrc.color
          opacity: 0.25
          z: -1
        }
        Text {
          id: knobText
          anchors.centerIn: parent
          text: card.batSrc.glyph + " " + Math.round(card.batSrc.percent * 100)
          color: "#0b0b0c"
          font.family: card.host.fontFamily
          font.pixelSize: 13
          font.weight: Font.Bold
        }
      }
    }
    Text {
      id: timeLabel
      anchors.right: parent.right
      anchors.rightMargin: 24
      anchors.verticalCenter: parent.verticalCenter
      text: card.batSrc.label
      color: card.batSrc.color
      font.family: card.host.uiFont
      font.pixelSize: 14
      font.weight: Font.DemiBold
    }
  }
}
