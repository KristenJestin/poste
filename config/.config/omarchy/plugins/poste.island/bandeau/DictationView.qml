import QtQuick

// Le texte dicté en direct (Vocaline, ou toute source du slot « dictation » de l'île qui donne
// du texte), en haut de la pile, juste sous l'île, sans icône : les phrases figées en
// couleur normale, puis la phrase en cours en gris. Trois lignes au plus : trop long, c'est la
// fin qui reste visible. Le dernier texte reste affiché pendant le fondu de sortie. La pastille
// animée (micro, onde, points) est à part, en bas de l'écran (DictationPill.qml).
//
// Quand le texte s'allonge, la pilule grandit en douceur, en largeur puis en hauteur quand il
// passe sur deux ou trois lignes. Le texte, lui, est posé tout de suite à sa taille finale
// (sinon il repasserait à la ligne pendant l'animation) : seuls le fond et la zone visible
// (`shownWidth`, `shownHeight`) s'animent. Le texte suit le bord gauche de la pilule et reste
// collé en bas, donc la fin reste visible ; ce qui dépasse est coupé le temps de l'animation.
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
      // Revenue alors qu'elle était tout à fait cachée : elle apparaît à sa taille.
      resize(!live)
    }
    followActive()
  }
  // Relu un tour de boucle plus tard, une fois toutes les valeurs de l'île posées : quand la
  // dictée s'arrête, le texte se vide en même temps qu'elle se cache, et la pilule doit garder
  // son dernier état pendant le fondu au lieu de rétrécir.
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

  // Taille finale, puis taille montrée (animée vers la finale).
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

  // Ce que la pièce prend dans la pile (pour celles du dessous) : sa hauteur montrée, à la
  // proportion de sa présence. Rien en dessous ne peut donc la chevaucher.
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
  // La pièce n'apparaît que lorsque sa place peut la contenir (l'écart absorbe la fin de
  // l'ouverture) ; en partant, elle s'efface avant que sa place se referme.
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

  // La zone visible du texte : la place qu'il a dans la pilule montrée.
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
