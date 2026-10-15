import QtQuick
import "../components"

// The banner's OSD (volume, brightness, microphone, music, power): a pill, the icon in
// the accent, then the bar and its value, or the message ('Title - Artist' for music).
// A single place: an OSD that replaces another changes in place, the pill widens or
// narrows (the content, laid out at its final width, is revealed with it) and the bar slides
// to the new value.
Item {
  id: view
  required property var bandeau
  property bool active: false

  readonly property var osd: bandeau.osd
  readonly property bool progress: !!osd.hasProgress
  readonly property int padX: 18
  readonly property int gap: 14
  readonly property int iconBox: 22
  readonly property int barWidth: 150
  readonly property int maxMessageWidth: osd.media ? 440 : 300
  readonly property int fontSize: 14

  // What the piece takes in the stack (for those below): its height, in proportion
  // to its presence.
  property real presence: 0
  Component.onCompleted: presence = active ? 1 : 0
  function followActive() {
    presenceAnim.stop()
    presencePause.duration = active ? 0 : 200
    presenceMove.to = active ? 1 : 0
    presenceAnim.start()
  }
  SequentialAnimation {
    id: presenceAnim
    PauseAnimation { id: presencePause }
    NumberAnimation { id: presenceMove; target: view; property: "presence"; duration: 340; easing.type: Easing.OutQuint }
  }
  readonly property real space: (height + bandeau.gap) * presence
  // The piece only appears when its place can contain it (the gap absorbs the end of
  // the opening); when leaving, it fades before its place closes.
  readonly property bool shownIn: active && space >= height - 0.5
  readonly property bool live: opacity > 0.01

  TextMetrics {
    id: messageMetrics
    font.family: view.bandeau.fontFamily
    font.pixelSize: view.fontSize
    font.bold: true
    text: view.osd.message || ""
  }
  TextMetrics {
    id: valueMetrics
    font: messageMetrics.font
    text: "100%"
  }
  readonly property real valueWidth: Math.ceil(valueMetrics.advanceWidth)
  readonly property real messageWidth: Math.min(maxMessageWidth, Math.ceil(messageMetrics.advanceWidth) + 1)
  readonly property real contentWidth: iconBox + gap
    + (progress ? barWidth + gap + valueWidth : messageWidth)

  width: Math.ceil(contentWidth + 2 * padX)
  height: 44

  // Entry: comes down from a bit higher, grows a bit, appears. Exit: fades,
  // shrinks, goes up a bit.
  opacity: shownIn ? 1 : 0
  scale: shownIn ? 1 : 0.94
  property real drop: shownIn ? 0 : -bandeau.gap
  transform: Translate { y: view.drop }
  Behavior on opacity { NumberAnimation { duration: view.shownIn ? 260 : 200; easing.type: view.shownIn ? Easing.OutCubic : Easing.InCubic } }
  Behavior on scale { NumberAnimation { duration: view.shownIn ? 340 : 220; easing.type: view.shownIn ? Easing.OutBack : Easing.InCubic; easing.overshoot: 1.2 } }
  Behavior on drop { NumberAnimation { duration: view.shownIn ? 340 : 220; easing.type: view.shownIn ? Easing.OutQuint : Easing.InCubic } }
  visible: live

  // Shown width: from one OSD to the next, it slides to the new width (the content is
  // laid out right away at its final width and is revealed with it). An OSD that appears while
  // nothing was visible takes its width right away.
  property real shownWidth: width
  onWidthChanged: {
    if (live) bandeau.animateTo(widthAnim, shownWidth, width)
    else { widthAnim.stop(); shownWidth = width }
  }
  onActiveChanged: {
    if (active && !live) { widthAnim.stop(); shownWidth = width }
    followActive()
  }
  NumberAnimation { id: widthAnim; target: view; property: "shownWidth"; duration: 260; easing.type: Easing.OutQuint }

  BandeauSurface {
    id: surface
    bandeau: view.bandeau
    anchors.centerIn: parent
    width: view.shownWidth
    height: view.height
  }

  Item {
    anchors.centerIn: parent
    width: view.shownWidth
    height: view.height
    clip: true

    Row {
      anchors.centerIn: parent
      width: view.contentWidth
      spacing: view.gap

      Item {
        width: view.iconBox
        height: view.height
        Glyph {
          anchors.centerIn: parent
          glyph: view.osd.icon || ""
          color: view.bandeau.colorAccent
          pixelSize: 18
          fontFamily: view.bandeau.fontFamily
        }
      }

      Rectangle {
        visible: view.progress
        anchors.verticalCenter: parent.verticalCenter
        width: view.barWidth
        height: 6
        radius: 3
        color: view.bandeau.withAlpha(view.bandeau.colorText, 0.14)
        Rectangle {
          height: parent.height
          radius: parent.radius
          width: parent.width * (view.progress && view.osd.maxValue > 0 ? view.osd.value / view.osd.maxValue : 0)
          color: view.osd.dim ? view.bandeau.colorMuted : view.bandeau.colorAccent
          Behavior on color { ColorAnimation { duration: 160 } }
          Behavior on width {
            enabled: view.active
            NumberAnimation { duration: 160; easing.type: Easing.OutCubic }
          }
        }
      }

      Text {
        visible: view.osd.message !== ""
        anchors.verticalCenter: parent.verticalCenter
        width: view.progress ? view.valueWidth : view.messageWidth
        horizontalAlignment: view.progress ? Text.AlignRight : Text.AlignLeft
        text: view.osd.message || ""
        textFormat: Text.PlainText
        elide: Text.ElideRight
        maximumLineCount: 1
        color: view.bandeau.colorText
        font: messageMetrics.font
      }
    }
  }
}
