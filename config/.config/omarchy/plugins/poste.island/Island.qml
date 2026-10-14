// Poste Island — a floating, per-monitor island that replaces the Omarchy bar.
//
// Parts of this plugin (the morphing pill, the sound wave, the media and
// volume pill, the player view) are adapted from Island
// (https://github.com/Guilhermerisu/island), MIT License,
// Copyright (c) 2026 Guilherme Pimenta. See LICENSE-island.
//
// One island per screen shows that screen's workspaces, the clock, and the
// now-playing cover + wave; clicking the music opens a player on that screen.
// Ce qui passe (notifications, volume, OSD, texte dicté) s'affiche dans le bandeau (bandeau/),
// empilé juste sous l'île ; seule la pastille de la dictée (micro, onde) va en bas de l'écran
// actif. L'île ne se transforme pas pour le montrer.
import QtQuick
import Qt.labs.folderlistmodel
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import Quickshell.Services.Pipewire
import Quickshell.Wayland
import qs.Commons
import "components"
import "bandeau"
import "file:///usr/share/flea/shelf" as Flea

Item {
  id: root

  // Injected by the shell (see configureBar in shell.qml).
  property var shell: null
  property var manifest: null
  property var pluginRegistry: null
  property var barWidgetRegistry: null
  property var barConfig: ({})
  property string omarchyPath: ""

  // ---------------------------------------------------------------- bar API
  // Other plugins (notification placement, panels) read these off shell.bar.
  readonly property int restHeight: 34
  readonly property int restTop: 5
  readonly property int barSize: barHidden ? 0 : restHeight + restTop - 1
  readonly property string position: "top"
  readonly property string fontFamily: "JetBrainsMono Nerd Font"
  readonly property bool vertical: false
  readonly property string home: Quickshell.env("HOME")
  readonly property bool barHidden: barOffFlag.count > 0
  FolderListModel {
    id: barOffFlag
    folder: "file://" + root.home + "/.local/state/omarchy/toggles"
    nameFilters: ["bar-off"]
    showDirs: false
    showHidden: true
  }

  // ---------------------------------------------------------------- modules poste
  // Morceaux d'île fournis par les modules (lanceur, éléments QML) : voir ModuleRegistry.qml.
  ModuleRegistry { id: moduleRegistry }
  readonly property var modules: moduleRegistry

  // ---------------------------------------------------------------- style
  readonly property color colorIsland: Qt.darker(Color.background, 1.9)
  readonly property color colorText: Qt.lighter(Color.foreground, 1.08)
  readonly property color colorMuted: withAlpha(Color.foreground, 0.45)
  readonly property color colorAccent: Color.accent
  readonly property color colorAccentText: luminance(Color.accent) > 0.45 ? "#0b0b0c" : "#ffffff"
  readonly property string uiFont: "Adwaita Sans"
  readonly property real motionScale: 1.2

  function withAlpha(c, a) { return Qt.rgba(c.r, c.g, c.b, a) }
  function luminance(c) { return 0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b }

  // ---------------------------------------------------------------- clock
  SystemClock { id: clock; precision: SystemClock.Minutes }
  readonly property var clockDate: clock.date

  // ---------------------------------------------------------------- media
  readonly property var media: shell ? shell.firstPartyServiceFor("omarchy.media") : null
  readonly property var player: media ? media.activePlayer : null
  readonly property bool mediaPlaying: !!(player && player.isPlaying)
  readonly property string reportedArt: player && player.trackArtUrl ? String(player.trackArtUrl) : ""
  readonly property string mediaTitle: player ? String(player.trackTitle || "") : ""
  // Some players drop the cover URL between tracks; keep the last one for the
  // same title so the pill doesn't flicker to the placeholder.
  property string keptArt: ""
  property string keptArtTitle: ""
  onReportedArtChanged: if (reportedArt) { keptArt = reportedArt; keptArtTitle = mediaTitle }
  onMediaTitleChanged: if (mediaTitle !== keptArtTitle) { keptArt = reportedArt; keptArtTitle = mediaTitle }
  readonly property string mediaArt: reportedArt || (mediaTitle === keptArtTitle ? keptArt : "")

  ColorQuantizer {
    id: coverColors
    // Images locales seulement : ColorQuantizer ne sait pas charger les
    // pochettes HTTP(S) (lecteurs en ligne) et journalise alors l'URL
    // complète, jeton d'authentification Subsonic compris. L'onde garde
    // l'accent quand la pochette est distante.
    source: /^(file:|\/)/.test(root.mediaArt) ? root.mediaArt : ""
    depth: 2
    rescaleSize: 64
  }
  // The wave takes the cover's most vivid color, the theme accent otherwise.
  readonly property color mediaTint: {
    var best = null, bestScore = -1
    var colors = coverColors.colors || []
    for (var i = 0; i < colors.length; i++) {
      var c = colors[i]
      var score = c.hsvSaturation * 0.7 + c.hsvValue * 0.3
      if (score > bestScore) { bestScore = score; best = c }
    }
    if (!best || best.hsvSaturation < 0.12) return colorAccent
    return Qt.hsva(best.hsvHue, Math.min(1, best.hsvSaturation), Math.max(0.75, best.hsvValue), 1)
  }
  function mediaAction(name) { if (media) media.runAction(name, false, "") }

  // Slot « player-extra » : un module peut ajouter à la carte du lecteur ce qui est propre à
  // son lecteur (qualité du flux, cœur…) par un QML déclaré dans son island/island.json.
  // Contrat de l'élément chargé : propriété injectée `player` (lecteur MPRIS courant) ;
  // propriétés lues `active` (bool, ce module prend en charge ce lecteur), `detail` (ligne
  // discrète sous l'artiste), `actionGlyph` / `actionOn` (bouton en haut à droite) avec la
  // fonction `action()`, et `launcherExt` (extension du lanceur ouverte par la pastille de la
  // source du son). Sans module (ou lecteur non pris en charge), rien de tout cela ne s'affiche.
  property var playerExtraItems: []
  function collectPlayerExtras() {
    var list = []
    for (var i = 0; i < playerExtraLoaders.count; i++) {
      var ld = playerExtraLoaders.itemAt(i)
      if (ld && ld.item) list.push(ld.item)
    }
    playerExtraItems = list
  }
  Repeater {
    id: playerExtraLoaders
    model: moduleRegistry.slots["player-extra"] || []
    delegate: Loader {
      required property string modelData
      source: modelData
      onLoaded: {
        if (item && "player" in item) item.player = Qt.binding(function() { return root.player })
        root.collectPlayerExtras()
      }
    }
    onItemAdded: Qt.callLater(root.collectPlayerExtras)
    onItemRemoved: Qt.callLater(root.collectPlayerExtras)
  }
  readonly property var playerExtra: {
    var list = playerExtraItems
    for (var i = 0; i < list.length; i++) if (list[i].active) return list[i]
    return null
  }
  readonly property string playerDetail: playerExtra ? String(playerExtra.detail || "") : ""
  readonly property bool playerHasAction: !!playerExtra && String(playerExtra.actionGlyph || "") !== ""
  readonly property string playerActionGlyph: playerHasAction ? String(playerExtra.actionGlyph) : ""
  readonly property bool playerActionOn: playerHasAction && !!playerExtra.actionOn
  readonly property string playerSourceExt: playerExtra ? String(playerExtra.launcherExt || "") : ""
  function playerAction() { if (playerExtra && playerExtra.action) playerExtra.action() }

  // ---------------------------------------------------------------- volume du lecteur
  // Volume de l'application du lecteur courant : son ou ses flux PipeWire
  // (nœuds « Stream/Output/Audio » dont `application.name` correspond à
  // l'identité ou à l'entrée de bureau MPRIS : « Musique », « Zen »...), sinon
  // la propriété MPRIS `volume` si le lecteur la gère, sinon -1 (rien à régler).
  readonly property var pwAllStreams: (Pipewire.nodes ? Pipewire.nodes.values : []).filter(function(n) { return n && n.isStream })
  PwObjectTracker { objects: root.pwAllStreams }

  function streamMatchesPlayer(n, p) {
    if (!n.ready || !n.properties) return false
    var props = n.properties
    if (String(props["media.class"] || "") !== "Stream/Output/Audio") return false
    var names = [String(props["application.name"] || ""), String(props["application.process.binary"] || "")]
    var keys = [String(p.identity || ""), String(p.desktopEntry || "")]
    for (var i = 0; i < names.length; i++) {
      var a = names[i].toLowerCase()
      if (a === "") continue
      for (var j = 0; j < keys.length; j++) {
        var k = keys[j].toLowerCase()
        if (k === "") continue
        if (a === k || (a.length >= 3 && k.indexOf(a) >= 0) || (k.length >= 3 && a.indexOf(k) >= 0)) return true
      }
    }
    return false
  }
  readonly property var playerStreams: {
    var p = player
    if (!p) return []
    return pwAllStreams.filter(function(n) { return n.audio && streamMatchesPlayer(n, p) })
  }
  readonly property bool playerMprisVolume: !!player && !!player.canControl && !!player.volumeSupported
  readonly property real playerVolume: {
    if (playerStreams.length > 0) {
      var m = 0
      for (var i = 0; i < playerStreams.length; i++) m = Math.max(m, playerStreams[i].audio.volume)
      return m
    }
    return playerMprisVolume ? Number(player.volume) : -1
  }
  readonly property bool playerMuted: {
    if (playerStreams.length > 0) return playerStreams.every(function(n) { return n.audio.muted })
    return playerMprisVolume && Number(player.volume) <= 0
  }
  // Dernier niveau avant la mise en sourdine MPRIS (qui n'a pas de « muet »).
  property real mprisVolumeBeforeMute: 0.5
  function setPlayerVolume(v) {
    v = Math.max(0, Math.min(1, v))
    if (playerStreams.length > 0) {
      for (var i = 0; i < playerStreams.length; i++) playerStreams[i].audio.volume = v
    } else if (playerMprisVolume) {
      player.volume = v
    }
  }
  function stepPlayerVolume(delta) {
    if (playerVolume < 0) return
    setPlayerVolume(Math.round((playerVolume + delta) * 100) / 100)
    playerVolumeHint.restart()
  }
  // Molette : 120 unités = un cran = 5 % (les pavés tactiles cumulent leurs petits pas).
  property real wheelAccum: 0
  function wheelPlayerVolume(dy) {
    if (playerVolume < 0) return
    wheelAccum += dy
    var notches = Math.trunc(wheelAccum / 120)
    if (notches === 0) return
    wheelAccum -= notches * 120
    stepPlayerVolume(notches * 0.05)
  }
  function togglePlayerMute() {
    if (playerStreams.length > 0) {
      var mute = !playerMuted
      for (var i = 0; i < playerStreams.length; i++) playerStreams[i].audio.muted = mute
    } else if (playerMprisVolume) {
      if (playerMuted) setPlayerVolume(mprisVolumeBeforeMute)
      else { mprisVolumeBeforeMute = Math.max(0.05, Number(player.volume)); setPlayerVolume(0) }
    }
  }
  // Pastille « NN % » brève après un coup de molette (pastille et carte).
  readonly property bool playerVolumeHintShown: playerVolumeHint.running
  Timer { id: playerVolumeHint; interval: 1200 }

  // ---------------------------------------------------------------- focus
  readonly property string focusedScreen: Hyprland.focusedMonitor ? String(Hyprland.focusedMonitor.name) : ""

  Component.onCompleted: {
    lockKnown = true
    refreshWorkspaces()
    refreshUnread()
  }

  // ---------------------------------------------------------------- workspaces
  // Per-monitor workspaces (see ~/.config/hypr/monitors.lua): left 1-10,
  // center 11-20, right 21-30, any other screen 31-40. State comes straight
  // from hyprctl on every workspace/window event.
  // Read from workspace_monitors in monitors.lua, so each machine keeps its own list.
  // An entry is a port name (DP-1) or "desc:" + the start of the screen's EDID
  // description, matched like Hyprland does.
  property var workspaceMonitors: []
  function parseWorkspaceMonitors(text) {
    var m = /^\s*workspace_monitors\s*=\s*\{([^}]*)\}/m.exec(text || "")
    if (!m) return []
    var out = []
    var re = /["']([^"']+)["']/g
    var s
    while ((s = re.exec(m[1])) !== null) out.push(s[1])
    return out
  }
  FileView {
    id: monitorsFile
    path: Quickshell.env("HOME") + "/.config/hypr/monitors.lua"
    watchChanges: true
    printErrors: false
    onFileChanged: reload()
    onLoaded: root.workspaceMonitors = root.parseWorkspaceMonitors(text())
    onLoadFailed: root.workspaceMonitors = []
  }
  // Screen name -> EDID description, from hyprctl.
  property var monitorDescriptions: ({})
  function screenMatches(entry, screenName) {
    if (entry.indexOf("desc:") === 0) {
      var want = entry.slice(5).trim()
      return String(monitorDescriptions[screenName] || "").indexOf(want) === 0
    }
    return entry === screenName
  }
  function workspaceSlot(screenName) {
    for (var i = 0; i < workspaceMonitors.length; i++)
      if (screenMatches(workspaceMonitors[i], screenName)) return i
    return workspaceMonitors.length
  }
  function workspaceOffset(screenName) {
    return workspaceSlot(screenName) * 10
  }
  // Screen name -> ids of workspaces it holds outside its own 1-10: those of
  // an unplugged screen, which Hyprland parks here until it comes back.
  property var visitingWorkspaces: ({})
  property var activeWorkspaces: ({})
  // Screens whose shown workspace has a fullscreen window: their island hides.
  property var fullscreenScreens: ({})
  property var windowCounts: ({})
  property bool workspacesPending: false
  function refreshWorkspaces() {
    if (workspaceProc.running) { workspacesPending = true; return }
    workspacesPending = false
    workspaceProc.running = true
  }
  Connections {
    target: Hyprland
    function onRawEvent(event) {
      // Un monitors.lua recâblé (lien vers un autre fichier) ne déclenche pas
      // watchChanges : on le relit quand Hyprland recharge sa config.
      if (event.name === "configreloaded") monitorsFile.reload()
      if (/workspace|window|monitor|focus|fullscreen/.test(event.name)) root.refreshWorkspaces()
    }
  }
  Process {
    id: workspaceProc
    command: ["sh", "-c", "printf '{\"monitors\":'; hyprctl -j monitors; printf ',\"workspaces\":'; hyprctl -j workspaces; printf '}'"]
    onRunningChanged: if (!running && root.workspacesPending) root.refreshWorkspaces()
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        var state
        try { state = JSON.parse(text) } catch (e) { return }
        var active = {}
        var descriptions = {}
        for (var i = 0; i < state.monitors.length; i++) {
          active[state.monitors[i].name] = state.monitors[i].activeWorkspace.id
          descriptions[state.monitors[i].name] = state.monitors[i].description || ""
        }
        if (JSON.stringify(descriptions) !== JSON.stringify(root.monitorDescriptions))
          root.monitorDescriptions = descriptions
        var counts = {}
        var fullscreenWs = {}
        var visiting = {}
        for (var j = 0; j < state.workspaces.length; j++) {
          var ws = state.workspaces[j]
          counts[ws.id] = ws.windows
          if (ws.hasfullscreen) fullscreenWs[ws.id] = true
          var own = root.workspaceOffset(ws.monitor)
          if (ws.id > 0 && (ws.id <= own || ws.id > own + 10) && (ws.windows > 0 || active[ws.monitor] === ws.id)) {
            if (!visiting[ws.monitor]) visiting[ws.monitor] = []
            visiting[ws.monitor].push(ws.id)
          }
        }
        for (var name in visiting) visiting[name].sort(function(a, b) { return a - b })
        root.visitingWorkspaces = visiting
        var fullscreen = {}
        for (var m in active) if (fullscreenWs[active[m]]) fullscreen[m] = true
        root.activeWorkspaces = active
        root.windowCounts = counts
        root.fullscreenScreens = fullscreen
      }
    }
  }
  function focusWorkspace(id) {
    Quickshell.execDetached(["hyprctl", "dispatch", "hl.dsp.focus({ workspace = \"" + id + "\" })"])
  }

  // ---------------------------------------------------------------- native panels
  // The island is shell.bar, so the shell routes summon/hide/toggle of
  // bar-widget panels (omarchy.network/audio/bluetooth, SUPER+CTRL+W/A) here.
  // Those widgets are mounted invisibly in each screen's reserve strip and
  // get this object as their `bar`; the members below are the part of the
  // bar surface they use.
  readonly property string vpnId: "io.github.grichard99.omaproton-vpn"
  readonly property string tsId: "omarchy.tailscale"
  readonly property var nativePanelIds: ["omarchy.network", "omarchy.bluetooth", "omarchy.audio", vpnId, tsId]
  // Widgets that carry their own service (the VPN one connects, notifies,
  // forwards ports) are mounted once, on the middle screen, not per screen.
  readonly property var singleScreenPanelIds: [vpnId, tsId]
  readonly property string primaryScreenName: {
    var list = Quickshell.screens.slice().sort(function(a, b) { return a.x - b.x })
    return list.length ? String(list[Math.floor((list.length - 1) / 2)].name) : ""
  }
  function nativePanelIdsFor(screenName) {
    return nativePanelIds.filter(function(id) {
      return singleScreenPanelIds.indexOf(id) < 0 || String(screenName) === primaryScreenName
    })
  }
  readonly property color foreground: Color.bar.text
  readonly property color barForeground: Color.bar.text
  readonly property color background: Color.bar.background
  readonly property color urgent: Color.urgent
  readonly property bool transparent: false
  readonly property bool foregroundAnimationEnabled: true
  readonly property var layoutConfig: ({})
  property var clickTargets: []
  property var activePopout: null
  function requestPopout(owner) {
    if (activePopout === owner) return
    if (activePopout) {
      if ("closeForPopoutSwitch" in activePopout) activePopout.closeForPopoutSwitch()
      else if ("close" in activePopout) activePopout.close()
    }
    activePopout = owner
  }
  function releasePopout(owner) { if (activePopout === owner) activePopout = null }
  function switchPanelFrom(owner, direction) { return false }
  function targetBelongsToWindow(target, window) { return false }
  function registerClickTarget(target) { }
  function unregisterClickTarget(target) { }
  function showTooltip(target, text) { }
  function hideTooltip(target) { }
  function run(command) { if (command) Util.execDetached(command) }

  property var nativeWidgets: []
  function registerNativeWidget(id, screenName, item) {
    nativeWidgets = nativeWidgets.filter(function(w) { return w.item !== item }).concat([{ id: id, screen: screenName, item: item }])
  }
  function unregisterNativeWidget(item) {
    nativeWidgets = nativeWidgets.filter(function(w) { return w.item !== item })
  }
  function moduleWidgets(id) {
    return nativeWidgets.filter(function(w) { return w.id === id }).map(function(w) { return w.item })
  }
  function nativeWidgetOn(id, screenName) {
    var list = nativeWidgets.filter(function(w) { return w.id === id })
    for (var i = 0; i < list.length; i++) if (list[i].screen === screenName) return list[i].item
    return list.length ? list[0].item : null
  }
  function summonBarWidget(id) {
    var item = nativeWidgetOn(id, focusedScreen)
    if (!item || typeof item.open !== "function") return false
    item.open()
    return true
  }
  function hideBarWidget(id) {
    var items = moduleWidgets(id)
    for (var i = 0; i < items.length; i++) if (items[i].opened) items[i].close()
    return items.length > 0
  }
  function isBarWidgetOpen(id) {
    var items = moduleWidgets(id)
    for (var i = 0; i < items.length; i++) if (items[i].opened === true) return true
    return false
  }
  // A widget's own settings, from its entry in the bar layout (the VPN widget
  // writes them back there through shell.updateEntryInline).
  function barEntrySettings(id) {
    var layout = barConfig && barConfig.layout ? barConfig.layout : {}
    var sections = ["left", "center", "right"]
    for (var s = 0; s < sections.length; s++) {
      var list = layout[sections[s]] || []
      for (var i = 0; i < list.length; i++) if (list[i] && list[i].id === id) return list[i]
    }
    return ({})
  }

  // VPN (OmaProton): its panel keeps the Proton CLI state in a child Service
  // object, found once the widget is mounted. The tile shows once the CLI is
  // installed (or a tunnel is up), the lock only while connected.
  property var vpnService: null
  function findVpnService(item) {
    var kids = item && item.data ? item.data : []
    for (var i = 0; i < kids.length; i++) {
      var k = kids[i]
      if (k && "installed" in k && "statusConnected" in k && "connected" in k) return k
    }
    return null
  }
  readonly property bool vpnAvailable: !!vpnService && vpnService.installed === true
  readonly property bool vpnConnected: !!vpnService && vpnService.connected === true

  // Tailscale (Omarchy's own widget): same idea, its Service child holds the
  // CLI state. The tile shows once tailscale is installed.
  property var tsService: null
  function findTsService(item) {
    var kids = item && item.data ? item.data : []
    for (var i = 0; i < kids.length; i++) {
      var k = kids[i]
      if (k && "fileSharing" in k && "peers" in k && "selfIp" in k) return k
    }
    return null
  }
  readonly property bool tsAvailable: !!tsService && tsService.installed === true
  readonly property bool tsConnected: !!tsService && tsService.active === true

  function toggleNativePanel(id, screenName) {
    var item = nativeWidgetOn(id, screenName)
    if (!item) return
    if (item.opened) item.close()
    else item.open()
  }

  // ---------------------------------------------------------------- bord de l'île
  // Chaque île donne au bandeau son bord bas dessiné (IslandWindow.qml, `edge`) : la pile du
  // bandeau se pose juste en dessous, sur le même écran.
  function reportEdge(screenName, bottom) { Bandeau.setIslandEdge(screenName, bottom) }

  // ---------------------------------------------------------------- notifications list
  // Unread count and the list both come from the Notification Center
  // plugin's store (its CLI), so "read" and "cleared" mean the same thing as
  // in its own panel. Opening the list marks everything read.
  readonly property string ncScript: home + "/.config/omarchy/plugins/jankeesvw.notification-center/bin/notification-center"
  readonly property string ncStore: home + "/.local/state/omarchy-notification-center"
  property int unreadCount: 0
  property double seenAt: 0
  // Une notification critique non lue : le point de l'île passe au rouge.
  readonly property bool unreadImportant: {
    for (var i = 0; i < notificationList.length; i++) {
      var n = notificationList[i]
      if (n && Number(n.urgency) === 2 && Number(n.timestamp) > seenAt) return true
    }
    return false
  }
  property var notificationList: []
  property bool storePending: false
  function refreshStore() {
    if (storeProc.running) { storePending = true; return }
    storePending = false
    storeProc.running = true
  }
  // Kept name for callers.
  function refreshUnread() { refreshStore() }
  Process {
    id: storeProc
    // `list` can print a trailing second array; keep only the first value.
    command: ["sh", "-c", "[ -x \"$1\" ] || exit 0; u=$(\"$1\" unread); s=$(\"$1\" seen); l=$(\"$1\" list 20 | jq -cs '.[0] // []'); printf '{\"unread\":%s,\"seen\":%s,\"list\":%s}' \"$u\" \"$s\" \"$l\"", "store", root.ncScript]
    onRunningChanged: if (!running && root.storePending) root.refreshStore()
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        var data
        try { data = JSON.parse(text) } catch (e) { return }
        root.unreadCount = Number(data.unread && data.unread.unread) || 0
        root.seenAt = Number(data.seen && data.seen.seen) || 0
        root.notificationList = Array.isArray(data.list) ? data.list : []
      }
    }
  }
  FileView { path: root.ncStore + "/archive.jsonl"; watchChanges: true; printErrors: false; onFileChanged: root.refreshStore() }
  FileView { path: root.ncStore + "/seen"; watchChanges: true; printErrors: false; onFileChanged: root.refreshStore() }
  FileView { path: root.ncStore + "/dismissed"; watchChanges: true; printErrors: false; onFileChanged: root.refreshStore() }
  // The store rewrites its files by rename, which a file watch can miss.
  Timer { interval: 4000; running: true; repeat: true; onTriggered: root.refreshStore() }

  function markAllRead() {
    unreadCount = 0
    Quickshell.execDetached(["sh", "-c", "[ -x \"$1\" ] && exec \"$1\" seen \"$(date +%s%3N)\"", "seen", ncScript])
  }
  function removeNotification(key) {
    notificationList = notificationList.filter(function(n) { return n.key !== key })
    Quickshell.execDetached([ncScript, "remove", String(key)])
  }
  function clearNotifications() {
    notificationList = []
    Quickshell.execDetached([ncScript, "clear"])
  }
  // Same rule as the Notification Center's "Auto": open the picture a
  // notification pointed at, otherwise focus the app that sent it. A stored
  // notification's own command is never run.
  function activateStoredNotification(n) {
    if (!n) return
    if (n.file) Quickshell.execDetached(["xdg-open", String(n.file)])
    else if (n.app) Quickshell.execDetached(["omarchy-hyprland-focus-app", String(n.app)])
  }
  function relativeTime(ms) {
    var d = (Date.now() - Number(ms || 0)) / 1000
    if (d < 60) return "now"
    if (d < 3600) return Math.floor(d / 60) + " min ago"
    if (d < 86400) return Math.floor(d / 3600) + " h ago"
    if (d < 172800) return "yesterday"
    return Qt.formatDateTime(new Date(Number(ms)), "d MMM")
  }

  // ---------------------------------------------------------------- voxtype
  // Dictation state from the daemon's state file (idle, recording, streaming,
  // transcribing) and live mic levels from its audio socket through the
  // voxtype-audio-bridge sidecar (NDJSON {"peak","rms","vad"} frames). The
  // voxtype OSD is disabled (osd.enabled = false); the bandeau shows it.
  readonly property string voxStatePath: (Quickshell.env("XDG_RUNTIME_DIR") || "/tmp") + "/voxtype/state"
  property string voxtypeState: "idle"
  readonly property bool voxtypeListening: voxtypeState === "recording" || voxtypeState === "streaming"
  readonly property bool voxtypeTranscribing: voxtypeState === "transcribing"
  // The daemon may replace the file by rename, which a watch can miss: also
  // re-read it a few times a second (a 9-byte file).
  Timer { interval: 250; running: true; repeat: true; onTriggered: voxStateFile.reload() }
  FileView {
    id: voxStateFile
    path: root.voxStatePath
    watchChanges: true
    printErrors: false
    onFileChanged: reload()
    onLoaded: root.voxtypeState = (text() || "idle").trim() || "idle"
    onLoadFailed: root.voxtypeState = "idle"
  }
  // Niveau courant de voxtype, 0..1 (boosted: speech RMS is small). Le bridge envoie 100 images
  // par seconde (fenêtres de 10 ms, source voxtype : src/audio/levels.rs), `rms` y vaut le pic de
  // la fenêtre divisé par racine de 2. L'historique des barres est rempli par le bandeau.
  property real voxtypeLevel: 0
  Process {
    id: voxBridge
    running: root.voxtypeListening
    command: ["voxtype-audio-bridge"]
    stdout: SplitParser {
      onRead: function(line) {
        var frame
        try { frame = JSON.parse(line) } catch (e) { return }
        if (frame.rms === undefined) return
        var level = Math.min(1, Math.sqrt(Math.max(0, Number(frame.rms))) * 2.2)
        root.voxtypeLevel = level
      }
    }
  }
  onVoxtypeListeningChanged: if (!voxtypeListening) voxtypeLevel = 0

  // Slot « dictation » : un module peut fournir sa propre source de dictée, affichée comme
  // voxtype. Contrat de l'élément chargé : propriétés lues `listening` (micro ouvert),
  // `transcribing` (fin en cours), `level` (niveau courant de 0 à 1, à l'échelle de voxtype ;
  // la source le met à jour à son rythme et le lisse si elle est lente), `finals` (texte figé
  // de la dictée) et `partial` (phrase en cours). Le micro et l'onde vont en bas de l'écran actif,
  // le texte, s'il y en a, juste sous l'île (le bandeau, bandeau/).
  property var dictationItems: []
  function collectDictations() {
    var list = []
    for (var i = 0; i < dictationLoaders.count; i++) {
      var ld = dictationLoaders.itemAt(i)
      if (ld && ld.item) list.push(ld.item)
    }
    dictationItems = list
  }
  Repeater {
    id: dictationLoaders
    model: moduleRegistry.slots["dictation"] || []
    delegate: Loader {
      required property string modelData
      source: modelData
      onLoaded: root.collectDictations()
    }
    onItemAdded: Qt.callLater(root.collectDictations)
    onItemRemoved: Qt.callLater(root.collectDictations)
  }
  readonly property var dictation: {
    var list = dictationItems
    for (var i = 0; i < list.length; i++) if (list[i].listening || list[i].transcribing) return list[i]
    return null
  }

  // La dictée en cours : voxtype ou la source du slot qui dicte.
  readonly property bool voxListening: voxtypeListening || (!!dictation && dictation.listening)
  readonly property bool voxTranscribing: voxtypeTranscribing || (!!dictation && dictation.transcribing)
  // Une seule onde pour toutes les sources : chacune ne donne que son niveau courant, que le
  // bandeau échantillonne à la cadence de voxtype (bandeau/Bandeau.qml, « dictée »).
  readonly property real voxLevel: dictation && dictation.listening ? Number(dictation.level) || 0 : voxtypeLevel
  readonly property string dictationFinals: dictation ? String(dictation.finals || "") : ""
  readonly property string dictationPartial: dictation ? String(dictation.partial || "") : ""
  // La dictée s'affiche dans le bandeau : la pastille en bas, le texte sous l'île.
  Binding { target: Bandeau; property: "dictationListening"; value: root.voxListening }
  Binding { target: Bandeau; property: "dictationTranscribing"; value: root.voxTranscribing }
  Binding { target: Bandeau; property: "dictationLevel"; value: root.voxLevel }
  Binding { target: Bandeau; property: "dictationFinals"; value: root.dictationFinals }
  Binding { target: Bandeau; property: "dictationPartial"; value: root.dictationPartial }

  // Sons de dictée (poste-sfx) : boucle pendant l'écoute, « stop » à la fin.
  readonly property string sfx: root.home + "/.config/omarchy/plugins/poste.island/bin/poste-sfx"
  // Verrouillage / déverrouillage (écran de verrouillage d'Omarchy, raccourci
  // comme mise en veille automatique) : sons « lock » et « unlock ». Le service
  // de verrouillage n'est pas exposé aux plugins ; comme omarchy-hyprland-
  // session-locked, on lit « LOCK » dans solitaryBlockedBy des écrans Hyprland.
  // SUPER+L passe par l'île (lockNow) : son immédiat, puis verrouillage. La
  // détection ne sert qu'au verrouillage automatique et au déverrouillage :
  // relue toutes les 60 ms tant que c'est verrouillé (700 ms sinon), et à
  // chaque événement Hyprland.
  property bool sessionLocked: false
  property bool lockKnown: false
  property bool lockAnnounced: false
  function lockNow() {
    lockAnnounced = true
    Quickshell.execDetached(["omarchy-system-lock"])
    lockSoundDelay.restart()   // le son suit l'apparition de l'écran de verrouillage
  }
  Timer { id: lockSoundDelay; interval: 175; onTriggered: Quickshell.execDetached([root.sfx, "lock"]) }
  function readLock() {
    var mons = Hyprland.monitors.values || []
    var locked = false
    for (var i = 0; i < mons.length; i++) {
      var info = mons[i].lastIpcObject || {}
      if ((info.solitaryBlockedBy || []).indexOf("LOCK") !== -1) locked = true
    }
    if (locked !== root.sessionLocked) root.sessionLocked = locked
  }
  function probeLock() {
    Hyprland.refreshMonitors()
    Qt.callLater(root.readLock)
    lockReread.restart()
  }
  // refreshMonitors() répond de façon asynchrone : relecture juste après.
  Timer { id: lockReread; interval: 30; onTriggered: root.readLock() }
  Timer {
    interval: root.sessionLocked ? 60 : 700
    repeat: true
    running: true
    onTriggered: root.probeLock()
  }
  Connections {
    target: Hyprland
    function onRawEvent(event) { if (root.sessionLocked) root.probeLock() }
  }
  onSessionLockedChanged: {
    if (lockKnown && !(sessionLocked && lockAnnounced))
      Quickshell.execDetached([root.sfx, sessionLocked ? "lock" : "unlock"])
    if (!sessionLocked) lockAnnounced = false
    lockKnown = true
  }
  Process {
    id: voxLoop
    running: root.voxListening
    command: ["sh", "-c", 'while :; do "$1" dictation-start || exit; done', "vox-loop", root.sfx]
  }
  // Le son d'écoute tourne en boucle (voxLoop) ; à l'arrêt, « stop ». Même chose pour
  // voxtype et pour la source du slot « dictation ».
  onVoxListeningChanged: if (!voxListening) Quickshell.execDetached([root.sfx, "dictation-stop"])

  // ---------------------------------------------------------------- AI usage
  // Records written by omarchy-agent-usage-update into
  // ~/.local/state/omarchy/agents/usage/<id>.json (one per agent): limits are
  // [{ label, percent (0..1), resetsAt (ISO), title? }]. The native agents
  // widget that ran the updater isn't mounted without the bar, so the island
  // runs it: every 5 minutes, and a cheaper --limits-only pass when the
  // control center opens. A run in progress absorbs further requests.
  // OBS : enregistrement / diffusion (ObsState.qml, bin/poste-obs).
  ObsState { id: obsState }
  // Slot « capture » : un module peut fournir sa propre source d'enregistrement (élément
  // de même interface qu'ObsState : active, recording, paused, recordElapsed, request…).
  // La pastille de capture et sa carte suivent celle qui enregistre, OBS sinon.
  property var captureItems: []
  function collectCaptures() {
    var list = []
    for (var i = 0; i < captureLoaders.count; i++) {
      var ld = captureLoaders.itemAt(i)
      if (ld && ld.item) list.push(ld.item)
    }
    captureItems = list
  }
  Repeater {
    id: captureLoaders
    model: moduleRegistry.slots["capture"] || []
    delegate: Loader {
      required property string modelData
      source: modelData
      onLoaded: root.collectCaptures()
    }
    onItemAdded: Qt.callLater(root.collectCaptures)
    onItemRemoved: Qt.callLater(root.collectCaptures)
  }
  // Cible encadrée pendant le choix dans une extension du lanceur qui le demande
  // (« highlight »: true) : { x, y, w, h, window } en coordonnées globales Hyprland, ou null.
  property var recordHighlight: null
  readonly property var obs: {
    var list = captureItems
    for (var i = 0; i < list.length; i++) if (list[i].active) return list[i]
    if (obsState.active) return obsState
    return list.length > 0 ? list[0] : obsState
  }

  // Batteries des périphériques sans fil (clavier, souris, casque) : DevicesState.qml.
  DevicesState { id: devicesState }
  readonly property var devices: devicesState

  // La batterie (activité de l'île, portables seulement) : BatteryState.qml.
  BatteryState { id: batteryState; profile: powerProfileState }
  readonly property var battery: batteryState

  // Le profil d'alimentation (centre de contrôle, activité de la batterie) :
  // PowerProfileState.qml. Chaque changement, d'où qu'il vienne, passe en OSD dans le bandeau.
  PowerProfileState {
    id: powerProfileState
    onSwitched: function(name) { Bandeau.showOsd(glyphOf(name), label(name), "", "100", "", "1600") }
  }
  readonly property var powerProfile: powerProfileState

  // La conversation avec l'IA, commune à tous les écrans (AskService.qml).
  AskService { id: aiService }
  readonly property var ai: aiService

  // Fournisseur affiché par la vue « Usage » (claude ou codex).
  property string usageProvider: "claude"
  readonly property string agentUsageDir: home + "/.local/state/omarchy/agents/usage"
  property var agentUsage: []
  property string agentPending: ""
  function runAgentUpdate(kind) {
    if (agentUpdate.running) { if (agentPending === "") agentPending = kind; return }
    agentUpdate.command = kind === "limits" ? ["omarchy-agent-usage-update", "--limits-only"] : ["omarchy-agent-usage-update"]
    agentUpdate.running = true
  }
  Process {
    id: agentUpdate
    onExited: {
      agentRead.running = true
      if (root.agentPending !== "") { var k = root.agentPending; root.agentPending = ""; root.runAgentUpdate(k) }
    }
  }
  Timer { interval: 300000; running: true; repeat: true; triggeredOnStart: true; onTriggered: root.runAgentUpdate("normal") }
  Process {
    id: agentRead
    command: ["sh", "-c", "for f in \"$1\"/*.json; do jq -c . \"$f\" 2>/dev/null; done", "read", root.agentUsageDir]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        var out = []
        var lines = String(text || "").split("\n")
        for (var i = 0; i < lines.length; i++) {
          var r
          try { r = JSON.parse(lines[i]) } catch (e) { continue }
          // Unavailable agents (e.g. no account) and ones without limits are skipped.
          if (!r || r.ready === false || String(r.usageStatusText || "") !== "" || !Array.isArray(r.limits) || !r.limits.length) continue
          out.push(r)
        }
        out.sort(function(a, b) { return String(a.name).localeCompare(String(b.name)) })
        root.agentUsage = out
      }
    }
  }
  Timer { interval: 50; running: true; onTriggered: agentRead.running = true }

  // ---------------------------------------------------------------- mail (Omamail)
  // Omamail publishes a detached, credential-free snapshot through its
  // bar/Bridge.js library for replacement bars. It is loaded at runtime so the
  // island still works when the plugin is absent.
  readonly property string mailBridgeUrl: "file://" + home + "/.config/omarchy/plugins/omamail/ui/bar/Bridge.js"
  property var mailBridge: null
  property bool mailAvailable: false
  property int mailUnread: 0
  function openMail() {
    if (shell && typeof shell.toggle === "function") shell.toggle("omamail", "{}")
    else Quickshell.execDetached(["omarchy-shell", "shell", "toggle", "omamail", "{}"])
  }
  Timer {
    interval: 2000; running: true; repeat: true; triggeredOnStart: true
    onTriggered: {
      if (!root.mailBridge) {
        try {
          root.mailBridge = Qt.createQmlObject('import QtQuick; import "' + root.mailBridgeUrl
            + '" as B; QtObject { function current() { return B.current() } }', root, "mailBridge")
        } catch (e) { root.mailAvailable = false; root.mailUnread = 0; return }
      }
      var api = root.mailBridge.current()
      var snap = api ? api.snapshot() : null
      root.mailAvailable = true
      root.mailUnread = snap && snap.ready ? snap.unreadTotal : 0
    }
  }

  // ---------------------------------------------------------------- shared state
  // Each screen's open view and island bottom edge, written to
  // $XDG_RUNTIME_DIR/poste-island.json (reset on unload). The toasts do not
  // read it (the bandeau gets the edge directly, reportEdge); the island still uses
  // the views to refresh the list while it is open.
  readonly property string statePath: (Quickshell.env("XDG_RUNTIME_DIR") || "/tmp") + "/poste-island.json"
  property var islandStates: ({})
  function reportState(screenName, view, bottom) {
    var next = {}
    for (var k in islandStates) next[k] = islandStates[k]
    next[screenName] = { view: view, bottom: Math.round(bottom) }
    islandStates = next
    stateWriteTimer.restart()
  }
  Timer {
    id: stateWriteTimer
    interval: 16
    onTriggered: stateFile.setText(JSON.stringify({ screens: root.islandStates }))
  }
  FileView { id: stateFile; path: root.statePath; printErrors: false }
  Component.onDestruction: {
    stateFile.setText(JSON.stringify({ screens: {} }))
    Bandeau.dictationListening = false
    Bandeau.dictationTranscribing = false
    Bandeau.islandEdges = ({})
  }
  // Refresh the list quickly while it is open, so arrivals slide in.
  readonly property bool anyListOpen: {
    for (var k in islandStates) if (islandStates[k].view === "notifications") return true
    return false
  }
  Timer { interval: 900; running: root.anyListOpen; repeat: true; onTriggered: root.refreshStore() }

  // ---------------------------------------------------------------- timers
  // Minuteurs lancés depuis le lanceur (« timer 10m pâtes ») ou par IPC
  // (omarchy-shell poste.island timer "10m pâtes"). Chacun s'affiche dans sa
  // petite île à droite de l'île principale (TimerPill), dont le fond se vide
  // avec le temps. Fin : son en boucle et pastille qui pulse jusqu'au clic. Survivent à un redémarrage
  // du shell (~/.local/state/poste-island/timers.json).
  // Les rappels d'Omarchy (omarchy-reminder) s'y ajoutent à l'affichage
  // (allTimers) sans être stockés : systemd reste leur source de vérité.
  property var timers: []
  property var omarchyReminders: []
  readonly property var allTimers: timers.concat(omarchyReminders)
  property double now: Date.now()
  readonly property string timersPath: root.home + "/.local/state/poste-island/timers.json"
  readonly property bool timersTicking: {
    if (omarchyReminders.length > 0) return true
    for (var i = 0; i < timers.length; i++) if (!timers[i].paused && !timers[i].done) return true
    return false
  }
  Timer {
    interval: 250
    repeat: true
    running: root.timersTicking
    onTriggered: { root.now = Date.now(); root.checkTimers() }
  }
  FileView {
    id: timersFile
    path: root.timersPath
    printErrors: false
    onLoaded: {
      try { var list = JSON.parse(text()); if (Array.isArray(list)) root.timers = list } catch (e) {}
      root.now = Date.now()
      root.checkTimers()
    }
  }
  function saveTimers() { timersFile.setText(JSON.stringify(timers)) }
  function setTimers(list) { timers = list; saveTimers() }
  function timerLeft(t) { return t.done ? 0 : t.paused ? t.left : Math.max(0, t.end - now) }
  // « 10m », « 1h30 », « 90s », « 1:30 », « 25 » (minutes), « 2 min »…
  function parseDuration(text) {
    var m = String(text || "").trim().match(/^((?:\d+(?:[.,]\d+)?\s*(?:h|hr|hrs|hours?|heures?|m|mn|min|mins?|minutes?|s|sec|secs?|seconds?|secondes?)?\s*)+|\d+:\d{1,2}(?::\d{1,2})?)(?:\s+(.*))?$/i)
    if (!m) return null
    var spec = m[1].trim(), ms = 0
    if (/^\d+:\d/.test(spec)) {
      var parts = spec.split(":").map(Number)
      ms = parts.length === 3 ? ((parts[0] * 60 + parts[1]) * 60 + parts[2]) * 1000 : (parts[0] * 60 + parts[1]) * 1000
    } else {
      var re = /(\d+(?:[.,]\d+)?)\s*([a-z]*)/gi, g, lastUnit = ""
      while ((g = re.exec(spec)) !== null) {
        var n = Number(g[1].replace(",", ".")), u = g[2].toLowerCase()
        if (!u) u = lastUnit === "h" ? "m" : lastUnit === "m" ? "s" : "m"   // « 1h30 », « 5m30 », « 25 »
        var unit = u.charAt(0)
        ms += n * (unit === "h" ? 3600000 : unit === "s" ? 1000 : 60000)
        lastUnit = unit
      }
    }
    if (!(ms >= 1000) || ms > 24 * 3600000) return null
    return { ms: Math.round(ms), label: String(m[2] || "").trim() }
  }
  function formatLeft(ms) {
    var s = Math.ceil(ms / 1000), h = Math.floor(s / 3600), mm = Math.floor((s % 3600) / 60), ss = s % 60
    return (h ? h + ":" + (mm < 10 ? "0" : "") : "") + mm + ":" + (ss < 10 ? "0" : "") + ss
  }
  function startTimer(ms, label) {
    now = Date.now()
    setTimers(timers.concat([{ id: String(now), label: String(label || ""), duration: ms, end: now + ms,
                              paused: false, left: ms, done: false }]))
  }
  function updateTimer(id, fn) {
    setTimers(timers.map(function(t) { if (t.id !== id) return t; var c = {}; for (var k in t) c[k] = t[k]; fn(c); return c }))
  }
  function cancelTimer(id) {
    if (isReminder(id)) { cancelReminder(id); return }
    setTimers(timers.filter(function(t) { return t.id !== id }))
  }
  function togglePause(id) {
    if (isReminder(id)) return   // un minuteur systemd ne se met pas en pause
    now = Date.now()
    updateTimer(id, function(t) {
      if (t.done) return
      if (t.paused) { t.end = now + t.left; t.paused = false }
      else { t.left = Math.max(0, t.end - now); t.paused = true }
    })
  }
  function addToTimer(id, ms) {
    if (isReminder(id)) return   // ni prolongé : l'unité systemd n'est pas la nôtre
    now = Date.now()
    updateTimer(id, function(t) {
      if (t.done) { t.done = false; t.end = now + ms; t.duration = ms; return }
      if (t.paused) t.left += ms; else t.end += ms
      t.duration += ms
    })
  }
  function checkTimers() {
    var changed = false
    var list = timers.map(function(t) {
      if (t.done || t.paused || t.end > now) return t
      changed = true
      var c = {}; for (var k in t) c[k] = t[k]
      c.done = true
      root.ringTimer(c)
      return c
    })
    if (changed) setTimers(list)
    // Un rappel dont l'heure est passée disparaît tout de suite, sans attendre
    // la prochaine lecture : l'unité a sonné, sa notification est l'alerte.
    var live = omarchyReminders.filter(function(r) { return r.end > now })
    if (live.length !== omarchyReminders.length) omarchyReminders = live
  }
  function ringTimer(t) { }  // le son part tout seul (ringProc) tant qu'un minuteur est terminé
  // Terminé : « complete » (jeu cinematic, uisfx, CC0) en boucle jusqu'au clic
  // qui ferme la pastille, 2 min au plus.
  readonly property bool timerRinging: {
    for (var i = 0; i < timers.length; i++) if (timers[i].done) return true
    return false
  }
  Process {
    id: ringProc
    running: root.timerRinging
    command: ["timeout", "120", "sh", "-c", 'while :; do "$1" timer-done; sleep 1.4; done', "ring", root.sfx]
  }

  // ---------------------------------------------------------------- rappels d'Omarchy
  // `omarchy-reminder <minutes> [message]` crée une unité systemd transitoire
  // omarchy-reminder-<N>m-<instant>.timer ; son message, s'il y en a un, est
  // dans $XDG_RUNTIME_DIR/omarchy-reminders/<unité>.message. Omarchy les
  // montrait dans sa barre, que l'île remplace : on les lit donc ici et on les
  // affiche comme des minuteurs (même bulle, même carte). Rien n'est écrit dans
  // timers.json, systemd reste la source de vérité. Pas de pause ni de +1 min
  // (un minuteur systemd ne se suspend pas, et recréer l'unité ferait dépendre
  // l'île de la commande interne d'Omarchy) ; pas de sonnerie à la fin non
  // plus : l'unité envoie sa propre notification, qui est l'alerte, et la
  // bulle disparaît à l'heure dite. Sans systemctl ni unité, il n'y a rien.
  // Lecture toutes les 5 s (un `systemctl list-timers` ; pas de signal à
  // écouter pour une unité transitoire), et aussitôt après une annulation.
  property var remindersGone: []   // unités annulées d'ici, ignorées jusqu'à ce que systemd les ait lâchées
  readonly property string remindersMarker: "--messages--"
  function isReminder(id) { return String(id).indexOf("omarchy:") === 0 }
  function pollReminders() { if (!remindersProc.running) remindersProc.running = true }
  function applyReminders(out) {
    var cut = out.indexOf(remindersMarker + "\n")
    var json = cut < 0 ? out : out.substring(0, cut)
    var msgs = {}
    if (cut >= 0) out.substring(cut + remindersMarker.length + 1).split("\n").forEach(function(line) {
      var tab = line.indexOf("\t")
      if (tab > 0) msgs[line.substring(0, tab)] = line.substring(tab + 1).trim()
    })
    var units = []
    try { units = JSON.parse(json) } catch (e) {}
    if (!Array.isArray(units)) units = []
    var at = Date.now(), list = [], seen = []
    units.forEach(function(u) {
      var unit = String(u.unit || "").replace(/\.timer$/, "")
      if (!/^omarchy-reminder-[A-Za-z0-9_.-]+$/.test(unit)) return
      seen.push(unit)
      var end = Number(u.next) / 1000      // microsecondes depuis l'epoch
      if (!(end > at) || remindersGone.indexOf(unit) !== -1) return
      var m = unit.match(/^omarchy-reminder-(\d+)m-/)
      list.push({ id: "omarchy:" + unit, unit: unit, label: msgs[unit + ".message"] || "Rappel",
                  duration: Math.max(m ? Number(m[1]) * 60000 : 0, end - at), end: end,
                  paused: false, left: 0, done: false, omarchy: true })
    })
    remindersGone = remindersGone.filter(function(u) { return seen.indexOf(u) !== -1 })
    // Pas de nouvelle liste si rien n'a changé : la bande d'activités ne se reconstruit pas pour rien.
    var sig = function(l) { return l.map(function(r) { return r.id + "|" + r.end + "|" + r.label + "|" + r.duration }).join("\n") }
    if (sig(list) !== sig(omarchyReminders)) { now = at; omarchyReminders = list }
  }
  // Annule un rappel : arrête son minuteur systemd et retire son message, comme
  // `omarchy-reminder clear` le fait pour tous.
  function cancelReminder(id) {
    var unit = String(id).substring(8)
    if (!/^omarchy-reminder-[A-Za-z0-9_.-]+$/.test(unit)) return
    remindersGone = remindersGone.concat([unit])
    omarchyReminders = omarchyReminders.filter(function(r) { return r.id !== id })
    Quickshell.execDetached(["sh", "-c",
      'systemctl --user stop "$1.timer" 2>/dev/null; rm -f "${XDG_RUNTIME_DIR:-/tmp}/omarchy-reminders/$1.message"',
      "reminder-stop", unit])
    remindersRecheck.restart()
  }
  Timer { id: remindersRecheck; interval: 700; onTriggered: root.pollReminders() }
  Timer { interval: 5000; running: true; repeat: true; triggeredOnStart: true; onTriggered: root.pollReminders() }
  Process {
    id: remindersProc
    command: ["sh", "-c",
      'command -v systemctl >/dev/null 2>&1 || exit 0\n' +
      'systemctl --user list-timers --all --no-pager --output=json "omarchy-reminder-*.timer" 2>/dev/null\n' +
      'printf "\\n%s\\n" "$1"\n' +
      'for f in "${XDG_RUNTIME_DIR:-/tmp}"/omarchy-reminders/omarchy-reminder-*.message; do\n' +
      '  [ -f "$f" ] || continue\n' +
      '  printf "%s\\t" "${f##*/}"; tr "\\n\\t\\r" "   " < "$f"; printf "\\n"\n' +
      'done', "reminders", root.remindersMarker]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: root.applyReminders(text)
    }
  }


  // ---------------------------------------------------------------- shelf (Flea)
  // Bac de fichiers de Flea, affiché par l'île (vue « shelf ») : la pile, les
  // épinglés et les dernières captures viennent de la logique de Flea
  // (/usr/share/flea/shelf, paquet flea) ; l'île ne fait que l'affichage.
  // SUPER+D (flea shelf toggle) sonne via summon.json.
  readonly property bool shelfAvailable: shelfProbe.count > 0
  FolderListModel {
    id: shelfProbe
    folder: "file:///usr/share/flea/shelf"
    nameFilters: ["ShelfService.qml"]
    showDirs: false
  }
  readonly property bool shelfDrawing: {
    for (var k in islandStates) if (islandStates[k] && islandStates[k].view === "shelf") return true
    return false
  }
  property var shelf: shelfLoader.item
  property var shelfActions: shelfActionsLoader.item
  Loader {
    id: shelfLoader
    active: root.shelfAvailable
    sourceComponent: Flea.ShelfService {
      drawing: root.shelfDrawing
      onSummoned: root.openModuleOn(root.focusedScreen, "shelf")
    }
  }
  Loader {
    id: shelfActionsLoader
    active: root.shelfAvailable
    sourceComponent: Flea.ShelfActions {
      fleaCommand: root.shelf ? root.shelf.fleaCommand : "flea"
      onRan: if (root.shelf) root.shelf.reread()
    }
  }
  // Tout ce qui entre dans le Shelf y reste jusqu'à ce qu'on le retire : on
  // l'y met « épinglé » (Flea ne retire alors rien après un déplacement ou un
  // zip), y compris ce qui arrive par « Add to shelf » dans Flea.
  function addToShelf(paths) { if (shelf && paths && paths.length) shelf.pinAll(paths, true) }
  Connections {
    target: root.shelf
    ignoreUnknownSignals: true
    function onPileChanged() {
      // Seuls les nouveaux éléments : un élément qu'on désépingle pour le
      // retirer ne doit pas être réépinglé entre-temps.
      var items = root.shelf.pile && root.shelf.pile.items ? root.shelf.pile.items : []
      var known = {}
      var fresh = []
      for (var i = 0; i < items.length; i++) {
        known[items[i].path] = true
        if (!items[i].pinned && !root.shelfKnown[items[i].path]) fresh.push(items[i].path)
      }
      root.shelfKnown = known
      if (fresh.length) root.shelf.pinAll(fresh, true)
    }
  }
  property var shelfKnown: ({})

  // ---------------------------------------------------------------- IPC
  // omarchy-shell poste.island player | close
  // omarchy-shell poste.island controls | notifications
  signal closeAll()
  signal openPlayerOn(string screenName)
  signal toggleControlsOn(string screenName)
  signal openModuleOn(string screenName, string module)
  signal toggleNotificationsOn(string screenName)
  signal toggleLauncherOn(string screenName)
  signal openLauncherExtOn(string screenName, string ext)
  signal toggleMenuOn(string screenName)
  // Question à l'IA (vue « ask ») : texte pré-rempli, envoyé tout de suite si sendNow.
  signal openAskOn(string screenName, string text, bool sendNow)
  function openAsk(text, sendNow) { openAskOn(focusedScreen, text || "", !!sendNow) }
  IpcHandler {
    target: "poste.island"
    function player(): string { root.openPlayerOn(root.focusedScreen); return "ok" }
    function controls(): string { root.toggleControlsOn(root.focusedScreen); return "ok" }
    // Control-center modules: wifi | bluetooth | audio | vpn | tailscale.
    function wifi(): string { root.openModuleOn(root.focusedScreen, "wifi"); return "ok" }
    function bluetooth(): string { root.openModuleOn(root.focusedScreen, "bluetooth"); return "ok" }
    function audio(): string { root.openModuleOn(root.focusedScreen, "audio"); return "ok" }
    function vpn(): string { root.openModuleOn(root.focusedScreen, "vpn"); return "ok" }
    function tailscale(): string { root.openModuleOn(root.focusedScreen, "tailscale"); return "ok" }
    function module(name: string): string { root.openModuleOn(root.focusedScreen, name); return "ok" }
    function launcher(): string { root.toggleLauncherOn(root.focusedScreen); return "ok" }
    // Le lanceur ouvert directement dans une extension (« record », « ssh »…).
    function launcherIn(ext: string): string { root.openLauncherExtOn(root.focusedScreen, ext); return "ok" }
    // omarchy-shell poste.island ask "question" (vide = ouvre la vue)
    function ask(question: string): string { root.openAsk(question, question !== ""); return "ok" }
    // omarchy-shell poste.island timer "10m pâtes"
    function timer(spec: string): string {
      var d = root.parseDuration(spec)
      if (!d) return "invalid duration"
      root.startTimer(d.ms, d.label)
      return "ok"
    }
    // omarchy-shell poste.island timerStop : arrête tous les minuteurs.
    // omarchy-shell poste.island lock : son « lock » immédiat puis verrouillage (SUPER+L).
    function lock(): string { root.lockNow(); return "ok" }
    function timerStop(): string { root.setTimers([]); return "ok" }
    // omarchy-shell poste.island timerCancel <id> : annule un seul minuteur.
    // Un rappel d'Omarchy se désigne par « omarchy:<unité> ».
    function timerCancel(id: string): string {
      if (root.isReminder(id)) root.cancelReminder(id)
      else root.setTimers(root.timers.filter(function(t) { return String(t.id) !== String(id) }))
      return "ok"
    }
    function shelf(): string { root.openModuleOn(root.focusedScreen, "shelf"); return "ok" }
    // omarchy-shell poste.island usage : détail de la consommation de Claude Code.
    // omarchy-shell poste.island batterySim "charging 75" | "low 15" | "critical 6" | "off" :
    // simule une batterie (pour voir son activité sur un PC qui n'en a pas).
    function batterySim(spec: string): string { return root.battery.simulate(spec) }
    // omarchy-shell poste.island profileSim "power-saver" | "balanced" | "performance" | "off" :
    // simule le profil d'alimentation (un choix dans le centre de contrôle ne change alors que
    // la simulation). profile : le profil en place et ceux de la machine (JSON).
    function profileSim(spec: string): string { return root.powerProfile.simulate(spec) }
    function profile(): string {
      return JSON.stringify({ active: root.powerProfile.present ? root.powerProfile.active : "",
        profiles: root.powerProfile.shownProfiles, simulated: root.powerProfile.simulated })
    }
    // omarchy-shell poste.island obs <Requête OBS> : ToggleRecord, StopRecord,
    // ToggleRecordPause, SaveReplayBuffer… (pour un raccourci clavier).
    function obs(kind: string): string { root.obs.request(kind || "ToggleRecord"); return "ok" }
    // omarchy-shell poste.island devices : batteries des périphériques (JSON).
    function devices(): string { return JSON.stringify(root.devices.list) }
    // omarchy-shell poste.island deviceSim "mouse 12" | "off" : simule un périphérique faible.
    function deviceSim(spec: string): string { return root.devices.simulate(spec) }
    // omarchy-shell poste.island askBg "question" [claude|codex] : lance la
    // question en arrière-plan, dans une pastille épinglée (vue non ouverte).
    function askBg(question: string): string { return root.ai.spawn(question, "") ? "ok" : "empty" }
    function askBgWith(question: string, provider: string): string { return root.ai.spawn(question, provider) ? "ok" : "empty" }
    // omarchy-shell poste.island askUnpinAll : retire toutes les pastilles IA
    // au repos (celles qui travaillent restent).
    function askUnpinAll(): string { return String(root.ai.unpinAll()) }
    // omarchy-shell poste.island askPin : range la conversation IA courante dans
    // sa pastille de droite (vue fermée), ou l'en retire si elle y est déjà.
    function askPin(): string {
      var c = root.ai.current
      if (!c || c.messages.count === 0) return "empty"
      if (c.pinned && !root.ai.viewOpen) { c.pinned = false; return "unpinned" }
      c.pinned = true; root.closeAll(); return "pinned"
    }
    function usage(): string { root.usageProvider = "claude"; root.openModuleOn(root.focusedScreen, "usage"); return "ok" }
    function usageOf(provider: string): string { root.usageProvider = provider || "claude"; root.openModuleOn(root.focusedScreen, "usage"); return "ok" }
    function menu(): string { root.toggleMenuOn(root.focusedScreen); return "ok" }
    function notifications(): string { root.toggleNotificationsOn(root.focusedScreen); return "ok" }
    function unread(): string { return String(root.unreadCount) }
    function close(): string { root.closeAll(); return "ok" }
  }

  // ---------------------------------------------------------------- windows
  Variants {
    model: Quickshell.screens
    delegate: Component {
      Item {
        id: screenScope
        required property var modelData

        // Reserves the strip the island sits in, so tiled windows start
        // below it (the island window itself ignores exclusive zones).
        PanelWindow {
          screen: screenScope.modelData
          color: "transparent"
          anchors { top: true; left: true; right: true }
          readonly property bool fullscreen: root.fullscreenScreens[String(screenScope.modelData.name)] === true
          implicitHeight: Math.max(1, fullscreen ? 1 : root.barSize)
          exclusiveZone: fullscreen ? 0 : root.barSize
          WlrLayershell.namespace: "poste-island-reserve"
          WlrLayershell.layer: WlrLayer.Bottom
          mask: Region {}

          // Invisible anchors for the native panels, stacked on the screen's
          // center so every card opens right under the island.
          Item {
            anchors.fill: parent
            opacity: 0
            Repeater {
              model: root.nativePanelIdsFor(screenScope.modelData.name)
              delegate: Loader {
                id: nativeLoader
                required property string modelData
                anchors.centerIn: parent
                readonly property var entry: root.barWidgetRegistry && root.barWidgetRegistry.widgets
                  ? root.barWidgetRegistry.widgets[modelData] : null
                sourceComponent: entry ? entry.component : null
                onLoaded: {
                  if ("bar" in item) item.bar = root
                  if ("moduleName" in item) item.moduleName = modelData
                  if ("settings" in item) item.settings = root.barEntrySettings(modelData)
                  root.registerNativeWidget(modelData, String(screenScope.modelData.name), item)
                  if (modelData === root.vpnId) root.vpnService = root.findVpnService(item)
                  if (modelData === root.tsId) root.tsService = root.findTsService(item)
                }
                Component.onDestruction: if (item) {
                  if (modelData === root.vpnId) root.vpnService = null
                  if (modelData === root.tsId) root.tsService = null
                  root.unregisterNativeWidget(item)
                }
              }
            }
          }
        }

        DimWindow {
          screen: screenScope.modelData
          shown: islandWindow.dimWanted
          onDismissed: islandWindow.view = "rest"
        }

        // Cadre autour de la cible d'enregistrement sélectionnée dans le
        // lanceur : au-dessus du voile, sous l'île, sans prendre la souris.
        PanelWindow {
          id: highlightWindow
          screen: screenScope.modelData
          readonly property var hl: root.recordHighlight
          readonly property real sx: screenScope.modelData.x
          readonly property real sy: screenScope.modelData.y
          readonly property bool here: !!hl && hl.x < sx + screenScope.modelData.width && hl.x + hl.w > sx
            && hl.y < sy + screenScope.modelData.height && hl.y + hl.h > sy
          visible: here
          color: "transparent"
          surfaceFormat.opaque: false
          exclusionMode: ExclusionMode.Ignore
          anchors { top: true; left: true; right: true; bottom: true }
          WlrLayershell.namespace: "poste-island-record-target"
          WlrLayershell.layer: WlrLayer.Top
          WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
          mask: Region {}
          Rectangle {
            visible: highlightWindow.here
            readonly property real pad: highlightWindow.hl && highlightWindow.hl.window ? 3 : 0
            x: highlightWindow.here ? highlightWindow.hl.x - highlightWindow.sx - pad : 0
            y: highlightWindow.here ? highlightWindow.hl.y - highlightWindow.sy - pad : 0
            width: highlightWindow.here ? highlightWindow.hl.w + 2 * pad : 0
            height: highlightWindow.here ? highlightWindow.hl.h + 2 * pad : 0
            radius: highlightWindow.hl && highlightWindow.hl.window ? 11 : 0
            color: root.withAlpha(root.colorAccent, 0.08)
            border.width: 5
            border.color: root.colorAccent
            Behavior on x { NumberAnimation { duration: 160; easing.type: Easing.OutCubic } }
            Behavior on y { NumberAnimation { duration: 160; easing.type: Easing.OutCubic } }
            Behavior on width { NumberAnimation { duration: 160; easing.type: Easing.OutCubic } }
            Behavior on height { NumberAnimation { duration: 160; easing.type: Easing.OutCubic } }
          }
        }

        IslandWindow {
          id: islandWindow
          host: root
          screen: screenScope.modelData
        }

        // Rail de bord du Shelf (réglage « Edge rail » de Flea), écran du milieu.
        Loader {
          active: root.shelfAvailable && !!root.shelf && root.shelf.railEdge !== "off"
            && String(screenScope.modelData.name) === root.primaryScreenName
          sourceComponent: Flea.ShelfRail {
            screen: screenScope.modelData
            edge: root.shelf.railEdge
            dwellMs: root.shelf.railDwellMs
            held: root.shelf.count
            accent: root.colorAccent
            onDropped: function(paths) { root.addToShelf(paths) }
            onOpened: root.openModuleOn(String(screenScope.modelData.name), "shelf")
          }
        }
      }
    }
  }
}
