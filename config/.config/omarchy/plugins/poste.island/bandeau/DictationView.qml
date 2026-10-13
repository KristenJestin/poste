import QtQuick
import "../components"

// La dictée en cours (SUPER+A voxtype, SUPER+Z Vocaline ou toute source du slot « dictation »
// de l'île), pareille pour toutes les sources : le micro, puis l'onde de la voix (14 barres, la
// plus récente à droite, depuis l'historique des niveaux tenu par le bandeau), ou trois points
// qui pulsent pendant la transcription. Si la source donne du texte (Vocaline), il suit l'onde :
// les phrases figées en couleur normale, puis la phrase en cours en gris. Trois lignes au plus :
// trop long, c'est la fin qui reste visible. Le dernier état reste affiché pendant le fondu de
// sortie.
//
// Quand le texte arrive ou s'allonge, la pilule grandit en douceur, en largeur puis en hauteur
// quand il passe sur deux ou trois lignes. Le texte, lui, est posé tout de suite à sa taille
// finale (sinon il repasserait à la ligne pendant l'animation) : seuls le fond et la zone
// visible (`shownWidth`, `shownHeight`) s'animent. Le texte suit le bord gauche de la pilule et
// reste collé en bas, donc la fin reste visible ; ce qui dépasse est coupé le temps de
// l'animation.
Item {
  id: view
  required property var bandeau
  property bool active: false
  property real maxTextWidth: 900

  property string finals: ""
  property string partial: ""
  // Les points de la transcription à la place de l'onde.
  property bool transcribing: false
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
    function onDictListeningChanged() { view.queueKeep() }
    function onDictTranscribingChanged() { view.queueKeep() }
  }
  property bool keepQueued: false
  function queueKeep() {
    if (keepQueued) return
    keepQueued = true
    Qt.callLater(function() {
      view.keepQueued = false
      if (view.active && view.bandeau.dictShown) view.keep()
    })
  }
  function keep() {
    finals = bandeau.dictFinals
    partial = bandeau.dictPartial
    transcribing = bandeau.dictTranscribing && !bandeau.dictListening
  }

  function escapeHtml(s) {
    return String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;")
  }
  readonly property string plain: finals + (finals !== "" && partial !== "" ? " " : "") + partial
  readonly property string styled: escapeHtml(finals) + (finals !== "" && partial !== "" ? " " : "")
    + (partial !== "" ? "<font color=\"" + bandeau.colorMuted + "\">" + escapeHtml(partial) + "</font>" : "")
  readonly property bool hasText: plain !== ""

  readonly property int fontSize: 15
  readonly property int lines: 3
  readonly property int padX: 18
  readonly property int padY: 11
  readonly property int iconBox: 18
  readonly property int gap: 10
  // L'onde : 14 barres de 2,5 px espacées de 2 px (l'aspect de la pastille voxtype de l'île).
  readonly property int bars: bandeau.dictBars
  readonly property real barWidth: 2.5
  readonly property real barSpacing: 2
  readonly property real waveWidth: bars * barWidth + (bars - 1) * barSpacing
  readonly property real waveMax: 18

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
  readonly property real lineHeight: Math.max(waveMax, Math.ceil(fm.lineSpacing))
  readonly property real textWidth: hasText ? Math.min(maxTextWidth, Math.ceil(oneLine.advanceWidth) + 2) : 0
  readonly property real textHeight: Math.max(lineHeight, Math.min(label.implicitHeight, lineHeight * lines))
  readonly property bool multiLine: hasText && label.lineCount > 1
  // Début du texte, après le micro et l'onde.
  readonly property real textX: padX + iconBox + gap + waveWidth + 2 * gap

  // Taille finale, puis taille montrée (animée vers la finale).
  readonly property real finalWidth: hasText ? textX + textWidth + padX : padX + iconBox + gap + waveWidth + padX
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

  // Ce que la pièce prend dans la pile (pour celles du dessus) : sa hauteur montrée, à la
  // proportion de sa présence. Rien au-dessus ne peut donc la chevaucher.
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
  property real drop: shownIn ? 0 : bandeau.gap
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

  // Le micro, gris clair et immobile : l'onde à côté porte le mouvement.
  Item {
    x: view.padX
    y: view.padY
    width: view.iconBox
    height: view.lineHeight
    Glyph {
      anchors.centerIn: parent
      glyph: "󰍬"
      color: view.bandeau.withAlpha(view.bandeau.colorText, 0.75)
      pixelSize: 15
      fontFamily: view.bandeau.fontFamily
    }
  }

  // L'onde (écoute) ou les points (transcription), à hauteur de la première ligne.
  Item {
    id: indicator
    x: view.padX + view.iconBox + view.gap
    y: view.padY
    width: view.waveWidth
    height: view.lineHeight

    Row {
      anchors.verticalCenter: parent.verticalCenter
      visible: !view.transcribing
      spacing: view.barSpacing
      Repeater {
        model: view.bars
        delegate: Rectangle {
          required property int index
          readonly property int back: view.bars - 1 - index
          readonly property var levels: view.bandeau.dictLevels
          readonly property real level: back < levels.length ? levels[levels.length - 1 - back] : 0
          anchors.verticalCenter: parent.verticalCenter
          width: view.barWidth
          height: 3 + (view.waveMax - 3) * level
          radius: view.barWidth / 2
          color: view.bandeau.colorAccent
          opacity: 0.45 + 0.55 * (index / view.bars)
          Behavior on height { NumberAnimation { duration: 90; easing.type: Easing.OutQuad } }
        }
      }
    }

    Row {
      anchors.centerIn: parent
      visible: view.transcribing
      spacing: 4
      Repeater {
        model: 3
        delegate: Rectangle {
          id: dot
          required property int index
          width: 5; height: 5; radius: 2.5
          color: view.bandeau.colorText
          opacity: 0.35
          SequentialAnimation on opacity {
            running: view.transcribing && view.live
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

  // La zone visible du texte : la place qu'il a dans la pilule montrée.
  Item {
    id: clipper
    x: view.textX
    y: view.padY
    width: Math.max(0, view.shownWidth - x - view.padX)
    height: Math.max(0, view.shownHeight - 2 * view.padY)
    clip: true
    Text {
      id: label
      anchors.bottom: parent.bottom
      width: Math.max(1, view.textWidth)
      text: view.styled
      textFormat: Text.StyledText
      wrapMode: Text.WordWrap
      color: view.bandeau.colorText
      font: oneLine.font
    }
  }
}
