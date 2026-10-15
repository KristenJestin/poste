import QtQuick
import Quickshell.Services.UPower

// The battery, for the island activity (Island.qml: root.battery). Nothing on
// a PC without a battery (UPower sees no laptop battery). Otherwise,
// next to the island if it is low (below 30 %, yellow; below 10 %, red), and
// 6 s after unplugging, with the icon of the current power profile in the
// bubble. Always in the control center. To
// test elsewhere ('discharging 80' plays the unplugging):
//   omarchy-shell poste.island batterySim "charging 75" | "low 15" | "critical 6" | "off"
Item {
  id: battery
  readonly property var dev: UPower.displayDevice
  // The power profile (PowerProfileState.qml), set by Island.qml.
  property var profile: null
  // Right after the unplugging, the profile to show with it (empty otherwise, and without
  // power-profiles-daemon); the low or critical bubble that remains does not carry it.
  readonly property string profileGlyph: flash && !charging && profile && profile.present ? profile.glyph : ""

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
  // Seconds until full charge or empty battery (0: unknown).
  readonly property real secondsLeft: simulated ? (simCharging ? 900 : 5400)
    : (dev ? Number(charging ? dev.timeToFull : dev.timeToEmpty) || 0 : 0)
  readonly property string level: percent <= 0.10 ? "critical" : percent <= 0.30 ? "low" : "ok"

  readonly property color color: charging ? "#30d158" : level === "critical" ? "#ff453a" : level === "low" ? "#ffd60a" : "#f2f2f2"

  // Passing display after an event.
  property bool flash: false
  Timer { id: flashTimer; interval: 6000; onTriggered: battery.flash = false }
  function pulse() { if (present) { flash = true; flashTimer.restart() } }
  // At startup, UPower fills onBattery after the fact: this first pass is not an
  // unplugging, the first seconds do not count.
  property bool armed: false
  Timer { interval: 3000; running: true; onTriggered: battery.armed = true }
  onChargingChanged: if (armed) pulse()
  onLevelChanged: if (!charging && level !== "ok") pulse()

  // Next to the island: low (below 30 %), or just after the unplugging (the
  // time to see the profile taken on battery). Otherwise, it is only in the
  // control center (not even on plugging in).
  readonly property bool showing: present && (level !== "ok" || (flash && !charging))

  // Icon according to the level (eleven steps), or the bolt while charging.
  readonly property string glyph: charging ? "󰂄"
    : ["󰂎", "󰁺", "󰁻", "󰁼", "󰁽", "󰁾", "󰁿", "󰂀", "󰂁", "󰂂", "󰁹"][Math.max(0, Math.min(10, Math.round(percent * 10)))]

  // '~15 min', '2 h 10'
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
