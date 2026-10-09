import QtQuick
import Quickshell.Widgets

// Une activité de l'île (minuteur, conversation IA, batterie, musique,
// dictée) en bulle, à
// côté de l'île : « compact » = capsule, une info à gauche et une à droite
// (anneau et temps restant) ; « minimal » = un rond (quand plusieurs
// activités se partagent la place). La version dépliée est ActivityCard.qml.
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

  // ------------------------------------------------ état de l'activité
  readonly property real timerLeft: timer ? host.timerLeft(timer) : 0
  readonly property real progress: kind === "timer" ? (timer.done ? 1 : timer.duration > 0 ? timerLeft / timer.duration : 0)
    : kind === "battery" ? battery.percent : kind === "device" ? device.percent : 0
  readonly property bool aiReady: !!conv && conv.unseen && !conv.busy
  readonly property color tint: kind === "battery" ? battery.color
    : kind === "device" ? device.color
    : kind === "obs" ? (obs.paused ? "#ffd60a" : "#ff453a")
    : kind === "music" ? host.mediaTint
    : kind === "vox" ? "#ff453a"
    : kind === "timer" ? (timer.paused ? host.colorMuted : host.colorAccent)
    : (aiReady || (conv && conv.busy) ? host.colorAccent : host.colorText)
  readonly property string trailing: {
    if (kind === "music" || kind === "vox") return ""
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
    : kind === "vox" ? voxPill.implicitWidth + 28
    : kind === "music" ? 22 + 20 + 10 + musicWave.width + 4
    : Math.max(height, content.implicitWidth + 26)
  radius: height / 2
  color: host.colorIsland
  // Même liseré que l'île : sans lui, la bulle paraît plus grande qu'elle.
  border.width: 1
  border.color: host.withAlpha(host.colorText, 0.07)
  Behavior on width { NumberAnimation { duration: 280 * bubble.host.motionScale; easing.type: Easing.OutQuint } }

  HoverHandler { id: hover }
  // Molette sur la pastille média : volume du lecteur, 5 % par cran. Un
  // handler ne bloque ni le clic ni le survol.
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

  // Minuteur terminé : la bulle respire jusqu'au clic.
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

  // ------------------------------------------------ à gauche (ou seul, en rond)
  Item {
    id: leading
    visible: bubble.kind !== "music" && bubble.kind !== "vox"
    width: 18
    height: 18
    x: bubble.compact ? 11 : (bubble.width - width) / 2
    anchors.verticalCenter: parent.verticalCenter
    Behavior on x { NumberAnimation { duration: 280 * bubble.host.motionScale; easing.type: Easing.OutQuint } }

    ProgressRing {
      anchors.fill: parent
      // Batterie en capsule : son icône plutôt que l'anneau.
      visible: bubble.kind === "timer" || ((bubble.kind === "battery" || bubble.kind === "device") && !bubble.compact)
      progress: bubble.progress
      color: bubble.tint
      track: bubble.host.withAlpha(bubble.tint, 0.22)
      lineWidth: 2.6
    }
    // OBS : un point rouge qui pulse (jaune et fixe en pause).
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
    // Périphérique : son icône (souris, clavier, casque), dans l'anneau en rond.
    Glyph {
      anchors.centerIn: parent
      visible: bubble.kind === "device"
      glyph: bubble.device ? bubble.device.glyph : ""
      color: bubble.tint
      fontFamily: bubble.host.fontFamily
      pixelSize: bubble.compact ? 16 : 10
    }
    // Batterie en rond : le pourcentage dans l'anneau ; en capsule, l'éclair.
    Text {
      anchors.centerIn: parent
      visible: bubble.kind === "battery"
      text: bubble.compact ? bubble.battery.glyph : Math.round(bubble.battery.percent * 100)
      color: bubble.tint
      font.family: bubble.compact ? bubble.host.fontFamily : bubble.host.uiFont
      font.pixelSize: bubble.compact ? 17 : 8
      font.weight: Font.Bold
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

  // ------------------------------------------------ musique : pochette (+ onde en capsule)
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
  // Niveau après un coup de molette, à la place de l'onde.
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

  // ------------------------------------------------ dictée : micro et niveau (VoxPill)
  VoxPill {
    id: voxPill
    visible: bubble.kind === "vox" && bubble.compact
    x: 14
    anchors.verticalCenter: parent.verticalCenter
    host: bubble.host
  }
  Glyph {
    anchors.centerIn: parent
    visible: bubble.kind === "vox" && !bubble.compact
    glyph: "󰍬"
    color: "#ff453a"
    fontFamily: bubble.host.fontFamily
    pixelSize: 15
  }

  // Réponse prête, en rond : point dans le coin.
  Rectangle {
    visible: !bubble.compact && bubble.aiReady
    anchors.right: parent.right
    anchors.top: parent.top
    anchors.margins: 3
    width: 8; height: 8; radius: 4
    color: bubble.conv && bubble.conv.lastError ? "#ff453a" : bubble.host.colorAccent
  }

  // ------------------------------------------------ à droite (capsule)
  Row {
    id: content
    x: 11
    anchors.verticalCenter: parent.verticalCenter
    spacing: 9
    opacity: bubble.compact && bubble.kind !== "music" && bubble.kind !== "vox" ? 1 : 0
    visible: opacity > 0.01
    Behavior on opacity { NumberAnimation { duration: 160 } }
    Item { width: 18; height: 18 }   // place de l'élément de gauche
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
  }
}
