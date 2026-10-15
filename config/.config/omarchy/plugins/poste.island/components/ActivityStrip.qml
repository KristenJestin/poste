import QtQuick

// The ongoing activities (iPhone Live Activities style), stuck to
// the right of the island: the most important one as a capsule (one piece of info on the left, one on
// the right), the next ones as circles, the surplus in '+N'. Hover (for a
// moment) or click: the unfolded card below (ActivityCard.qml).
//
// Order: battery (event); finished timer; AI answer ready;
// running timers (the closest to its end first); AI at work;
// pinned AIs; paused timers.
Item {
  id: strip
  required property var host
  required property real roomRight     // free width for the strip (on its side of the island)
  required property real screenWidth
  // The activities of this strip (empty: all). To the right of the island, what
  // moves (timers, AI, dictation); to the left, what stays (music, battery).
  property var kinds: []
  // "right": the strip goes from the island to the right; "left": to the left
  // (the most important one stays stuck to the island).
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

  // ------------------------------------------------ the activities
  readonly property var activities: {
    host.ai.stateRevision
    var out = []
    // OBS: recording or streaming in progress, always first.
    if (host.obs && host.obs.active) out.push({ kind: "obs", key: "obs", rank: -2, order: 0 })
    // Music: during playback, and 30 s after a pause (then it leaves).
    var p = host.player
    if (p && String(p.trackTitle || "") !== "" && (host.mediaPlaying || musicLinger))
      out.push({ kind: "music", key: "music", rank: host.mediaPlaying ? 3.5 : 6.5, order: 0 })
    var b = host.battery
    // Battery: only low (below 30 %) or just after an unplug;
    // otherwise it is in the control center.
    if (b && b.showing) out.push({ kind: "battery", key: "battery", rank: 0, order: 0 })
    // Wireless devices below 15 % (DevicesState.qml).
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
    // Stable sort (Qt's is not).
    for (var k = 0; k < out.length; k++) out[k].index = k
    out.sort(function(a, b) { return (a.rank - b.rank) || (a.order - b.order) || (a.index - b.index) })
    return out
  }
  // Paused music: still 30 s on screen, the time to resume.
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
  // How many bubbles fit (the first as a capsule if possible).
  readonly property bool capsuleFits: roomRight >= capsuleWidth
  // Estimated width of an activity as a capsule (from its texts, not from
  // the bubble, which changes size).
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
  // Few activities: all as capsules. Otherwise the first as a capsule, the
  // others as circles, the surplus in '+N'.
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

  // ------------------------------------------------ unfolding
  // Key of the unfolded activity; 'pinned': opened by a click, stays
  // open until clicked again or left.
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
  // Closing when the mouse has left bubble and card (a delay to go from one to the other).
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

  // Area to give the mouse in addition to the strip when a card is open.
  readonly property bool cardOpen: openKey !== ""
  // Rectangle of the unfolded card (in the strip's frame), empty otherwise:
  // for the window's mouse area.
  readonly property rect cardRect: {
    if (!cardOpen) return Qt.rect(0, 0, 0, 0)
    var it = openKey === "more" ? moreCard : cardLoader
    return Qt.rect(it.x, it.y, it.width, it.height)
  }

  // ------------------------------------------------ bubbles
  // One row per displayed activity, updated by key: a bubble keeps its
  // identity as long as its activity exists (animated entry and exit, no
  // rebuild at each state change).
  ListModel { id: bubbles }   // { key, leaving }
  onShownChanged: syncBubbles()
  Component.onCompleted: syncBubbles()
  function syncBubbles() {
    var keys = shown.map(function(a) { return a.key })
    // Exits: animated, then removed.
    for (var r = 0; r < bubbles.count; r++) {
      var row = bubbles.get(r)
      if (keys.indexOf(row.key) === -1 && !row.leaving) { bubbles.setProperty(r, "leaving", true); leaveTimer.restart() }
      else if (keys.indexOf(row.key) !== -1 && row.leaving) bubbles.setProperty(r, "leaving", false)
    }
    // Entries and order, among the rows that remain.
    for (var i = 0; i < keys.length; i++) {
      var at = -1, live = -1
      for (var j = 0; j < bubbles.count; j++) {
        if (bubbles.get(j).leaving) continue
        live++
        if (bubbles.get(j).key === keys[i]) { at = j; break }
      }
      // Target position: after the i-th live row.
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
        // Live activity, or the last known one during the exit.
        property var lastActivity: null
        readonly property var activity: strip.findActivity(key) || lastActivity
        onActivityChanged: if (strip.findActivity(key)) lastActivity = strip.findActivity(key)
        readonly property int shownIndex: { var n = 0; for (var i = 0; i < index; i++) if (!bubbles.get(i).leaving) n++; return n }
        height: strip.h
        width: leaving ? 0 : bubbleItem.width
        visible: !!activity
        clip: leaving
        Behavior on width { NumberAnimation { duration: 240 * strip.host.motionScale; easing.type: Easing.OutQuint } }
        // Entry: grows from the center; exit: shrinks and fades.
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
    // '+N'
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

  // ------------------------------------------------ unfolded card
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
    // Keeps the last activity during the animated closing.
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

  // ------------------------------------------------ '+N' list
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
