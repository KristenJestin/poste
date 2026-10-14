import QtQuick

// Les activités en cours (façon Live Activities de l'iPhone), collées à
// droite de l'île : la plus importante en capsule (une info à gauche, une à
// droite), les suivantes en ronds, le surplus dans « +N ». Survol (un
// instant) ou clic : la carte dépliée dessous (ActivityCard.qml).
//
// Ordre : batterie (événement) ; minuteur terminé ; réponse IA prête ;
// minuteurs en cours (le plus proche de la fin d'abord) ; IA au travail ;
// IA épinglées ; minuteurs en pause.
Item {
  id: strip
  required property var host
  required property real roomRight     // largeur libre pour la bande (de son côté de l'île)
  required property real screenWidth
  // Les activités de cette bande (vide : toutes). À droite de l'île, ce qui
  // bouge (minuteurs, IA, dictée) ; à gauche, ce qui reste (musique, batterie).
  property var kinds: []
  // "right" : la bande part de l'île vers la droite ; "left" : vers la gauche
  // (la plus importante reste collée à l'île).
  property string side: "right"
  property bool hidden: false
  signal openConversation(var conv)
  signal openPlayer()

  readonly property real h: host.restHeight
  height: h
  width: row.width
  opacity: hidden ? 0 : 1
  visible: opacity > 0.01
  Behavior on opacity { NumberAnimation { duration: 200 } }

  // ------------------------------------------------ les activités
  readonly property var activities: {
    host.ai.stateRevision
    var out = []
    // OBS : enregistrement ou diffusion en cours, toujours en tête.
    if (host.obs && host.obs.active) out.push({ kind: "obs", key: "obs", rank: -2, order: 0 })
    // Musique : pendant la lecture, et 30 s après une pause (puis elle part).
    var p = host.player
    if (p && String(p.trackTitle || "") !== "" && (host.mediaPlaying || musicLinger))
      out.push({ kind: "music", key: "music", rank: host.mediaPlaying ? 3.5 : 6.5, order: 0 })
    var b = host.battery
    // Batterie : seulement basse (sous 30 %) ou juste après un débranchement ;
    // sinon elle est dans le centre de contrôle.
    if (b && b.showing) out.push({ kind: "battery", key: "battery", rank: 0, order: 0 })
    // Périphériques sans fil sous 15 % (DevicesState.qml).
    var lows = host.devices ? host.devices.low : []
    for (var d = 0; d < lows.length; d++) out.push({ kind: "device", key: "device:" + lows[d].key, device: lows[d], rank: 0.5, order: d })
    var ts = host.allTimers || []
    for (var i = 0; i < ts.length; i++) {
      var t = ts[i]
      out.push({ kind: "timer", key: "timer:" + t.id, timer: t,
                 rank: t.done ? 1 : t.paused ? 6 : 3, order: t.paused ? i : t.end })
    }
    var cs = host.ai.docked || []
    for (var j = 0; j < cs.length; j++) {
      var c = cs[j]
      out.push({ kind: "ai", key: "ai:" + c.uid, conv: c,
                 rank: c.unseen && !c.busy ? 2 : c.busy ? 4 : 5, order: -c.uid })
    }
    if (kinds.length) out = out.filter(function(a) { return kinds.indexOf(a.kind) !== -1 })
    // Tri stable (le sort de Qt ne l'est pas).
    for (var k = 0; k < out.length; k++) out[k].index = k
    out.sort(function(a, b) { return (a.rank - b.rank) || (a.order - b.order) || (a.index - b.index) })
    return out
  }
  // Musique en pause : encore 30 s à l'écran, le temps de reprendre.
  property bool musicLinger: false
  Timer { id: musicLingerTimer; interval: 30000; onTriggered: strip.musicLinger = false }
  Connections {
    target: strip.host
    function onMediaPlayingChanged() {
      if (strip.host.mediaPlaying) { musicLingerTimer.stop(); strip.musicLinger = false }
      else { strip.musicLinger = true; musicLingerTimer.restart() }
    }
  }

  function findActivity(key) {
    for (var i = 0; i < activities.length; i++) if (activities[i].key === key) return activities[i]
    return null
  }

  // ------------------------------------------------ place
  readonly property real capsuleWidth: 170
  readonly property real circle: h + 8
  // Combien de bulles tiennent (la première en capsule si possible).
  readonly property bool capsuleFits: roomRight >= capsuleWidth
  // Largeur estimée d'une activité en capsule (depuis ses textes, pas depuis
  // la bulle, qui change de taille).
  function capsuleWidthOf(a) {
    var t = ""
    if (a.kind === "timer") t = a.timer.done ? "Done" : host.formatLeft(a.timer.duration)
    else if (a.kind === "battery" && host.battery.profileGlyph !== "") t = "100 %   "
    else if (a.kind === "battery" || a.kind === "device") t = "100 %"
    else if (a.kind === "music") return 96
    else if (a.kind === "obs") return 120
    else if (a.conv) t = a.conv.busy ? "Searching the web…" : a.conv.unseen ? "Ready" : a.conv.question
    return 56 + Math.min(150, String(t).length * 7.6)
  }
  // Peu d'activités : toutes en capsule. Sinon la première en capsule, les
  // autres en ronds, le surplus dans « +N ».
  readonly property bool allCapsules: {
    var w = 0
    for (var i = 0; i < activities.length; i++) w += capsuleWidthOf(activities[i]) + (i > 0 ? 8 : 0)
    return w <= roomRight
  }
  readonly property int maxShown: allCapsules ? activities.length
    : Math.max(1, Math.floor((roomRight - (capsuleFits ? capsuleWidth + 8 : 0)) / circle) + (capsuleFits ? 1 : 0))
  readonly property int hiddenCount: activities.length > maxShown ? activities.length - (maxShown - 1) : 0
  readonly property var shown: hiddenCount > 0 ? activities.slice(0, maxShown - 1) : activities
  readonly property var overflow: hiddenCount > 0 ? activities.slice(maxShown - 1) : []

  // ------------------------------------------------ dépliage
  // Clé de l'activité dépliée ; « pinned » : ouverte par un clic, reste
  // ouverte tant qu'on ne clique pas à nouveau ou qu'on ne la quitte pas.
  property string openKey: ""
  property bool openPinned: false
  property real openX: 0
  property string hoverKey: ""
  property real hoverX: 0
  readonly property var openActivity: openKey === "" ? null : (openKey === "more" ? null : findActivity(openKey))
  onActivitiesChanged: if (openKey !== "" && openKey !== "more" && !findActivity(openKey)) close()
  onHiddenChanged: if (hidden) close()
  function close() { openKey = ""; openPinned = false }
  function hoverIn(key, x) { hoverKey = key; hoverX = x; if (openKey === "" || !openPinned) openTimer.restart() }
  function hoverOut(key) { if (hoverKey === key) hoverKey = "" }
  Timer {
    id: openTimer
    interval: 320
    onTriggered: if (strip.hoverKey !== "") { strip.openKey = strip.hoverKey; strip.openX = strip.hoverX; strip.openPinned = false }
  }
  // Fermeture quand la souris a quitté bulle et carte (un délai pour passer de l'une à l'autre).
  readonly property bool anyHover: hoverKey !== "" || cardLoader.hovered || moreCard.hovered
  Timer {
    interval: 380
    running: strip.openKey !== "" && !strip.openPinned && !strip.anyHover
    onTriggered: strip.close()
  }
  function toggle(key, x) {
    if (openKey === key && openPinned) { close(); return }
    openKey = key; openX = x; openPinned = true
  }

  // Zone à donner à la souris en plus de la bande quand une carte est ouverte.
  readonly property bool cardOpen: openKey !== ""
  // Rectangle de la carte dépliée (dans le repère de la bande), vide sinon :
  // pour la zone souris de la fenêtre.
  readonly property rect cardRect: {
    if (!cardOpen) return Qt.rect(0, 0, 0, 0)
    var it = openKey === "more" ? moreCard : cardLoader
    return Qt.rect(it.x, it.y, it.width, it.height)
  }

  // ------------------------------------------------ bulles
  // Une ligne par activité affichée, mise à jour par clé : une bulle garde son
  // identité tant que son activité existe (entrée et sortie animées, pas de
  // reconstruction à chaque changement d'état).
  ListModel { id: bubbles }   // { key, leaving }
  onShownChanged: syncBubbles()
  Component.onCompleted: syncBubbles()
  function syncBubbles() {
    var keys = shown.map(function(a) { return a.key })
    // Sorties : animées, puis retirées.
    for (var r = 0; r < bubbles.count; r++) {
      var row = bubbles.get(r)
      if (keys.indexOf(row.key) === -1 && !row.leaving) { bubbles.setProperty(r, "leaving", true); leaveTimer.restart() }
      else if (keys.indexOf(row.key) !== -1 && row.leaving) bubbles.setProperty(r, "leaving", false)
    }
    // Entrées et ordre, parmi les lignes qui restent.
    for (var i = 0; i < keys.length; i++) {
      var at = -1, live = -1
      for (var j = 0; j < bubbles.count; j++) {
        if (bubbles.get(j).leaving) continue
        live++
        if (bubbles.get(j).key === keys[i]) { at = j; break }
      }
      // Position cible : après la i-ème ligne vivante.
      var target = 0, seen = 0
      while (target < bubbles.count && (bubbles.get(target).leaving || seen < i)) { if (!bubbles.get(target).leaving) seen++; target++ }
      if (at === -1) bubbles.insert(Math.min(target, bubbles.count), { key: keys[i], leaving: false })
      else if (at !== target && target < bubbles.count) bubbles.move(at, target, 1)
    }
  }
  Timer {
    id: leaveTimer
    interval: 260
    onTriggered: { for (var r = bubbles.count - 1; r >= 0; r--) if (bubbles.get(r).leaving) bubbles.remove(r) }
  }

  Row {
    id: row
    height: strip.h
    spacing: 8
    layoutDirection: strip.side === "left" ? Qt.RightToLeft : Qt.LeftToRight
    Repeater {
      model: bubbles
      delegate: Item {
        id: slot
        required property string key
        required property bool leaving
        required property int index
        // Activité vivante, ou la dernière connue pendant la sortie.
        property var lastActivity: null
        readonly property var activity: strip.findActivity(key) || lastActivity
        onActivityChanged: if (strip.findActivity(key)) lastActivity = strip.findActivity(key)
        readonly property int shownIndex: { var n = 0; for (var i = 0; i < index; i++) if (!bubbles.get(i).leaving) n++; return n }
        height: strip.h
        width: leaving ? 0 : bubbleItem.width
        visible: !!activity
        clip: leaving
        Behavior on width { NumberAnimation { duration: 240 * strip.host.motionScale; easing.type: Easing.OutQuint } }
        // Entrée : grossit depuis le centre ; sortie : rétrécit et s'efface.
        opacity: leaving ? 0 : 1
        scale: leaving ? 0.5 : 1
        Behavior on opacity { NumberAnimation { duration: 200 } }
        Behavior on scale { NumberAnimation { duration: 240; easing.type: Easing.InCubic } }
        Component.onCompleted: appear.start()
        ParallelAnimation {
          id: appear
          NumberAnimation { target: bubbleItem; property: "scale"; from: 0.4; to: 1; duration: 420 * strip.host.motionScale; easing.type: Easing.OutBack; easing.overshoot: 1.6 }
          NumberAnimation { target: bubbleItem; property: "opacity"; from: 0; to: 1; duration: 200 }
        }
        ActivityBubble {
          id: bubbleItem
          host: strip.host
          activity: slot.activity || { kind: "" }
          compact: strip.allCapsules || (slot.shownIndex === 0 && strip.capsuleFits)
          onHoveredChanged: hovered ? strip.hoverIn(slot.key, slot.x + width / 2) : strip.hoverOut(slot.key)
          onClicked: {
            var a = slot.activity
            if (!a || slot.leaving) return
            if (a.kind === "ai") { strip.close(); strip.openConversation(a.conv) }
            else if (a.kind === "music") { strip.close(); strip.openPlayer() }
            else if (a.kind === "timer" && a.timer.done) strip.host.cancelTimer(a.timer.id)
            else strip.toggle(slot.key, slot.x + width / 2)
          }
        }
      }
    }
    // « +N »
    Rectangle {
      visible: strip.hiddenCount > 0
      height: strip.h
      width: Math.max(height, moreText.implicitWidth + 20)
      radius: height / 2
      color: strip.openKey === "more" ? strip.host.withAlpha(strip.host.colorText, 0.16) : strip.host.colorIsland
      Text {
        id: moreText
        anchors.centerIn: parent
        text: "+" + strip.hiddenCount
        color: strip.host.colorText
        font.family: strip.host.uiFont
        font.pixelSize: 12
        font.weight: Font.DemiBold
      }
      HoverHandler { onHoveredChanged: hovered ? strip.hoverIn("more", parent.x + parent.width / 2) : strip.hoverOut("more") }
      MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: strip.toggle("more", parent.x + parent.width / 2) }
    }
  }

  // ------------------------------------------------ carte dépliée
  Item {
    id: cardLoader
    readonly property bool hovered: card.hovered
    readonly property bool shownNow: strip.openKey !== "" && strip.openKey !== "more" && !!strip.openActivity
    x: Math.max(-strip.x + 8, Math.min(strip.openX - width / 2, strip.screenWidth - strip.x - width - 10))
    y: strip.h + 8
    width: card.width
    height: card.height
    opacity: shownNow ? 1 : 0
    scale: shownNow ? 1 : 0.94
    transformOrigin: Item.Top
    visible: opacity > 0.01
    Behavior on opacity { NumberAnimation { duration: cardLoader.shownNow ? 180 : 130; easing.type: Easing.OutCubic } }
    Behavior on scale { NumberAnimation { duration: cardLoader.shownNow ? 320 * strip.host.motionScale : 140; easing.type: cardLoader.shownNow ? Easing.OutBack : Easing.InCubic; easing.overshoot: 1.2 } }
    // Garde la dernière activité pendant la fermeture animée.
    property var lastActivity: null
    Connections {
      target: strip
      function onOpenActivityChanged() { if (strip.openActivity) cardLoader.lastActivity = strip.openActivity }
    }
    ActivityCard {
      id: card
      host: strip.host
      activity: strip.openActivity || cardLoader.lastActivity || { kind: "" }
      onOpenConversation: function(c) { strip.openConversation(c) }
      onOpenPlayer: strip.openPlayer()
      onDone: strip.close()
    }
  }

  // ------------------------------------------------ liste de « +N »
  Rectangle {
    id: moreCard
    readonly property bool hovered: moreHover.hovered
    readonly property bool shownNow: strip.openKey === "more"
    x: Math.max(-strip.x + 8, Math.min(strip.openX - width / 2, strip.screenWidth - strip.x - width - 10))
    y: strip.h + 8
    width: 320
    height: moreList.implicitHeight + 12
    radius: 16
    color: strip.host.colorIsland
    border.width: 1
    border.color: strip.host.withAlpha(strip.host.colorText, 0.08)
    opacity: shownNow ? 1 : 0
    scale: shownNow ? 1 : 0.94
    transformOrigin: Item.Top
    visible: opacity > 0.01
    Behavior on opacity { NumberAnimation { duration: moreCard.shownNow ? 180 : 130; easing.type: Easing.OutCubic } }
    Behavior on scale { NumberAnimation { duration: moreCard.shownNow ? 320 * strip.host.motionScale : 140; easing.type: moreCard.shownNow ? Easing.OutBack : Easing.InCubic; easing.overshoot: 1.2 } }
    HoverHandler { id: moreHover }
    Column {
      id: moreList
      x: 6
      y: 6
      width: parent.width - 12
      Repeater {
        model: strip.overflow
        delegate: Rectangle {
          id: moreRow
          required property var modelData
          width: moreList.width
          height: 32
          radius: 9
          color: rowMouse.containsMouse ? strip.host.withAlpha(strip.host.colorText, 0.08) : "transparent"
          ActivityBubble {
            id: miniBubble
            anchors.left: parent.left
            anchors.leftMargin: 2
            anchors.verticalCenter: parent.verticalCenter
            scale: 0.85
            host: strip.host
            activity: moreRow.modelData
            compact: false
          }
          Text {
            anchors.left: miniBubble.right
            anchors.leftMargin: 6
            anchors.right: parent.right
            anchors.rightMargin: 10
            anchors.verticalCenter: parent.verticalCenter
            elide: Text.ElideMiddle
            text: moreRow.modelData.kind === "timer" ? (moreRow.modelData.timer.label || "Timer") + "  ·  " + (moreRow.modelData.timer.done ? "Done" : strip.host.formatLeft(strip.host.timerLeft(moreRow.modelData.timer)))
              : moreRow.modelData.kind === "ai" ? moreRow.modelData.conv.question
              : moreRow.modelData.kind === "music" ? (strip.host.player ? String(strip.host.player.trackTitle || "Music") : "Music")
              : "Battery"
            color: strip.host.colorText
            font.family: strip.host.uiFont
            font.pixelSize: 12
            font.features: { "tnum": 1 }
          }
          MouseArea {
            id: rowMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: {
              var a = moreRow.modelData
              if (a.kind === "ai") { strip.close(); strip.openConversation(a.conv) }
              else { strip.openKey = a.key; strip.openPinned = true }
            }
          }
        }
      }
    }
  }
}
