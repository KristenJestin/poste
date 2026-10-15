import QtQuick
import Quickshell
import Quickshell.Io

// A conversation with the island's AI: its messages, its session at the
// provider, its process (bin/poste-ask). Created and stored by
// AskService.qml, displayed by AskView.qml, summarized by AiPill.qml.
Item {
  id: conv
  required property var service
  property int uid: 0          // stable identifier (island activities)
  property string provider: "claude"
  readonly property string providerName: service.providerNames[provider] || provider

  property alias messages: messages
  ListModel { id: messages }   // { role: "user"|"assistant", text, tool, error }
  property string sessionId: ""
  property bool busy: false
  property string lastAnswer: ""
  // Answer that arrived while it was not displayed.
  property bool unseen: false
  // Pinned: kept in its pill, on the right, even when idle.
  property bool pinned: false
  // Currently displayed in an AI view.
  property bool shown: false
  onShownChanged: if (shown) unseen = false

  property int revision: 0
  Connections {
    target: messages
    function onDataChanged() { conv.revision++ }
    function onCountChanged() { conv.revision++ }
  }
  readonly property int lastIndex: { revision; return messages.count - 1 }
  readonly property string currentTool: { revision; return lastIndex >= 0 ? String(messages.get(lastIndex).tool || "") : "" }
  readonly property string question: { revision; return lastIndex >= 1 ? String(messages.get(lastIndex - 1).text) : "" }
  readonly property string firstQuestion: { revision; return messages.count > 0 ? String(messages.get(0).text) : "" }
  readonly property bool lastError: { revision; return lastIndex >= 0 ? !!messages.get(lastIndex).error : false }
  readonly property string answerText: { revision; return lastIndex >= 0 ? String(messages.get(lastIndex).text) : "" }

  function send(q) {
    q = String(q || "").trim()
    if (q === "" || busy) return false
    messages.append({ role: "user", text: q, tool: "", error: false })
    messages.append({ role: "assistant", text: "", tool: "", error: false })
    busy = true
    unseen = false
    var args = [service.bin, "--provider", provider]
    if (sessionId !== "") args.push("--session", sessionId)
    proc.command = args.concat([q])
    proc.running = true
    return true
  }
  function stop() {
    if (!proc.running) return
    proc.running = false
    var i = messages.count - 1
    if (i >= 0 && messages.get(i).role === "assistant") {
      messages.setProperty(i, "tool", "")
      if (messages.get(i).text === "") messages.setProperty(i, "text", "_Stopped._")
    }
    busy = false
  }
  // Changing provider: only on an empty conversation (a
  // conversation belongs to its provider).
  function cycleProvider() {
    if (busy || messages.count > 0) return
    var ids = service.providerIds
    provider = ids[(ids.indexOf(provider) + 1) % ids.length]
    service.provider = provider
  }
  function onEvent(line) {
    var d
    try { d = JSON.parse(line) } catch (e) { return }
    var i = messages.count - 1
    if (i < 0) return
    if (d.t === "session") sessionId = d.id || sessionId
    else if (d.t === "delta") { messages.setProperty(i, "text", messages.get(i).text + d.text); messages.setProperty(i, "tool", "") }
    else if (d.t === "tool") messages.setProperty(i, "tool", String(d.name || ""))
    else if (d.t === "done") {
      if (d.error) { messages.setProperty(i, "text", String(d.text || "Error")); messages.setProperty(i, "error", true) }
      else if (messages.get(i).text === "") messages.setProperty(i, "text", String(d.text || ""))
      messages.setProperty(i, "tool", "")
      lastAnswer = messages.get(i).text
      busy = false
      if (!shown) unseen = true
      if (!d.error) Quickshell.execDetached([Quickshell.env("HOME") + "/.config/omarchy/plugins/poste.island/bin/poste-sfx", "ai-done"])
    }
  }
  Process {
    id: proc
    stdout: SplitParser { onRead: function(data) { conv.onEvent(data) } }
    onExited: conv.busy = false
  }
}
