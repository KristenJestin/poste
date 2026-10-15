pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import Quickshell.Services.Pipewire
import qs.Commons
import "file:///usr/share/omarchy/shell/plugins/osd/OsdModel.js" as OsdModel

// The banner: what passes on the screen (notifications, OSD, dictation), with a single look, that of
// the island (solid background, light text, accent icon, pill-shaped rounding).
//
// A single object for the whole shell: poste.island imports it with `import "bandeau"`,
// poste.notifications and poste.osd with `import "../poste.island/bandeau"`. The three paths
// lead to the same folder, hence to the same singleton (checked with a throwaway Quickshell config,
// linked folder and files linked one by one like on the machine). It holds the state and draws one
// window per screen (BandeauWindow.qml), created on first use.
//
// Two places:
// - Just under the island, a stack that follows its bottom edge (that of the open view if there is one):
//   from top to bottom, the dictated text, the OSD, then the notifications (three at most, the most
//   recent on top). The island is never covered or enlarged.
// - At the bottom of the active screen, alone, the dictation pill: the microphone and the wave while
//   listening, the dots during the transcription.
//
// Who feeds it:
// - OSD: poste.osd (clone of omarchy.osd) calls showOsdPayload(Omarchy's JSON payload).
// - Volume: the banner itself follows PipeWire's default output (see 'volume').
// - Dictation: poste.island sets dictationListening, dictationTranscribing, dictationLevel (current
//   level of the source that is dictating), dictationFinals, dictationPartial.
// - Island edge: poste.island calls setIslandEdge(screen, y) at each frame of the spring.
// - Notifications: poste.notifications registers with attachNotifications(service). The service
//   keeps all its logic (server, do not disturb, history, toast screen); it must
//   offer popupModel, popupScreensRev, popupKey(row), popupOnScreen(key, screen),
//   durationFor(urgency, expireTimeout), present(row) (what the card shows, see
//   NotificationView.qml), closePopup(i), invokePopupDefault(i), expirePopup(i).
Singleton {
  id: root

  // ---------------------------------------------------------------- style
  // Text and accent of the island (Island.qml: colorText, colorAccent). The background is a dark glass:
  // the theme's background tinted 6 % toward the text, 86 % opaque, blurred by Hyprland
  // (looknfeel.lua) to stand out even from a black window.
  readonly property color colorSurface: Qt.tint(Color.background, withAlpha(Color.foreground, 0.06))
  readonly property real surfaceOpacity: 0.86
  readonly property color colorText: Qt.lighter(Color.foreground, 1.08)
  // Opaque grey (text mixed with the background): secondary text, sentence being dictated.
  readonly property color colorMuted: mix(colorText, colorSurface, 0.5)
  readonly property color colorAccent: Color.accent
  readonly property color colorUrgent: Color.urgent
  readonly property string fontFamily: "JetBrainsMono Nerd Font"
  readonly property int bottomMargin: 56
  readonly property int gap: 10
  // Under the island: more than the margins of Hyprland's windows (gaps_out 10), otherwise the first
  // piece starts right on the top edge of the windows.
  readonly property int islandGap: 20
  readonly property int maxNotifications: 3
  // Several rows: same rounding everywhere.
  readonly property int blockRadius: 20

  function withAlpha(c, a) { return Qt.rgba(c.r, c.g, c.b, a) }
  // `t` of color a, the rest of b.
  function mix(a, b, t) {
    return Qt.rgba(a.r * t + b.r * (1 - t), a.g * t + b.g * (1 - t), a.b * t + b.b * (1 - t), 1)
  }

  // Size change of a piece: `anim` (a NumberAnimation on the shown size) restarts
  // from the current value toward `target`, unless it is already going there or is there. Restarting from where it
  // is keeps the movement continuous when the content changes during the animation.
  function animateTo(anim, current, target) {
    if (anim.running ? anim.to === target : current === target) return
    anim.stop()
    anim.from = current
    anim.to = target
    anim.start()
  }

  readonly property string focusedScreen: Hyprland.focusedMonitor ? String(Hyprland.focusedMonitor.name) : ""

  // ---------------------------------------------------------------- OSD
  // A single place: a new OSD replaces the one shown, in place (the bar slides from
  // the old value to the new one), and its timer restarts.
  property bool osdOpen: false
  property var osd: ({ iconKey: "", icon: "", message: "", hasProgress: false, value: 0, maxValue: 100, media: false, dim: false })

  // `dim`: the bar turns grey (output muted).
  function showOsd(icon, message, value, max, progressText, duration, dim) {
    var next = OsdModel.stateForShow(icon, message, value, max, progressText, duration)
    next.media = next.iconKey.indexOf("media") === 0 || next.iconKey.indexOf("player") === 0
    next.dim = !!dim
    // The state before opening: a new OSD starts from its value, only an OSD already there slides.
    osd = next
    osdOpen = true
    osdTimer.interval = Math.max(1, next.duration)
    if (next.duration > 0) osdTimer.restart()
    else osdTimer.stop()
  }
  // Same reading of the payload as Omarchy's Osd.qml (open()).
  function showOsdPayload(payloadJson) {
    var p = null
    try { p = JSON.parse(payloadJson || "{}") } catch (e) { return false }
    if (!p || typeof p !== "object") return false
    // The volume comes from PipeWire (below), never from the payload: a single source, so
    // never two displays or two different states for the same change.
    if (String(p.icon || "").toLowerCase().indexOf("volume") === 0) return true
    showOsd(p.icon || "", p.message || "", p.value === undefined ? "" : String(p.value),
      p.max === undefined ? "100" : String(p.max), p.progressText || "",
      p.duration === undefined ? "1200" : String(p.duration))
    return true
  }
  function hideOsd() {
    osdTimer.stop()
    osdOpen = false
  }
  Timer {
    id: osdTimer
    onTriggered: root.osdOpen = false
  }

  // ---------------------------------------------------------------- volume
  // The volume of the default output, read from PipeWire: it takes every change, from
  // wherever it comes (keys, wheel, application, pactl), which the keys' OSD does not see.
  // Same icons as Omarchy's volume OSD (its script omarchy-audio-output-volume):
  // 'volume-muted' when the output is muted or at 0, otherwise 'volume-high'. Muted, the
  // bar keeps its level but turns grey.
  readonly property var sink: Pipewire.defaultAudioSink
  PwObjectTracker { objects: [root.sink] }
  readonly property real sinkVolume: sink && sink.audio ? sink.audio.volume : -1
  readonly property bool sinkMuted: !!(sink && sink.audio && sink.audio.muted)
  readonly property bool sinkReady: !!(sink && sink.ready)
  // The first reading of an output (startup, output change) is only kept.
  property var seenSink: null
  property real seenVolume: -1
  property bool seenMuted: false
  function volumeMoved() {
    if (!sink || !sink.ready || sinkVolume < 0) return
    var changed = seenSink === sink && (sinkVolume !== seenVolume || sinkMuted !== seenMuted)
    seenSink = sink
    seenVolume = sinkVolume
    seenMuted = sinkMuted
    if (!changed) return
    var percent = Math.round(sinkVolume * 100)
    var silent = sinkMuted || percent <= 0
    showOsd(silent ? "volume-muted" : "volume-high", "", String(Math.min(100, percent)), "100",
      percent + "%", "1600", sinkMuted)
  }
  onSinkVolumeChanged: volumeMoved()
  onSinkMutedChanged: volumeMoved()
  onSinkReadyChanged: volumeMoved()

  // ---------------------------------------------------------------- dictation
  // The bottom pill as soon as the dictation listens or transcribes, whatever the source; the
  // text under the island when the source gives some (Vocaline).
  property bool dictationListening: false
  property bool dictationTranscribing: false
  property real dictationLevel: 0
  property string dictationFinals: ""
  property string dictationPartial: ""
  readonly property bool dictListening: demoDictation || dictationListening
  readonly property bool dictTranscribing: !demoDictation && dictationTranscribing
  readonly property bool dictActive: dictListening || dictTranscribing
  readonly property real dictLevel: demoDictation ? demoLevel : dictationLevel
  readonly property string dictFinals: demoDictation ? demoFinals : dictationFinals
  readonly property string dictPartial: demoDictation ? demoPartial : dictationPartial
  readonly property bool dictTextShown: dictActive && (dictFinals !== "" || dictPartial !== "")

  // A single wave for all the sources: each only gives its current level,
  // sampled here 100 times per second into the bars' history, the
  // most recent last. The wave therefore moves the same whatever the source; it restarts from
  // zero at each dictation.
  readonly property int dictBars: 14
  property var dictLevels: []
  Timer {
    interval: 10
    repeat: true
    running: root.dictListening
    onTriggered: {
      var next = root.dictLevels.slice(Math.max(0, root.dictLevels.length - root.dictBars + 1))
      next.push(root.dictLevel)
      root.dictLevels = next
    }
  }
  onDictListeningChanged: if (!dictListening) dictLevels = []

  // ---------------------------------------------------------------- island edge
  // Bottom edge of the drawn island, per screen (poste.island, at each frame of the spring). Without an island
  // on the screen, the stack starts from the top.
  property var islandEdges: ({})
  function setIslandEdge(screenName, bottom) {
    var y = Math.max(0, Math.round(bottom))
    if (islandEdges[screenName] === y) return
    var next = {}
    for (var k in islandEdges) next[k] = islandEdges[k]
    next[screenName] = y
    islandEdges = next
  }

  // ---------------------------------------------------------------- notifications
  // The service registers here, each window reads its model (NotificationView.qml).
  property var notifications: null
  function attachNotifications(service) { notifications = service }
  function detachNotifications(service) { if (notifications === service) notifications = null }
  // During the demo, the real notifications wait (they stay in their service).
  readonly property var source: demoRunning ? demoSource : notifications

  // ---------------------------------------------------------------- windows
  Variants {
    model: Quickshell.screens
    delegate: Component {
      BandeauWindow {
        required property var modelData
        screen: modelData
        bandeau: root
      }
    }
  }

  // ---------------------------------------------------------------- demo
  // omarchy-shell bandeau demo: a rising volume OSD, a music OSD, two
  // notifications (one of which has a text that grows longer), a dictation (the pill and its wave at the bottom,
  // then a text under the island that goes onto two lines), then the volume muted, all invented, on
  // the active screen (the demo follows it). Nothing is sent to the notification server or written to
  // the history.
  property bool demoRunning: false
  property bool demoDictation: false
  // Invented level of the demo dictation: a voice that rises and falls, 25 times per second.
  property real demoLevel: 0
  property real demoPhase: 0
  Timer {
    interval: 40
    repeat: true
    running: root.demoDictation
    onTriggered: {
      root.demoPhase += 0.33
      var voice = Math.max(0, Math.sin(root.demoPhase) * 0.5 + Math.sin(root.demoPhase * 2.7) * 0.3)
      root.demoLevel = Math.min(1, voice + Math.random() * 0.15)
    }
  }
  property string demoFinals: ""
  property string demoPartial: ""
  property int demoStep: 0
  ListModel { id: demoModel }
  QtObject {
    id: demoSource
    readonly property var popupModel: demoModel
    property int popupScreensRev: 0
    // Unique key from one run to the next (the time, like the service): a card still
    // leaving cannot be mistaken for a new one.
    function popupKey(row) { return String(row.originalId) + ":" + String(row.timestamp) }
    // The whole demo follows the active screen, notifications included, like the OSD and the dictation.
    function popupOnScreen(key, screenName) { return root.focusedScreen === "" || root.focusedScreen === String(screenName) }
    function durationFor(urgency, expireTimeout) { return 6000 }
    function present(row) {
      return { title: row.summary, body: row.body, icon: "", crop: false, glyph: row.glyph,
        tile: "", ink: "", critical: row.urgency === 2 }
    }
    function closePopup(index) { if (index >= 0 && index < demoModel.count) demoModel.remove(index) }
    function invokePopupDefault(index) { closePopup(index) }
    function expirePopup(index) { closePopup(index) }
  }
  function demoNotify(id, glyph, summary, body, urgency) {
    demoModel.insert(0, { originalId: id, timestamp: Date.now(), app: "demo", appIcon: "",
      summary: summary, body: body, image: "", glyph: glyph, urgency: urgency, expireTimeout: 0 })
  }
  // A replaced notification (same id): its text changes in place, the card grows.
  function demoBody(id, body) {
    for (var i = 0; i < demoModel.count; i++)
      if (demoModel.get(i).originalId === id) demoModel.setProperty(i, "body", body)
  }
  readonly property var demoSteps: [
    function() { root.showOsd("volume", "", "30", "100", "", "1600") },
    function() { root.showOsd("volume", "", "45", "100", "", "1600") },
    function() { root.showOsd("volume", "", "62", "100", "", "1600") },
    function() {},
    function() { root.showOsd("media-play", "Invented track - Example band", "", "100", "", "2500") },
    function() { root.demoNotify(1, "󰄬", "Build finished", "example-api · 42 tests passed", 1) },
    function() {},
    function() { root.demoNotify(2, "󰃰", "Reminder", "Example meeting in ten minutes, demo room", 2) },
    function() { root.demoDictation = true },
    function() { root.demoPartial = "I need to" },
    function() { root.demoPartial = "I need to remember to resend" },
    function() { root.demoFinals = "I need to remember to resend the quote tomorrow morning"; root.demoPartial = "" },
    function() { root.demoBody(1, "example-api · 42 tests passed, three warnings to review before going live") },
    function() { root.demoPartial = "and to call" },
    function() { root.demoPartial = "and to call the carrier before noon" },
    function() { root.demoFinals = "I need to remember to resend the quote tomorrow morning and to call the carrier before noon"; root.demoPartial = "then prepare the minutes of the example meeting for the whole team" },
    function() { root.showOsd("volume", "", "40", "100", "", "1600") },
    function() { root.showOsd("volume-muted", "", "40", "100", "", "1600", true) },
    function() { root.demoDictation = false },
    function() {}, function() {}, function() {}, function() {}, function() {},
    function() { root.demoRunning = false; demoModel.clear() }
  ]
  Timer {
    id: demoTimer
    interval: 700
    repeat: true
    running: root.demoRunning
    onTriggered: {
      if (root.demoStep >= root.demoSteps.length) { root.demoRunning = false; return }
      root.demoSteps[root.demoStep]()
      root.demoStep++
    }
  }
  function startDemo() {
    demoModel.clear()
    demoFinals = ""
    demoPartial = ""
    demoDictation = false
    demoStep = 0
    demoSource.popupScreensRev++
    demoRunning = true
  }

  IpcHandler {
    target: "bandeau"
    function demo(): string { root.startDemo(); return "ok" }
    function ping(): string { return "ok" }
  }
}
