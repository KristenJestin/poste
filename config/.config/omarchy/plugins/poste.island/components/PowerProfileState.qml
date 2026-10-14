import QtQuick
import Quickshell.Io
import Quickshell.Services.UPower

// Le profil d'alimentation (power-profiles-daemon), pour l'île (Island.qml : root.powerProfile).
// Lu dans le singleton PowerProfiles de Quickshell (D-Bus) : il suit tout changement, d'où
// qu'il vienne (bascule d'Omarchy au débranchement, menu d'Omarchy, powerprofilesctl).
// La présence du démon et la liste des profils viennent de omarchy-powerprofiles-list (vide :
// pas de démon, rien n'est montré). Le choix passe par omarchy-powerprofiles-set <ac|battery>
// <profil>, qui le retient pour cette source d'alimentation, comme le panneau d'Omarchy ;
// jamais powerprofilesctl set. Pour tester sans toucher au vrai profil :
//   omarchy-shell poste.island profileSim "power-saver" | "balanced" | "performance" | "off"
// (les trois profils sont alors offerts, et un choix ne change que la simulation).
Item {
  id: state

  // Profils de la machine, dans l'ordre d'Omarchy (économie, équilibré, performance).
  property var profiles: []
  readonly property bool present: simulated || profiles.length > 0
  readonly property var shownProfiles: simulated ? ["power-saver", "balanced", "performance"] : profiles

  // Simulation (commande profileSim).
  property bool simulated: false
  property string simProfile: "balanced"

  readonly property string liveProfile: PowerProfiles.profile === PowerProfile.PowerSaver ? "power-saver"
    : PowerProfiles.profile === PowerProfile.Performance ? "performance" : "balanced"
  readonly property string active: simulated ? simProfile : liveProfile

  function glyphOf(name) {
    if (name === "power-saver") return "󰌪"
    if (name === "performance") return "󰓅"
    return "󰊚"
  }
  // Libellé court (centre de contrôle) et complet (OSD).
  function shortLabel(name) { return name === "power-saver" ? "Saver" : name === "performance" ? "Performance" : "Balanced" }
  function label(name) { return name === "power-saver" ? "Power saver" : name === "performance" ? "Performance" : "Balanced" }
  readonly property string glyph: glyphOf(active)

  // Un changement de profil (pas la première lecture) : l'île montre l'OSD. Le singleton lit
  // D-Bus après le démarrage de l'île, et la liste arrive après lui : les premières secondes
  // ne comptent pas.
  signal switched(string name)
  property string seen: ""
  property bool armed: false
  Timer { interval: 3000; running: true; onTriggered: { state.seen = state.active; state.armed = true } }
  onActiveChanged: {
    if (!present || !armed) { seen = active; return }
    if (seen !== "" && seen !== active) switched(active)
    seen = active
  }
  onPresentChanged: seen = active

  function set(name) {
    if (!present || shownProfiles.indexOf(name) < 0 || name === active) return
    if (simulated) { simProfile = name; return }
    if (setProc.running) return
    setProc.command = ["omarchy-powerprofiles-set", UPower.onBattery ? "battery" : "ac", name]
    setProc.running = true
  }
  Process { id: setProc }

  function refresh() { if (!listProc.running) listProc.running = true }
  Process {
    id: listProc
    command: ["omarchy-powerprofiles-list"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        var list = String(text || "").split("\n").map(function(l) { return l.trim() }).filter(function(l) { return l !== "" })
        // Une lecture vide de passage (le démon redémarre) garde la liste connue.
        if (list.length > 0 || state.profiles.length === 0) state.profiles = list
      }
    }
  }
  Component.onCompleted: refresh()

  // "power-saver", "balanced", "performance", "off"
  function simulate(spec) {
    var name = String(spec || "").trim()
    // Fin de simulation : retour au vrai profil, sans OSD.
    if (name === "off" || name === "") { seen = liveProfile; simulated = false; return "off" }
    if (["power-saver", "balanced", "performance"].indexOf(name) < 0) return "unknown profile"
    if (!simulated) { simProfile = liveProfile; simulated = true }
    simProfile = name
    return "ok"
  }
}
