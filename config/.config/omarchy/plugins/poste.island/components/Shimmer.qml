import QtQuick

// Common loading of the island: ghost rows in place of the expected
// content, swept by a reflection, and above them a label (what is happening)
// crossed by the same reflection. Used by the AI (while waiting for the answer) and by the
// Usage view (while reading the logs).
//   lines: width of each ghost row (fraction of the width)
//   blocks: taller rows (title, big number), same rules
Item {
  id: shimmer
  required property var host
  property string label: ""
  property var lines: [0.92, 0.78, 0.55]
  property real lineHeight: 9
  property real spacing: 9
  property bool running: visible

  // 0 → 1: position of the reflection, common to all the rows and to the label.
  property real phase: 0
  NumberAnimation on phase {
    running: shimmer.running
    from: 0; to: 1
    duration: 1500
    loops: Animation.Infinite
  }
  readonly property color base: host.withAlpha(host.colorText, 0.07)
  readonly property color glow: host.withAlpha(host.colorText, 0.22)
  readonly property real band: Math.max(120, width * 0.45)
  // X of the reflection in the shimmer's frame (starts from the left, out of view).
  readonly property real sweepX: -band + phase * (width + 2 * band)

  implicitHeight: col.implicitHeight

  Column {
    id: col
    width: parent.width
    spacing: shimmer.spacing

    ShimmerText {
      visible: shimmer.label !== ""
      host: shimmer.host
      text: shimmer.label
      sweepX: shimmer.sweepX - x
      band: shimmer.band
    }

    Repeater {
      model: shimmer.lines
      delegate: Rectangle {
        id: bar
        required property var modelData
        width: col.width * Number(modelData)
        height: shimmer.lineHeight
        radius: height / 2
        color: shimmer.base
        clip: true
        Rectangle {
          x: shimmer.sweepX - bar.x
          width: shimmer.band
          height: parent.height
          gradient: Gradient {
            orientation: Gradient.Horizontal
            GradientStop { position: 0; color: "transparent" }
            GradientStop { position: 0.5; color: shimmer.glow }
            GradientStop { position: 1; color: "transparent" }
          }
        }
      }
    }
  }
}
