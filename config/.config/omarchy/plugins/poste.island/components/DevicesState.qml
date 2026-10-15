import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Services.UPower

// Batteries of wireless devices (Island.qml: root.devices): keyboard
// and mouse seen by UPower (Logitech HID++…), headset read by headsetcontrol
// if it is installed. An activity to the left of the island only below 15 %
// (red below 5 %) and not charging; always listed in the control
// center.
Item {
  id: devices
  readonly property real lowLevel: 0.15

  // UPower types: 5 mouse, 6 keyboard, 12 gamepad, 13 stylus, 14 touchpad,
  // 17 headset, 19 earphones.
  function kindOf(type, name) {
    var t = Number(type)
    if (t === 5) return "mouse"
    if (t === 6) return "keyboard"
    if (t === 17 || t === 19) return "headset"
    if (t === 12) return "gamepad"
    if (/mouse|souris/i.test(name)) return "mouse"   // device names, which some vendors write in French
    if (/keyboard|clavier/i.test(name)) return "keyboard"
    return "device"
  }
  function glyphOf(kind) {
    return kind === "mouse" ? "󰍽" : kind === "keyboard" ? "󰌌" : kind === "headset" ? "󰋎" : kind === "gamepad" ? "󰊴" : "󰂯"
  }

  // Headset: headsetcontrol (package extra/headsetcontrol), read again every minute.
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

  // All the devices: { key, name, kind, glyph, percent, charging, low, color }
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
      if (simAll >= 0) { p = simAll; charging = false }   // 'all' simulation
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

  // Simulation (deviceSim command): "mouse 12" adds a fake device;
  // "all 8" sets all the real ones to 8 %; "off" stops.
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
