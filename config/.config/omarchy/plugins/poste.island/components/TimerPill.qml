import QtQuick
import QtQuick.Effects

// La petite île d'un minuteur, en haut à gauche de l'écran : son fond orange
// (accent) se vide vers la gauche à mesure que le temps passe. Le texte est
// sombre sur l'orange et clair sur le noir (deux couches, découpées à la
// limite de la barre). Clic : déplie les commandes (+1 min, pause / reprise,
// annuler) ; clic droit : pause. Terminé : pastille orange qui pulse, son en
// boucle (Island.qml) ; un clic la ferme et coupe le son.
Rectangle {
  id: pill
  required property var host
  required property var timer
  property bool expanded: false
  // Compacte (place manquante, voir IslandWindow) : le temps seul, sans libellé.
  property bool compact: false

  readonly property double remaining: host.timerLeft(timer)
  readonly property real progress: timer.duration > 0 ? Math.max(0, Math.min(1, remaining / timer.duration)) : 0
  readonly property bool done: timer.done === true
  readonly property bool paused: timer.paused === true && !done
  readonly property real fillWidth: done ? width : width * progress

  readonly property color light: host.colorText
  readonly property color dark: host.colorAccentText   // lisible sur l'accent, quel qu'il soit

  height: host.restHeight
  width: content.implicitWidth + 22
  radius: height / 2
  color: host.colorIsland
  Behavior on width { NumberAnimation { duration: 260 * host.motionScale; easing.type: Easing.OutQuint } }

  component PillButton: Rectangle {
    id: btn
    property string glyph: ""
    property string label: ""
    property color ink: "white"
    property bool live: false
    signal activated()
    width: Math.max(24, btnText.implicitWidth + 12); height: 24; radius: 12
    color: Qt.rgba(ink.r, ink.g, ink.b, btnMouse.containsMouse ? 0.18 : 0.10)
    Text {
      id: btnText
      anchors.centerIn: parent
      text: btn.label !== "" ? btn.label : btn.glyph
      color: btn.ink
      font.family: btn.label !== "" ? pill.host.uiFont : pill.host.fontFamily
      font.pixelSize: btn.label !== "" ? 11 : 12
      font.weight: Font.DemiBold
    }
    MouseArea {
      id: btnMouse
      anchors.fill: parent
      enabled: btn.live
      hoverEnabled: true
      cursorShape: Qt.PointingHandCursor
      onClicked: btn.activated()
    }
  }

  // Contenu : temps, libellé, commandes. Dessiné deux fois (clair / sombre).
  component PillContent: Row {
    id: rowContent
    property color ink: "white"
    property bool live: false      // la copie sombre n'est pas cliquable
    spacing: 8
    Text {
      anchors.verticalCenter: parent.verticalCenter
      text: pill.done ? "Done" : pill.host.formatLeft(pill.remaining)
      color: rowContent.ink
      opacity: pill.paused ? 0.6 : 1
      font.family: pill.host.uiFont
      font.pixelSize: 13
      font.weight: Font.Bold
      font.features: { "tnum": 1 }
    }
    Text {
      anchors.verticalCenter: parent.verticalCenter
      visible: text !== "" && !pill.compact
      width: Math.min(implicitWidth, 140)
      elide: Text.ElideRight
      text: pill.timer.label || ""
      color: rowContent.ink
      opacity: 0.85
      font.family: pill.host.uiFont
      font.pixelSize: 12
      font.weight: Font.DemiBold
    }
    Row {
      anchors.verticalCenter: parent.verticalCenter
      spacing: 2
      visible: pill.expanded && !pill.done
      PillButton { ink: rowContent.ink; live: rowContent.live; label: "+1m"; onActivated: pill.host.addToTimer(pill.timer.id, 60000) }
      PillButton { ink: rowContent.ink; live: rowContent.live; glyph: pill.paused ? "󰐊" : "󰏤"; onActivated: pill.host.togglePause(pill.timer.id) }
      PillButton { ink: rowContent.ink; live: rowContent.live; glyph: "󰅖"; onActivated: pill.host.cancelTimer(pill.timer.id) }
    }
  }

  MouseArea {
    anchors.fill: parent
    hoverEnabled: true
    cursorShape: Qt.PointingHandCursor
    acceptedButtons: Qt.LeftButton | Qt.RightButton
    onClicked: function(mouse) {
      if (pill.done) { host.cancelTimer(timer.id); return }
      if (mouse.button === Qt.RightButton) { host.togglePause(timer.id); return }
      pill.expanded = !pill.expanded
    }
  }

  // Couche claire (sur le noir), avec les commandes cliquables.
  PillContent {
    id: content
    anchors.centerIn: parent
    ink: pill.light
    live: true
  }

  // Barre orange + couche sombre, découpées à la forme de la pastille.
  Item {
    id: fillLayer
    anchors.fill: parent
    layer.enabled: true
    layer.effect: MultiEffect {
      maskEnabled: true
      maskSource: fillMask
      maskThresholdMin: 0.5
      maskSpreadAtMin: 1.0
    }
    Item {
      width: pill.fillWidth
      height: parent.height
      clip: true
      Behavior on width { NumberAnimation { duration: 250 } }
      Rectangle {
        width: pill.width
        height: pill.height
        color: pill.paused ? Qt.darker(pill.host.colorAccent, 1.9) : pill.host.colorAccent
        Behavior on color { ColorAnimation { duration: 200 } }
      }
      PillContent {
        x: content.x
        y: content.y
        ink: pill.paused ? pill.light : pill.dark
      }
    }
  }
  Item {
    id: fillMask
    anchors.fill: parent
    visible: false
    layer.enabled: true
    Rectangle { anchors.fill: parent; radius: pill.radius; color: "black" }
  }

  // Terminé : respiration (voile clair) jusqu'au clic.
  Rectangle {
    anchors.fill: parent
    radius: parent.radius
    color: "white"
    opacity: 0
    SequentialAnimation on opacity {
      running: pill.done
      loops: Animation.Infinite
      NumberAnimation { to: 0.28; duration: 520; easing.type: Easing.InOutSine }
      NumberAnimation { to: 0; duration: 520; easing.type: Easing.InOutSine }
    }
  }
}
