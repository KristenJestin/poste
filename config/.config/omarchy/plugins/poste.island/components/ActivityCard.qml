import QtQuick
import Quickshell
import Quickshell.Widgets

// An island activity unfolded, under its bubble (hover or click):
//   timer: end time, big time left, pause / cancel, +1 min, and
//     the progress that goes around the edge;
//   AI: the question, where it stands (or the start of the answer), Open and
//     stop / unpin;
//   battery: the gauge, the percentage and the estimated time;
//   music: cover, title, artist, progress, previous / play / next;
//   remote: the other machine, what it is doing, the modes to start, or the mic and stop.
Rectangle {
  id: card
  required property var host
  required property var activity
  readonly property string kind: activity ? activity.kind : ""
  readonly property var timer: activity ? activity.timer || null : null
  readonly property bool isReminder: !!timer && timer.omarchy === true   // Omarchy reminder (Island.qml)
  readonly property var conv: activity ? activity.conv || null : null
  readonly property var battery: host.battery
  readonly property var device: activity ? activity.device || null : null
  readonly property var obs: host.obs
  readonly property var remote: kind === "remote" ? host.remote : null
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
  height: kind === "remote" ? remoteCol.implicitHeight + 36 : kind === "obs" ? 132 : kind === "timer" ? 132 : kind === "battery" || kind === "device" ? 64 : kind === "music" ? Math.max(100, musicCard.implicitHeight) : aiCol.implicitHeight + 28
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

  // ================================================ timer
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

  // ================================================ remote
  // The other machine of a work-from-home mode (slot 'remote'): on the desk, its modes to start,
  // then, while one runs, where the mic goes and stop; on the machine driven, what drives it.
  Column {
    id: remoteCol
    visible: card.kind === "remote"
    anchors.centerIn: parent
    width: parent.width - 40
    spacing: 4
    readonly property var rm: card.remote
    readonly property bool desk: !!rm && rm.role === "desk"
    Text {
      anchors.horizontalCenter: parent.horizontalCenter
      width: Math.min(implicitWidth, parent.width)
      elide: Text.ElideRight
      text: {
        var rm = remoteCol.rm
        if (!rm) return ""
        if (rm.phase === "starting") return "Starting…"
        if (rm.phase === "stopping") return "Stopping…"
        if (rm.phase === "incompatible") {
          var late = rm.behind()
          return "Other version" + (rm.peerVersion ? " (" + rm.peerVersion + ")" : "") + ": update "
            + (late === "here" ? "this machine" : late === "peer" ? rm.peer : "one of them")
        }
        if (rm.phase === "mode") return remoteCol.desk
          ? (rm.mode ? rm.mode + "  ·  " : "") + rm.monitors.join(", ")
          : "Driven from"
        return remoteCol.desk && !rm.modes.length ? "No mode: write one in Navette's mode.conf" : "Here"
      }
      color: remoteCol.rm && remoteCol.rm.phase === "incompatible" ? "#ff9f0a" : card.host.colorMuted
      font.family: card.host.uiFont
      font.pixelSize: 12
      font.weight: Font.DemiBold
    }
    Text {
      anchors.horizontalCenter: parent.horizontalCenter
      width: Math.min(implicitWidth, parent.width)
      elide: Text.ElideRight
      text: remoteCol.rm ? remoteCol.rm.peer : ""
      color: card.host.colorText
      font.family: card.host.uiFont
      font.pixelSize: 30
      font.weight: Font.Bold
    }
    Item { width: 1; height: 6; visible: remoteActions.visible }
    Flow {
      id: remoteActions
      anchors.horizontalCenter: parent.horizontalCenter
      // As wide as its chips, up to the card: centered under the text, wrapped when too many.
      width: Math.min(parent.width, natural)
      readonly property real natural: {
        var w = 0, n = 0
        for (var i = 0; i < children.length; i++) if (children[i].visible && children[i].width > 0) { w += children[i].width; n++ }
        return w + Math.max(0, n - 1) * spacing
      }
      spacing: 8
      visible: remoteCol.desk && ((remoteCol.rm.phase === "linked" && remoteCol.rm.modes.length > 0) || remoteCol.rm.phase === "mode")
      // There: one chip per mode the provider lists (none listed: nothing to start).
      Repeater {
        model: remoteCol.desk && remoteCol.rm.phase === "linked" ? remoteCol.rm.modes : []
        delegate: Chip {
          required property var modelData
          required property int index
          label: modelData.name ? "Start " + modelData.name : "Start"
          accent: index === 0
          onActivated: { if (card.remote) card.remote.start(modelData.name); card.done() }
        }
      }
      // Running: the mic, then stop.
      Chip {
        visible: remoteCol.desk && remoteCol.rm.phase === "mode" && remoteCol.rm.mic !== "off"
        label: !remoteCol.rm ? "" : remoteCol.rm.mic === "remote" ? "Mic to " + remoteCol.rm.peer
          : remoteCol.rm.mic === "failed" ? "Mic failed" : "Mic here"
        accent: !!remoteCol.rm && remoteCol.rm.mic === "remote"
        onActivated: if (card.remote) card.remote.toggleMic()
      }
      Chip {
        visible: remoteCol.desk && remoteCol.rm.phase === "mode"
        label: "Stop"
        onActivated: { if (card.remote) card.remote.stop(); card.done() }
      }
    }
  }

  // ================================================ music
  // One tile per player with a track (MusicTile), the island's first, so each can be paused.
  Column {
    id: musicCard
    anchors.left: parent.left
    anchors.right: parent.right
    visible: card.kind === "music"
    Repeater {
      model: ScriptModel { values: card.kind === "music" ? card.host.shownPlayers : [] }
      delegate: Item {
        required property var modelData
        required property int index
        width: musicCard.width
        height: tile.implicitHeight
        Rectangle {
          visible: index > 0
          x: 18
          width: parent.width - 36
          height: 1
          color: card.host.withAlpha(card.host.colorText, 0.08)
        }
        MusicTile {
          id: tile
          anchors.fill: parent
          host: card.host
          mpris: modelData
          shown: musicCard.visible && card.visible
          onOpenPlayer: { card.openPlayer(); card.done() }
        }
      }
    }
  }

  // ================================================ battery
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
