import QtQuick
import Quickshell
import Quickshell.Io

// Navette, read on its event socket (navette add-on), provided to the 'remote' slot of the
// island (island/island.json). On connection Navette sends the current state as one JSON line,
// then one line per change, and nothing in between: the island sleeps until something changes.
// The contract is docs/events.md of the Navette repository: an unknown field or value is ignored,
// and `home` is read as `desk` (Navette before 0.2.0). The actions run the navette command, never
// written on the socket: its lines stay events only.
Item {
  id: nav
  readonly property string socketPath: (Quickshell.env("XDG_RUNTIME_DIR") || "/tmp") + "/navette/events.sock"

  property string role: ""           // desk, remote
  property string phase: "off"       // desk: waiting, linked, starting, mode, stopping; remote: away, dialing, linked, starting, mode; both: incompatible
  property string peer: ""           // label of the other machine
  property string peerVersion: ""
  property string mode: ""
  property var monitors: []
  property var modes: []             // [{ name, monitors }], on the desk
  property string mic: "off"         // desk, remote, failed, off

  // The other machine is there to drive (desk), a session runs, or the versions differ.
  readonly property bool shown: phase === "starting" || phase === "mode" || phase === "stopping"
    || phase === "incompatible" || (role === "desk" && phase === "linked")

  function start(name) { Quickshell.execDetached(name ? ["navette", "mode", "start", name] : ["navette", "mode", "start"]) }
  function stop() { Quickshell.execDetached(["navette", "mode", "stop"]) }
  function toggleMic() { Quickshell.execDetached(["navette", "mic", "toggle"]) }

  readonly property var knownStates: ["waiting", "linked", "starting", "mode", "stopping", "away", "dialing", "incompatible", "off"]

  function reset() {
    role = ""
    phase = "off"
    peer = ""
    peerVersion = ""
    mode = ""
    monitors = []
    modes = []
    mic = "off"
  }

  function word(v) { return v === "home" ? "desk" : String(v) }

  function handle(line) {
    if (line.charAt(0) !== "{") return
    var ev
    try { ev = JSON.parse(line) } catch (e) { return }
    if (!ev || typeof ev !== "object") return
    if (typeof ev.role === "string") role = word(ev.role)
    if (typeof ev.state === "string" && knownStates.indexOf(ev.state) !== -1) phase = ev.state
    if (typeof ev.peer === "string") peer = ev.peer
    if (typeof ev.peer_version === "string") peerVersion = ev.peer_version
    if ("mode" in ev) mode = typeof ev.mode === "string" ? ev.mode : ""
    if (Array.isArray(ev.monitors)) monitors = ev.monitors.map(String)
    if (Array.isArray(ev.modes)) modes = ev.modes.filter(function(m) { return m && typeof m.name === "string" })
    if (typeof ev.mic === "string") {
      var m = word(ev.mic)
      if (["desk", "remote", "failed", "off"].indexOf(m) !== -1) mic = m
    }
  }

  // Same connection as Vocaline's: a Quickshell Socket keeps a dead connection and no longer
  // retries, so a new one is made at each try, and only once the socket exists (each failure
  // would write a warning to the log). poste starts Navette with the session: the probe only
  // runs while it is not there.
  property bool linked: !!link.item && link.item.connected
  Loader {
    id: link
    active: false
    sourceComponent: Socket {
      path: nav.socketPath
      connected: true
      parser: SplitParser { onRead: function(line) { nav.handle(line) } }
      onConnectionStateChanged: if (!connected) nav.reset()
    }
  }
  Process {
    id: probe
    command: ["test", "-S", nav.socketPath]
    onExited: function(code) {
      if (code !== 0 || nav.linked) return
      link.active = false
      link.active = true
    }
  }
  Timer {
    interval: 5000
    repeat: true
    triggeredOnStart: true
    running: !nav.linked
    onTriggered: if (!probe.running) probe.running = true
  }
}
