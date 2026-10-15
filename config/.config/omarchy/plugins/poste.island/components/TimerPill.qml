import QtQuick
import QtQuick.Effects

// A timer's small island, at the top left of the screen: its orange (accent)
// background empties to the left as time passes. The text is
// dark on the orange and light on the black (two layers, cut at the
// bar's limit). Click: unfolds the controls (+1 min, pause / resume,
// cancel); right click: pause. Finished: pulsing orange pill, looping sound
// (Island.qml); a click closes it and stops the sound.
Rectangle {
  id: pill
  required property var host
  required property var timer
  property bool expanded: false
  // Compact (room missing, see IslandWindow): the time alone, without a label.
  property bool compact: false

  readonly property double remaining: host.timerLeft(timer)
  readonly property real progress: timer.duration > 0 ? Math.max(0, Math.min(1, remaining / timer.duration)) : 0
  readonly property bool done: timer.done === true
  readonly property bool paused: timer.paused === true && !done
  readonly property real fillWidth: done ? width : width * progress

  readonly property color light: host.colorText
  readonly property color dark: host.colorAccentText   // readable on the accent, whatever it is

  height: host.restHeight
  width: content.implicitWidth + 22
  radius: height / 2
  color: host.colorIsland
  Behavior on width { NumberAnimation { duration: 260 * host.motionScale; easing.type: Easing.OutQuint } }

  component PillButton: Rectangle {
    id: btn
    property string glyph: ""
    property string label: ""
    property color ink: "white"
    property bool live: false
    signal activated()
    width: Math.max(24, btnText.implicitWidth + 12); height: 24; radius: 12
    color: Qt.rgba(ink.r, ink.g, ink.b, btnMouse.containsMouse ? 0.18 : 0.10)
    Text {
      id: btnText
      anchors.centerIn: parent
      text: btn.label !== "" ? btn.label : btn.glyph
      color: btn.ink
      font.family: btn.label !== "" ? pill.host.uiFont : pill.host.fontFamily
      font.pixelSize: btn.label !== "" ? 11 : 12
      font.weight: Font.DemiBold
    }
    MouseArea {
      id: btnMouse
      anchors.fill: parent
      enabled: btn.live
      hoverEnabled: true
      cursorShape: Qt.PointingHandCursor
      onClicked: btn.activated()
    }
  }

  // Content: time, label, controls. Drawn twice (light / dark).
  component PillContent: Row {
    id: rowContent
    property color ink: "white"
    property bool live: false      // the dark copy is not clickable
    spacing: 8
    Text {
      anchors.verticalCenter: parent.verticalCenter
      text: pill.done ? "Done" : pill.host.formatLeft(pill.remaining)
      color: rowContent.ink
      opacity: pill.paused ? 0.6 : 1
      font.family: pill.host.uiFont
      font.pixelSize: 13
      font.weight: Font.Bold
      font.features: { "tnum": 1 }
    }
    Text {
      anchors.verticalCenter: parent.verticalCenter
      visible: text !== "" && !pill.compact
      width: Math.min(implicitWidth, 140)
      elide: Text.ElideRight
      text: pill.timer.label || ""
      color: rowContent.ink
      opacity: 0.85
      font.family: pill.host.uiFont
      font.pixelSize: 12
      font.weight: Font.DemiBold
    }
    Row {
      anchors.verticalCenter: parent.verticalCenter
      spacing: 2
      visible: pill.expanded && !pill.done
      PillButton { ink: rowContent.ink; live: rowContent.live; label: "+1m"; onActivated: pill.host.addToTimer(pill.timer.id, 60000) }
      PillButton { ink: rowContent.ink; live: rowContent.live; glyph: pill.paused ? "󰐊" : "󰏤"; onActivated: pill.host.togglePause(pill.timer.id) }
      PillButton { ink: rowContent.ink; live: rowContent.live; glyph: "󰅖"; onActivated: pill.host.cancelTimer(pill.timer.id) }
    }
  }

  MouseArea {
    anchors.fill: parent
    hoverEnabled: true
    cursorShape: Qt.PointingHandCursor
    acceptedButtons: Qt.LeftButton | Qt.RightButton
    onClicked: function(mouse) {
      if (pill.done) { host.cancelTimer(timer.id); return }
      if (mouse.button === Qt.RightButton) { host.togglePause(timer.id); return }
      pill.expanded = !pill.expanded
    }
  }

  // Light layer (on the black), with the clickable controls.
  PillContent {
    id: content
    anchors.centerIn: parent
    ink: pill.light
    live: true
  }

  // Orange bar + dark layer, cut to the shape of the pill.
  Item {
    id: fillLayer
    anchors.fill: parent
    layer.enabled: true
    layer.effect: MultiEffect {
      maskEnabled: true
      maskSource: fillMask
      maskThresholdMin: 0.5
      maskSpreadAtMin: 1.0
    }
    Item {
      width: pill.fillWidth
      height: parent.height
      clip: true
      Behavior on width { NumberAnimation { duration: 250 } }
      Rectangle {
        width: pill.width
        height: pill.height
        color: pill.paused ? Qt.darker(pill.host.colorAccent, 1.9) : pill.host.colorAccent
        Behavior on color { ColorAnimation { duration: 200 } }
      }
      PillContent {
        x: content.x
        y: content.y
        ink: pill.paused ? pill.light : pill.dark
      }
    }
  }
  Item {
    id: fillMask
    anchors.fill: parent
    visible: false
    layer.enabled: true
    Rectangle { anchors.fill: parent; radius: pill.radius; color: "black" }
  }

  // Finished: breathing (light veil) until the click.
  Rectangle {
    anchors.fill: parent
    radius: parent.radius
    color: "white"
    opacity: 0
    SequentialAnimation on opacity {
      running: pill.done
      loops: Animation.Infinite
      NumberAnimation { to: 0.28; duration: 520; easing.type: Easing.InOutSine }
      NumberAnimation { to: 0; duration: 520; easing.type: Easing.InOutSine }
    }
  }
}
