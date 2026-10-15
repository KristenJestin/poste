import QtQuick
import Quickshell
import Quickshell.Io

// Vocaline dictation, read on the daemon's socket (vocaline module), provided to the 'dictation' slot
// of the island (island/island.json). The stream gives one JSON object per line: hello, state, level,
// partial, final, silence, error; what is unknown or malformed is ignored. Vocaline pastes
// the whole dictation only once, at the stop: the banner, under the island, is the only place where
// the text can be seen while you talk. Daemon absent or stream cut: nothing is in progress,
// new try 5 s later, without writing anything to the log.
Item {
  id: voc
  readonly property string socketPath: (Quickshell.env("XDG_RUNTIME_DIR") || "/tmp") + "/vocaline.sock"

  property string phase: "idle"         // idle, loading, listening, stopping
  readonly property bool listening: phase === "loading" || phase === "listening"
  readonly property bool transcribing: phase === "stopping"
  // Current level from 0 to 1, on voxtype's scale, which the banner samples at a fixed rate.
  // The daemon sends 10 per second (voxtype, 100): between two events the level glides to
  // the next one instead of jumping, so that the wave does not make a staircase.
  property real level: 0
  Behavior on level { NumberAnimation { duration: 100; easing.type: Easing.InOutQuad } }
  // Vocaline's rms is the effective volume of the last 100 ms; voxtype's is the peak of
  // 10 ms windows divided by the square root of 2, hence stronger on speech (crest factor of
  // 2 to 3: estimated, not measured on the microphone). This gain brings Vocaline to voxtype's scale.
  readonly property real voxRmsGain: 1.8
  property string finals: ""            // frozen sentences of this dictation
  property string partial: ""           // sentence in progress
  property int partialUtterance: -1

  function reset() {
    finals = ""
    partial = ""
    partialUtterance = -1
    level = 0
  }

  function handle(line) {
    var ev
    try { ev = JSON.parse(line) } catch (e) { return }
    if (!ev || typeof ev !== "object") return
    if (ev.type === "state") {
      var s = String(ev.state || "")
      if (["idle", "loading", "listening", "stopping"].indexOf(s) === -1) return   // unknown: ignored
      var was = phase
      phase = s
      // End (a partial still open is dropped), or new dictation: the text disappears. The
      // phase changes first, so that the island does not copy a half-erased text.
      if (s === "idle" || was === "idle") reset()
    } else if (ev.type === "level") {
      if (!listening) return
      // Scaled to voxtype (voxRmsGain), then the same formula as the island for voxtype.
      level = Math.min(1, Math.sqrt(Math.max(0, (Number(ev.rms) || 0) * voxRmsGain)) * 2.2)
    } else if (ev.type === "partial") {
      if (phase === "idle") return
      // Last reading of the engine: it replaces the previous one (it can get shorter).
      partialUtterance = Number(ev.utterance)
      partial = String(ev.text || "").trim()
    } else if (ev.type === "final") {
      if (phase === "idle") return
      var t = String(ev.text || "").trim()
      if (t !== "") finals = finals === "" ? t : finals + " " + t
      if (Number(ev.utterance) === partialUtterance) {
        partial = ""
        partialUtterance = -1
      }
    }
  }

  // The stream is read on the daemon's socket (`watch` command), without going through a process: nothing
  // depends on how the program flushes its output into a pipe. After a failure, a
  // Quickshell Socket keeps its dead connection and no longer retries: a new one is created at each
  // try, and only when the socket exists (each failure would write a warning to the log).
  property bool linked: !!link.item && link.item.connected
  Loader {
    id: link
    active: false
    sourceComponent: Socket {
      path: voc.socketPath
      connected: true
      parser: SplitParser { onRead: function(line) { voc.handle(line) } }
      onConnectionStateChanged: {
        if (connected) {
          write("watch\n")
          flush()
        } else {
          voc.phase = "idle"
          voc.reset()
        }
      }
    }
  }
  Process {
    id: probe
    command: ["test", "-S", voc.socketPath]
    onExited: function(code) {
      if (code !== 0 || voc.linked) return
      link.active = false
      link.active = true
    }
  }
  Timer {
    interval: 5000
    repeat: true
    triggeredOnStart: true
    running: !voc.linked
    onTriggered: if (!probe.running) probe.running = true
  }
}
