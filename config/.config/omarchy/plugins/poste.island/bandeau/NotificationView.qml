import QtQuick
import QtQuick.Effects
import "../components"

// Les notifications du bandeau, sous le texte dicté et l'OSD : la plus récente en haut (au
// plus près de l'île), les autres descendent. Trois au plus à la fois sur un écran ; les plus
// anciennes attendent, cachées et sans décompte, qu'une place se libère.
//
// Le service (voir Bandeau.qml) donne le modèle, l'écran de chaque toast et, par present(row),
// ce que montre la carte : { title, body, icon (URL d'image ou ""), crop (image à recadrer),
// glyph, tile (fond de tuile d'une marque ou ""), ink (couleur du glyphe sur cette tuile),
// critical }.
//
// La vue garde sa propre liste des cartes affichées (`cards`), tenue à jour depuis le modèle
// du service : une carte partie y reste le temps de sa sortie, puis la vue la retire. Les
// cartes sont posées par une Column ; chacune y prend une place (`slot`) dont la hauteur est
// animée. La place s'ouvre avant que la carte apparaisse, suit sa hauteur quand le contenu
// change, et se referme une fois la carte effacée : deux cartes ne se chevauchent jamais, même
// au milieu d'une animation.
Item {
  id: view
  required property var bandeau
  required property string screenName

  readonly property var source: bandeau.source
  readonly property int cardWidth: 400
  width: cardWidth

  // La fenêtre reste là tant qu'une carte est affichée ou en train de partir.
  readonly property bool live: cards.count > 0
  // Hauteur occupée par les cartes, depuis le haut de la vue (zone qui prend la souris).
  readonly property real cardsHeight: stack.height

  // ---------------------------------------------------------------- la liste affichée
  // Du haut (la plus récente) vers le bas (la plus ancienne), comme la Column les pose.
  ListModel { id: cards }
  property int nextUid: 1

  // Ce qui doit être affiché : les lignes de cet écran, de la plus récente à la plus ancienne,
  // trois au plus.
  function wanted() {
    var src = source
    var out = []
    if (!src || !src.popupModel) return out
    var m = src.popupModel
    for (var i = 0; i < m.count && out.length < bandeau.maxNotifications; i++) {
      var row = m.get(i)
      var key = src.popupKey(row)
      if (src.popupOnScreen(key, screenName)) out.push({ key: key, row: row })
    }
    return out
  }
  // Ce que la carte montre d'une ligne : tout est copié, la carte ne relit plus le service.
  function lookOf(row) {
    var look = source.present(row) || {}
    return {
      title: String(look.title || "Notification"),
      body: String(look.body || ""),
      icon: String(look.icon || ""),
      crop: !!look.crop,
      glyph: String(look.glyph || ""),
      tile: String(look.tile || ""),
      ink: String(look.ink || ""),
      critical: !!look.critical,
      lifetime: Number(source.durationFor(row.urgency, row.expireTimeout)) || 0
    }
  }
  function sync() {
    var want = wanted()
    var rank = {}
    for (var w = 0; w < want.length; w++) rank[want[w].key] = w
    // Les cartes qui n'ont plus de place partent ; les autres reprennent leur contenu.
    var present = {}
    for (var c = 0; c < cards.count; c++) {
      var e = cards.get(c)
      if (e.leaving) continue
      if (rank[e.entryKey] === undefined) {
        cards.setProperty(c, "leaving", true)
        continue
      }
      present[e.entryKey] = true
      var look = lookOf(want[rank[e.entryKey]].row)
      for (var name in look)
        if (e[name] !== look[name]) cards.setProperty(c, name, look[name])
    }
    // Les nouvelles entrent à leur rang : au-dessus de la première carte plus ancienne.
    for (var k = 0; k < want.length; k++) {
      if (present[want[k].key]) continue
      var at = cards.count
      for (var j = 0; j < cards.count; j++) {
        var other = cards.get(j)
        if (!other.leaving && rank[other.entryKey] !== undefined && rank[other.entryKey] > k) { at = j; break }
      }
      var entry = lookOf(want[k].row)
      entry.uid = nextUid++
      entry.entryKey = want[k].key
      entry.leaving = false
      cards.insert(at, entry)
    }
  }
  function drop(uid) {
    for (var c = 0; c < cards.count; c++)
      if (cards.get(c).uid === uid) { cards.remove(c); return }
  }
  // Un clic : la ligne est retrouvée par sa clé dans le modèle d'aujourd'hui.
  function act(entryKey, what) {
    var src = source
    if (!src || !src.popupModel) return
    var m = src.popupModel
    for (var i = 0; i < m.count; i++) {
      if (src.popupKey(m.get(i)) !== entryKey) continue
      if (what === "close") src.closePopup(i)
      else if (what === "invoke") src.invokePopupDefault(i)
      else src.expirePopup(i)
      return
    }
  }

  // Une mise à jour par tour de boucle, quel que soit le nombre de changements.
  property bool syncQueued: false
  function queueSync() {
    if (syncQueued) return
    syncQueued = true
    Qt.callLater(function() {
      view.syncQueued = false
      view.sync()
    })
  }
  onSourceChanged: queueSync()
  onScreenNameChanged: queueSync()
  Component.onCompleted: queueSync()
  Connections {
    target: view.source ? view.source.popupModel : null
    ignoreUnknownSignals: true
    function onCountChanged() { view.queueSync() }
    function onDataChanged() { view.queueSync() }
    function onRowsMoved() { view.queueSync() }
    function onModelReset() { view.queueSync() }
  }
  Connections {
    target: view.source
    ignoreUnknownSignals: true
    function onPopupScreensRevChanged() { view.queueSync() }
  }
  // Un toast sans écran connu va sur l'écran actif.
  Connections {
    target: view.bandeau
    function onFocusedScreenChanged() { view.queueSync() }
  }

  // ---------------------------------------------------------------- la pile
  Column {
    id: stack
    anchors.top: parent.top
    anchors.horizontalCenter: parent.horizontalCenter
    width: view.cardWidth

    Repeater {
      model: cards

      delegate: Item {
        id: slot
        required property int uid
        required property string entryKey
        required property bool leaving
        required property string title
        required property string body
        required property string icon
        required property bool crop
        required property string glyph
        required property string tile
        required property string ink
        required property bool critical
        required property double lifetime

        width: stack.width
        // La place de la carte et de l'écart en dessous, à la proportion de son ouverture.
        property real openness: 0
        property real collapse: 0
        height: (card.height + view.bandeau.gap) * openness * (1 - collapse)
        Behavior on openness { NumberAnimation { duration: 300; easing.type: Easing.OutQuint } }
        Component.onCompleted: Qt.callLater(function() { slot.openness = 1 })

        // Entrée : la carte apparaît dès que sa place peut la contenir (l'écart absorbe la fin
        // de l'ouverture). Sortie : la carte s'efface sur place, puis sa place se referme.
        property bool entered: false
        onHeightChanged: if (!entered && !leaving && openness > 0 && height >= card.height - 0.5) entered = true
        property real leave: 0
        onLeavingChanged: if (leaving) {
          life.stop()
          exit.start()
        }
        SequentialAnimation {
          id: exit
          NumberAnimation { target: slot; property: "leave"; to: 1; duration: 200; easing.type: Easing.InCubic }
          NumberAnimation { target: slot; property: "collapse"; to: 1; duration: 260; easing.type: Easing.InOutCubic }
          ScriptAction { script: view.drop(slot.uid) }
        }

        // Durée de vie : une animation sur toute la durée, mise en pause au survol, reprise du
        // début quand le contenu change.
        property real remaining: 1
        NumberAnimation {
          id: life
          target: slot
          property: "remaining"
          to: 0
          duration: Math.max(1, slot.lifetime)
          paused: running && hover.hovered
          onFinished: if (!slot.leaving && slot.remaining <= 0.001) view.act(slot.entryKey, "expire")
        }
        function restartLife() {
          life.stop()
          remaining = 1
          if (entered && !leaving && lifetime > 0) {
            life.from = 1
            life.start()
          }
        }
        onEnteredChanged: restartLife()
        onTitleChanged: restartLife()
        onBodyChanged: restartLife()
        onIconChanged: restartLife()
        onLifetimeChanged: restartLife()

        Item {
          id: card
          width: parent.width
          anchors.top: parent.top
          // Hauteur finale (le contenu y est posé tout de suite), puis hauteur montrée, animée
          // quand le contenu change ; la place suit, les cartes du dessous descendent avec.
          readonly property real fullHeight: Math.max(52, content.implicitHeight + 2 * 12)
          height: fullHeight
          Behavior on height {
            enabled: slot.entered && !slot.leaving
            NumberAnimation { duration: 260; easing.type: Easing.OutCubic }
          }

          property real fall: slot.entered ? 0 : -view.bandeau.gap
          Behavior on fall { NumberAnimation { duration: 360; easing.type: Easing.OutQuint } }
          transform: Translate { y: card.fall - slot.leave * 8 }
          opacity: (slot.entered ? 1 : 0) * (1 - slot.leave)
          Behavior on opacity { enabled: slot.leave === 0; NumberAnimation { duration: 260; easing.type: Easing.OutCubic } }
          scale: (slot.entered ? (hover.hovered && !slot.leaving ? 1.012 : 1) : 0.94) - slot.leave * 0.05
          Behavior on scale { enabled: slot.leave === 0; NumberAnimation { duration: 340; easing.type: Easing.OutBack; easing.overshoot: 1.2 } }

          readonly property bool oneLine: slot.body === ""

          HoverHandler { id: hover }

          BandeauSurface {
            id: surface
            anchors.fill: parent
            bandeau: view.bandeau
            radius: card.oneLine ? height / 2 : view.bandeau.blockRadius
          }

          MouseArea {
            anchors.fill: parent
            enabled: !slot.leaving
            acceptedButtons: Qt.LeftButton | Qt.RightButton
            cursorShape: Qt.PointingHandCursor
            onClicked: function(mouse) { view.act(slot.entryKey, mouse.button === Qt.RightButton ? "close" : "invoke") }
          }

          // Le contenu, à sa hauteur finale, coupé à la hauteur montrée.
          Item {
            anchors.fill: parent
            clip: card.height < card.fullHeight - 0.5

            Item {
              anchors.left: parent.left
              anchors.right: parent.right
              anchors.top: parent.top
              height: card.fullHeight

              Row {
                id: content
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.leftMargin: 11
                anchors.rightMargin: 40
                anchors.verticalCenter: parent.verticalCenter
                spacing: 12

                // L'icône : l'image ou l'icône de l'application, sinon le glyphe à l'accent (ou
                // la tuile de la marque, Claude Code ou Codex).
                Item {
                  id: tile
                  width: 32; height: 32
                  anchors.verticalCenter: parent.verticalCenter
                  readonly property bool hasImage: tileImage.status === Image.Ready
                  readonly property bool branded: slot.tile !== ""
                  Rectangle {
                    anchors.fill: parent
                    radius: card.oneLine ? width / 2 : 10
                    visible: tile.branded || !tile.hasImage
                    color: tile.branded ? slot.tile : view.bandeau.withAlpha(view.bandeau.colorAccent, 0.14)
                  }
                  Image {
                    id: tileImage
                    anchors.fill: parent
                    source: tile.branded ? "" : slot.icon
                    sourceSize.width: 64
                    sourceSize.height: 64
                    fillMode: slot.crop ? Image.PreserveAspectCrop : Image.PreserveAspectFit
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
                    glyph: slot.glyph || "󰂚"
                    color: tile.branded ? slot.ink : view.bandeau.colorAccent
                    pixelSize: tile.branded ? 20 : 17
                    fontFamily: view.bandeau.fontFamily
                  }
                }

                Column {
                  id: texts
                  anchors.verticalCenter: parent.verticalCenter
                  width: content.width - tile.width - content.spacing
                  spacing: 2
                  Row {
                    width: parent.width
                    spacing: 7
                    // Critique : un point rouge discret devant le titre.
                    Rectangle {
                      visible: slot.critical
                      anchors.verticalCenter: parent.verticalCenter
                      width: 7; height: 7; radius: 3.5
                      color: view.bandeau.colorUrgent
                    }
                    Text {
                      width: parent.width - (slot.critical ? 14 : 0)
                      text: slot.title
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
                    text: slot.body
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
            }
          }

          // Fermer, au survol.
          Rectangle {
            anchors.right: parent.right
            anchors.rightMargin: 11
            anchors.verticalCenter: parent.verticalCenter
            width: 22; height: 22; radius: 11
            color: view.bandeau.withAlpha(view.bandeau.colorText, closeMouse.containsMouse ? 0.16 : 0.07)
            opacity: hover.hovered && !slot.leaving ? 1 : 0
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
              enabled: !slot.leaving
              hoverEnabled: true
              cursorShape: Qt.PointingHandCursor
              onClicked: view.act(slot.entryKey, "close")
            }
          }
        }
      }
    }
  }
}
