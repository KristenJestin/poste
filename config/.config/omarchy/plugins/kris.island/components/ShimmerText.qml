import QtQuick
import QtQuick.Effects

// Libellé traversé par un reflet (Shimmer.qml) : le texte, en gris, sert de
// masque à une bande claire qui le balaie.
Item {
  id: root
  required property var host
  property string text: ""
  property real sweepX: 0
  property real band: 120

  implicitWidth: label.implicitWidth
  implicitHeight: label.implicitHeight

  Text {
    id: label
    text: root.text
    color: root.host.colorMuted
    font.family: root.host.uiFont
    font.pixelSize: 12
  }
  Item {
    id: mask
    anchors.fill: label
    visible: false
    layer.enabled: true
    Text {
      text: root.text
      color: "black"
      font: label.font
    }
  }
  Item {
    anchors.fill: label
    layer.enabled: true
    layer.effect: MultiEffect {
      maskEnabled: true
      maskSource: mask
      maskThresholdMin: 0.3
      maskSpreadAtMin: 0.4
    }
    Rectangle {
      x: root.sweepX
      width: root.band
      height: parent.height
      gradient: Gradient {
        orientation: Gradient.Horizontal
        GradientStop { position: 0; color: "transparent" }
        GradientStop { position: 0.5; color: root.host.colorText }
        GradientStop { position: 1; color: "transparent" }
      }
    }
  }
}
