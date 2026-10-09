import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Services.UPower

// Batteries des périphériques sans fil (Island.qml : root.devices) : clavier
// et souris vus par UPower (Logitech HID++…), casque lu par headsetcontrol
// s'il est installé. Une activité à gauche de l'île seulement sous 15 %
// (rouge sous 5 %) et pas en charge ; toujours listés dans le centre de
// contrôle.
Item {
  id: devices
  readonly property real lowLevel: 0.15

  // Types UPower : 5 souris, 6 clavier, 12 manette, 13 stylet, 14 pavé
  // tactile, 17 casque, 19 écouteurs.
  function kindOf(type, name) {
    var t = Number(type)
    if (t === 5) return "mouse"
    if (t === 6) return "keyboard"
    if (t === 17 || t === 19) return "headset"
    if (t === 12) return "gamepad"
    if (/mouse|souris/i.test(name)) return "mouse"
    if (/keyboard|clavier/i.test(name)) return "keyboard"
    return "device"
  }
  function glyphOf(kind) {
    return kind === "mouse" ? "󰍽" : kind === "keyboard" ? "󰌌" : kind === "headset" ? "󰋎" : kind === "gamepad" ? "󰊴" : "󰂯"
  }

  // Casque : headsetcontrol (paquet extra/headsetcontrol), relu chaque minute.
  property var headset: null       // { name, percent, charging }
  Process {
    id: headsetProc
    command: ["sh", "-c", "command -v headsetcontrol >/dev/null || exit 0; headsetcontrol -b -o json 2>/dev/null"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        var d = null
        try { d = JSON.parse(String(this.text || "")) } catch (e) { d = null }
        var dev = d && d.devices && d.devices[0]
        var bat = dev && dev.battery
        if (!bat || bat.status === "BATTERY_UNAVAILABLE" || !(Number(bat.level) >= 0)) { devices.headset = null; return }
        devices.headset = { name: String(dev.device || dev.product || "Headset"), percent: Number(bat.level) / 100,
                            charging: bat.status === "BATTERY_CHARGING" }
      }
    }
  }
  Timer { interval: 60000; running: true; repeat: true; triggeredOnStart: true; onTriggered: headsetProc.running = true }

  // Tous les périphériques : { key, name, kind, glyph, percent, charging, low, color }
  readonly property var list: {
    var out = []
    var all = UPower.devices.values || []
    for (var i = 0; i < all.length; i++) {
      var d = all[i]
      if (!d || d.isLaptopBattery || !d.isPresent) continue
      var name = String(d.model || "")
      var kind = kindOf(d.type, name)
      if (kind === "device" && name === "") continue
      var p = Number(d.percentage) || 0
      p = p > 1 ? p / 100 : p
      var charging = Number(d.state) === 1 || Number(d.state) === 4
      if (simAll >= 0) { p = simAll; charging = false }   // simulation « all »
      out.push(entry("upower:" + String(d.nativePath || name), name, kind, p, charging))
    }
    if (headset) out.push(entry("headset", headset.name, "headset", headset.percent, headset.charging))
    if (sim) out.push(entry("sim:" + sim.kind, "Simulated " + sim.kind, sim.kind, sim.percent, false))
    return out
  }
  function entry(key, name, kind, p, charging) {
    var low = !charging && p < lowLevel
    return { key: key, name: name, kind: kind, glyph: glyphOf(kind), percent: p, charging: charging, low: low,
             color: charging ? "#30d158" : p < 0.05 ? "#ff453a" : low ? "#ffd60a" : "#f2f2f2" }
  }
  readonly property var low: list.filter(function(d) { return d.low })

  // Simulation (commande deviceSim) : "mouse 12" ajoute un faux périphérique ;
  // "all 8" met tous les vrais à 8 % ; "off" arrête.
  property var sim: null
  property real simAll: -1
  function simulate(spec) {
    var parts = String(spec || "").trim().split(/\s+/)
    if (parts[0] === "off" || parts[0] === "") { sim = null; simAll = -1; return "off" }
    if (parts[0] === "all") { simAll = Math.max(0, Math.min(100, Number(parts[1] || 10))) / 100; return "ok" }
    sim = { kind: parts[0], percent: Math.max(0, Math.min(100, Number(parts[1] || 10))) / 100 }
    return "ok"
  }
}
