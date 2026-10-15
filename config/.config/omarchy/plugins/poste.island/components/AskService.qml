import QtQuick
import Quickshell
import Quickshell.Io

// The conversations with the island's AI, common to all screens
// (Island.qml: root.ai). Each one is an AiConversation; the AI view
// (AskView.qml) displays `current`. Closing the view stops nothing: a
// conversation that is working, whose answer is waiting, or that has been pinned
// keeps its pill at the top right (AiPill.qml, one per conversation, list
// `docked`). A conversation that is neither pinned, nor running, nor waiting
// disappears when another takes its place. The provider (Claude by
// default, Codex…) goes through bin/poste-ask; the list is set in
// launcher.json, key "ai".
Item {
  id: ai
  readonly property string bin: Quickshell.env("HOME") + "/.config/omarchy/plugins/poste.island/bin/poste-ask"

  // ---------------------------------------------------------------- providers
  property var providerIds: ["claude", "codex"]
  property var providerNames: ({ claude: "Claude", codex: "Codex" })
  // Provider of the new conversations (the last chosen, otherwise "default").
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
  // Changes when a conversation changes state (pinned, running, waiting).
  property int stateRevision: 0

  // An AI view is open on one of the screens (kept up to date by AskView).
  property int openViews: 0
  readonly property bool viewOpen: openViews > 0
  onViewOpenChanged: syncShown()
  onCurrentChanged: syncShown()
  function syncShown() {
    for (var i = 0; i < conversations.length; i++) conversations[i].shown = viewOpen && conversations[i] === current
    stateRevision++
  }

  // The right-hand pills: the conversations to keep at hand, except
  // the one being read. Most recent on the right.
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
  // A new conversation becomes the current one; the old one disappears if it
  // has no more reason to be kept.
  function newConversation() {
    var old = current
    var c = conversationComponent.createObject(ai, { uid: ++ai.uidCounter, service: ai, provider: ai.provider })
    var bump = function() { ai.stateRevision++ }
    c.pinnedChanged.connect(bump)
    c.busyChanged.connect(bump)
    c.unseenChanged.connect(bump)
    conversations = conversations.concat([c])
    current = c
    // The old one afterwards: the view is never without a conversation, not even for a moment.
    if (old && disposable(old)) dispose(old)
    return c
  }
  // Start a question in the background (askBg command): new
  // pinned conversation, without touching the displayed one.
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
  // Unpin everything: the idle conversations lose their pill
  // (pin and 'answer ready'); those that are working keep it.
  function unpinAll() {
    var n = 0
    for (var i = 0; i < conversations.length; i++) {
      var c = conversations[i]
      if (c.busy || !(c.pinned || c.unseen)) continue
      c.pinned = false
      c.unseen = false
      n++
    }
    // Those that have no more reason to be kept disappear.
    var keep = conversations.slice()
    for (var j = 0; j < keep.length; j++) if (keep[j] !== current && disposable(keep[j])) dispose(keep[j])
    return n
  }
  // Reopen a conversation (click on its pill).
  function show(c) {
    if (!c || c === current) return
    var old = current
    current = c
    if (old && disposable(old)) dispose(old)
  }

  // ---------------------------------------------------------------- persistence
  // The conversations that have a pill (pinned, running, waiting)
  // are kept in ~/.local/state/poste-island/ai/conversations.json and
  // come back at the shell's startup, with their session at the provider
  // (one can continue). An answer cut by the restart is marked
  // 'Interrupted'. Plain text, on this machine only.
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
      if (d.busy) {   // cut by the restart
        var last = c.messages.count - 1
        if (last >= 0 && c.messages.get(last).text === "") c.messages.setProperty(last, "text", "_Interrupted._")
      }
      c.unseen = !!d.unseen || !!d.busy
      c.pinned = !!d.pinned || !!d.busy
    }
    restored = true
    // An empty current conversation, for the next opening.
    if (!current || current.messages.count > 0) newConversation()
  }
  // A current conversation always exists, even before the reload.
  Component.onCompleted: if (!current) newConversation()
  // Any text change in a kept conversation is also saved.
  Connections {
    target: ai.current
    function onRevisionChanged() { if (ai.restored && ai.current && (ai.current.pinned || ai.current.busy || ai.current.unseen)) saveTimer.restart() }
  }

  // What the AI is doing, according to the tool in progress (claude stream-json, codex);
  // no tool: it is thinking.
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
