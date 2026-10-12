import QtQuick
import QtQuick.Effects
import "../components"

// Les notifications du bandeau, empilées au-dessus de l'OSD : la plus récente en bas, les
// autres montent. Trois au plus à la fois sur un écran ; les plus anciennes attendent, cachées
// et sans décompte, qu'une place se libère.
//
// Le service (voir Bandeau.qml) donne le modèle, l'écran de chaque toast et, par present(row),
// ce que montre la carte : { title, body, icon (URL d'image ou ""), crop (image à recadrer),
// glyph, tile (fond de tuile d'une marque ou ""), ink (couleur du glyphe sur cette tuile),
// critical }.
//
// Animations : l'entrée se fait dans chaque carte (`entered`, posé par Qt.callLater) et la vue
// n'anime que la sortie et le glissement de la pile. Une transition `add` de ListView
// interrompue par un `displaced` laisse des cartes à moitié transparentes.
Item {
  id: view
  required property var bandeau
  required property string screenName

  readonly property var source: bandeau.source
  readonly property int cardWidth: 400
  width: cardWidth

  // Rang d'une ligne parmi celles de cet écran (0 = la plus récente).
  function rankOf(index) {
    var src = source
    if (!src) return 0
    src.popupScreensRev
    var m = src.popupModel
    var rank = 0
    for (var k = 0; k < index && k < m.count; k++)
      if (src.popupOnScreen(src.popupKey(m.get(k)), screenName)) rank++
    return rank
  }

  readonly property int shownCount: {
    var src = source
    if (!src) return 0
    src.popupScreensRev
    var m = src.popupModel
    var n = 0
    for (var k = 0; k < m.count; k++)
      if (src.popupOnScreen(src.popupKey(m.get(k)), screenName)) n++
    return Math.min(n, bandeau.maxNotifications)
  }
  // La fenêtre reste là le temps que la dernière carte s'en aille.
  readonly property bool live: shownCount > 0 || linger.running
  onShownCountChanged: if (shownCount === 0) linger.restart()
  Timer { id: linger; interval: 400 }

  // Hauteur occupée par les cartes, depuis le bas de la vue (zone qui prend la souris).
  readonly property real cardsHeight: Math.min(height, list.contentHeight)

  ListView {
    id: list
    anchors.fill: parent
    verticalLayoutDirection: ListView.BottomToTop
    interactive: false
    spacing: 0
    model: view.source ? view.source.popupModel : null

    remove: Transition {
      NumberAnimation { property: "leave"; to: 1; duration: 240; easing.type: Easing.InCubic }
    }
    displaced: Transition {
      NumberAnimation { property: "y"; duration: 340; easing.type: Easing.OutQuint }
    }

    delegate: Item {
      id: slot
      required property int index
      required property double originalId
      required property double timestamp
      required property string app
      required property string appIcon
      required property string summary
      required property string body
      required property string image
      required property string glyph
      required property int urgency
      required property double expireTimeout

      readonly property var src: view.source
      readonly property bool here: {
        if (!src) return false
        src.popupScreensRev
        return src.popupOnScreen(src.popupKey(slot), view.screenName)
      }
      readonly property bool shown: here && view.rankOf(index) < view.bandeau.maxNotifications
      readonly property var look: src ? src.present(slot) : ({})

      // Retirée du modèle, la carte garde sa place le temps de sa sortie.
      property bool removing: false
      ListView.onRemove: removing = true
      readonly property bool held: shown || removing

      width: list.width
      height: held ? card.height + view.bandeau.gap : 0
      Behavior on height {
        enabled: slot.entered
        NumberAnimation { duration: 300; easing.type: Easing.OutQuint }
      }
      visible: held || height > 0.5

      // Durée de vie : un trait fin en bas de la carte ; le survol la met en pause.
      readonly property real lifetime: src ? src.durationFor(urgency, expireTimeout) : 0
      property real remaining: 1.0
      readonly property bool ticking: shown && lifetime > 0 && !hover.hovered && !removing
      onSummaryChanged: remaining = 1.0
      onBodyChanged: remaining = 1.0
      onImageChanged: remaining = 1.0
      Timer {
        interval: 50
        repeat: true
        running: slot.ticking
        onTriggered: {
          if (slot.lifetime <= 0) return
          slot.remaining -= 50.0 / slot.lifetime
          if (slot.remaining <= 0) {
            slot.remaining = 0
            slot.src.expirePopup(slot.index)
          }
        }
      }

      // Entrée (dans la carte) et sortie (transition `remove`, qui pousse `leave` à 1).
      property bool entered: false
      property real leave: 0
      Component.onCompleted: Qt.callLater(function() { slot.entered = true })
      // Une carte qui attendait sa place entre comme une nouvelle.
      onShownChanged: if (shown && entered) {
        entered = false
        Qt.callLater(function() { slot.entered = true })
      }

      Item {
        id: card
        width: view.cardWidth
        height: Math.max(52, content.implicitHeight + 2 * 11)
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom

        property real rise: slot.entered ? 0 : 16
        Behavior on rise { NumberAnimation { duration: 360; easing.type: Easing.OutQuint } }
        transform: Translate { y: card.rise + slot.leave * 12 }
        opacity: (slot.entered && slot.held ? 1 : 0) * (1 - slot.leave)
        Behavior on opacity { enabled: slot.leave === 0; NumberAnimation { duration: 260; easing.type: Easing.OutCubic } }
        scale: (slot.entered ? (hover.hovered ? 1.012 : 1) : 0.94) - slot.leave * 0.05
        Behavior on scale { enabled: slot.leave === 0; NumberAnimation { duration: 340; easing.type: Easing.OutBack; easing.overshoot: 1.2 } }

        readonly property bool oneLine: (slot.look.body || "") === ""
        readonly property bool critical: !!slot.look.critical

        HoverHandler { id: hover }

        BandeauSurface {
          id: surface
          anchors.fill: parent
          bandeau: view.bandeau
          radius: card.oneLine ? height / 2 : view.bandeau.blockRadius
        }

        MouseArea {
          anchors.fill: parent
          acceptedButtons: Qt.LeftButton | Qt.RightButton
          cursorShape: Qt.PointingHandCursor
          onClicked: function(mouse) {
            if (mouse.button === Qt.RightButton) slot.src.closePopup(slot.index)
            else slot.src.invokePopupDefault(slot.index)
          }
        }

        Row {
          id: content
          anchors.left: parent.left
          anchors.right: parent.right
          anchors.leftMargin: 11
          anchors.rightMargin: 40
          anchors.verticalCenter: parent.verticalCenter
          spacing: 12

          // L'icône : l'image ou l'icône de l'application, sinon le glyphe à l'accent (ou la
          // tuile de la marque, Claude Code ou Codex).
          Item {
            id: tile
            width: 32; height: 32
            anchors.verticalCenter: parent.verticalCenter
            readonly property bool hasImage: tileImage.status === Image.Ready
            readonly property bool branded: (slot.look.tile || "") !== ""
            Rectangle {
              anchors.fill: parent
              radius: card.oneLine ? width / 2 : 10
              visible: tile.branded || !tile.hasImage
              color: tile.branded ? slot.look.tile : view.bandeau.withAlpha(view.bandeau.colorAccent, 0.14)
            }
            Image {
              id: tileImage
              anchors.fill: parent
              source: tile.branded ? "" : (slot.look.icon || "")
              sourceSize.width: 64
              sourceSize.height: 64
              fillMode: slot.look.crop ? Image.PreserveAspectCrop : Image.PreserveAspectFit
              asynchronous: true
              visible: false
            }
            MultiEffect {
              anchors.fill: parent
              source: tileImage
              visible: !tile.branded && tile.hasImage
              maskEnabled: true
              maskSource: tileMask
            }
            Item {
              id: tileMask
              anchors.fill: parent
              layer.enabled: true
              visible: false
              Rectangle { anchors.fill: parent; radius: card.oneLine ? width / 2 : 10 }
            }
            Glyph {
              anchors.centerIn: parent
              visible: tile.branded || !tile.hasImage
              glyph: slot.look.glyph || "󰂚"
              color: tile.branded ? slot.look.ink : view.bandeau.colorAccent
              pixelSize: tile.branded ? 20 : 17
              fontFamily: view.bandeau.fontFamily
            }
          }

          Column {
            anchors.verticalCenter: parent.verticalCenter
            width: content.width - tile.width - content.spacing
            spacing: 2
            Row {
              width: parent.width
              spacing: 7
              // Critique : un point rouge discret devant le titre.
              Rectangle {
                visible: card.critical
                anchors.verticalCenter: parent.verticalCenter
                width: 7; height: 7; radius: 3.5
                color: view.bandeau.colorUrgent
              }
              Text {
                width: parent.width - (card.critical ? 14 : 0)
                text: slot.look.title || "Notification"
                textFormat: Text.PlainText
                elide: Text.ElideRight
                maximumLineCount: 1
                color: view.bandeau.colorText
                font.family: view.bandeau.fontFamily
                font.pixelSize: 14
                font.bold: true
              }
            }
            Text {
              width: parent.width
              visible: text !== ""
              text: slot.look.body || ""
              textFormat: Text.PlainText
              wrapMode: Text.Wrap
              maximumLineCount: 2
              elide: Text.ElideRight
              color: view.bandeau.colorMuted
              font.family: view.bandeau.fontFamily
              font.pixelSize: 13
            }
          }
        }

        // Fermer, au survol.
        Rectangle {
          anchors.right: parent.right
          anchors.rightMargin: 11
          anchors.verticalCenter: parent.verticalCenter
          width: 22; height: 22; radius: 11
          color: view.bandeau.withAlpha(view.bandeau.colorText, closeMouse.containsMouse ? 0.16 : 0.07)
          opacity: hover.hovered ? 1 : 0
          Behavior on opacity { NumberAnimation { duration: 150 } }
          Glyph {
            anchors.centerIn: parent
            glyph: "󰅖"
            color: view.bandeau.colorText
            pixelSize: 12
            fontFamily: view.bandeau.fontFamily
          }
          MouseArea {
            id: closeMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: slot.src.closePopup(slot.index)
          }
        }

        // Durée de vie restante.
        Rectangle {
          visible: slot.lifetime > 0
          anchors.bottom: parent.bottom
          anchors.bottomMargin: 2
          x: surface.radius > 0 ? Math.min(surface.radius, card.width / 4) : 0
          height: 2
          radius: 1
          width: (card.width - 2 * x) * Math.max(0, Math.min(1, slot.remaining))
          color: view.bandeau.withAlpha(view.bandeau.colorText, 0.2)
        }
      }
    }
  }
}
