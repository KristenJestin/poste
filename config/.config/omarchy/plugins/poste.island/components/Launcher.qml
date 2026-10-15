import QtQuick
import Quickshell
import Quickshell.Io
import "file:///usr/share/omarchy/shell/plugins/clipboard/ClipboardHistory.js" as ClipboardHistory

// The island's launcher (SUPER+SPACE). Two tabs, switched with Tab:
//   Apps: pinned apps first, fuzzy app search ranked by use, an inline
//          calculator (qalc) when the query is an expression, "Open <url>"
//          when it looks like an address, system commands, and a web search
//          as the last row.
//   Clipboard: clipboard history (text and images); Enter pastes it
//          the same way Omarchy's clipboard panel does.
// Up/Down move, Enter runs, Ctrl+B opens the selected row's actions, Esc
// closes the actions then the launcher (a click outside closes too).
Item {
  id: launcher
  required property var host
  property bool active: false
  signal closeRequested()

  property string tab: "apps"
  property string query: ""
  property int selected: 0
  readonly property int maxRows: 8
  readonly property real rowHeight: 48
  // Tallest the launcher may get (set by the island from the screen height);
  // the list takes what the search field and footer leave, and scrolls.
  property real maxHeight: 600
  readonly property real chrome: header.height + 12 + footer.height + 2
  readonly property real listRoom: Math.max(rowHeight + 4, maxHeight - chrome)
  readonly property real listHeight: Math.min(listRoom, Math.min(maxRows, Math.max(1, results.count)) * (rowHeight + 4))

  // Grows to fit the actions panel when the list is short.
  implicitHeight: Math.min(maxHeight, Math.max(header.height + 12 + listHeight,
    actionsPanel.shown ? header.height + 12 + actionsPanel.height + 8 : 0) + footer.height + 2)

  onActiveChanged: {
    if (!active) {
      if (ownsHighlight) { host.recordHighlight = null; ownsHighlight = false }
      return
    }
    tab = "apps"
    currentExt = ""
    subStack = []
    subDefs = ({})
    refreshExtensions(false)
    if (!hiddenScan.running) hiddenScan.running = true
    windowsProc.running = true
    selected = 0
    field.text = ""
    query = ""
    rebuild()
    Qt.callLater(function() { field.forceActiveFocus() })
  }

  // ---------------------------------------------------------------- usage (frecency) & pins
  // State in ~/.local/state/poste-island/launcher.json:
  //   usage:  { "<key>": [last run timestamps in ms, newest last, at most 10] }
  //   pinned: [ "<key>", ... ]   (pin order)
  //   hidden: [ desktop ids ], searchUrl: "...%s..."
  // Keys: app:<desktop id>, item:<name>, ext:<extension id>, ext:<extension id>:<item id>.
  // Frecency = sum over the recorded runs of a weight by age:
  //   100 (< 4 days), 70 (< 14 d), 50 (< 31 d), 30 (< 90 d), 10 (older).
  readonly property string statePath: Quickshell.env("HOME") + "/.local/state/poste-island/launcher.json"
  property var usage: ({})
  // Extra ids to hide, on top of what the desktop hides (launcher.json "hidden").
  property var blacklist: ["avahi-discover", "bssh", "bvnc"]
  property var pinned: []
  property string searchUrl: "https://www.google.com/search?q=%s"
  FileView {
    id: usageFile
    path: launcher.statePath
    printErrors: false
    onLoaded: {
      try {
        var data = JSON.parse(text())
        var u = data.usage || {}
        // Migrate the old per-app counters: n runs dated three weeks ago.
        var old = data.counts || {}
        var migrated = false
        for (var id in old) {
          if (u["app:" + id]) continue
          var stamps = []
          for (var n = 0; n < Math.min(10, Number(old[id]) || 0); n++) stamps.push(Date.now() - 21 * 86400000)
          u["app:" + id] = stamps
          migrated = true
        }
        launcher.usage = u
        if (Array.isArray(data.hidden)) launcher.blacklist = data.hidden
        if (Array.isArray(data.pinned))
          launcher.pinned = data.pinned.map(function(k) { return String(k).indexOf(":") === -1 ? "app:" + k : String(k) })
        if (migrated || data.counts) Qt.callLater(launcher.saveState)
        if (typeof data.searchUrl === "string" && data.searchUrl.indexOf("%s") !== -1) launcher.searchUrl = data.searchUrl
      } catch (e) { launcher.usage = {} }
    }
  }

  // Entries the desktop hides (NoDisplay, Hidden, OnlyShowIn/NotShowIn for
  // Hyprland), from Omarchy's own scan script, refreshed on each open.
  property var desktopHidden: ({})
  Process {
    id: hiddenScan
    // + the entries whose program no longer exists (uninstalled but the .desktop remained).
    command: ["bash", "-c", "\"$1\" \"$2\"; \"$3\"", "scan",
      (launcher.host.omarchyPath || "/usr/share/omarchy") + "/shell/services/hidden-entries.sh",
      [Quickshell.env("XDG_CURRENT_DESKTOP"), Quickshell.env("XDG_SESSION_DESKTOP"), Quickshell.env("DESKTOP_SESSION")].filter(function(v) { return String(v || "").length > 0 }).join(":"),
      Quickshell.env("HOME") + "/.config/omarchy/plugins/poste.island/bin/poste-missing-entries"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        var map = {}
        var ids = String(text || "").split("\n")
        for (var i = 0; i < ids.length; i++) if (ids[i].trim()) map[ids[i].trim()] = true
        launcher.desktopHidden = map
        launcher.rebuild()
      }
    }
  }
  function isHidden(entry) {
    var id = String(entry.id || "")
    return entry.noDisplay || desktopHidden[id] === true || blacklist.indexOf(id) !== -1
  }
  Component.onCompleted: Quickshell.execDetached(["mkdir", "-p", Quickshell.env("HOME") + "/.local/state/poste-island"])
  function saveState() {
    usageFile.setText(JSON.stringify({ usage: usage, pinned: pinned, hidden: blacklist, searchUrl: searchUrl }, null, 2))
  }
  function record(key) {
    if (!key) return
    var next = {}
    for (var k in usage) next[k] = usage[k]
    next[key] = (next[key] || []).concat([Date.now()]).slice(-10)
    usage = next
    saveState()
  }
  function frecency(key) {
    var stamps = usage[key] || []
    var now = Date.now(), score = 0
    for (var i = 0; i < stamps.length; i++) {
      var days = (now - stamps[i]) / 86400000
      score += days < 4 ? 100 : days < 14 ? 70 : days < 31 ? 50 : days < 90 ? 30 : 10
    }
    return score
  }
  function isPinned(key) { return !!key && pinned.indexOf(key) !== -1 }
  function togglePin(key) {
    if (!key) return
    pinned = isPinned(key) ? pinned.filter(function(k) { return k !== key }) : pinned.concat([key])
    saveState()
  }

  // ---------------------------------------------------------------- scoring
  // Subsequence match with bonuses for a prefix, word starts and contiguous
  // runs; -1 when the query doesn't match at all.
  function fuzzy(text, q) {
    text = String(text || "").toLowerCase()
    if (!q) return 0
    if (text.indexOf(q) === 0) return 1000 - text.length
    var idx = text.indexOf(q)
    if (idx > 0) return 700 - idx + ((text.charAt(idx - 1) === " " || text.charAt(idx - 1) === "-") ? 150 : 0)
    var score = 0, ti = 0, run = 0
    for (var qi = 0; qi < q.length; qi++) {
      var found = text.indexOf(q.charAt(qi), ti)
      if (found === -1) return typoScore(text, q)
      run = found === ti ? run + 1 : 0
      score += 10 + run * 8 + ((found === 0 || text.charAt(found - 1) === " ") ? 20 : 0)
      ti = found + 1
    }
    return score
  }

  // Typo tolerance (last resort, below exact matches):
  // Damerau-Levenshtein distance between the input and the start of each word
  // (swap of two letters = 1 typo). 1 typo from 3 letters, 2 from 6.
  function typoScore(text, q) {
    if (q.length < 3) return -1
    var maxErr = q.length >= 6 ? 2 : 1
    var words = text.split(/[\s\-_.]+/)
    var best = -1
    for (var w = 0; w < words.length; w++) {
      var word = words[w]
      if (word.length < 2) continue
      // compare with prefixes of the word of a length close to the input
      for (var len = Math.max(1, q.length - maxErr); len <= Math.min(word.length, q.length + maxErr); len++) {
        var d = editDistance(q, word.substring(0, len))
        if (d <= maxErr) best = Math.max(best, 60 - d * 20 - (w > 0 ? 5 : 0))
      }
    }
    return best
  }
  function editDistance(a, b) {
    var m = a.length, n = b.length, d = []
    for (var i = 0; i <= m; i++) { d.push([i]); for (var j = 1; j <= n; j++) d[i].push(i === 0 ? j : 0) }
    for (i = 1; i <= m; i++) for (j = 1; j <= n; j++) {
      var cost = a.charAt(i - 1) === b.charAt(j - 1) ? 0 : 1
      d[i][j] = Math.min(d[i - 1][j] + 1, d[i][j - 1] + 1, d[i - 1][j - 1] + cost)
      if (i > 1 && j > 1 && a.charAt(i - 1) === b.charAt(j - 2) && a.charAt(i - 2) === b.charAt(j - 1))
        d[i][j] = Math.min(d[i][j], d[i - 2][j - 2] + 1)
    }
    return d[m][n]
  }

  // ---------------------------------------------------------------- calculator
  property string calcResult: ""
  // Functions and not bound properties: onQueryChanged and rebuild() read them while
  // the input changes, when a binding may still hold the previous input.
  function looksLikeMath() {
    return /[0-9]/.test(query) && /^[0-9\s.,+\-*\/^%()a-zA-Z²³√π!€$]+$/.test(query)
      && /[+\-*\/^%()!√]|sqrt|sin|cos|tan|log|\s(to|in|of)\s/.test(query)
  }
  // qalc speaks English and prints conversions in mixed units ("6 mi + 376
  // yd…"): "15% de 80" becomes (15/100)*(80), "10 km en miles" becomes
  // (10 km)/(1 miles) with the unit put back on the number.
  property string calcUnit: ""
  function calcExpression(q) {
    q = q.trim()
    calcUnit = ""
    var pct = q.match(/^([0-9.,]+)\s*%\s*(de|of)\s+(.+)$/i)
    if (pct) return "(" + pct[1].replace(",", ".") + "/100)*(" + pct[3] + ")"
    var conv = q.match(/^(.+?)\s+(to|in|->)\s+([a-zA-Z€$ ]+)$/i)
    if (conv) { calcUnit = conv[3].trim(); return "(" + conv[1] + ")/(1 " + calcUnit + ")" }
    return q
  }
  Timer { id: calcDelay; interval: 120; onTriggered: if (launcher.looksLikeMath()) { calcProc.command = ["qalc", "-t", launcher.calcExpression(launcher.query)]; calcProc.running = true } }
  Process {
    id: calcProc
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        var out = String(text || "").trim().split("\n").pop()
        var ok = out && out !== launcher.query && !/error|warning|rem\(|interval|[a-zA-Z]{2,}\(/i.test(out)
        launcher.calcResult = ok ? out + (launcher.calcUnit ? " " + launcher.calcUnit : "") : ""
        launcher.rebuild()
      }
    }
  }

  // ---------------------------------------------------------------- clipboard
  property var clipHistory: []
  FileView {
    id: clipFile
    path: Quickshell.env("HOME") + "/.local/state/omarchy/clipboard-history.json"
    watchChanges: true
    printErrors: false
    onFileChanged: reload()
    onLoaded: { launcher.clipHistory = ClipboardHistory.parseHistory(text()); if (launcher.tab === "clip") launcher.rebuild() }
  }

  // ---------------------------------------------------------------- results
  readonly property var systemCommands: [
    { title: "Lock", subtitle: "Lock the session", glyph: "󰌾", command: ["omarchy-system-lock"], words: "lock" },
    { title: "Sleep", subtitle: "Suspend the computer", glyph: "󰒲", command: ["systemctl", "suspend"], words: "suspend sleep", confirm: true },
    { title: "Restart", subtitle: "Restart the computer", glyph: "󰜉", command: ["omarchy-system-reboot"], words: "reboot restart", confirm: true },
    { title: "Shut down", subtitle: "Power off the computer", glyph: "󰐥", command: ["omarchy-system-shutdown"], words: "shutdown poweroff", confirm: true }
  ]
  // Commands that make you lose something ('confirm'): the first Enter
  // asks for confirmation (row in red), the second, within 4 s, runs it.
  property string confirmPending: ""
  Timer { id: confirmTimer; interval: 4000; onTriggered: launcher.confirmPending = "" }
  // A system command only comes out if the input is the exact start of one of
  // its words (no typo tolerance: 'test' used to give 'Restart').
  function systemMatch(cmd, q) {
    if (q.length < 2) return false
    var words = (cmd.title + " " + cmd.words).toLowerCase().split(/\s+/)
    for (var i = 0; i < words.length; i++) if (words[i].indexOf(q) === 0) return true
    return false
  }

  // ---------------------------------------------------------------- user items & extensions
  // ~/.config/poste-island/launcher/launcher.json (hot-reloaded), plus that of each module:
  //   items:      [{ name, subtitle, icon, keywords, type: url|command|terminal|app|copy,
  //                  url | command | id (+args) | text, cwd, enabled }]
  //   extensions: [{ id, name, subtitle, icon, keywords, ttl, enabled,
  //                  source: { type: "script", command } }]
  //   An extension can have a 'keyword' (keyword + space to enter it)
  //   and source.search: command called with the input ("$1") at every
  //   keystroke, whose output replaces the list (live search); without
  //   input, the cached list is shown.
  //   engines:    [{ keyword, name, url ("…%s…"), icon, keywords, enabled }]:
  //               search engines. 'yt' + space (or Enter / → on the
  //               YouTube row) enters the engine, then Enter opens the results
  //               page. Ctrl+B on 'Search the web' offers them too.
  // A source script prints [{ id, name, subtitle, icon, keywords, path?, action: {...},
  // actions: [{ label, icon, action }] }]. Results are cached `ttl` seconds.
  readonly property string configPath: Quickshell.env("HOME") + "/.config/poste-island/launcher/launcher.json"
  property var userItems: []
  property var extensions: []
  property var engines: []
  // The modules' extensions and entries (~/.local/share/poste/modules/*/island/launcher.json,
  // same format) are added to those of the user's config, which wins in case of an identical id.
  property var userDoc: ({})
  readonly property var moduleDocs: host && host.modules ? host.modules.launcherDocs : []
  onModuleDocsChanged: applyConfig()
  // Specific to each machine (not versioned): ~/.config/poste-island/launcher/hidden.json,
  // ["Jellyfin", "unity", …]: names of entries and engines, ids of extensions, keywords of engines.
  // And 'requires' on an entry, extension or engine: "unityhub" (command in the PATH)
  // or "plugin:<id>" (installed Omarchy plugin), one value or a list; otherwise the entry is hidden.
  property var localHidden: []
  FileView {
    path: Quickshell.env("HOME") + "/.config/poste-island/launcher/hidden.json"
    watchChanges: true
    printErrors: false
    onFileChanged: reload()
    onLoaded: {
      var list = []
      try { list = JSON.parse(text()) } catch (e) { list = [] }
      launcher.localHidden = Array.isArray(list) ? list.map(function(s) { return String(s).toLowerCase() }) : []
      launcher.applyConfig()
    }
    onLoadFailed: { launcher.localHidden = []; launcher.applyConfig() }
  }
  // Requirements missing from this machine (null until the check has answered).
  property var missingRequires: null
  property string requiresKey: ""
  Process {
    id: requiresScan
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        var map = {}
        var lines = String(text || "").split("\n")
        for (var i = 0; i < lines.length; i++) if (lines[i].trim()) map[lines[i].trim()] = true
        launcher.missingRequires = map
        launcher.applyConfig()
      }
    }
  }
  function requiresOf(e) {
    var r = e && e.requires
    if (!r) return []
    return (Array.isArray(r) ? r : [r]).map(String).filter(function(s) { return s.length > 0 })
  }
  function shownHere(e) {
    var names = [e.name, e.id].concat(Array.isArray(e.keyword) ? e.keyword : [e.keyword])
    for (var n = 0; n < names.length; n++)
      if (names[n] && localHidden.indexOf(String(names[n]).toLowerCase()) !== -1) return false
    var req = requiresOf(e)
    if (req.length === 0) return true
    if (missingRequires === null) return false
    for (var i = 0; i < req.length; i++) if (missingRequires[req[i]]) return false
    return true
  }
  function scanRequires(docs) {
    var all = {}
    for (var d = 0; d < docs.length; d++) {
      var doc = docs[d] || {}
      var entries = (doc.items || []).concat(doc.extensions || [], doc.engines || [])
      for (var i = 0; i < entries.length; i++) {
        var req = requiresOf(entries[i])
        for (var j = 0; j < req.length; j++) all[req[j]] = true
      }
    }
    var list = Object.keys(all).sort()
    var key = list.join("\n")
    if (key === requiresKey && missingRequires !== null) return
    requiresKey = key
    if (list.length === 0) { missingRequires = ({}); return }
    requiresScan.command = ["sh", "-c",
      "for r; do case $r in plugin:*) [ -d \"$HOME/.config/omarchy/plugins/${r#plugin:}\" ] || echo \"$r\" ;; *) command -v \"$r\" >/dev/null 2>&1 || echo \"$r\" ;; esac; done",
      "requires"].concat(list)
    requiresScan.running = true
  }
  function applyConfig() {
    var docs = [userDoc, machineDoc]
    for (var m = 0; m < moduleDocs.length; m++) docs.push(moduleDocs[m].data || {})
    scanRequires(docs)
    var items = [], exts = [], engs = [], ids = {}
    for (var d = 0; d < docs.length; d++) {
      var doc = docs[d] || {}
      items = items.concat((doc.items || []).filter(function(i) { return i && i.enabled !== false && i.name && launcher.shownHere(i) }))
      exts = exts.concat((doc.extensions || []).filter(function(e) {
        if (!e || e.enabled === false || !e.id || !e.source || ids[e.id] || !launcher.shownHere(e)) return false
        ids[e.id] = true
        return true
      }))
      engs = engs.concat((doc.engines || []).filter(function(e) { return e && e.enabled !== false && e.keyword && e.name && String(e.url || "").indexOf("%s") !== -1 && launcher.shownHere(e) }))
    }
    launcher.userItems = items
    launcher.extensions = exts
    launcher.engines = engs
    launcher.extCache = ({})
    launcher.refreshExtensions(false)
    launcher.rebuild()
  }
  FileView {
    path: launcher.configPath
    watchChanges: true
    printErrors: false
    onFileChanged: reload()
    onLoaded: {
      var data = {}
      try { data = JSON.parse(text()) } catch (e) { data = {} }
      launcher.userDoc = data
      launcher.applyConfig()
    }
    onLoadFailed: { launcher.userDoc = ({}); launcher.applyConfig() }
  }
  // Specific to the machine and versioned elsewhere (data repository, poste-data):
  // ~/.config/poste-island/launcher/machine.json, same format, added after launcher.json
  // (entries to private services, for example).
  property var machineDoc: ({})
  FileView {
    path: Quickshell.env("HOME") + "/.config/poste-island/launcher/machine.json"
    watchChanges: true
    printErrors: false
    onFileChanged: reload()
    onLoaded: {
      var data = {}
      try { data = JSON.parse(text()) } catch (e) { data = {} }
      launcher.machineDoc = data
      launcher.applyConfig()
    }
    onLoadFailed: { launcher.machineDoc = ({}); launcher.applyConfig() }
  }

  function expandHome(path) { path = String(path || ""); return path.indexOf("~") === 0 ? Quickshell.env("HOME") + path.slice(1) : path }
  function extensionById(id) {
    if (String(id).indexOf("engine:") === 0) return engineById(id)
    if (String(id).indexOf("sub:") === 0) return subDefs[id] || null
    for (var i = 0; i < extensions.length; i++) if (extensions[i].id === id) return extensions[i]
    return null
  }
  // An engine behaves like an extension ('engine:<keyword>'): it is
  // entered, the breadcrumb shows its name, Backspace leaves it.
  function engineById(id) {
    var kw = String(id).replace(/^engine:/, "")
    // 'keyword': a keyword or a list (["jf", "media"]); the first one serves as identifier.
    for (var i = 0; i < engines.length; i++) {
      var e = engines[i]
      var kws = [].concat(e.keyword).map(function(k) { return String(k).toLowerCase() })
      if (kws.indexOf(kw.toLowerCase()) === -1) continue
      return { id: "engine:" + kws[0], name: String(e.name), icon: e.icon, url: String(e.url),
               keyword: kws.join(" "), keywords: e.keywords || [],
               subtitle: String(e.url).replace(/^https?:\/\//, "").split("/")[0] + " · " + kws.join(" or ") + " + space" }
    }
    return null
  }
  function engineUrlFor(engine, text) { return engine.url.replace("%s", encodeURIComponent(String(text).trim())) }

  // extCache[id] = { items: [...], at: ms, loading: bool }
  property var extCache: ({})
  property var extQueue: []
  function setCache(id, entry) {
    var next = {}
    for (var k in extCache) next[k] = extCache[k]
    next[id] = entry
    extCache = next
  }
  function refreshExtensions(force) {
    for (var i = 0; i < extensions.length; i++) {
      var ext = extensions[i]
      var c = extCache[ext.id]
      var ttl = (Number(ext.ttl) || 300) * 1000
      if (!force && c && (c.loading || Date.now() - c.at < ttl)) continue
      if (extQueue.indexOf(ext.id) === -1) extQueue = extQueue.concat([ext.id])
    }
    runNextSource()
  }
  function runNextSource() {
    if (sourceProc.running || extQueue.length === 0) return
    var id = extQueue[0]
    extQueue = extQueue.slice(1)
    var ext = extensionById(id)
    if (!ext || ext.source.type !== "script") { runNextSource(); return }
    var prior = extCache[id]
    setCache(id, { items: prior ? prior.items : [], at: prior ? prior.at : 0, loading: true })
    sourceProc.extId = id
    sourceProc.command = ["bash", "-c", expandHome(ext.source.command)]
    sourceProc.running = true
  }
  Process {
    id: sourceProc
    property string extId: ""
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        var list = []
        try { list = JSON.parse(text) } catch (e) { console.warn("poste.island: source " + sourceProc.extId + " printed invalid JSON") }
        launcher.setCache(sourceProc.extId, { items: Array.isArray(list) ? list : [], at: Date.now(), loading: false })
        launcher.rebuild()
      }
    }
    onExited: Qt.callLater(launcher.runNextSource)
  }

  // Live search in an extension (source.search).
  property var liveItems: []
  property string liveFor: ""
  Timer { id: liveDebounce; interval: 180; onTriggered: launcher.runLive() }
  function runLive() {
    var ext = extensionById(currentExt)
    var q = query.trim()
    if (!ext || !ext.source || !ext.source.search || q === "") return
    liveRestarting = true
    liveProc.running = false
    liveProc.forQuery = q
    liveProc.command = ["bash", "-c", expandHome(ext.source.search) + ' "$1"', "live", q]
    liveProc.running = true
    liveRestarting = false
    if (!liveSlow) liveSlowTimer.restart()
  }
  // 'It is searching' indicator: only if the search lasts more than 120 ms
  // (no flicker for fast searches), off as soon as it ends.
  property bool liveSlow: false
  // True while the search is restarted: the already lit indicator does not go off.
  property bool liveRestarting: false
  Timer { id: liveSlowTimer; interval: 120; onTriggered: launcher.liveSlow = liveProc.running }
  Process {
    id: liveProc
    property string forQuery: ""
    onRunningChanged: {
      if (launcher.liveRestarting) return
      if (running) liveSlowTimer.restart()
      else { liveSlowTimer.stop(); launcher.liveSlow = false }
    }
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        if (liveProc.forQuery !== launcher.query.trim()) return
        var list = []
        try { list = JSON.parse(text) } catch (e) { list = [] }
        launcher.liveItems = Array.isArray(list) ? list : []
        launcher.liveFor = liveProc.forQuery
        launcher.rebuild()
      }
    }
  }
  // Same gesture for all the lists: 'keyword + space' enters it, as
  // for engines. 'keyword': a word or a list (["fs", "music"]).
  function extensionByKeyword(word) {
    word = String(word).toLowerCase()
    for (var i = 0; i < extensions.length; i++) {
      var kws = [].concat(extensions[i].keyword || [])
      for (var k = 0; k < kws.length; k++) if (String(kws[k]).toLowerCase() === word) return extensions[i]
    }
    return null
  }

  // Current extension (drilled into), "" at the root.
  property string currentExt: ""
  property int slideDirection: 1
  function enterExtension(id) {
    slideDirection = 1
    currentExt = id
    selected = 0
    field.text = ""
    query = ""
    actionsOpen = false
    refreshExtensions(false)
    rebuild()
    slide.restart()
  }
  function leaveExtension() {
    if (currentExt === "") return false
    // Sub-list: back to the list we come from (extension, search and
    // selection included), without closing the launcher.
    if (currentExt.indexOf("sub:") === 0 && subStack.length > 0) {
      var back = subStack[subStack.length - 1]
      subStack = subStack.slice(0, -1)
      slideDirection = -1
      currentExt = back.ext
      actionsOpen = false
      field.text = back.query
      field.cursorPosition = back.query.length
      query = back.query
      rebuild()
      selected = Math.max(0, Math.min(back.selected, results.count - 1))
      list.currentIndex = selected
      list.positionViewAtIndex(selected, ListView.Contain)
      slide.restart()
      return true
    }
    slideDirection = -1
    currentExt = ""
    selected = 0
    field.text = ""
    query = ""
    actionsOpen = false
    rebuild()
    slide.restart()
    return true
  }

  // Sub-lists: an action {"type":"submenu","command":"…"} runs the command,
  // reads its output (same JSON as the sources) and shows it as an
  // ephemeral extension 'sub:<n>' (same breadcrumb, same rows, same slide).
  // subStack keeps the list we come from; Escape or Backspace goes back to it.
  property var subDefs: ({})
  property var subStack: []
  property int subCounter: 0
  property var subPending: null
  function openSubmenu(command, title, icon) {
    var id = "sub:" + (++subCounter)
    var defs = {}
    for (var k in subDefs) defs[k] = subDefs[k]
    defs[id] = { id: id, name: String(title || "…"), icon: icon, subtitle: "", source: { type: "submenu" } }
    subDefs = defs
    subStack = subStack.concat([{ ext: currentExt, query: query, selected: selected }])
    setCache(id, { items: [], at: Date.now(), loading: true })
    enterExtension(id)
    // One command at a time: the next one waits for the end of the previous one.
    if (subProc.running) { subPending = { id: id, command: command }; return }
    startSub(id, command)
  }
  function startSub(id, command) {
    subProc.subId = id
    subProc.command = ["bash", "-c", String(command)]
    subProc.running = true
  }
  Process {
    id: subProc
    property string subId: ""
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        var list = null
        try { list = JSON.parse(text) } catch (e) { console.warn("poste.island: submenu printed invalid JSON") }
        if (!Array.isArray(list))
          list = [{ id: "sub-error", name: "List unavailable", subtitle: "The command did not return valid JSON", icon: "󰅛" }]
        launcher.setCache(subProc.subId, { items: list, at: Date.now(), loading: false })
        launcher.rebuild()
      }
    }
    onExited: {
      var next = launcher.subPending
      launcher.subPending = null
      if (next) Qt.callLater(function() { launcher.startSub(next.id, next.command) })
    }
  }

  // An icon field is an icon name, a file path, or an emoji / glyph.
  function iconSourceOf(icon) {
    icon = String(icon || "")
    if (!icon) return ""
    if (icon.charAt(0) === "/" || icon.charAt(0) === "~") return "file://" + expandHome(icon)
    if (/^[a-z0-9._+-]+$/i.test(icon)) return Quickshell.iconPath(icon, true)
    return ""
  }
  function glyphOf(icon, fallback) {
    icon = String(icon || "")
    return icon && !/^[a-z0-9._+\/~-]+$/i.test(icon) ? icon : fallback
  }
  function matchScore(entry, q) {
    return Math.max(fuzzy(entry.name, q), fuzzy(entry.subtitle, q) - 150, fuzzy((entry.keywords || []).join(" "), q) - 120)
  }
  function itemKey(entry, extId) {
    return extId ? "ext:" + extId + ":" + String(entry.id || entry.name) : "item:" + String(entry.name)
  }
  function itemRow(entry, pathLabel, extId) {
    // The rows of a sub-list are not pinned and do not count in the usage.
    var key = String(extId).indexOf("sub:") === 0 ? "" : itemKey(entry, extId)
    return { kind: "item", title: String(entry.name || ""), subtitle: pathLabel || String(entry.subtitle || ""),
      icon: iconSourceOf(entry.icon), glyph: glyphOf(entry.icon, "󰀻"), chevron: false, key: key, pinned: isPinned(key),
      ref: JSON.stringify({ item: entry, ext: extId || "" }) }
  }
  function extensionRow(ext) {
    var c = extCache[ext.id]
    var count = c && c.items ? c.items.length : 0
    var key = "ext:" + ext.id
    // Engines and live-search extensions: 'Search …', so as not to
    // confuse them with an app of the same name.
    var searchable = String(ext.id).indexOf("engine:") === 0 || !!(ext.source && ext.source.search)
    // The keyword always shows in the same place: 'rec ␣'.
    var kw = String([].concat(ext.keyword || [])[0] || "").split(" ")[0]
    return { kind: "extension", title: (searchable ? "Search " : "") + String(ext.name),
      subtitle: String(ext.subtitle || "") + (count && !searchable ? " · " + count : "") + (kw ? " · " + kw + " ␣" : ""),
      icon: iconSourceOf(ext.icon), glyph: glyphOf(ext.icon, "󰉋"), chevron: true, key: key, pinned: isPinned(key), ref: ext.id }
  }
  // An item's own action spec (items use top-level fields, sources use .action).
  function actionSpecOf(entry) {
    if (entry.action) return entry.action
    return { type: entry.type, url: entry.url, command: entry.command, id: entry.id, args: entry.args, text: entry.text, cwd: entry.cwd, title: entry.name }
  }
  function runSpec(spec, title, icon) {
    if (!spec) return
    if (spec.type === "submenu") { openSubmenu(spec.command, title, icon); return }
    var cwd = spec.cwd ? expandHome(spec.cwd) : Quickshell.env("HOME")
    if (spec.type === "url") openUrl(spec.url)
    else if (spec.type === "copy") copyText(spec.text)
    else if (spec.type === "command")
      Quickshell.execDetached(["setsid", "-f", "bash", "-c", "cd \"$1\" 2>/dev/null; eval \"$2\"", "cmd", cwd, spec.command])
    else if (spec.type === "terminal")
      Quickshell.execDetached(["uwsm-app", "--", "xdg-terminal-exec", "--dir=" + cwd, "--title=" + String(spec.title || title || "Terminal"), "--", "bash", "-c", spec.command])
    else if (spec.type === "app")
      Quickshell.execDetached(["uwsm-app", "--", "gtk-launch", String(spec.id) + ".desktop"].concat(spec.args || []))
  }

  ListModel { id: results }

  // ---------------------------------------------------------------- open windows
  // The open windows (hyprctl clients): typing the name of an open app
  // or the title of a window offers 'Switch to …', which goes to the window
  // (screen and workspace included). Taken from the audit of omarchy-jump-to: the address
  // is validated before entering the Hyprland command (Lua), and the
  // windows are sorted by recent use.
  property var windows: []
  Process {
    id: windowsProc
    command: ["hyprctl", "clients", "-j"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        var list = []
        try { list = JSON.parse(text) } catch (e) { list = [] }
        var out = []
        for (var i = 0; i < list.length && i < 512; i++) {
          var c = list[i]
          if (!c || !c.mapped || !/^0x[0-9a-f]{1,16}$/.test(String(c.address))) continue
          out.push({ address: String(c.address), cls: String(c.class || c.initialClass || "").slice(0, 200),
                     title: String(c.title || "").slice(0, 300), wsId: Number((c.workspace || {}).id) || 0,
                     wsName: String((c.workspace || {}).name || ""), history: c.focusHistoryID >= 0 ? c.focusHistoryID : 999 })
        }
        launcher.windows = out
        launcher.rebuild()
      }
    }
  }
  property var appByClass: ({})
  function appForClass(cls) {
    var key = String(cls).toLowerCase()
    if (key === "") return null
    if (appByClass[key] !== undefined) return appByClass[key]
    var apps = DesktopEntries.applications.values || [], found = null
    for (var i = 0; i < apps.length && !found; i++) {
      var e = apps[i]
      if (String(e.startupClass || "").toLowerCase() === key || String(e.id || "").toLowerCase() === key) found = e
    }
    for (var j = 0; j < apps.length && !found; j++) {
      var id = String(apps[j].id || "").toLowerCase()
      if (id.split(".").pop() === key.split(".").pop() || id.indexOf(key) === 0) found = apps[j]
    }
    var next = {}
    for (var k in appByClass) next[k] = appByClass[k]
    next[key] = found
    appByClass = next
    return found
  }
  // Quickshell apps (Omamail…) all have the class org.quickshell:
  // they are recognized by their title, equal to the app's name.
  function appForWindow(w) {
    if (String(w.cls).toLowerCase() !== "org.quickshell") return appForClass(w.cls)
    var t = String(w.title).toLowerCase(), apps = DesktopEntries.applications.values || []
    for (var i = 0; i < apps.length; i++) if (String(apps[i].name || "").toLowerCase() === t) return apps[i]
    return null
  }
  // 'Center · 3': the screen (workspaces 1-10 on the left, 11-20 in the center,
  // 21-30 on the right) then the local number.
  function windowWhere(w) {
    if (w.wsId < 0) return "Scratchpad"
    var side = ["Left", "Center", "Right"][Math.floor((w.wsId - 1) / 10)] || "Screen"
    return side + " · " + (w.wsName || String(((w.wsId - 1) % 10) + 1))
  }
  function windowRow(w) {
    var app = appForWindow(w)
    var name = app ? String(app.name) : (w.cls || "Window")
    return { kind: "window", title: "Switch to " + name,
      subtitle: (w.title && w.title !== name ? w.title + "  ·  " : "") + windowWhere(w),
      icon: app && app.icon ? Quickshell.iconPath(app.icon, true) : Quickshell.iconPath(w.cls, true),
      glyph: "󰖯", chevron: false, key: "", pinned: false,
      ref: JSON.stringify({ address: w.address, app: app ? String(app.id) : "" }) }
  }
  function windowScore(w, q) {
    var app = appForWindow(w)
    var best = Math.max(fuzzy(app ? app.name : "", q), fuzzy(w.title, q) - 60, fuzzy(w.cls, q) - 80)
    return best < 0 ? -1 : best + 120 - Math.min(w.history, 30)
  }
  function focusWindow(address) {
    if (!/^0x[0-9a-f]{1,16}$/.test(String(address))) return
    Quickshell.execDetached(["bash", "-c", 'hyprctl dispatch "hl.dsp.focus({ window = \\"address:$1\\" })" >/dev/null', "poste-switch", address])
  }
  function closeWindow(address) {
    if (!/^0x[0-9a-f]{1,16}$/.test(String(address))) return
    Quickshell.execDetached(["bash", "-c", 'hyprctl dispatch "hl.dsp.window.close({ window = \\"address:$1\\" })" >/dev/null', "poste-close", address])
  }

  function appRow(app) {
    var key = "app:" + app.id
    return { kind: "app", title: String(app.name), subtitle: String(app.genericName || app.comment || ""),
      icon: app.icon ? Quickshell.iconPath(app.icon, true) : "", glyph: "󰀻", chevron: false, key: key, pinned: isPinned(key), ref: String(app.id) }
  }
  // An address: a domain name, localhost or an IPv4, with optional port and path.
  // Without protocol: https, except localhost and IPs (a local service is rarely on https).
  function looksLikeUrl() { return /^(https?:\/\/)?(([a-z0-9-]+\.)+[a-z]{2,}|localhost|(\d{1,3}\.){3}\d{1,3})(:[0-9]+)?(\/\S*)?$/i.test(query.trim()) }
  function urlFor(text) {
    text = text.trim()
    if (/^https?:\/\//i.test(text)) return text
    return (/^(localhost|(\d{1,3}\.){3}\d{1,3})([:\/]|$)/i.test(text) ? "http://" : "https://") + text
  }
  function webUrlFor(text) { return searchUrl.replace("%s", encodeURIComponent(text.trim())) }

  function rebuild() {
    var q = query.trim().toLowerCase()
    var rows = []
    if (tab === "apps" && currentExt.indexOf("engine:") === 0) {
      var eng = engineById(currentExt)
      if (eng && q)
        rows.push({ kind: "web", title: "Search " + eng.name + " for “" + query.trim() + "”",
          subtitle: engineUrlFor(eng, query).replace(/^https?:\/\//, "").split("/")[0],
          icon: iconSourceOf(eng.icon), glyph: glyphOf(eng.icon, "󰖟"), chevron: false, key: "", pinned: false, ref: engineUrlFor(eng, query) })
      else if (eng)
        rows.push({ kind: "loading", title: "Type to search " + eng.name, subtitle: "Enter opens the results",
          icon: iconSourceOf(eng.icon), glyph: glyphOf(eng.icon, "󰖟"), chevron: false, key: "", pinned: false, ref: "" })
    } else if (tab === "apps" && currentExt !== "" && q && (extensionById(currentExt) || {}).source
               && extensionById(currentExt).source.search) {
      // Live-search extension: the script's output, as is.
      if (liveFor === query.trim())
        for (var lv = 0; lv < liveItems.length; lv++) rows.push(itemRow(liveItems[lv], "", currentExt))
      else if (shownUids.length === 0 || liveItems.length === 0)
        rows.push({ kind: "loading", title: "Searching…", subtitle: "", icon: "", glyph: "󰔟", chevron: false, key: "", pinned: false, ref: "" })
      else
        for (var lo = 0; lo < liveItems.length; lo++) rows.push(itemRow(liveItems[lo], "", currentExt))
    } else if (tab === "apps" && currentExt !== "") {
      // Inside an extension: its items, fuzzy-filtered.
      var extItems = (extCache[currentExt] || {}).items || []
      var matched = []
      for (var x = 0; x < extItems.length; x++) {
        var sc = q ? matchScore(extItems[x], q) : 1000 - x
        if (sc >= 0) matched.push({ e: extItems[x], s: sc })
      }
      matched.sort(function(a, b) { return b.s - a.s })
      for (var y = 0; y < matched.length; y++) rows.push(itemRow(matched[y].e, "", currentExt))
      if (extItems.length === 0 && (extCache[currentExt] || {}).loading)
        rows.push({ kind: "loading", title: "Loading…", subtitle: "", icon: "", glyph: "󰔟", chevron: false, key: "", pinned: false, ref: "" })
    } else if (tab === "clip") {
      var clips = ClipboardHistory.displayRows(clipHistory, query, 40)
      for (var c = 0; c < clips.length; c++) {
        var r = clips[c]
        rows.push({ kind: "clip", title: String(r.previewText || "").replace(/\s+/g, " ").trim(), subtitle: r.entryType === "image" ? "Image" : "",
          icon: r.previewImage ? "file://" + r.previewImage : "", glyph: r.entryType === "image" ? "󰋩" : "󰅍",
          chevron: false, key: "", pinned: false, ref: JSON.stringify({ type: r.entryType, path: r.path || "", mime: r.mime || "", index: r.index }) })
      }
    } else {
      if (q && looksLikeUrl())
        rows.push({ kind: "url", title: "Open " + query.trim(), subtitle: urlFor(query), icon: "", glyph: "󰖟", chevron: false, key: "", pinned: false, ref: urlFor(query) })
      // 'timer 10m pasta': starts a timer in the island.
      var tm = query.trim().match(/^(timer)\b\s*(.*)$/i)
      if (tm) {
        var dur = tm[2] ? host.parseDuration(tm[2]) : null
        if (dur)
          rows.push({ kind: "timer", title: "Start a " + host.formatLeft(dur.ms) + " timer" + (dur.label ? " · " + dur.label : ""),
            subtitle: "Shows next to the island, rings at the end", icon: "", glyph: "󰔛", chevron: false, key: "", pinned: false,
            ref: JSON.stringify(dur) })
        else
          rows.push({ kind: "loading", title: "timer <duration> [label]", subtitle: "e.g. timer 10m pasta · timer 1h30 · timer 90s tea",
            icon: "", glyph: "󰔛", chevron: false, key: "", pinned: false, ref: "" })
      }
      if (calcResult !== "" && looksLikeMath())
        rows.push({ kind: "calc", title: calcResult, subtitle: "Enter to copy the result", icon: "", glyph: "󰃬", chevron: false, key: "", pinned: false, ref: calcResult })
      // Every candidate row with its match score (0 when the query is empty):
      // apps, extensions, the user's items, and items already loaded in extensions.
      var cands = []
      var apps = DesktopEntries.applications.values || []
      for (var i = 0; i < apps.length; i++) {
        var e = apps[i]
        if (launcher.isHidden(e)) continue
        var s = q ? Math.max(fuzzy(e.name, q), fuzzy(e.genericName, q) - 80, fuzzy((e.keywords || []).join(" "), q) - 120) : 0
        if (s >= 0) cands.push({ row: appRow(e), s: s })
      }
      for (var ex = 0; ex < extensions.length; ex++) {
        var ext = extensions[ex]
        var es = q ? Math.max(fuzzy(ext.name, q), fuzzy((ext.keywords || []).join(" "), q) - 120) : 0
        if (es >= 0) cands.push({ row: extensionRow(ext), s: es })
      }
      if (q) {
        for (var wi = 0; wi < windows.length; wi++) {
          var wsc = windowScore(windows[wi], q)
          if (wsc >= 0) cands.push({ row: windowRow(windows[wi]), s: wsc })
        }
      }
      for (var en = 0; en < engines.length; en++) {
        var engRow = engineById("engine:" + [].concat(engines[en].keyword)[0])
        // (a -1 must never become positive through a bonus)
        var kwScore = fuzzy(engRow.keyword, q)
        var ens = q ? Math.max(fuzzy(engRow.name, q), kwScore >= 0 ? kwScore + 50 : -1, fuzzy(engRow.keywords.join(" "), q) - 120) : 0
        if (ens >= 0) cands.push({ row: extensionRow(engRow), s: ens })
      }
      for (var ui = 0; ui < userItems.length; ui++) {
        var us = q ? matchScore(userItems[ui], q) : 0
        if (us >= 0) cands.push({ row: itemRow(userItems[ui], "", ""), s: us })
      }
      for (var ey = 0; ey < extensions.length; ey++) {
        // "rootItems": false: its elements only come out inside the extension.
        if (extensions[ey].rootItems === false) continue
        var cached = (extCache[extensions[ey].id] || {}).items || []
        for (var ci = 0; ci < cached.length; ci++) {
          var ns = q ? matchScore(cached[ci], q) : 0
          if (ns >= 0) cands.push({ row: itemRow(cached[ci], extensions[ey].name + " › " + String(cached[ci].subtitle || cached[ci].name), extensions[ey].id), s: ns })
        }
      }
      for (var c2 = 0; c2 < cands.length; c2++) cands[c2].f = frecency(cands[c2].row.key)

      if (!q) {
        // Empty search: pins (in pin order), then by frecency, then A→Z.
        // Extension items only show once used or pinned, so the list isn't
        // flooded by every playlist / project.
        var byKey = {}
        for (var c3 = 0; c3 < cands.length; c3++) byKey[cands[c3].row.key] = cands[c3]
        for (var p = 0; p < pinned.length; p++) if (byKey[pinned[p]]) rows.push(byKey[pinned[p]].row)
        var rest = cands.filter(function(c) {
          return !c.row.pinned && (c.f > 0 || c.row.key.split(":").length < 3)
        })
        rest.sort(function(a, b) { return b.f - a.f || a.row.title.localeCompare(b.row.title) })
        for (var r2 = 0; r2 < rest.length; r2++) rows.push(rest[r2].row)
      } else {
        // Search: match score first; frecency and pins as a moderate bonus.
        for (var c4 = 0; c4 < cands.length; c4++)
          cands[c4].t = cands[c4].s + Math.log(1 + cands[c4].f) * 25 + (cands[c4].row.pinned ? 40 : 0)
        // An open window always comes right before its app (otherwise
        // the app's usage, counted as a bonus, would put the app before it).
        var appT = {}
        for (var c5 = 0; c5 < cands.length; c5++)
          if (cands[c5].row.kind === "app") appT[cands[c5].row.ref] = cands[c5].t
        for (var c6 = 0; c6 < cands.length; c6++) {
          if (cands[c6].row.kind !== "window") continue
          var wApp = JSON.parse(cands[c6].row.ref).app
          if (appT[wApp] !== undefined) cands[c6].t = Math.max(cands[c6].t, appT[wApp] + 0.5)
        }
        cands.sort(function(a, b) { return b.t - a.t || a.row.title.localeCompare(b.row.title) })
        for (var mi = 0; mi < Math.min(cands.length, 16); mi++) rows.push(cands[mi].row)
      }
      if (q) {
        for (var k = 0; k < systemCommands.length; k++) {
          var cmd = systemCommands[k]
          if (systemMatch(cmd, q))
            rows.push({ kind: "system", title: cmd.title, subtitle: cmd.subtitle, icon: "", glyph: cmd.glyph, chevron: false, key: "", pinned: false, ref: String(k) })
        }
        rows.push({ kind: "web", title: "Search the web for “" + query.trim() + "”", subtitle: webUrlFor(query).replace(/^https?:\/\//, "").split("/")[0],
          icon: "", glyph: "󰖟", chevron: false, key: "", pinned: false, ref: webUrlFor(query) })
      }
    }
    applyRows(rows)
    updateHighlight()
  }

  // Extension with 'highlight': true (for example 'Record'): the target of the
  // selected row (window, screen, preset) is framed on screen by the island (host.recordHighlight).
  property bool ownsHighlight: false
  function updateHighlight() {
    if (!active) return
    var hl = null
    var cur = currentExt ? extensionById(currentExt) : null
    if (cur && cur.highlight === true && selected >= 0 && selected < results.count) {
      var row = results.get(selected)
      if (row && row.kind === "item") {
        try { hl = JSON.parse(row.ref).item.highlight || null } catch (e) { hl = null }
      }
    }
    host.recordHighlight = hl
    ownsHighlight = true
  }
  onCurrentExtChanged: updateHighlight()

  // Differential update of `results`, by a stable id per logical row: rows
  // already shown stay (set() for changed roles, move() when they change
  // place, animated by `displaced`/`move`), only new rows play `add`, and
  // gone rows fade out through `remove`. The selection follows its row.
  property var shownUids: []
  property string shownQuery: ""
  function rowUid(r) {
    if (r.kind === "web" || r.kind === "url" || r.kind === "calc" || r.kind === "loading" || r.kind === "timer") return r.kind
    if (r.key) return r.kind + "|" + r.key
    return r.kind + "|" + r.ref
  }
  function applyRows(rows) {
    // The default selection (first row) stays on the best match; a row the
    // user moved to is followed while it stays in the list. When the input
    // changes, not the rows made of the input (address, web search,
    // calculation…): following 'Search the web' made it search an address
    // instead of opening it.
    var prevUid = selected > 0 && selected < shownUids.length ? shownUids[selected] : ""
    if (query !== shownQuery && prevUid.indexOf("|") === -1) prevUid = ""
    shownQuery = query
    var uids = []
    var wanted = {}
    for (var i = 0; i < rows.length; i++) {
      var u = rowUid(rows[i])
      if (wanted[u]) { u = u + "#" + i }
      wanted[u] = true
      uids.push(u)
    }
    var cur = shownUids.slice()
    for (var j = cur.length - 1; j >= 0; j--) if (!wanted[cur[j]]) { results.remove(j); cur.splice(j, 1) }
    for (var k = 0; k < rows.length; k++) {
      var at = cur.indexOf(uids[k], k)
      if (at === -1) { results.insert(k, rows[k]); cur.splice(k, 0, uids[k]) }
      else {
        if (at !== k) { results.move(at, k, 1); cur.splice(k, 0, cur.splice(at, 1)[0]) }
        results.set(k, rows[k])
      }
    }
    while (results.count > rows.length) { results.remove(results.count - 1); cur.pop() }
    shownUids = uids
    var keep = prevUid !== "" ? uids.indexOf(prevUid) : -1
    selected = keep >= 0 ? keep : 0
    // ListView moves its currentIndex along with the current item on
    // insert/move (which also breaks the binding): put it back.
    list.currentIndex = selected
    list.positionViewAtIndex(selected, ListView.Contain)
  }
  onSelectedChanged: { confirmPending = ""; if (list.currentIndex !== selected) list.currentIndex = selected; updateHighlight() }

  onQueryChanged: {
    confirmPending = ""
    // 'yt' + space: enters the engine (the rest of the input follows).
    if (tab === "apps" && currentExt === "") {
      var m = query.match(/^(\S+)\s(.*)$/)
      // 'ai' + space: switches to the island's AI view.
      if (m && /^(ai|ia)$/i.test(m[1])) { host.openAsk(m[2], false); return }
      var hit = m ? (engineById("engine:" + m[1]) || extensionByKeyword(m[1])) : null
      if (hit) {
        var rest = m[2]
        enterExtension(hit.id)
        if (rest !== "") { field.text = rest; field.cursorPosition = rest.length }
        return
      }
    }
    actionsOpen = false
    if (currentExt !== "" && query.trim() !== "") liveDebounce.restart()
    if (!looksLikeMath()) calcResult = ""
    else calcDelay.restart()
    rebuild()
  }
  onTabChanged: { selected = 0; actionsOpen = false; rebuild() }

  // ---------------------------------------------------------------- actions
  function paste(c) {
    var bin = host.omarchyPath || "/usr/share/omarchy"
    if (c.type === "image") Quickshell.execDetached([bin + "/bin/omarchy-clipboard-paste-file", c.mime, c.path])
    else Quickshell.execDetached([bin + "/bin/omarchy-clipboard-paste-text", "--shift-insert", "--history-index", String(c.index)])
  }
  function copyClip(c) {
    var bin = host.omarchyPath || "/usr/share/omarchy"
    if (c.type === "image") Quickshell.execDetached([bin + "/bin/omarchy-clipboard-paste-file", "--copy-only", c.mime, c.path])
    else Quickshell.execDetached([bin + "/bin/omarchy-clipboard-paste-text", "--copy-only", "--history-index", String(c.index)])
  }
  function copyText(text) { Quickshell.execDetached(["wl-copy", "--", String(text)]) }
  function openUrl(url) { Quickshell.execDetached(["xdg-open", String(url)]) }
  function launchApp(id) { Quickshell.execDetached(["uwsm-app", "--", "gtk-launch", id + ".desktop"]) }

  // Each row kind's actions; the first is what Enter does. Rows with a key
  // (apps, items, extensions, extension items) can also be pinned.
  function pinAction(row) {
    var on = isPinned(row.key)
    return { id: "pin", label: on ? "Unpin" : "Pin to top", glyph: on ? "󰤰" : "󰐃" }
  }
  function actionsFor(row) {
    if (!row) return []
    var acts = actionsForKind(row)
    if (row.key) acts.splice(Math.min(1, acts.length), 0, pinAction(row))
    return acts
  }
  function actionsForKind(row) {
    if (row.kind === "app") {
      return [
        { id: "open", label: "Open", glyph: "󰏌" },
        { id: "copy-name", label: "Copy name", glyph: "󰆏" },
        { id: "reveal", label: "Show .desktop file", glyph: "󰉋" },
        { id: "hide", label: "Hide from launcher", glyph: "󰈉" }
      ]
    }
    if (row.kind === "clip") return [
      { id: "paste", label: "Paste", glyph: "󰆒" },
      { id: "copy", label: "Copy", glyph: "󰆏" },
      { id: "delete", label: "Delete from history", glyph: "󰩹" }
    ]
    if (row.kind === "calc") return [{ id: "copy", label: "Copy result", glyph: "󰆏" }]
    if (row.kind === "timer") return [{ id: "start", label: "Start timer", glyph: "󰔛" }]
    if (row.kind === "window") {
      var wref = JSON.parse(row.ref)
      var wacts = [{ id: "switch", label: "Switch to window", glyph: "󰖯" }]
      if (wref.app) wacts.push({ id: "new", label: "New window", glyph: "󰐕" })
      wacts.push({ id: "close", label: "Close window", glyph: "󰅖" })
      return wacts
    }
    if (row.kind === "url") return [
      { id: "open", label: "Open", glyph: "󰏌" },
      { id: "copy-url", label: "Copy URL", glyph: "󰆏" }
    ]
    if (row.kind === "web") {
      var webActs = [{ id: "open", label: "Open", glyph: "󰏌" }, { id: "copy-url", label: "Copy URL", glyph: "󰆏" },
                     { id: "ask-ai", label: "Ask AI", glyph: "󰚩" }]
      for (var ei = 0; ei < engines.length; ei++) {
        var eid = "engine:" + String([].concat(engines[ei].keyword)[0]).toLowerCase()
        if (eid !== currentExt)
          webActs.push({ id: eid, label: "Search on " + engines[ei].name, glyph: glyphOf(engines[ei].icon, "󰖟") })
      }
      return webActs
    }
    if (row.kind === "extension" && String(row.ref).indexOf("engine:") === 0) return [
      { id: "enter", label: "Search " + row.title, glyph: "󰅂" }
    ]
    if (row.kind === "system") return [{ id: "run", label: row.title, glyph: row.glyph }]
    if (row.kind === "extension") return [
      { id: "enter", label: "Open", glyph: "󰅂" },
      { id: "refresh", label: "Refresh", glyph: "󰑐" }
    ]
    if (row.kind === "item") {
      var ref = JSON.parse(row.ref)
      var spec = actionSpecOf(ref.item)
      // "verb": the word of the main action ('Play' for music).
      var acts = [{ id: "open", label: String(ref.item.verb || "Open"), glyph: ref.item.verb === "Play" ? "󰐊" : spec.type === "submenu" ? "󰅂" : "󰏌" }]
      if (spec.type === "url") acts.push({ id: "copy-url", label: "Copy URL", glyph: "󰆏" })
      if (ref.item.path || spec.cwd) acts.push({ id: "copy-path", label: "Copy path", glyph: "󰆏" })
      var extra = ref.item.actions || []
      for (var a = 0; a < extra.length; a++)
        acts.push({ id: "extra:" + a, label: String(extra[a].label || "Action"), glyph: String(extra[a].icon || "󰘳") })
      return acts
    }
    return []
  }

  function runAction(index, actionId) {
    if (index < 0 || index >= results.count) return
    var row = results.get(index)
    var keepOpen = false
    if (actionId === "pin") {
      // Keep the cursor on the same row once the list is re-sorted.
      var pinKey = row.key
      togglePin(pinKey)
      actionsOpen = false
      rebuild()
      for (var pi = 0; pi < results.count; pi++) if (results.get(pi).key === pinKey) { selected = pi; break }
      list.positionViewAtIndex(selected, ListView.Contain)
      return
    }
    // Anything that runs the row counts as a use (frecency).
    if (row.key && ["hide", "refresh", "delete", "copy-name", "copy-url", "copy-path", "reveal"].indexOf(actionId) === -1)
      record(row.key)
    if (row.kind === "app") {
      if (actionId === "open") launchApp(row.ref)
      else if (actionId === "copy-name") copyText(row.title)
      else if (actionId === "reveal") Quickshell.execDetached(["sh", "-c",
        "for d in \"$HOME/.local/share/applications\" /usr/local/share/applications /usr/share/applications \"$HOME/.local/share/flatpak/exports/share/applications\" /var/lib/flatpak/exports/share/applications; do " +
        "f=\"$d/$1.desktop\"; [ -e \"$f\" ] && exec nautilus --select \"$f\"; done", "reveal", row.ref])
      else if (actionId === "hide") {
        blacklist = blacklist.concat([row.ref]); pinned = pinned.filter(function(k) { return k !== row.key })
        saveState(); keepOpen = true
      }
    } else if (row.kind === "clip") {
      var c = JSON.parse(row.ref)
      if (actionId === "paste") paste(c)
      else if (actionId === "copy") copyClip(c)
      else if (actionId === "delete") {
        clipHistory = ClipboardHistory.removeEntryAt(clipHistory, c.index)
        clipFile.setText(JSON.stringify(clipHistory, null, 2) + "\n")
        keepOpen = true
      }
    } else if (row.kind === "calc") {
      copyText(row.ref)
    } else if (row.kind === "window") {
      var wr = JSON.parse(row.ref)
      if (actionId === "new") launchApp(wr.app)
      else if (actionId === "close") { closeWindow(wr.address); keepOpen = true; windowsProc.running = true }
      else focusWindow(wr.address)
    } else if (row.kind === "timer") {
      var d = JSON.parse(row.ref)
      host.startTimer(d.ms, d.label)
    } else if (row.kind === "web" || row.kind === "url") {
      if (actionId === "copy-url") copyText(row.ref)
      else if (actionId === "ask-ai") { actionsOpen = false; host.openAsk(query, true); return }
      else if (actionId.indexOf("engine:") === 0) { var other = engineById(actionId); if (other) openUrl(engineUrlFor(other, query)) }
      else openUrl(row.ref)
    } else if (row.kind === "system") {
      var sys = systemCommands[Number(row.ref)]
      if (sys.confirm && confirmPending !== row.ref) {
        confirmPending = row.ref
        confirmTimer.restart()
        actionsOpen = false
        return
      }
      confirmPending = ""
      Quickshell.execDetached(sys.command)
    } else if (row.kind === "extension") {
      if (actionId === "refresh") { refreshExtensions(true); keepOpen = true }
      else { actionsOpen = false; enterExtension(row.ref); return }
    } else if (row.kind === "item") {
      var r = JSON.parse(row.ref)
      var sp = actionSpecOf(r.item)
      if (actionId === "copy-url") copyText(sp.url)
      else if (actionId === "copy-path") copyText(expandHome(r.item.path || sp.cwd))
      else if (actionId.indexOf("extra:") === 0) {
        var ex = (r.item.actions || [])[Number(actionId.slice(6))]
        if (ex) runSpec(ex.action, ex.label, ex.icon)
        // A sub-list opens in the launcher, which stays open.
        if (ex && ex.action && ex.action.type === "submenu") { actionsOpen = false; return }
      } else {
        runSpec(sp, r.item.name, r.item.icon)
        if (sp.type === "submenu") { actionsOpen = false; return }
      }
    } else if (row.kind === "loading") {
      keepOpen = true
    }
    actionsOpen = false
    if (keepOpen) rebuild()
    else closeRequested()
  }

  function activate(index) {
    if (index < 0 || index >= results.count) return
    var acts = actionsFor(results.get(index))
    if (acts.length) runAction(index, acts[0].id)
  }

  property bool actionsOpen: false
  property int actionIndex: 0
  readonly property var currentActions: actionsOpen && selected < results.count ? actionsFor(results.get(selected)) : []
  function toggleActions() {
    if (results.count === 0) return
    actionIndex = 0
    actionsOpen = !actionsOpen
  }

  // ---------------------------------------------------------------- UI
  Column {
    id: header
    anchors.left: parent.left
    anchors.right: parent.right
    spacing: 10

    Item {
      width: parent.width
      height: 30
      Glyph {
        id: searchGlyph
        anchors.left: parent.left
        anchors.leftMargin: 4
        anchors.verticalCenter: parent.verticalCenter
        glyph: launcher.tab === "clip" ? "󰅍" : launcher.currentExt !== "" ? "󰁍" : "󰍉"
        color: launcher.host.colorMuted
        fontFamily: launcher.host.fontFamily
        pixelSize: 16
      }
      Text {
        id: crumb
        anchors.left: searchGlyph.right
        anchors.leftMargin: 10
        anchors.verticalCenter: parent.verticalCenter
        visible: launcher.currentExt !== ""
        text: launcher.currentExt !== "" ? (launcher.extensionById(launcher.currentExt) || { name: "" }).name + "  ›" : ""
        color: launcher.host.colorText
        font.family: launcher.host.uiFont
        font.pixelSize: 15
        font.weight: Font.DemiBold
        MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: launcher.leaveExtension() }
      }
      TextInput {
        id: field
        // (without breadcrumb, the field starts from the glyph: no width bound to implicitWidth,
        // which looped when the name changes from one sub-list to another)
        anchors.left: crumb.visible ? crumb.right : searchGlyph.right
        anchors.leftMargin: 10
        anchors.right: tabs.left
        anchors.rightMargin: 10
        anchors.verticalCenter: parent.verticalCenter
        color: launcher.host.colorText
        selectionColor: launcher.host.withAlpha(launcher.host.colorAccent, 0.4)
        font.family: launcher.host.uiFont
        font.pixelSize: 17
        clip: true
        onTextChanged: launcher.query = text
        Text {
          anchors.fill: parent
          visible: !field.text
          text: launcher.tab === "clip" ? "Search clipboard"
            : launcher.currentExt !== "" ? "Search…" : "Search apps, calculate…"
          color: launcher.host.colorMuted
          font: field.font
        }
        Keys.onPressed: function(event) {
          if (event.key === Qt.Key_B && (event.modifiers & Qt.ControlModifier)) { launcher.toggleActions(); event.accepted = true; return }
          if (launcher.actionsOpen) {
            var n = launcher.currentActions.length
            if (event.key === Qt.Key_Escape || event.key === Qt.Key_Left) launcher.actionsOpen = false
            else if (event.key === Qt.Key_Down) launcher.actionIndex = Math.min(n - 1, launcher.actionIndex + 1)
            else if (event.key === Qt.Key_Up) launcher.actionIndex = Math.max(0, launcher.actionIndex - 1)
            else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
              var act = launcher.currentActions[launcher.actionIndex]
              if (act) launcher.runAction(launcher.selected, act.id)
            } else return
            event.accepted = true
            return
          }
          var rowNow = results.count ? results.get(Math.min(launcher.selected, results.count - 1)) : null
          if (event.key === Qt.Key_Escape) { if (!launcher.leaveExtension()) launcher.closeRequested(); event.accepted = true; return }
          if (event.key === Qt.Key_Right && field.cursorPosition === field.text.length && rowNow && rowNow.kind === "extension") { launcher.enterExtension(rowNow.ref); event.accepted = true; return }
          if ((event.key === Qt.Key_Left || event.key === Qt.Key_Backspace) && field.text === "" && launcher.currentExt !== "") { launcher.leaveExtension(); event.accepted = true; return }
          else if (event.key === Qt.Key_Down) { launcher.selected = Math.min(results.count - 1, launcher.selected + 1); list.positionViewAtIndex(launcher.selected, ListView.Contain); event.accepted = true }
          else if (event.key === Qt.Key_Up) { launcher.selected = Math.max(0, launcher.selected - 1); list.positionViewAtIndex(launcher.selected, ListView.Contain); event.accepted = true }
          else if (event.key === Qt.Key_Tab || event.key === Qt.Key_Backtab) { launcher.tab = launcher.tab === "apps" ? "clip" : "apps"; event.accepted = true }
          else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) { launcher.activate(launcher.selected); event.accepted = true }
        }
      }
      Row {
        id: tabs
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        spacing: 4
        Repeater {
          model: [{ id: "apps", label: "Apps" }, { id: "clip", label: "Clipboard" }]
          delegate: Rectangle {
            required property var modelData
            readonly property bool current: launcher.tab === modelData.id
            height: 24
            width: tabLabel.implicitWidth + 18
            radius: 12
            color: current ? launcher.host.withAlpha(launcher.host.colorText, 0.12) : "transparent"
            Behavior on color { ColorAnimation { duration: 140 } }
            Text {
              id: tabLabel
              anchors.centerIn: parent
              text: modelData.label
              color: current ? launcher.host.colorText : launcher.host.colorMuted
              font.family: launcher.host.uiFont
              font.pixelSize: 12
              font.weight: Font.DemiBold
            }
            MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: launcher.tab = modelData.id }
          }
        }
      }
    }
    // Rule under the field; during a slow extension search, an accent
    // line slides along in a loop.
    Item {
      id: searchRule
      width: parent.width
      height: 1
      Rectangle { anchors.fill: parent; color: launcher.host.withAlpha(launcher.host.colorText, 0.08) }
      Item {
        id: busyBar
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        height: 2
        clip: true
        opacity: launcher.liveSlow && launcher.currentExt !== "" && launcher.query.trim() !== "" ? 1 : 0
        visible: opacity > 0
        Behavior on opacity { NumberAnimation { duration: 140 } }
        Rectangle {
          id: busySeg
          width: parent.width * 0.28
          height: parent.height
          radius: 1
          color: launcher.host.colorAccent
          NumberAnimation on x {
            running: busyBar.visible
            loops: Animation.Infinite
            from: -busySeg.width
            to: busyBar.width
            duration: 1100
            easing.type: Easing.InOutSine
          }
        }
      }
    }
  }

  Text {
    anchors.top: header.bottom
    anchors.topMargin: 20
    anchors.horizontalCenter: parent.horizontalCenter
    visible: results.count === 0
    text: launcher.tab === "clip" ? "Clipboard is empty" : "No results"
    color: launcher.host.colorMuted
    font.family: launcher.host.uiFont
    font.pixelSize: 13
  }

  ListView {
    WheelScroll {}
    id: list
    anchors.top: header.bottom
    anchors.topMargin: 12
    anchors.left: parent.left
    anchors.right: parent.right
    height: launcher.listHeight
    clip: true
    spacing: 4
    model: results
    currentIndex: launcher.selected
    boundsBehavior: Flickable.StopAtBounds
    highlightMoveDuration: 140
    highlightFollowsCurrentItem: true
    highlight: Rectangle {
      width: list.width
      radius: 14
      color: launcher.host.withAlpha(launcher.host.colorText, 0.09)
      Rectangle {
        anchors.left: parent.left
        anchors.leftMargin: 6
        anchors.verticalCenter: parent.verticalCenter
        width: 3; height: 18; radius: 1.5
        color: launcher.host.colorAccent
      }
    }
    add: Transition {
      ParallelAnimation {
        NumberAnimation { property: "opacity"; from: 0; to: 1; duration: 160; easing.type: Easing.OutCubic }
        NumberAnimation { property: "x"; from: 8; to: 0; duration: 220; easing.type: Easing.OutQuint }
      }
    }
    remove: Transition { NumberAnimation { property: "opacity"; to: 0; duration: 110; easing.type: Easing.OutCubic } }
    // Rows that stay only slide to their new place (and finish an interrupted entry).
    displaced: Transition {
      ParallelAnimation {
        NumberAnimation { property: "y"; duration: 170; easing.type: Easing.OutQuint }
        NumberAnimation { property: "opacity"; to: 1; duration: 120 }
        NumberAnimation { property: "x"; to: 0; duration: 120 }
      }
    }
    // Also finishes an entry the move interrupted (else the row stays invisible).
    move: Transition {
      ParallelAnimation {
        NumberAnimation { property: "y"; duration: 170; easing.type: Easing.OutQuint }
        NumberAnimation { property: "opacity"; to: 1; duration: 120 }
        NumberAnimation { property: "x"; to: 0; duration: 120 }
      }
    }

    // Hover selection from one static area over the list: it only sees real
    // pointer moves, never rows sliding under a still pointer while typing.
    MouseArea {
      anchors.fill: parent
      z: 10
      hoverEnabled: true
      acceptedButtons: Qt.NoButton
      onPositionChanged: function(mouse) {
        if (launcher.actionsOpen) return
        var i = list.indexAt(mouse.x + list.contentX, mouse.y + list.contentY)
        if (i >= 0) launcher.selected = i
      }
    }

    // Entering / leaving an extension slides the list sideways.
    transform: Translate { id: slideShift }
    SequentialAnimation {
      id: slide
      ParallelAnimation {
        NumberAnimation { target: slideShift; property: "x"; from: 36 * launcher.slideDirection; to: 0; duration: 260; easing.type: Easing.OutQuint }
        NumberAnimation { target: list; property: "opacity"; from: 0; to: 1; duration: 200; easing.type: Easing.OutCubic }
      }
    }

    delegate: Item {
      id: row
      required property int index
      required property string kind
      required property string title
      required property string subtitle
      required property string icon
      required property string glyph
      required property bool chevron
      required property bool pinned
      required property string ref
      // Pill on the logo: 'switch' for an open window, or the one
      // an extension element asks for ('badge', e.g. preset).
      readonly property string badge: kind === "window" ? "󰁔" : kind === "item" ? (function() {
        try { return String(JSON.parse(ref).item.badge || "") } catch (e) { return "" }
      })() : ""
      // System command waiting for confirmation (second Enter).
      readonly property bool confirming: kind === "system" && launcher.confirmPending !== "" && launcher.confirmPending === ref
      width: list.width
      height: launcher.rowHeight

      MouseArea {
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        onClicked: function(mouse) {
          launcher.selected = row.index
          if (mouse.button === Qt.RightButton) { launcher.actionIndex = 0; launcher.actionsOpen = true }
          else launcher.activate(row.index)
        }
      }

      Item {
        id: tile
        anchors.left: parent.left
        anchors.leftMargin: 16
        anchors.verticalCenter: parent.verticalCenter
        width: 32; height: 32
        Image {
          id: tileImage
          anchors.fill: parent
          source: row.icon
          sourceSize.width: 64
          sourceSize.height: 64
          fillMode: row.kind === "clip" ? Image.PreserveAspectCrop : Image.PreserveAspectFit
          asynchronous: true
          visible: status === Image.Ready
        }
        Rectangle {
          anchors.fill: parent
          radius: 9
          visible: tileImage.status !== Image.Ready
          color: row.kind === "calc" ? launcher.host.withAlpha(launcher.host.colorAccent, 0.18) : launcher.host.withAlpha(launcher.host.colorText, 0.07)
          Glyph {
            anchors.centerIn: parent
            glyph: row.glyph
            color: row.kind === "calc" ? launcher.host.colorAccent : launcher.host.colorText
            fontFamily: launcher.host.fontFamily
            pixelSize: 16
          }
        }
        // Small pill on the logo (open window, preset…).
        Rectangle {
          visible: row.badge !== ""
          anchors.right: parent.right
          anchors.bottom: parent.bottom
          anchors.rightMargin: -4
          anchors.bottomMargin: -4
          width: 16; height: 16; radius: 8
          color: launcher.host.colorAccent
          border.width: 2
          border.color: launcher.host.colorIsland
          Glyph {
            anchors.centerIn: parent
            glyph: row.badge
            color: launcher.host.colorAccentText
            fontFamily: launcher.host.fontFamily
            pixelSize: 9
          }
        }
      }

      Glyph {
        id: rowChevron
        anchors.right: parent.right
        anchors.rightMargin: 16
        anchors.verticalCenter: parent.verticalCenter
        visible: row.chevron
        glyph: "󰅂"
        color: launcher.host.colorMuted
        fontFamily: launcher.host.fontFamily
        pixelSize: 13
      }

      Column {
        anchors.left: tile.right
        anchors.leftMargin: 12
        anchors.right: row.chevron ? rowChevron.left : parent.right
        anchors.rightMargin: 14
        anchors.verticalCenter: parent.verticalCenter
        spacing: 1
        Row {
          width: parent.width
          spacing: 6
          Text {
            id: titleText
            width: Math.min(implicitWidth, parent.width - (row.pinned ? 17 : 0))
            text: row.confirming ? "Press Enter again to " + row.title.toLowerCase() : row.title
            textFormat: Text.PlainText
            elide: Text.ElideRight
            maximumLineCount: 1
            color: row.confirming ? "#e5484d" : launcher.host.colorText
            font.family: launcher.host.uiFont
            font.pixelSize: row.kind === "calc" ? 17 : 14
            font.weight: row.kind === "calc" ? Font.Bold : Font.DemiBold
          }
          // Pinned: a small grey pin beside the title.
          Glyph {
            id: pinMark
            anchors.verticalCenter: titleText.verticalCenter
            visible: row.pinned
            glyph: "󰐃"
            color: launcher.host.colorMuted
            fontFamily: launcher.host.fontFamily
            pixelSize: 11
          }
        }
        Text {
          width: parent.width
          visible: text !== ""
          text: row.confirming ? "Any other key cancels" : row.subtitle
          textFormat: Text.PlainText
          elide: Text.ElideRight
          color: launcher.host.colorMuted
          font.family: launcher.host.uiFont
          font.pixelSize: 12
        }
      }
    }
  }

  // ---------------------------------------------------------------- footer
  Item {
    id: footer
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.bottom: parent.bottom
    // Separator, then the hints centered in their band; the island adds its
    // own bottom padding below, clear of the rounded corners.
    // The band runs down to the island's edge: hints centered in it leave the
    // same space above (to the separator) and below (to the rounded edge).
    height: 40
    Rectangle { anchors.top: parent.top; width: parent.width; height: 1; color: launcher.host.withAlpha(launcher.host.colorText, 0.06) }
    // Same column as the rows: hints end where the rows' text ends (14 px
    // inside the highlight's right edge).
    Row {
      anchors.right: parent.right
      anchors.rightMargin: 14
      anchors.verticalCenter: parent.verticalCenter
      spacing: 14
      component Hint: Row {
        property string keys: ""
        property string label: ""
        spacing: 6
        Rectangle {
          anchors.verticalCenter: parent.verticalCenter
          width: keyText.implicitWidth + 10; height: 16; radius: 4
          color: launcher.host.withAlpha(launcher.host.colorText, 0.08)
          Text { id: keyText; anchors.centerIn: parent; text: keys; color: launcher.host.colorMuted; font.family: launcher.host.uiFont; font.pixelSize: 10; font.weight: Font.DemiBold }
        }
        Text { anchors.verticalCenter: parent.verticalCenter; text: label; color: launcher.host.colorMuted; font.family: launcher.host.uiFont; font.pixelSize: 11 }
      }
      Hint { keys: "↵"; label: launcher.currentActions.length ? "Confirm" : (launcher.actionsFor(results.count ? results.get(Math.min(launcher.selected, results.count - 1)) : null)[0] || { label: "Open" }).label }
      Hint { keys: "Ctrl B"; label: "Actions" }
      Hint { keys: "Tab"; label: launcher.tab === "apps" ? "Clipboard" : "Apps" }
    }
  }

  // ---------------------------------------------------------------- actions panel
  Rectangle {
    id: actionsPanel
    readonly property bool shown: launcher.actionsOpen && launcher.currentActions.length > 0
    anchors.right: parent.right
    anchors.bottom: footer.top
    anchors.bottomMargin: 8
    width: 260
    height: actionsColumn.implicitHeight + 12
    radius: 16
    color: Qt.lighter(launcher.host.colorIsland, 2.4)
    border.width: 1
    border.color: launcher.host.withAlpha(launcher.host.colorText, 0.1)
    transformOrigin: Item.BottomRight
    opacity: shown ? 1 : 0
    scale: shown ? 1 : 0.9
    visible: opacity > 0.01
    Behavior on opacity { NumberAnimation { duration: 160; easing.type: Easing.OutCubic } }
    Behavior on scale { NumberAnimation { duration: 240; easing.type: Easing.OutBack; easing.overshoot: 1.3 } }
    Column {
      id: actionsColumn
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.top: parent.top
      anchors.margins: 6
      spacing: 2
      Repeater {
        model: actionsPanel.shown ? launcher.currentActions : []
        delegate: Rectangle {
          required property var modelData
          required property int index
          width: actionsColumn.width
          height: 34
          radius: 10
          color: launcher.actionIndex === index ? launcher.host.withAlpha(launcher.host.colorText, 0.1) : "transparent"
          Behavior on color { ColorAnimation { duration: 100 } }
          Row {
            anchors.left: parent.left
            anchors.leftMargin: 10
            anchors.verticalCenter: parent.verticalCenter
            spacing: 10
            Glyph { anchors.verticalCenter: parent.verticalCenter; glyph: modelData.glyph; color: launcher.actionIndex === index ? launcher.host.colorAccent : launcher.host.colorMuted; fontFamily: launcher.host.fontFamily; pixelSize: 14 }
            Text { anchors.verticalCenter: parent.verticalCenter; text: modelData.label; color: launcher.host.colorText; font.family: launcher.host.uiFont; font.pixelSize: 13 }
          }
          MouseArea {
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onEntered: launcher.actionIndex = index
            onClicked: launcher.runAction(launcher.selected, modelData.id)
          }
        }
      }
    }
  }
}
