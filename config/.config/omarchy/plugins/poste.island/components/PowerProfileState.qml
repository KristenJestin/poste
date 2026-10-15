import QtQuick
import Quickshell.Io
import Quickshell.Services.UPower

// The power profile (power-profiles-daemon), for the island (Island.qml: root.powerProfile).
// Read from Quickshell's PowerProfiles singleton (D-Bus): it follows every change, from
// wherever it comes (Omarchy's switch on unplugging, Omarchy's menu, powerprofilesctl).
// The presence of the daemon and the list of profiles come from omarchy-powerprofiles-list (empty:
// no daemon, nothing is shown). The choice goes through omarchy-powerprofiles-set <ac|battery>
// <profile>, which remembers it for this power source, like Omarchy's panel;
// never powerprofilesctl set. To test without touching the real profile:
//   omarchy-shell poste.island profileSim "power-saver" | "balanced" | "performance" | "off"
// (the three profiles are then offered, and a choice only changes the simulation).
Item {
  id: state

  // The machine's profiles, in Omarchy's order (power saver, balanced, performance).
  property var profiles: []
  readonly property bool present: simulated || profiles.length > 0
  readonly property var shownProfiles: simulated ? ["power-saver", "balanced", "performance"] : profiles

  // Simulation (profileSim command).
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
  // Short label (control center tile) and full label (OSD).
  function shortLabel(name) { return name === "power-saver" ? "Saver" : name === "performance" ? "Perf" : "Balanced" }
  function label(name) { return name === "power-saver" ? "Power saver" : name === "performance" ? "Performance" : "Balanced" }
  readonly property string glyph: glyphOf(active)

  // A profile change (not the first reading): the island shows the OSD. The singleton reads
  // D-Bus after the island starts, and the list arrives after it: the first seconds
  // do not count.
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
  Process {
    id: setProc
    onExited: function(code) { if (code !== 0) console.warn("omarchy-powerprofiles-set failed (" + code + ")") }
  }
  // The machine's next profile (after the last, the first).
  function cycle() {
    var list = shownProfiles
    if (list.length === 0) return
    set(list[(list.indexOf(active) + 1) % list.length])
  }

  function refresh() { if (!listProc.running) listProc.running = true }
  Process {
    id: listProc
    command: ["omarchy-powerprofiles-list"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        var list = String(text || "").split("\n").map(function(l) { return l.trim() }).filter(function(l) { return l !== "" })
        // An empty reading in passing (the daemon restarts) keeps the known list.
        if (list.length > 0 || state.profiles.length === 0) state.profiles = list
      }
    }
  }
  Component.onCompleted: refresh()
  // The daemon may not be ready when the island starts: list read again as long as it is empty.
  Timer { interval: 30000; repeat: true; running: state.profiles.length === 0; onTriggered: state.refresh() }

  // "power-saver", "balanced", "performance", "off"
  function simulate(spec) {
    var name = String(spec || "").trim()
    // End of simulation: back to the real profile, without OSD.
    if (name === "off" || name === "") { seen = liveProfile; simulated = false; return "off" }
    if (["power-saver", "balanced", "performance"].indexOf(name) < 0) return "unknown profile"
    if (!simulated) { simProfile = liveProfile; simulated = true }
    simProfile = name
    return "ok"
  }
}
