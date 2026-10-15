import QtQuick
import Quickshell
import Quickshell.Io
import "ModulePlayers.js" as ModulePlayers

// Découverte des morceaux d'île fournis par les modules poste, sans connaître aucun module.
//
// Chaque module installé peut avoir, dans ~/.local/share/poste/modules/<nom>/island/ :
//   launcher.json  entrées / extensions / moteurs du lanceur (même format que la config du lanceur)
//   island.json    { "slots": [ { "slot": "<nom du slot>", "file": "Fichier.qml" } ],
//                    "player": { … } }
// "player" declares what the player card can show for one MPRIS player, without any QML
// (see Island.qml): "bus" (its MPRIS bus name, required), "quality" (metadata key of a short
// line under the artist), "starred" (boolean metadata key, the heart is lit when true),
// "like" (command run by the heart), "app" (desktop entry of the player's app, when its MPRIS
// has no DesktopEntry: the sound source pill shows it and brings its window up or opens it),
// "launcher" (launcher extension opened by the pill instead, for a player without a window).
// Slots que l'île charge (voir Island.qml) : « capture »
// (source d'enregistrement pour la pastille de capture), « dictation » (source de dictée pour
// la pastille de dictée en bas de l'écran et le texte sous l'île).
//
// Exposé : launcherDocs [{ module, data }], slots { <slot>: [url de fichier, …] },
// players [{ module, bus, quality, starred, like, launcher }].
// Re-lecture au démarrage, puis à chaque changement sous le dossier des modules (inotifywait,
// avec une courte temporisation). Module absent : rien n'est ajouté, aucune trace.
Item {
  id: reg
  readonly property string dir: (Quickshell.env("XDG_DATA_HOME") || (Quickshell.env("HOME") + "/.local/share")) + "/poste/modules"

  property var launcherDocs: []
  property var slots: ({})
  property var players: []
  property string signature: ""

  // Lit les fichiers et les affiche en un seul document JSON (les erreurs de lecture sont ignorées).
  readonly property string script: [
    "import json, os, sys",
    "root = sys.argv[1]",
    "out = []",
    "try:",
    "    names = sorted(os.listdir(root))",
    "except OSError:",
    "    names = []",
    "for name in names:",
    "    d = os.path.join(root, name, 'island')",
    "    if not os.path.isdir(d):",
    "        continue",
    "    entry = {'module': name, 'dir': d}",
    "    for key in ('launcher', 'island'):",
    "        try:",
    "            with open(os.path.join(d, key + '.json')) as f:",
    "                entry[key] = json.load(f)",
    "        except (OSError, ValueError):",
    "            pass",
    "    out.append(entry)",
    "print(json.dumps(out))"
  ].join("\n")

  function parse(text) {
    var list = []
    try { list = JSON.parse(text) } catch (e) { return }
    var sig = JSON.stringify(list)
    if (sig === signature) return
    signature = sig
    var docs = [], map = {}, players = []
    for (var i = 0; i < list.length; i++) {
      var m = list[i]
      if (m.launcher && typeof m.launcher === "object") docs.push({ module: m.module, data: m.launcher })
      var pl = ModulePlayers.declaration(m.module, m.island && m.island.player)
      if (pl) players.push(pl)
      var declared = m.island && Array.isArray(m.island.slots) ? m.island.slots : []
      for (var k = 0; k < declared.length; k++) {
        var s = declared[k]
        // Un nom de fichier simple, dans le dossier island du module.
        if (!s || !s.slot || !s.file || !/^[A-Za-z0-9_.-]+\.qml$/.test(String(s.file))) continue
        if (!map[s.slot]) map[s.slot] = []
        map[s.slot].push("file://" + m.dir + "/" + s.file)
      }
    }
    slots = map
    reg.players = players
    launcherDocs = docs
  }

  property bool again: false
  function rescan() {
    if (scan.running) { again = true; return }
    scan.running = true
  }
  Process {
    id: scan
    command: ["python3", "-c", reg.script, reg.dir]
    stdout: StdioCollector { waitForEnd: true; onStreamFinished: reg.parse(text) }
    onExited: if (reg.again) { reg.again = false; running = true }
  }
  Component.onCompleted: rescan()

  // Surveille le dossier des modules ; s'il n'existe pas encore, nouvel essai plus tard.
  Timer { id: debounce; interval: 400; onTriggered: reg.rescan() }
  Process {
    id: watcher
    running: true
    command: ["inotifywait", "-m", "-r", "-q", "-e", "close_write,moved_to,moved_from,create,delete", "--format", "%w%f", reg.dir]
    stdout: SplitParser { onRead: function(line) { debounce.restart() } }
    onExited: retry.start()
  }
  Timer { id: retry; interval: 15000; onTriggered: { watcher.running = true; reg.rescan() } }
}
