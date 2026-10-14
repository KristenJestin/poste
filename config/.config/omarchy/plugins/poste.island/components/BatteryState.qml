import QtQuick
import Quickshell.Services.UPower

// La batterie, pour l'activité de l'île (Island.qml : root.battery). Rien sur
// un PC sans batterie (UPower ne voit pas de batterie de portable). Sinon,
// à côté de l'île si elle est basse (sous 30 %, jaune ; sous 10 %, rouge), et
// 6 s au débranchement, avec l'icône du profil d'alimentation en place dans la
// bulle. Toujours dans le centre de contrôle. Pour
// tester ailleurs (« discharging 80 » joue le débranchement) :
//   omarchy-shell poste.island batterySim "charging 75" | "low 15" | "critical 6" | "off"
Item {
  id: battery
  readonly property var dev: UPower.displayDevice
  // Le profil d'alimentation (PowerProfileState.qml), posé par Island.qml.
  property var profile: null
  // Sur batterie, le profil à montrer avec elle (vide sans power-profiles-daemon).
  readonly property string profileGlyph: !charging && profile && profile.present ? profile.glyph : ""

  // Simulation (commande batterySim).
  property bool simulated: false
  property real simPercent: 0.75
  property bool simCharging: true

  readonly property bool present: simulated || !!(dev && dev.isLaptopBattery && dev.isPresent)
  readonly property real percent: {
    if (simulated) return simPercent
    if (!dev) return 0
    var p = Number(dev.percentage) || 0
    return p > 1 ? p / 100 : p
  }
  readonly property bool charging: simulated ? simCharging : !UPower.onBattery
  // Secondes jusqu'à pleine charge ou batterie vide (0 : inconnu).
  readonly property real secondsLeft: simulated ? (simCharging ? 900 : 5400)
    : (dev ? Number(charging ? dev.timeToFull : dev.timeToEmpty) || 0 : 0)
  readonly property string level: percent <= 0.10 ? "critical" : percent <= 0.30 ? "low" : "ok"

  readonly property color color: charging ? "#30d158" : level === "critical" ? "#ff453a" : level === "low" ? "#ffd60a" : "#f2f2f2"

  // Affichage passager après un événement.
  property bool flash: false
  Timer { id: flashTimer; interval: 6000; onTriggered: battery.flash = false }
  function pulse() { if (present) { flash = true; flashTimer.restart() } }
  onChargingChanged: pulse()
  onLevelChanged: if (!charging && level !== "ok") pulse()

  // À côté de l'île : basse (sous 30 %), ou juste après le débranchement (le
  // temps de voir le profil pris sur batterie). Sinon, elle n'est que dans le
  // centre de contrôle (même pas au branchement).
  readonly property bool showing: present && (level !== "ok" || (flash && !charging))

  // Icône selon le niveau (onze crans), ou l'éclair en charge.
  readonly property string glyph: charging ? "󰂄"
    : ["󰂎", "󰁺", "󰁻", "󰁼", "󰁽", "󰁾", "󰁿", "󰂀", "󰂁", "󰂂", "󰁹"][Math.max(0, Math.min(10, Math.round(percent * 10)))]

  // « ~15 min », « 2 h 10 »
  function timeText() {
    var s = Math.round(secondsLeft)
    if (s <= 0) return ""
    var m = Math.round(s / 60)
    return m < 60 ? "~" + m + " min" : "~" + Math.floor(m / 60) + " h " + String(m % 60).padStart(2, "0")
  }

  // "charging 75", "low 15", "critical 6", "discharging 50", "off"
  function simulate(spec) {
    var parts = String(spec || "").trim().split(/\s+/)
    if (parts[0] === "off" || parts[0] === "") { simulated = false; flash = false; return "off" }
    simCharging = parts[0] === "charging"
    simPercent = Math.max(0, Math.min(100, Number(parts[1] || 75))) / 100
    simulated = true
    pulse()
    return "ok"
  }
}
