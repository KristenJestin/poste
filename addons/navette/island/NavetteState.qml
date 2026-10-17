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
  property string phase: "off"       // desk: waiting, linked, starting, mode, stopping; remote: away, dialing,
                                     // linked, starting, mode; both: incompatible; or a newer one, never shown
  property string peer: ""           // label of the other machine
  property string peerVersion: ""
  property string version: ""        // Navette here: with peerVersion, which machine is behind
  property string mode: ""
  property var monitors: []
  property var modes: []             // [{ name, monitors }], on the desk; name "" for the unnamed mode
  property string mic: "off"         // desk, remote, failed, off

  // A session starts, runs or stops; something to show: that, the other machine there to drive
  // (desk), or versions that do not talk.
  readonly property bool running: phase === "starting" || phase === "mode" || phase === "stopping"
  readonly property bool shown: running || phase === "incompatible" || (role === "desk" && phase === "linked")

  // The actions run the navette command; when it fails, what it says (`navette: ...` on stderr)
  // comes up as a notification, rather than a click that does nothing.
  function run(args) {
    Quickshell.execDetached(["sh", "-c", "out=$(navette \"$@\" 2>&1) || notify-send -a Navette -i navette Navette \"${out#navette: }\"", "sh"].concat(args))
  }
  function start(name) { run(name ? ["mode", "start", name] : ["mode", "start"]) }
  function stop() { run(["mode", "stop"]) }
  function toggleMic() { run(["mic", "toggle"]) }

  function reset() {
    role = ""
    phase = "off"
    peer = ""
    peerVersion = ""
    version = ""
    mode = ""
    monitors = []
    modes = []
    mic = "off"
  }

  function word(v) { return v === "home" ? "desk" : String(v) }

  // The machine to update when the two do not talk: the one with the older minor version ("" when
  // it cannot be told, or with a local build). "here" or "peer".
  function behind() {
    if (version.indexOf("-dev") !== -1 || peerVersion.indexOf("-dev") !== -1) return ""
    var a = version.split("."), b = peerVersion.split(".")
    for (var i = 0; i < 2; i++) {
      var x = parseInt(a[i]), y = parseInt(b[i])
      if (isNaN(x) || isNaN(y)) return ""
      if (x !== y) return x < y ? "here" : "peer"
    }
    return ""
  }

  // Each line is the whole state: every field is read again, a missing or null one is empty. A line
  // of another contract version is not guessed at: nothing is shown.
  function handle(line) {
    if (line.charAt(0) !== "{") return
    var ev
    try { ev = JSON.parse(line) } catch (e) { return }
    if (!ev || typeof ev !== "object") return
    if (ev.v !== 1) {
      console.warn("navette: events contract v" + ev.v + " is not known here; update poste")
      reset()
      return
    }
    function str(v) { return typeof v === "string" ? v : "" }
    role = word(str(ev.role))
    // An unknown state is kept as is: nothing is shown for it (`shown`), rather than the last one.
    phase = str(ev.state)
    peer = str(ev.peer) || (role === "remote" ? "Desk" : "Remote")
    peerVersion = str(ev.peer_version)
    version = str(ev.version)
    mode = str(ev.mode)
    monitors = Array.isArray(ev.monitors) ? ev.monitors.map(String) : []
    modes = !Array.isArray(ev.modes) ? [] : ev.modes
      .filter(function(m) { return m && typeof m === "object" })
      .map(function(m) { return { name: str(m.name), monitors: Array.isArray(m.monitors) ? m.monitors.map(String) : [] } })
    var m = word(str(ev.mic))
    mic = ["desk", "remote", "failed"].indexOf(m) !== -1 ? m : "off"
  }

  // Same connection as Vocaline's: a Quickshell Socket keeps a dead connection and no longer
  // retries, so a new one is made at each try, and only once the socket exists (each failure
  // would write a warning to the log). A stopped Navette may leave its socket file behind, so the
  // probe asks `navette watch --once`, which says `off` when nothing answers on it. poste starts
  // Navette with the session: the probe only runs while it is not there.
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
    command: ["sh", "-c", "[ -S \"$1\" ] && out=$(timeout 5 navette watch --json --once) && case $out in *'\"state\":\"off\"'*) exit 1 ;; esac",
      "sh", nav.socketPath]
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
