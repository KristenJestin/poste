import QtQuick
import "../components"

// L'OSD du bandeau (volume, luminosité, micro, musique, alimentation) : une pilule, l'icône à
// l'accent, puis la barre et sa valeur, ou le message (« Titre - Artiste » pour la musique).
// Une seule place : un OSD qui en remplace un autre change sur place, la pilule s'élargit ou
// se resserre (le contenu, posé à sa largeur finale, se découvre avec elle) et la barre glisse
// vers la nouvelle valeur.
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

  // Ce que la pièce prend dans la pile (pour celles du dessus), animé.
  property real space: active ? height + bandeau.gap : 0
  Behavior on space { NumberAnimation { duration: 340; easing.type: Easing.OutQuint } }
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

  // Entrée : monte d'un peu plus bas, grandit un peu, apparaît. Sortie : s'efface, rapetisse,
  // descend un peu.
  opacity: active ? 1 : 0
  scale: active ? 1 : 0.94
  property real drop: active ? 0 : 14
  transform: Translate { y: view.drop }
  Behavior on opacity { NumberAnimation { duration: view.active ? 260 : 200; easing.type: view.active ? Easing.OutCubic : Easing.InCubic } }
  Behavior on scale { NumberAnimation { duration: view.active ? 340 : 220; easing.type: view.active ? Easing.OutBack : Easing.InCubic; easing.overshoot: 1.2 } }
  Behavior on drop { NumberAnimation { duration: view.active ? 340 : 220; easing.type: view.active ? Easing.OutQuint : Easing.InCubic } }
  visible: live

  // La largeur ne s'anime que d'un OSD à l'autre, pas à l'ouverture.
  property real shownWidth: width
  Behavior on shownWidth {
    enabled: view.active && view.opacity > 0.99
    NumberAnimation { duration: 260; easing.type: Easing.OutQuint }
  }

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
          color: view.bandeau.colorAccent
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
