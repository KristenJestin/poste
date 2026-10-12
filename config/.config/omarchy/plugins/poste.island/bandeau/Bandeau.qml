pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Io
import qs.Commons
import "file:///usr/share/omarchy/shell/plugins/osd/OsdModel.js" as OsdModel

// Le bandeau : tout ce qui passe en bas de l'écran (notifications, OSD, texte dicté), d'un
// seul aspect, celui de l'île (fond plein, texte clair, icône à l'accent, arrondi en pilule).
//
// Un seul objet pour tout le shell : poste.island l'importe par `import "bandeau"`,
// poste.notifications et poste.osd par `import "../poste.island/bandeau"`. Les trois chemins
// mènent au même dossier, donc au même singleton (vérifié avec une config Quickshell jetable,
// dossier lié et fichiers liés un par un comme sur la machine). Il tient l'état et dessine une
// fenêtre par écran (BandeauWindow.qml), créée au premier usage.
//
// Du bas vers le haut : le texte dicté, l'OSD, puis les notifications (trois au plus à la fois).
//
// Qui l'alimente :
// - OSD : poste.osd (clone de omarchy.osd) appelle showOsdPayload(payload JSON d'Omarchy).
// - Dictée : poste.island pose dictationActive, dictationFinals, dictationPartial.
// - Notifications : poste.notifications s'inscrit avec attachNotifications(service). Le service
//   garde toute sa logique (serveur, ne pas déranger, historique, écran du toast) ; il doit
//   offrir popupModel, popupScreensRev, popupKey(row), popupOnScreen(key, écran),
//   durationFor(urgence, expireTimeout), present(row) (ce que montre la carte, voir
//   NotificationView.qml), closePopup(i), invokePopupDefault(i), expirePopup(i).
Singleton {
  id: root

  // ---------------------------------------------------------------- style
  // Les couleurs de l'île (Island.qml : colorIsland, colorText, colorAccent).
  readonly property color colorSurface: Qt.darker(Color.background, 1.9)
  readonly property color colorText: Qt.lighter(Color.foreground, 1.08)
  // Gris opaque (texte mêlé au fond) : texte secondaire, phrase dictée en cours.
  readonly property color colorMuted: mix(colorText, colorSurface, 0.5)
  readonly property color colorAccent: Color.accent
  readonly property color colorUrgent: Color.urgent
  readonly property string fontFamily: "JetBrainsMono Nerd Font"
  readonly property int bottomMargin: 56
  readonly property int gap: 10
  readonly property int maxNotifications: 3
  // Plusieurs lignes : même arrondi partout.
  readonly property int blockRadius: 20

  function withAlpha(c, a) { return Qt.rgba(c.r, c.g, c.b, a) }
  // `t` de la couleur a, le reste de b.
  function mix(a, b, t) {
    return Qt.rgba(a.r * t + b.r * (1 - t), a.g * t + b.g * (1 - t), a.b * t + b.b * (1 - t), 1)
  }

  readonly property string focusedScreen: Hyprland.focusedMonitor ? String(Hyprland.focusedMonitor.name) : ""

  // ---------------------------------------------------------------- OSD
  // Une seule place : un nouvel OSD remplace celui affiché, sur place (la barre glisse de
  // l'ancienne valeur à la nouvelle), et son minuteur repart.
  property bool osdOpen: false
  property var osd: ({ iconKey: "", icon: "", message: "", hasProgress: false, value: 0, maxValue: 100, media: false })

  function showOsd(icon, message, value, max, progressText, duration) {
    var next = OsdModel.stateForShow(icon, message, value, max, progressText, duration)
    next.media = next.iconKey.indexOf("media") === 0 || next.iconKey.indexOf("player") === 0
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

  // ---------------------------------------------------------------- dictée
  property bool dictationActive: false
  property string dictationFinals: ""
  property string dictationPartial: ""
  readonly property bool dictShown: demoDictation
    || (dictationActive && (dictationFinals !== "" || dictationPartial !== ""))
  readonly property string dictFinals: demoDictation ? demoFinals : dictationFinals
  readonly property string dictPartial: demoDictation ? demoPartial : dictationPartial

  // ---------------------------------------------------------------- notifications
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
  // notifications et une dictée qui s'allonge, tout inventé, sur l'écran actif. Rien n'est
  // envoyé au serveur de notifications ni écrit dans l'historique.
  property bool demoRunning: false
  property bool demoDictation: false
  property string demoFinals: ""
  property string demoPartial: ""
  property int demoStep: 0
  ListModel { id: demoModel }
  QtObject {
    id: demoSource
    readonly property var popupModel: demoModel
    property int popupScreensRev: 0
    property string screen: ""
    function popupKey(row) { return String(row.originalId) }
    function popupOnScreen(key, screenName) { return screen === "" || screen === String(screenName) }
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
  readonly property var demoSteps: [
    function() { root.showOsd("volume", "", "30", "100", "", "1600") },
    function() { root.showOsd("volume", "", "45", "100", "", "1600") },
    function() { root.showOsd("volume", "", "62", "100", "", "1600") },
    function() {},
    function() { root.showOsd("media-play", "Morceau inventé - Groupe exemple", "", "100", "", "2500") },
    function() { root.demoNotify(1, "󰄬", "Compilation terminée", "exemple-api · 42 tests passés", 1) },
    function() {},
    function() { root.demoNotify(2, "󰃰", "Rappel", "Réunion d'exemple dans dix minutes, salle de démonstration", 2) },
    function() { root.demoDictation = true; root.demoPartial = "Il faut que" },
    function() { root.demoPartial = "Il faut que je pense à renvoyer" },
    function() { root.demoFinals = "Il faut que je pense à renvoyer le devis demain matin"; root.demoPartial = "" },
    function() { root.demoPartial = "et à rappeler" },
    function() { root.demoPartial = "et à rappeler le transporteur avant midi" },
    function() { root.showOsd("volume", "", "40", "100", "", "1600") },
    function() {},
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
    demoSource.screen = focusedScreen
    demoSource.popupScreensRev++
    demoRunning = true
  }

  IpcHandler {
    target: "bandeau"
    function demo(): string { root.startDemo(); return "ok" }
    function ping(): string { return "ok" }
  }
}
