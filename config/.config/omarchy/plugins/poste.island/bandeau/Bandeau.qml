pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import Quickshell.Services.Pipewire
import qs.Commons
import "file:///usr/share/omarchy/shell/plugins/osd/OsdModel.js" as OsdModel

// Le bandeau : ce qui passe sur l'écran (notifications, OSD, dictée), d'un seul aspect, celui de
// l'île (fond plein, texte clair, icône à l'accent, arrondi en pilule).
//
// Un seul objet pour tout le shell : poste.island l'importe par `import "bandeau"`,
// poste.notifications et poste.osd par `import "../poste.island/bandeau"`. Les trois chemins
// mènent au même dossier, donc au même singleton (vérifié avec une config Quickshell jetable,
// dossier lié et fichiers liés un par un comme sur la machine). Il tient l'état et dessine une
// fenêtre par écran (BandeauWindow.qml), créée au premier usage.
//
// Deux endroits :
// - Juste sous l'île, une pile qui suit son bord bas (celui de la vue ouverte s'il y en a une) :
//   du haut vers le bas, le texte dicté, l'OSD, puis les notifications (trois au plus, la plus
//   récente en haut). L'île n'est jamais recouverte ni agrandie.
// - En bas de l'écran actif, seule, la pastille de la dictée : le micro et l'onde pendant
//   l'écoute, les points pendant la transcription.
//
// Qui l'alimente :
// - OSD : poste.osd (clone de omarchy.osd) appelle showOsdPayload(payload JSON d'Omarchy).
// - Volume : le bandeau suit lui-même la sortie par défaut de PipeWire (voir « volume »).
// - Dictée : poste.island pose dictationListening, dictationTranscribing, dictationLevel (niveau
//   courant de la source qui dicte), dictationFinals, dictationPartial.
// - Bord de l'île : poste.island appelle setIslandEdge(écran, y) à chaque image du ressort.
// - Notifications : poste.notifications s'inscrit avec attachNotifications(service). Le service
//   garde toute sa logique (serveur, ne pas déranger, historique, écran du toast) ; il doit
//   offrir popupModel, popupScreensRev, popupKey(row), popupOnScreen(key, écran),
//   durationFor(urgence, expireTimeout), present(row) (ce que montre la carte, voir
//   NotificationView.qml), closePopup(i), invokePopupDefault(i), expirePopup(i).
Singleton {
  id: root

  // ---------------------------------------------------------------- style
  // Texte et accent de l'île (Island.qml : colorText, colorAccent). Le fond est un verre sombre :
  // le fond du thème teinté de 6 % vers le texte, opaque à 86 %, flouté par Hyprland
  // (looknfeel.lua) pour se détacher même d'une fenêtre noire.
  readonly property color colorSurface: Qt.tint(Color.background, withAlpha(Color.foreground, 0.06))
  readonly property real surfaceOpacity: 0.86
  readonly property color colorText: Qt.lighter(Color.foreground, 1.08)
  // Gris opaque (texte mêlé au fond) : texte secondaire, phrase dictée en cours.
  readonly property color colorMuted: mix(colorText, colorSurface, 0.5)
  readonly property color colorAccent: Color.accent
  readonly property color colorUrgent: Color.urgent
  readonly property string fontFamily: "JetBrainsMono Nerd Font"
  readonly property int bottomMargin: 56
  readonly property int gap: 10
  // Sous l'île : plus que les marges des fenêtres de Hyprland (gaps_out 10), sinon la première
  // pièce commence pile sur le bord haut des fenêtres.
  readonly property int islandGap: 20
  readonly property int maxNotifications: 3
  // Plusieurs lignes : même arrondi partout.
  readonly property int blockRadius: 20

  function withAlpha(c, a) { return Qt.rgba(c.r, c.g, c.b, a) }
  // `t` de la couleur a, le reste de b.
  function mix(a, b, t) {
    return Qt.rgba(a.r * t + b.r * (1 - t), a.g * t + b.g * (1 - t), a.b * t + b.b * (1 - t), 1)
  }

  // Changement de taille d'une pièce : `anim` (une NumberAnimation sur la taille montrée) repart
  // de la valeur actuelle vers `target`, sauf si elle y va déjà ou y est. Repartir de là où on
  // en est garde le mouvement continu quand le contenu change pendant l'animation.
  function animateTo(anim, current, target) {
    if (anim.running ? anim.to === target : current === target) return
    anim.stop()
    anim.from = current
    anim.to = target
    anim.start()
  }

  readonly property string focusedScreen: Hyprland.focusedMonitor ? String(Hyprland.focusedMonitor.name) : ""

  // ---------------------------------------------------------------- OSD
  // Une seule place : un nouvel OSD remplace celui affiché, sur place (la barre glisse de
  // l'ancienne valeur à la nouvelle), et son minuteur repart.
  property bool osdOpen: false
  property var osd: ({ iconKey: "", icon: "", message: "", hasProgress: false, value: 0, maxValue: 100, media: false, dim: false })

  // `dim` : la barre passe en gris (sortie coupée).
  function showOsd(icon, message, value, max, progressText, duration, dim) {
    var next = OsdModel.stateForShow(icon, message, value, max, progressText, duration)
    next.media = next.iconKey.indexOf("media") === 0 || next.iconKey.indexOf("player") === 0
    next.dim = !!dim
    // L'état avant l'ouverture : un OSD neuf part de sa valeur, seul un OSD déjà là glisse.
    osd = next
    osdOpen = true
    osdTimer.interval = Math.max(1, next.duration)
    if (next.duration > 0) osdTimer.restart()
    else osdTimer.stop()
  }
  // Même lecture du payload que Osd.qml d'Omarchy (open()).
  function showOsdPayload(payloadJson) {
    var p = null
    try { p = JSON.parse(payloadJson || "{}") } catch (e) { return false }
    if (!p || typeof p !== "object") return false
    // Le volume vient de PipeWire (plus bas), jamais du payload : une seule source, donc
    // jamais deux affichages ni deux états différents pour un même changement.
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
  // Le volume de la sortie par défaut, lu dans PipeWire : il prend chaque changement, d'où
  // qu'il vienne (touches, molette, application, pactl), ce que l'OSD des touches ne voit pas.
  // Mêmes icônes que l'OSD de volume d'Omarchy (son script omarchy-audio-output-volume) :
  // « volume-muted » quand la sortie est coupée ou à 0, sinon « volume-high ». Coupée, la
  // barre garde son niveau mais passe en gris.
  readonly property var sink: Pipewire.defaultAudioSink
  PwObjectTracker { objects: [root.sink] }
  readonly property real sinkVolume: sink && sink.audio ? sink.audio.volume : -1
  readonly property bool sinkMuted: !!(sink && sink.audio && sink.audio.muted)
  readonly property bool sinkReady: !!(sink && sink.ready)
  // La première lecture d'une sortie (démarrage, changement de sortie) est seulement retenue.
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

  // ---------------------------------------------------------------- dictée
  // La pastille du bas dès que la dictée écoute ou transcrit, quelle que soit la source ; le
  // texte sous l'île quand la source en donne un (Vocaline).
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

  // Une seule onde pour toutes les sources : chacune ne donne que son niveau courant,
  // échantillonné ici 100 fois par seconde dans l'historique des barres, le
  // plus récent en dernier. L'onde bouge donc pareil quelle que soit la source ; elle repart de
  // zéro à chaque dictée.
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

  // ---------------------------------------------------------------- bord de l'île
  // Bord bas de l'île dessinée, par écran (poste.island, à chaque image du ressort). Sans île
  // sur l'écran, la pile part du haut.
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
  // Le service s'inscrit ici, chaque fenêtre lit son modèle (NotificationView.qml).
  property var notifications: null
  function attachNotifications(service) { notifications = service }
  function detachNotifications(service) { if (notifications === service) notifications = null }
  // Pendant la démo, les vraies notifications attendent (elles restent dans leur service).
  readonly property var source: demoRunning ? demoSource : notifications

  // ---------------------------------------------------------------- fenêtres
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

  // ---------------------------------------------------------------- démo
  // omarchy-shell bandeau demo : un OSD de volume qui monte, un OSD de musique, deux
  // notifications (dont une dont le texte s'allonge), une dictée (la pastille et son onde en bas,
  // puis un texte sous l'île qui passe sur deux lignes), puis le volume coupé, tout inventé, sur
  // l'écran actif (la démo le suit). Rien n'est envoyé au serveur de notifications ni écrit dans
  // l'historique.
  property bool demoRunning: false
  property bool demoDictation: false
  // Niveau inventé de la dictée de démo : une voix qui monte et retombe, 25 fois par seconde.
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
    // Clé unique d'un passage à l'autre (l'heure, comme le service) : une carte encore en
    // train de partir ne peut pas être prise pour une nouvelle.
    function popupKey(row) { return String(row.originalId) + ":" + String(row.timestamp) }
    // Toute la démo suit l'écran actif, notifications comprises, comme l'OSD et la dictée.
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
  // Une notification remplacée (même id) : son texte change sur place, la carte grandit.
  function demoBody(id, body) {
    for (var i = 0; i < demoModel.count; i++)
      if (demoModel.get(i).originalId === id) demoModel.setProperty(i, "body", body)
  }
  readonly property var demoSteps: [
    function() { root.showOsd("volume", "", "30", "100", "", "1600") },
    function() { root.showOsd("volume", "", "45", "100", "", "1600") },
    function() { root.showOsd("volume", "", "62", "100", "", "1600") },
    function() {},
    function() { root.showOsd("media-play", "Morceau inventé - Groupe exemple", "", "100", "", "2500") },
    function() { root.demoNotify(1, "󰄬", "Compilation terminée", "exemple-api · 42 tests passés", 1) },
    function() {},
    function() { root.demoNotify(2, "󰃰", "Rappel", "Réunion d'exemple dans dix minutes, salle de démonstration", 2) },
    function() { root.demoDictation = true },
    function() { root.demoPartial = "Il faut que" },
    function() { root.demoPartial = "Il faut que je pense à renvoyer" },
    function() { root.demoFinals = "Il faut que je pense à renvoyer le devis demain matin"; root.demoPartial = "" },
    function() { root.demoBody(1, "exemple-api · 42 tests passés, trois avertissements à relire avant la mise en ligne") },
    function() { root.demoPartial = "et à rappeler" },
    function() { root.demoPartial = "et à rappeler le transporteur avant midi" },
    function() { root.demoFinals = "Il faut que je pense à renvoyer le devis demain matin et à rappeler le transporteur avant midi"; root.demoPartial = "puis préparer le compte rendu de la réunion d'exemple pour toute l'équipe" },
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
