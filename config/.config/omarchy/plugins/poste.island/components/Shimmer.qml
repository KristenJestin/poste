import QtQuick

// Chargement commun de l'île : des lignes fantômes à la place du contenu
// attendu, balayées par un reflet, et au-dessus un libellé (ce qui se passe)
// traversé par le même reflet. Sert à l'IA (en attendant la réponse) et à la
// vue Usage (en lisant les journaux).
//   lines : largeur de chaque ligne fantôme (fraction de la largeur)
//   blocks : lignes plus hautes (titre, gros chiffre), mêmes règles
Item {
  id: shimmer
  required property var host
  property string label: ""
  property var lines: [0.92, 0.78, 0.55]
  property real lineHeight: 9
  property real spacing: 9
  property bool running: visible

  // 0 → 1 : position du reflet, commune à toutes les lignes et au libellé.
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
  // Abscisse du reflet dans le repère du shimmer (part de la gauche, hors champ).
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
