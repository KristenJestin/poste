import QtQuick
import Quickshell
import Quickshell.Io

// Les conversations avec l'IA de l'île, communes à tous les écrans
// (Island.qml : root.ai). Chacune est un AiConversation ; la vue IA
// (AskView.qml) affiche `current`. Fermer la vue n'arrête rien : une
// conversation qui travaille, dont la réponse attend, ou qu'on a épinglée
// garde sa pastille en haut à droite (AiPill.qml, une par conversation, liste
// `docked`). Une conversation ni épinglée, ni en cours, ni en attente
// disparaît quand une autre prend sa place. Le fournisseur (Claude par
// défaut, Codex…) passe par bin/poste-ask ; la liste se règle dans
// launcher.json, clé "ai".
Item {
  id: ai
  readonly property string bin: Quickshell.env("HOME") + "/.config/omarchy/plugins/poste.island/bin/poste-ask"

  // ---------------------------------------------------------------- providers
  property var providerIds: ["claude", "codex"]
  property var providerNames: ({ claude: "Claude", codex: "Codex" })
  // Fournisseur des nouvelles conversations (le dernier choisi, sinon "default").
  property string provider: "claude"
  property bool providerChosen: false
  FileView {
    path: Quickshell.env("HOME") + "/.config/poste-island/launcher/launcher.json"
    watchChanges: true
    printErrors: false
    onFileChanged: reload()
    onLoaded: {
      var conf = {}
      try { conf = JSON.parse(text()).ai || {} } catch (e) {}
      var ids = ["claude", "codex"], names = { claude: "Claude", codex: "Codex" }
      var extra = conf.providers || {}
      for (var id in extra) { if (ids.indexOf(id) === -1) ids.push(id); names[id] = String(extra[id].name || id) }
      ai.providerIds = ids
      ai.providerNames = names
      if (!ai.providerChosen && conf["default"] && ids.indexOf(conf["default"]) !== -1) ai.provider = conf["default"]
    }
  }
  onProviderChanged: providerChosen = true

  // ---------------------------------------------------------------- conversations
  Component { id: conversationComponent; AiConversation {} }
  property var conversations: []
  property int uidCounter: 0
  property var current: null
  // Change quand une conversation change d'état (épinglée, en cours, en attente).
  property int stateRevision: 0

  // Une vue IA est ouverte sur l'un des écrans (tenu à jour par AskView).
  property int openViews: 0
  readonly property bool viewOpen: openViews > 0
  onViewOpenChanged: syncShown()
  onCurrentChanged: syncShown()
  function syncShown() {
    for (var i = 0; i < conversations.length; i++) conversations[i].shown = viewOpen && conversations[i] === current
    stateRevision++
  }

  // Les pastilles de droite : les conversations à garder sous la main, sauf
  // celle qu'on est en train de lire. Plus récente à droite.
  readonly property var docked: {
    stateRevision
    var out = []
    for (var i = 0; i < conversations.length; i++) {
      var c = conversations[i]
      if ((c.pinned || c.busy || c.unseen) && !(viewOpen && c === current)) out.push(c)
    }
    return out
  }

  function disposable(c) { return !!c && !c.pinned && !c.busy && !c.unseen }
  function dispose(c) {
    var i = conversations.indexOf(c)
    if (i < 0) return
    var next = conversations.slice()
    next.splice(i, 1)
    conversations = next
    if (current === c) current = null
    c.destroy()
    stateRevision++
  }
  // Une conversation neuve devient la courante ; l'ancienne disparaît si elle
  // n'a plus de raison d'être gardée.
  function newConversation() {
    var old = current
    var c = conversationComponent.createObject(ai, { uid: ++ai.uidCounter, service: ai, provider: ai.provider })
    var bump = function() { ai.stateRevision++ }
    c.pinnedChanged.connect(bump)
    c.busyChanged.connect(bump)
    c.unseenChanged.connect(bump)
    conversations = conversations.concat([c])
    current = c
    // L'ancienne après : la vue n'est jamais sans conversation, même un instant.
    if (old && disposable(old)) dispose(old)
    return c
  }
  // Lancer une question en arrière-plan (commande askBg) : nouvelle
  // conversation épinglée, sans toucher à celle affichée.
  function spawn(q, providerId) {
    q = String(q || "").trim()
    if (q === "") return null
    var c = conversationComponent.createObject(ai, { uid: ++ai.uidCounter, service: ai, provider: providerId && providerIds.indexOf(providerId) !== -1 ? providerId : ai.provider })
    var bump = function() { ai.stateRevision++ }
    c.pinnedChanged.connect(bump)
    c.busyChanged.connect(bump)
    c.unseenChanged.connect(bump)
    conversations = conversations.concat([c])
    c.pinned = true
    c.send(q)
    return c
  }
  // Tout désépingler : les conversations au repos perdent leur pastille
  // (épingle et « réponse prête ») ; celles qui travaillent la gardent.
  function unpinAll() {
    var n = 0
    for (var i = 0; i < conversations.length; i++) {
      var c = conversations[i]
      if (c.busy || !(c.pinned || c.unseen)) continue
      c.pinned = false
      c.unseen = false
      n++
    }
    // Celles qui n'ont plus de raison d'être gardées disparaissent.
    var keep = conversations.slice()
    for (var j = 0; j < keep.length; j++) if (keep[j] !== current && disposable(keep[j])) dispose(keep[j])
    return n
  }
  // Rouvrir une conversation (clic sur sa pastille).
  function show(c) {
    if (!c || c === current) return
    var old = current
    current = c
    if (old && disposable(old)) dispose(old)
  }

  // ---------------------------------------------------------------- persistence
  // Les conversations qui ont une pastille (épinglées, en cours, en attente)
  // sont gardées dans ~/.local/state/poste-island/ai/conversations.json et
  // reviennent au démarrage du shell, avec leur session chez le fournisseur
  // (on peut continuer). Une réponse coupée par le redémarrage est marquée
  // « Interrupted ». Texte en clair, sur cette machine seulement.
  readonly property string stateDir: Quickshell.env("HOME") + "/.local/state/poste-island/ai"
  property bool restored: false
  FileView {
    id: store
    path: ai.stateDir + "/conversations.json"
    printErrors: false
    blockLoading: true
    onLoaded: ai.restore(text())
    onLoadFailed: ai.restore("")
  }
  Process { id: mkdir; command: ["mkdir", "-p", ai.stateDir] }
  Timer { id: saveTimer; interval: 600; onTriggered: ai.save() }
  onStateRevisionChanged: if (restored) saveTimer.restart()
  function save() {
    var out = []
    for (var i = 0; i < conversations.length; i++) {
      var c = conversations[i]
      if (!(c.pinned || c.busy || c.unseen) || c.messages.count === 0) continue
      var msgs = []
      for (var m = 0; m < c.messages.count; m++) {
        var e = c.messages.get(m)
        msgs.push({ role: e.role, text: e.text, error: !!e.error })
      }
      out.push({ provider: c.provider, sessionId: c.sessionId, pinned: c.pinned, unseen: c.unseen,
                 busy: c.busy, lastAnswer: c.lastAnswer, messages: msgs })
    }
    mkdir.running = true
    store.setText(JSON.stringify({ version: 1, conversations: out }))
  }
  function restore(text) {
    if (restored) return
    var data = {}
    try { data = JSON.parse(text || "{}") } catch (e) {}
    var list = data.conversations || []
    for (var i = 0; i < list.length; i++) {
      var d = list[i]
      var c = newConversation()
      c.provider = d.provider || provider
      c.sessionId = d.sessionId || ""
      c.lastAnswer = d.lastAnswer || ""
      for (var m = 0; m < (d.messages || []).length; m++) {
        var e = d.messages[m]
        c.messages.append({ role: e.role, text: String(e.text || ""), tool: "", error: !!e.error })
      }
      if (d.busy) {   // coupée par le redémarrage
        var last = c.messages.count - 1
        if (last >= 0 && c.messages.get(last).text === "") c.messages.setProperty(last, "text", "_Interrupted._")
      }
      c.unseen = !!d.unseen || !!d.busy
      c.pinned = !!d.pinned || !!d.busy
    }
    restored = true
    // Une conversation courante vide, pour la prochaine ouverture.
    if (!current || current.messages.count > 0) newConversation()
  }
  // Une conversation courante existe toujours, même avant le rechargement.
  Component.onCompleted: if (!current) newConversation()
  // Toute modification de texte d'une conversation gardée est aussi sauvée.
  Connections {
    target: ai.current
    function onRevisionChanged() { if (ai.restored && ai.current && (ai.current.pinned || ai.current.busy || ai.current.unseen)) saveTimer.restart() }
  }

  // Ce que fait l'IA, selon l'outil en cours (claude stream-json, codex) ;
  // aucun outil : elle réfléchit.
  function toolLabel(name) {
    var n = String(name || "")
    if (n === "") return "Thinking…"
    if (/edit|write|file_change/i.test(n)) return "Editing files…"
    if (/web_?search/i.test(n)) return "Searching the web…"
    if (/fetch/i.test(n)) return "Reading a page…"
    if (/read|glob|grep|file/i.test(n)) return "Reading files…"
    if (/command|bash|exec/i.test(n)) return "Running a command…"
    return "Working…"
  }
}
