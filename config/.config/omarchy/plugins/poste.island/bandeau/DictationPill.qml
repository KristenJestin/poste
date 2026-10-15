import QtQuick
import "../components"

// La pastille de la dictée, seule en bas de l'écran actif, pareille pour toutes les sources
// (SUPER+A Vocaline ou voxtype, ou toute source du slot « dictation » de l'île) : le micro
// gris clair, immobile, puis l'onde de la voix (14 barres, la plus récente à droite, depuis
// l'historique des niveaux tenu par le bandeau) pendant l'écoute, ou trois points qui pulsent
// pendant la transcription. L'aspect de l'île : son fond plein, sa hauteur, arrondi en pilule.
// Elle entre en grandissant depuis le bas et sort en se resserrant ; le passage de l'onde aux
// points resserre sa largeur en douceur.
Item {
  id: pill
  required property var bandeau
  property bool active: false

  // Les points à la place de l'onde. Gardé tel quel pendant le fondu de sortie.
  property bool transcribing: false
  function keep() { transcribing = bandeau.dictTranscribing && !bandeau.dictListening }
  onActiveChanged: if (active) keep()
  Connections {
    target: pill.bandeau
    function onDictListeningChanged() { if (pill.active) pill.keep() }
    function onDictTranscribingChanged() { if (pill.active) pill.keep() }
  }

  readonly property int bars: bandeau.dictBars
  readonly property real barWidth: 2.5
  readonly property real waveMax: 18

  height: 34
  width: row.implicitWidth + 28
  Behavior on width { enabled: pill.live; NumberAnimation { duration: 260; easing.type: Easing.OutQuint } }

  readonly property bool live: opacity > 0.01
  visible: live
  opacity: active ? 1 : 0
  scale: active ? 1 : 0.7
  transformOrigin: Item.Bottom
  Behavior on opacity { NumberAnimation { duration: pill.active ? 180 : 160; easing.type: pill.active ? Easing.OutCubic : Easing.InCubic } }
  Behavior on scale { NumberAnimation { duration: pill.active ? 360 : 180; easing.type: pill.active ? Easing.OutBack : Easing.InCubic; easing.overshoot: 1.3 } }

  BandeauSurface {
    anchors.fill: parent
    bandeau: pill.bandeau
  }

  Row {
    id: row
    x: 14
    anchors.verticalCenter: parent.verticalCenter
    spacing: 10

    Glyph {
      anchors.verticalCenter: parent.verticalCenter
      glyph: "󰍬"
      color: pill.bandeau.withAlpha(pill.bandeau.colorText, 0.75)
      fontFamily: pill.bandeau.fontFamily
      pixelSize: 15
    }

    // L'onde : une barre par niveau récent, la plus récente à droite.
    Row {
      anchors.verticalCenter: parent.verticalCenter
      visible: !pill.transcribing
      spacing: 2
      Repeater {
        model: pill.bars
        delegate: Rectangle {
          required property int index
          readonly property int back: pill.bars - 1 - index
          readonly property var levels: pill.bandeau.dictLevels
          readonly property real level: back < levels.length ? levels[levels.length - 1 - back] : 0
          anchors.verticalCenter: parent.verticalCenter
          width: pill.barWidth
          height: 3 + (pill.waveMax - 3) * level
          radius: pill.barWidth / 2
          color: pill.bandeau.colorAccent
          opacity: 0.45 + 0.55 * (index / pill.bars)
          Behavior on height { NumberAnimation { duration: 90; easing.type: Easing.OutQuad } }
        }
      }
    }

    // La transcription : trois points en vague.
    Row {
      anchors.verticalCenter: parent.verticalCenter
      visible: pill.transcribing
      spacing: 4
      Repeater {
        model: 3
        delegate: Rectangle {
          id: dot
          required property int index
          width: 5; height: 5; radius: 2.5
          color: pill.bandeau.colorText
          opacity: 0.35
          SequentialAnimation on opacity {
            running: pill.transcribing && pill.live
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
}
