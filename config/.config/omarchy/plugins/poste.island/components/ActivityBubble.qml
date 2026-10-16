import QtQuick
import Quickshell.Widgets

// An island activity (timer, AI conversation, battery, music, the other machine,
// dictation) as a bubble, next
// to the island: 'compact' = capsule, one piece of info on the left and one on the right
// (ring and time left); 'minimal' = a circle (when several
// activities share the room). The unfolded version is ActivityCard.qml.
Rectangle {
  id: bubble
  required property var host
  required property var activity       // { kind, key, timer | conv }
  property bool compact: false
  readonly property bool hovered: hover.hovered
  signal clicked()

  readonly property string kind: activity.kind
  readonly property var timer: activity.timer || null
  readonly property var conv: activity.conv || null
  readonly property var battery: host.battery
  readonly property var device: activity.device || null
  readonly property var obs: host.obs
  readonly property var remote: kind === "remote" ? host.remote : null
  readonly property bool remoteBusy: !!remote && (remote.phase === "starting" || remote.phase === "stopping")

  // ------------------------------------------------ state of the activity
  readonly property real timerLeft: timer ? host.timerLeft(timer) : 0
  readonly property real progress: kind === "timer" ? (timer.done ? 1 : timer.duration > 0 ? timerLeft / timer.duration : 0)
    : kind === "battery" ? battery.percent : kind === "device" ? device.percent : 0
  readonly property bool aiReady: !!conv && conv.unseen && !conv.busy
  readonly property color tint: kind === "battery" ? battery.color
    : kind === "device" ? device.color
    : kind === "obs" ? (obs.paused ? "#ffd60a" : "#ff453a")
    : kind === "remote" ? (!remote ? host.colorText : remote.phase === "incompatible" ? "#ff9f0a" : remote.phase === "mode" ? host.colorAccent : host.colorText)
    : kind === "music" ? host.mediaTint
    : kind === "timer" ? (timer.paused ? host.colorMuted : host.colorAccent)
    : (aiReady || (conv && conv.busy) ? host.colorAccent : host.colorText)
  readonly property string trailing: {
    if (kind === "music") return ""
    if (kind === "remote") return !remote ? "" : remote.phase === "starting" ? "Starting…" : remote.phase === "stopping" ? "Stopping…" : remote.phase === "incompatible" ? "Update" : remote.peer
    if (kind === "timer") return timer.done ? "Done" : host.formatLeft(timerLeft)
    if (kind === "battery") return Math.round(battery.percent * 100) + " %"
    if (kind === "device") return Math.round(device.percent * 100) + " %"
    if (kind === "obs") return obs.recording ? (obs.paused ? "Paused" : host.formatLeft(obs.recordElapsed)) : "LIVE " + host.formatLeft(obs.streamElapsed)
    if (!conv) return ""
    if (conv.busy) return host.ai.toolLabel(conv.currentTool)
    if (aiReady) return conv.lastError ? "Failed" : "Ready"
    return conv.question
  }

  height: host.restHeight
  width: !compact ? height
    : kind === "music" ? 22 + 20 + 10 + musicWave.width + 4
    : Math.max(height, content.implicitWidth + 26)
  radius: height / 2
  color: host.colorIsland
  // Same outline as the island: without it, the bubble looks bigger than it is.
  border.width: 1
  border.color: host.withAlpha(host.colorText, 0.07)
  Behavior on width { NumberAnimation { duration: 280 * bubble.host.motionScale; easing.type: Easing.OutQuint } }

  HoverHandler { id: hover }
  // Wheel on the media pill: player volume, 5 % per notch. A
  // handler blocks neither the click nor the hover.
  WheelHandler {
    enabled: bubble.kind === "music"
    onWheel: function(event) { bubble.host.wheelPlayerVolume(event.angleDelta.y) }
  }
  MouseArea {
    anchors.fill: parent
    cursorShape: Qt.PointingHandCursor
    acceptedButtons: Qt.LeftButton | Qt.RightButton
    onClicked: function(mouse) {
      if (mouse.button === Qt.RightButton && bubble.kind === "timer") bubble.host.togglePause(bubble.timer.id)
      else bubble.clicked()
    }
  }

  // Finished timer: the bubble breathes until the click.
  Rectangle {
    anchors.fill: parent
    radius: parent.radius
    color: bubble.host.colorAccent
    opacity: 0
    SequentialAnimation on opacity {
      running: bubble.kind === "timer" && bubble.timer.done
      loops: Animation.Infinite
      NumberAnimation { to: 0.32; duration: 520; easing.type: Easing.InOutSine }
      NumberAnimation { to: 0; duration: 520; easing.type: Easing.InOutSine }
    }
  }

  // ------------------------------------------------ on the left (or alone, as a circle)
  Item {
    id: leading
    visible: bubble.kind !== "music"
    width: 18
    height: 18
    x: bubble.compact ? 11 : (bubble.width - width) / 2
    anchors.verticalCenter: parent.verticalCenter
    Behavior on x { NumberAnimation { duration: 280 * bubble.host.motionScale; easing.type: Easing.OutQuint } }

    ProgressRing {
      anchors.fill: parent
      // Battery as a capsule: its icon rather than the ring.
      visible: bubble.kind === "timer" || ((bubble.kind === "battery" || bubble.kind === "device") && !bubble.compact)
      progress: bubble.progress
      color: bubble.tint
      track: bubble.host.withAlpha(bubble.tint, 0.22)
      lineWidth: 2.6
    }
    // OBS: a pulsing red dot (yellow and still when paused).
    Rectangle {
      id: recDot
      anchors.centerIn: parent
      visible: bubble.kind === "obs"
      width: 11; height: 11; radius: 5.5
      color: bubble.tint
      SequentialAnimation on opacity {
        running: bubble.kind === "obs" && !bubble.obs.paused
        loops: Animation.Infinite
        onRunningChanged: if (!running) recDot.opacity = 1
        NumberAnimation { to: 0.3; duration: 650; easing.type: Easing.InOutSine }
        NumberAnimation { to: 1; duration: 650; easing.type: Easing.InOutSine }
      }
    }
    // Device: its icon (mouse, keyboard, headset), in the ring as a circle.
    Glyph {
      anchors.centerIn: parent
      visible: bubble.kind === "device"
      glyph: bubble.device ? bubble.device.glyph : ""
      color: bubble.tint
      fontFamily: bubble.host.fontFamily
      pixelSize: bubble.compact ? 16 : 10
    }
    // Battery as a circle: the percentage in the ring; as a capsule, the bolt.
    Text {
      anchors.centerIn: parent
      visible: bubble.kind === "battery"
      text: bubble.compact ? bubble.battery.glyph : Math.round(bubble.battery.percent * 100)
      color: bubble.tint
      font.family: bubble.compact ? bubble.host.fontFamily : bubble.host.uiFont
      font.pixelSize: bubble.compact ? 17 : 8
      font.weight: Font.Bold
    }
    // The other machine: a screen, breathing while a mode starts or stops.
    Glyph {
      id: remoteGlyph
      anchors.centerIn: parent
      visible: bubble.kind === "remote"
      glyph: "󰍹"
      color: bubble.tint
      fontFamily: bubble.host.fontFamily
      pixelSize: 15
      SequentialAnimation on opacity {
        running: bubble.remoteBusy
        loops: Animation.Infinite
        onRunningChanged: if (!running) remoteGlyph.opacity = 1
        NumberAnimation { to: 0.35; duration: 700; easing.type: Easing.InOutSine }
        NumberAnimation { to: 1; duration: 700; easing.type: Easing.InOutSine }
      }
    }
    Glyph {
      id: aiGlyph
      anchors.centerIn: parent
      visible: bubble.kind === "ai"
      glyph: "󰚩"
      color: bubble.tint
      fontFamily: bubble.host.fontFamily
      pixelSize: 15
      SequentialAnimation on opacity {
        running: bubble.kind === "ai" && !!bubble.conv && bubble.conv.busy
        loops: Animation.Infinite
        onRunningChanged: if (!running) aiGlyph.opacity = 1
        NumberAnimation { to: 0.35; duration: 700; easing.type: Easing.InOutSine }
        NumberAnimation { to: 1; duration: 700; easing.type: Easing.InOutSine }
      }
    }
  }

  // ------------------------------------------------ music: cover (+ wave as a capsule)
  ClippingRectangle {
    visible: bubble.kind === "music"
    x: bubble.compact ? 9 : (bubble.width - width) / 2
    anchors.verticalCenter: parent.verticalCenter
    width: bubble.compact ? 20 : 24
    height: width
    radius: bubble.compact ? 6 : width / 2
    color: bubble.host.withAlpha(bubble.host.colorAccent, 0.3)
    Behavior on x { NumberAnimation { duration: 280 * bubble.host.motionScale; easing.type: Easing.OutQuint } }
    Image {
      id: cover
      anchors.fill: parent
      source: bubble.kind === "music" ? bubble.host.mediaArt : ""
      sourceSize.width: 48
      sourceSize.height: 48
      fillMode: Image.PreserveAspectCrop
      asynchronous: true
      cache: false
      visible: status === Image.Ready
    }
    Glyph {
      anchors.centerIn: parent
      visible: cover.status !== Image.Ready
      glyph: "󰝚"
      color: bubble.host.colorAccentText
      fontFamily: bubble.host.fontFamily
      pixelSize: 11
    }
  }
  SoundWave {
    id: musicWave
    visible: bubble.kind === "music" && bubble.compact
    x: 9 + 20 + 10
    anchors.verticalCenter: parent.verticalCenter
    color: bubble.host.mediaTint
    playing: bubble.kind === "music" && bubble.host.mediaPlaying
    bars: 4
    barWidth: 2.5
    maxHeight: 14
    opacity: bubble.host.playerVolumeHintShown ? 0 : 1
    Behavior on opacity { NumberAnimation { duration: 120 } }
  }
  // Level after a wheel notch, in place of the wave.
  Text {
    visible: bubble.kind === "music" && bubble.compact
    opacity: bubble.host.playerVolumeHintShown ? 1 : 0
    Behavior on opacity { NumberAnimation { duration: 120 } }
    x: musicWave.x
    anchors.verticalCenter: parent.verticalCenter
    text: Math.round(Math.max(0, bubble.host.playerVolume) * 100) + " %"
    color: bubble.host.colorText
    font.family: bubble.host.uiFont
    font.pixelSize: 11
    font.features: { "tnum": 1 }
  }

  // Answer ready, as a circle: dot in the corner.
  Rectangle {
    visible: !bubble.compact && bubble.aiReady
    anchors.right: parent.right
    anchors.top: parent.top
    anchors.margins: 3
    width: 8; height: 8; radius: 4
    color: bubble.conv && bubble.conv.lastError ? "#ff453a" : bubble.host.colorAccent
  }

  // ------------------------------------------------ on the right (capsule)
  Row {
    id: content
    x: 11
    anchors.verticalCenter: parent.verticalCenter
    spacing: 9
    opacity: bubble.compact && bubble.kind !== "music" ? 1 : 0
    visible: opacity > 0.01
    Behavior on opacity { NumberAnimation { duration: 160 } }
    Item { width: 18; height: 18 }   // place of the left element
    Text {
      anchors.verticalCenter: parent.verticalCenter
      width: Math.min(implicitWidth, 150)
      elide: Text.ElideMiddle
      text: bubble.trailing
      color: bubble.kind === "ai" && !bubble.aiReady && !(bubble.conv && bubble.conv.busy) ? bubble.host.colorText : bubble.tint
      font.family: bubble.host.uiFont
      font.pixelSize: 13
      font.weight: Font.DemiBold
      font.features: { "tnum": 1 }
    }
    // Mode running on the desk: where the mic goes (accent: to the other machine).
    Glyph {
      anchors.verticalCenter: parent.verticalCenter
      visible: !!bubble.remote && bubble.remote.role === "desk" && bubble.remote.phase === "mode" && bubble.remote.mic !== "off"
      glyph: bubble.remote && bubble.remote.mic === "failed" ? "󰍭" : "󰍬"
      color: !bubble.remote ? bubble.host.colorMuted : bubble.remote.mic === "remote" ? bubble.host.colorAccent : bubble.remote.mic === "failed" ? "#ff453a" : bubble.host.colorMuted
      fontFamily: bubble.host.fontFamily
      pixelSize: 14
    }
    // Battery on mains unplugged: the current power profile's icon.
    Glyph {
      anchors.verticalCenter: parent.verticalCenter
      visible: bubble.kind === "battery" && bubble.battery.profileGlyph !== ""
      glyph: bubble.battery.profileGlyph
      color: bubble.host.colorMuted
      fontFamily: bubble.host.fontFamily
      pixelSize: 14
    }
  }
}
