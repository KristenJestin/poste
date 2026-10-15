import QtQuick
import Quickshell
import Quickshell.Io

// Screen recording (gpu-screen-recorder) through "poste record watch" (record module).
// Provided to the island's "capture" slot (island/island.json). Same interface as ObsState (recording, paused, recordElapsed, request…),
// so that the capture pill and its card serve both. The time is
// read again every second and interpolated here; the "Recording saved" notification
// comes from poste-record itself.
Item {
  id: rec
  readonly property var bin: ["poste", "record"]

  readonly property bool connected: true
  property bool recording: false
  property bool paused: false
  readonly property bool streaming: false
  readonly property bool replay: false
  property string scene: ""            // the preset or the target ("Unity", "Screen DP-1")
  property real recordMs: 0
  readonly property real streamMs: 0
  property double stampedAt: 0
  readonly property bool active: recording

  property double now: Date.now()
  Timer { interval: 250; repeat: true; running: rec.recording && !rec.paused; onTriggered: rec.now = Date.now() }
  readonly property real recordElapsed: recording ? recordMs + (paused ? 0 : Math.max(0, now - stampedAt)) : 0
  readonly property real streamElapsed: 0

  Process {
    id: watcher
    running: true
    command: rec.bin.concat(["watch"])
    stdout: SplitParser {
      onRead: function(line) {
        var d
        try { d = JSON.parse(line) } catch (e) { return }
        if (d.t !== "status") return
        rec.recording = !!d.recording
        rec.paused = !!d.paused
        rec.scene = String(d.label || "")
        rec.recordMs = Number(d.recordMs) || 0
        rec.stampedAt = Date.now()
        rec.now = rec.stampedAt
      }
    }
    onExited: restartTimer.start()
  }
  Timer { id: restartTimer; interval: 5000; onTriggered: watcher.running = true }

  // Requests in the OBS format, translated for poste-record.
  function request(kind) {
    var map = { StopRecord: "stop", ToggleRecord: "stop", PauseRecord: "pause", ResumeRecord: "resume", ToggleRecordPause: "toggle" }
    var cmd = map[kind]
    if (cmd) Quickshell.execDetached(bin.concat([cmd]))
  }
}
