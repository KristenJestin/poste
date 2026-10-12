import QtQuick
import "../components"

// Le texte dicté en direct (slot « dictation » de l'île) : le micro à l'accent, les phrases
// figées en couleur normale, puis la phrase en cours en gris. Trois lignes au plus : trop long,
// c'est la fin qui reste visible. Le dernier texte reste affiché pendant le fondu de sortie.
//
// La largeur du texte n'est pas animée : le texte la prend telle quelle, sans repasser sur deux
// lignes le temps d'une animation chaque fois que la phrase s'allonge.
Item {
  id: view
  required property var bandeau
  property bool active: false
  property real maxTextWidth: 900

  property string finals: ""
  property string partial: ""
  onActiveChanged: if (active) keep()
  Connections {
    target: view.bandeau
    function onDictFinalsChanged() { if (view.active) view.keep() }
    function onDictPartialChanged() { if (view.active) view.keep() }
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
  readonly property int iconBox: 22
  readonly property int gap: 12

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
  readonly property bool multiLine: label.lineCount > 1

  width: textWidth + iconBox + gap + 2 * padX
  height: clipper.height + 2 * padY

  property real space: active ? height + bandeau.gap : 0
  Behavior on space { NumberAnimation { duration: 340; easing.type: Easing.OutQuint } }
  readonly property bool live: opacity > 0.01

  opacity: active ? 1 : 0
  scale: active ? 1 : 0.94
  property real drop: active ? 0 : 14
  transform: Translate { y: view.drop }
  Behavior on opacity { NumberAnimation { duration: view.active ? 260 : 200; easing.type: view.active ? Easing.OutCubic : Easing.InCubic } }
  Behavior on scale { NumberAnimation { duration: view.active ? 340 : 220; easing.type: view.active ? Easing.OutBack : Easing.InCubic; easing.overshoot: 1.2 } }
  Behavior on drop { NumberAnimation { duration: view.active ? 340 : 220; easing.type: view.active ? Easing.OutQuint : Easing.InCubic } }
  visible: live

  BandeauSurface {
    anchors.fill: parent
    bandeau: view.bandeau
    radius: view.multiLine ? view.bandeau.blockRadius : height / 2
  }

  Item {
    x: view.padX
    y: view.padY
    width: view.iconBox
    height: view.lineHeight
    Glyph {
      anchors.centerIn: parent
      glyph: "󰍬"
      color: view.bandeau.colorAccent
      pixelSize: 17
      fontFamily: view.bandeau.fontFamily
    }
  }

  Item {
    id: clipper
    x: view.padX + view.iconBox + view.gap
    y: view.padY
    width: view.textWidth
    height: Math.min(label.implicitHeight, view.lineHeight * view.lines)
    clip: true
    Text {
      id: label
      anchors.bottom: parent.bottom
      width: parent.width
      text: view.styled
      textFormat: Text.StyledText
      wrapMode: Text.WordWrap
      color: view.bandeau.colorText
      font: oneLine.font
    }
  }
}
