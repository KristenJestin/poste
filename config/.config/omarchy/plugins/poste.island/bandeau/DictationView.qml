import QtQuick

// The live dictated text (Vocaline, or any source of the island's 'dictation' slot that gives
// text), at the top of the stack, just under the island, without an icon: the frozen sentences in
// normal color, then the sentence in progress in grey. Three lines at most: too long, and it is the
// end that stays visible. The last text stays displayed during the exit fade. The animated pill
// (microphone, wave, dots) is apart, at the bottom of the screen (DictationPill.qml).
//
// When the text grows, the pill grows smoothly, in width then in height when it
// goes onto two or three lines. The text itself is laid out right away at its final size
// (otherwise it would wrap again during the animation): only the background and the visible area
// (`shownWidth`, `shownHeight`) animate. The text follows the pill's left edge and stays
// stuck to the bottom, so the end stays visible; what overflows is clipped for the time of the animation.
Item {
  id: view
  required property var bandeau
  property bool active: false
  property real maxTextWidth: 900

  property string finals: ""
  property string partial: ""
  onActiveChanged: {
    if (active) {
      keep()
      // Back while it was completely hidden: it appears at its size.
      resize(!live)
    }
    followActive()
  }
  // Read again one loop turn later, once all the island's values are set: when the
  // dictation stops, the text empties at the same time as it hides, and the pill must keep
  // its last state during the fade instead of shrinking.
  Connections {
    target: view.bandeau
    function onDictFinalsChanged() { view.queueKeep() }
    function onDictPartialChanged() { view.queueKeep() }
  }
  property bool keepQueued: false
  function queueKeep() {
    if (keepQueued) return
    keepQueued = true
    Qt.callLater(function() {
      view.keepQueued = false
      if (view.active && view.bandeau.dictTextShown) view.keep()
    })
  }
  function keep() {
    finals = bandeau.dictFinals
    partial = bandeau.dictPartial
  }

  function escapeHtml(s) {
    return String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;")
  }
  readonly property string plain: finals + (finals !== "" && partial !== "" ? " " : "") + partial
  readonly property string styled: escapeHtml(finals) + (finals !== "" && partial !== "" ? " " : "")
    + (partial !== "" ? "<font color=\"" + bandeau.colorMuted + "\">" + escapeHtml(partial) + "</font>" : "")

  readonly property int fontSize: 15
  readonly property int lines: 3
  readonly property int padX: 20
  readonly property int padY: 11

  TextMetrics {
    id: oneLine
    font.family: view.bandeau.fontFamily
    font.pixelSize: view.fontSize
    font.bold: true
    text: view.plain
  }
  FontMetrics {
    id: fm
    font: oneLine.font
  }
  readonly property real lineHeight: Math.ceil(fm.lineSpacing)
  readonly property real textWidth: Math.min(maxTextWidth, Math.ceil(oneLine.advanceWidth) + 2)
  readonly property real textHeight: Math.max(lineHeight, Math.min(label.implicitHeight, lineHeight * lines))
  readonly property bool multiLine: label.lineCount > 1

  // Final size, then shown size (animated toward the final one).
  readonly property real finalWidth: textWidth + 2 * padX
  readonly property real finalHeight: textHeight + 2 * padY
  property real shownWidth: finalWidth
  property real shownHeight: finalHeight
  onFinalWidthChanged: resize(!live)
  onFinalHeightChanged: resize(!live)
  NumberAnimation { id: widthAnim; target: view; property: "shownWidth"; duration: 260; easing.type: Easing.OutCubic }
  NumberAnimation { id: heightAnim; target: view; property: "shownHeight"; duration: 260; easing.type: Easing.OutCubic }
  function resize(snap) {
    if (snap) {
      widthAnim.stop()
      heightAnim.stop()
      shownWidth = finalWidth
      shownHeight = finalHeight
      return
    }
    bandeau.animateTo(widthAnim, shownWidth, finalWidth)
    bandeau.animateTo(heightAnim, shownHeight, finalHeight)
  }

  width: shownWidth
  height: shownHeight

  // What the piece takes in the stack (for those below): its shown height, in
  // proportion to its presence. Nothing below can therefore overlap it.
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
  readonly property real space: (shownHeight + bandeau.gap) * presence
  // The piece only appears when its place can contain it (the gap absorbs the end of
  // the opening); when leaving, it fades before its place closes.
  readonly property bool shownIn: active && space >= height - 0.5
  readonly property bool live: opacity > 0.01

  opacity: shownIn ? 1 : 0
  scale: shownIn ? 1 : 0.94
  property real drop: shownIn ? 0 : -bandeau.gap
  transform: Translate { y: view.drop }
  Behavior on opacity { NumberAnimation { duration: view.shownIn ? 260 : 200; easing.type: view.shownIn ? Easing.OutCubic : Easing.InCubic } }
  Behavior on scale { NumberAnimation { duration: view.shownIn ? 340 : 220; easing.type: view.shownIn ? Easing.OutBack : Easing.InCubic; easing.overshoot: 1.2 } }
  Behavior on drop { NumberAnimation { duration: view.shownIn ? 340 : 220; easing.type: view.shownIn ? Easing.OutQuint : Easing.InCubic } }
  visible: live

  BandeauSurface {
    anchors.fill: parent
    bandeau: view.bandeau
    radius: view.multiLine ? view.bandeau.blockRadius : height / 2
  }

  // The visible area of the text: the room it has in the shown pill.
  Item {
    id: clipper
    x: view.padX
    y: view.padY
    width: Math.max(0, view.shownWidth - x - view.padX)
    height: Math.max(0, view.shownHeight - 2 * view.padY)
    clip: true
    Text {
      id: label
      anchors.bottom: parent.bottom
      width: view.textWidth
      text: view.styled
      textFormat: Text.StyledText
      wrapMode: Text.WordWrap
      color: view.bandeau.colorText
      font: oneLine.font
    }
  }
}
