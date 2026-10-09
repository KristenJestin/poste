import QtQuick

// Dictation in the resting island: a still mic and a small waveform that follows
// the microphone level while voxtype listens, then three pulsing dots while
// it transcribes.
Row {
  id: vox
  required property var host
  spacing: 10

  // A fixed light-grey mic; the waveform beside it carries the motion.
  Glyph {
    anchors.verticalCenter: parent.verticalCenter
    glyph: "󰍬"
    color: vox.host.withAlpha(vox.host.colorText, 0.75)
    fontFamily: vox.host.fontFamily
    pixelSize: 15
  }

  // Live waveform: one bar per recent level, newest on the right.
  Row {
    anchors.verticalCenter: parent.verticalCenter
    visible: vox.host.voxListening
    spacing: 2
    Repeater {
      model: vox.host.voxBars
      delegate: Rectangle {
        required property int index
        readonly property int back: vox.host.voxBars - 1 - index
        readonly property var levels: vox.host.voxLevels
        readonly property real level: back < levels.length ? levels[levels.length - 1 - back] : 0
        anchors.verticalCenter: parent.verticalCenter
        width: 2.5
        height: 3 + 15 * level
        radius: 1.25
        color: vox.host.colorAccent
        opacity: 0.45 + 0.55 * (index / vox.host.voxBars)
        Behavior on height { NumberAnimation { duration: 90; easing.type: Easing.OutQuad } }
      }
    }
  }

  // Transcribing: three dots in a wave.
  Row {
    anchors.verticalCenter: parent.verticalCenter
    visible: vox.host.voxTranscribing
    spacing: 4
    Repeater {
      model: 3
      delegate: Rectangle {
        id: dot
        required property int index
        width: 5; height: 5; radius: 2.5
        color: vox.host.colorText
        opacity: 0.35
        SequentialAnimation on opacity {
          running: vox.host.voxTranscribing
          loops: Animation.Infinite
          PauseAnimation { duration: dot.index * 160 }
          NumberAnimation { to: 1; duration: 280; easing.type: Easing.OutQuad }
          NumberAnimation { to: 0.35; duration: 280; easing.type: Easing.InQuad }
          PauseAnimation { duration: (2 - dot.index) * 160 }
        }
      }
    }
  }
}
