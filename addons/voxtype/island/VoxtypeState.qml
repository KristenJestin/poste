import QtQuick
import Quickshell
import Quickshell.Io

// voxtype dictation for the island's "dictation" slot (island/island.json). The state comes from
// the daemon's state file (idle, recording, streaming, transcribing), the live mic level from its
// audio socket through voxtype-audio-bridge (NDJSON {"peak","rms","vad"} frames). voxtype gives
// no text while one speaks: `finals` and `partial` stay empty, only the pill shows.
// voxtype's own OSD is off (osd.enabled = false in its config).
Item {
  id: vox
  readonly property string statePath: (Quickshell.env("XDG_RUNTIME_DIR") || "/tmp") + "/voxtype/state"

  property string phase: "idle"
  readonly property bool listening: phase === "recording" || phase === "streaming"
  readonly property bool transcribing: phase === "transcribing"
  // Current level, 0 to 1 (boosted: the RMS of speech is small). The bridge sends 100 frames a
  // second (10 ms windows); its `rms` is the peak of the window divided by the square root of 2.
  property real level: 0
  readonly property string finals: ""
  readonly property string partial: ""

  // The daemon may replace the file by rename, which a watch can miss: it is also re-read a few
  // times a second (a 9-byte file).
  Timer { interval: 250; running: true; repeat: true; onTriggered: stateFile.reload() }
  FileView {
    id: stateFile
    path: vox.statePath
    watchChanges: true
    printErrors: false
    onFileChanged: reload()
    onLoaded: vox.phase = (text() || "idle").trim() || "idle"
    onLoadFailed: vox.phase = "idle"
  }

  Process {
    running: vox.listening
    command: ["voxtype-audio-bridge"]
    stdout: SplitParser {
      onRead: function(line) {
        var frame
        try { frame = JSON.parse(line) } catch (e) { return }
        if (frame.rms === undefined) return
        vox.level = Math.min(1, Math.sqrt(Math.max(0, Number(frame.rms))) * 2.2)
      }
    }
  }
  onListeningChanged: if (!listening) level = 0
}
