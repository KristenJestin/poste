import QtQuick
import Quickshell
import Quickshell.Io

// Dictée Vocaline, lue sur le socket du démon (module vocaline), fournie au slot « dictation »
// de l'île (island/island.json). Le flux donne un objet JSON par ligne : hello, state, level,
// partial, final, silence, error ; ce qui est inconnu ou mal formé est ignoré. Vocaline colle
// toute la dictée une seule fois, à l'arrêt : l'île est le seul endroit où le texte se voit
// pendant qu'on parle. Démon absent ou flux coupé : rien n'est en cours, nouvel essai 5 s plus
// tard, sans rien écrire au journal.
Item {
  id: voc
  readonly property string socketPath: (Quickshell.env("XDG_RUNTIME_DIR") || "/tmp") + "/vocaline.sock"

  property int bars: 14                 // injecté par l'île
  property string phase: "idle"         // idle, loading, listening, stopping
  readonly property bool listening: phase === "loading" || phase === "listening"
  readonly property bool transcribing: phase === "stopping"
  property var levels: []
  property string finals: ""            // phrases figées de cette dictée
  property string partial: ""           // phrase en cours
  property int partialUtterance: -1

  function reset() {
    finals = ""
    partial = ""
    partialUtterance = -1
    levels = []
  }

  function handle(line) {
    var ev
    try { ev = JSON.parse(line) } catch (e) { return }
    if (!ev || typeof ev !== "object") return
    if (ev.type === "state") {
      var s = String(ev.state || "")
      if (["idle", "loading", "listening", "stopping"].indexOf(s) === -1) return   // inconnu : ignoré
      var was = phase
      phase = s
      // Fin (un partial encore ouvert est jeté), ou nouvelle dictée : le texte disparaît. La
      // phase change d'abord, pour que l'île ne recopie pas un texte à moitié effacé.
      if (s === "idle" || was === "idle") reset()
    } else if (ev.type === "level") {
      if (!listening) return
      var level = Math.min(1, Math.sqrt(Math.max(0, Number(ev.rms) || 0)) * 2.2)
      var next = levels.slice(Math.max(0, levels.length - bars + 1))
      next.push(level)
      levels = next
    } else if (ev.type === "partial") {
      if (phase === "idle") return
      // Dernière lecture du moteur : elle remplace la précédente (elle peut raccourcir).
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

  // Le flux est lu sur le socket du démon (commande `watch`), sans passer par un processus : rien
  // ne dépend de la façon dont le programme vide sa sortie dans un tuyau. Après un échec, un
  // Socket de Quickshell garde sa connexion morte et ne réessaie plus : on en recrée un à chaque
  // essai, et seulement quand le socket existe (chaque échec écrirait un avertissement au journal).
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
