import QtQuick
import Quickshell
import Quickshell.Io

// OBS (Island.qml : root.obs), via bin/poste-obs watch : enregistrement,
// diffusion, tampon de replay, scène. Le temps est relu chaque seconde par le
// script et interpolé ici pour défiler sans à-coups. Un enregistrement fini :
// notification « Recording saved », un clic ouvre son dossier.
Item {
  id: obs
  readonly property string bin: Quickshell.env("HOME") + "/.config/omarchy/plugins/poste.island/bin/poste-obs"

  property bool connected: false
  property string error: ""
  property bool recording: false
  property bool paused: false
  property bool streaming: false
  property bool replay: false
  property string scene: ""
  property real recordMs: 0
  property real streamMs: 0
  property double stampedAt: 0         // quand recordMs / streamMs ont été lus
  readonly property bool active: connected && (recording || streaming)

  // Temps affiché : la dernière valeur lue plus le temps écoulé depuis.
  property double now: Date.now()
  Timer { interval: 250; repeat: true; running: obs.active; onTriggered: obs.now = Date.now() }
  readonly property real recordElapsed: recording ? recordMs + (paused ? 0 : Math.max(0, now - stampedAt)) : 0
  readonly property real streamElapsed: streaming ? streamMs + Math.max(0, now - stampedAt) : 0

  Process {
    id: watcher
    running: true
    command: [obs.bin, "watch"]
    stdout: SplitParser {
      onRead: function(line) {
        var d
        try { d = JSON.parse(line) } catch (e) { return }
        if (d.t === "status") {
          obs.connected = !!d.connected
          obs.error = String(d.error || "")
          obs.recording = !!d.recording
          obs.paused = !!d.paused
          obs.streaming = !!d.streaming
          obs.replay = !!d.replay
          obs.scene = String(d.scene || "")
          obs.recordMs = Number(d.recordMs) || 0
          obs.streamMs = Number(d.streamMs) || 0
          obs.stampedAt = Date.now()
          obs.now = obs.stampedAt
        } else if (d.t === "saved" && d.path) {
          var dir = String(d.path).replace(/\/[^\/]*$/, "")
          Quickshell.execDetached(["omarchy-notification-send", "--app-name", "OBS", "-g", "󰑋",
            "Recording saved", String(d.path).split("/").pop(), "--exec", "xdg-open", dir])
        }
      }
    }
    // Le script se reconnecte seul ; s'il s'arrête quand même, on le relance.
    onExited: restartTimer.start()
  }
  Timer { id: restartTimer; interval: 5000; onTriggered: watcher.running = true }

  // Commandes : StopRecord, ToggleRecordPause, SaveReplayBuffer, StopStream…
  function request(kind) { Quickshell.execDetached([bin, kind]) }
}
