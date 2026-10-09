import QtQuick
import Quickshell
import Quickshell.Io
import "file:///usr/share/flea/shelf/Model.js" as Model
import "file:///usr/share/flea/shelf/Run.js" as Run

// Fichiers (SUPER+D) : sans saisie, le Shelf de Flea (la pile, les épinglés
// et les dernières captures) ; en tapant, une recherche de dossiers et de
// fichiers dans le home (fd). Au style du lanceur. La logique du Shelf
// (pile, tailles, vignettes, actions `flea shelf …`) est celle de Flea, tenue
// par l'île (host.shelf / host.shelfActions) ; cette vue ne fait qu'afficher.
//
// Clavier : taper = chercher, ↑↓ bouge, ↵ ouvre, Ctrl+B (ou clic droit)
// panneau d'actions ; Alt+lettre lance l'action (m déplacer, c copier, a zip, p chemin,
// l LocalSend, + ajouter au Shelf, x retirer…), Ctrl+C contenu au
// presse-papiers, Ctrl+Maj+A tout cocher, Maj+↑↓ étend, Tab entre dans un dossier ; sans saisie : Espace
// coche, Suppr retire du Shelf. Échap vide la recherche, puis ferme.
// Souris : clic = curseur, clic sur la vignette / Ctrl+clic = coche,
// Maj+clic = plage, double-clic = ouvrir.
Item {
  id: view
  required property var host
  property bool active: false
  property real maxHeight: 600
  signal closeRequested()

  readonly property var shelf: host.shelf
  readonly property var doing: host.shelfActions
  // Une seule liste « Shelf » : pile et épinglés confondus (un élément reste
  // jusqu'à ce qu'on le retire), puis les dernières captures.
  readonly property var shelfRows: {
    if (!shelf) return []
    var list = Model.rows(shelf.pile, shelf.captures, shelf.sizes)
    var out = []
    for (var i = 0; i < list.length; i++) {
      var r = list[i]
      if (r.section === Model.PINNED) {
        var copy = {}
        for (var key in r) copy[key] = r[key]
        copy.section = Model.PILE
        r = copy
      }
      out.push(r)
    }
    return out
  }
  readonly property bool searching: query.trim() !== "" || scope !== ""
  // Dossier dans lequel on est « entré » (Tab / → sur un dossier) : la
  // recherche se limite à lui, et sans saisie on voit son contenu.
  property string scope: ""
  function enterFolder(path) {
    if (!path) return
    scope = path
    chosen = ({})
    if (field.text !== "") field.text = ""
    else runSearch()
  }
  function leaveFolder() {
    if (scope === "") return false
    var parent = scope.substring(0, scope.lastIndexOf("/"))
    scope = (scope === homeDir || parent === "" || parent.length < homeDir.length) ? "" : parent
    chosen = ({})
    runSearch()
    return true
  }
  readonly property var rows: searching ? results : shelfRows

  // ---------------------------------------------------------------- search (fd)
  // Recherche « intelligente » : « a v2 » trouve « ali-v2 » (chaque mot tapé
  // = début d'un mot du nom), « aliv2 » aussi (lettres dans l'ordre). Si un
  // mot n'est pas dans le nom, il peut désigner un dossier du chemin
  // (« unity textures »). Dossiers et chemins courts d'abord.
  property string query: ""
  property var results: []
  property bool searchBusy: false
  readonly property string homeDir: String(Quickshell.env("HOME") || "")
  readonly property var excluded: [".git", "node_modules", ".cache", "Library", "Temp", "obj", "__pycache__", ".venv", "venv", "target"]
  onQueryChanged: { selected = 0; chosen = ({}); searchDebounce.restart() }
  Timer { id: searchDebounce; interval: 90; onTriggered: view.runSearch() }
  function regexEscape(c) { return c.replace(/[.*+?^${}()|[\]\\\/]/g, "\\$&") }
  function subsequence(text) { return text.split("").map(regexEscape).join(".*") }
  function queryTokens() {
    return query.trim().toLowerCase().split(/[\s\-_]+/).filter(function(w) { return w !== "" })
  }
  // Dossiers fréquentés (zoxide, le « cd ali » du terminal) : bonus de
  // classement, et candidats même hors du home. Ouvrir un dossier d'ici
  // l'ajoute aussi à zoxide.
  property var zoxide: ({})
  Process {
    id: zoxideProc
    command: ["zoxide", "query", "--list", "--score"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        var map = {}
        var lines = String(text || "").split("\n")
        for (var i = 0; i < lines.length; i++) {
          var m = lines[i].match(/^\s*([0-9.]+)\s+(.+)$/)
          if (m) map[m[2]] = Number(m[1])
        }
        view.zoxide = map
      }
    }
  }
  function zoxideBonus(path, folder) {
    var score = zoxide[path]
    if (score > 0) return 120 + Math.min(260, 70 * Math.log(1 + score))
    var parent = path.substring(0, path.lastIndexOf("/"))
    return zoxide[parent] > 0 ? 25 : 0
  }
  // Mode chemin : une saisie qui commence par « ~ » ou « / » liste le dossier
  // visé (fichiers cachés compris), filtré par le début du dernier élément ;
  // Tab complète le chemin.
  readonly property bool pathMode: /^(~|\/)/.test(query.trim())
  function pathParts() {
    var q = query.trim()
    if (q === "~") q = "~/"
    var full = q.charAt(0) === "~" ? homeDir + q.substring(1) : q
    var cut = full.lastIndexOf("/")
    return { dir: cut <= 0 ? "/" : full.substring(0, cut), prefix: full.substring(cut + 1) }
  }
  function completePath(path, folder) {
    field.text = shortPath(path) + (folder ? "/" : "")
    field.cursorPosition = field.text.length
  }
  function runSearch() {
    if (pathMode) {
      var pp = pathParts()
      searchProc.tokens = ["\u0000path:" + query.trim()]
      searchProc.command = ["sh", "-c", 'fd -H -t d -t f -t l --max-depth 1 --max-results 2000 --exclude .git . "$1"', "poste-path", pp.dir]
      searchBusy = true
      searchProc.running = false
      searchProc.running = true
      return
    }
    var tokens = queryTokens()
    if (!tokens.length && scope === "") { results = []; return }
    var ex = []
    for (var i = 0; i < excluded.length; i++) ex.push("--exclude '" + excluded[i] + "'")
    // fd termine les dossiers par « / ».
    var fd = "fd -i -t d -t f -t l --max-results 8000 " + ex.join(" ")
    var script
    if (!tokens.length) script = fd + ' --max-depth 1 . "$3"'
    else {
      script = fd + ' -- "$1" "$3"'
      if (tokens.length > 1) script += "; " + fd + ' -- "$2" "$3"'
    }
    searchProc.tokens = tokens
    searchProc.command = ["sh", "-c", script, "poste-find",
      tokens.length ? subsequence(tokens.join("")) : "", tokens.length ? subsequence(tokens[tokens.length - 1]) : "",
      scope !== "" ? scope : homeDir]
    searchBusy = true
    searchProc.running = false
    searchProc.running = true
  }
  Process {
    id: searchProc
    property var tokens: []
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: view.rankResults(text, searchProc.tokens)
    }
    onExited: view.searchBusy = false
  }
  // Score d'un nom : -1 s'il ne correspond pas.
  function nameScore(name, tokens) {
    var b = String(name).toLowerCase()
    var sep = /[\s\-_.]+/
    var words = b.split(sep).filter(function(w) { return w !== "" })
    var joined = tokens.join("")
    var bs = b.replace(/[\s\-_.]+/g, "")
    if (bs === joined) return 1200
    // chaque mot tapé = début d'un mot du nom, dans l'ordre
    var wi = 0, inOrder = true
    for (var t = 0; t < tokens.length; t++) {
      while (wi < words.length && words[wi].indexOf(tokens[t]) !== 0) wi++
      if (wi >= words.length) { inOrder = false; break }
      wi++
    }
    if (inOrder) return 1000 - (words.length - tokens.length) * 15 - b.length
    if (bs.indexOf(joined) === 0) return 900 - bs.length
    var idx = bs.indexOf(joined)
    if (idx > 0) return 700 - idx - bs.length
    return fuzzy(bs, joined)
  }
  function rankResults(text, tokens) {
    if (tokens.length === 1 && tokens[0].indexOf("\u0000path:") === 0) {
      if (tokens[0] !== "\u0000path:" + query.trim()) return
      var pp = pathParts()
      var pref = pp.prefix.toLowerCase()
      var plines = String(text || "").split("\n")
      var prow = []
      for (var pi = 0; pi < plines.length; pi++) {
        var pl = plines[pi]
        if (!pl) continue
        var pfolder = pl.charAt(pl.length - 1) === "/"
        var ppath = pfolder ? pl.substring(0, pl.length - 1) : pl
        var pname = ppath.substring(ppath.lastIndexOf("/") + 1)
        if (pref && pname.toLowerCase().indexOf(pref) !== 0) continue
        prow.push({ path: ppath, name: pname, folder: pfolder, bytes: -1, partial: false, pinned: false,
                    recording: /\.(mp4|mkv|webm|mov)$/i.test(pname), section: "result" })
      }
      prow.sort(function(a, b) {
        if (a.folder !== b.folder) return a.folder ? -1 : 1
        var ah = a.name.charAt(0) === ".", bh = b.name.charAt(0) === "."
        if (ah !== bh) return ah ? 1 : -1
        return a.name.toLowerCase() < b.name.toLowerCase() ? -1 : 1
      })
      // Saisie qui finit par « / » : le dossier lui-même en tête (Entrée l'ouvre).
      if (pref === "" && pp.dir !== "") {
        var dname = pp.dir === "/" ? "/" : pp.dir.substring(pp.dir.lastIndexOf("/") + 1)
        prow.unshift({ path: pp.dir, name: dname + "/", folder: true, bytes: -1, partial: false, pinned: false,
                       recording: false, section: "result", self: true })
      }
      results = prow.slice(0, 300)
      selected = 0
      flick.contentY = 0
      return
    }
    if (tokens.join(" ") !== queryTokens().join(" ")) return
    if (!tokens.length) { listFolder(text); return }
    var last = tokens[tokens.length - 1]
    var lines = String(text || "").split("\n")
    for (var zp in zoxide) if (scope === "" || zp.indexOf(scope + "/") === 0) lines.push(zp + "/")
    var seen = {}
    var scored = []
    for (var i = 0; i < lines.length; i++) {
      var line = lines[i]
      if (!line || seen[line]) continue
      seen[line] = true
      var folder = line.charAt(line.length - 1) === "/"
      var path = folder ? line.substring(0, line.length - 1) : line
      var name = path.substring(path.lastIndexOf("/") + 1)
      var sc = nameScore(name, tokens)
      if (sc < 0 && tokens.length > 1) {
        // les autres mots dans le chemin (début d'un dossier)
        var rel = path.substring(homeDir.length).toLowerCase()
        var dirs = rel.split("/")
        var ok = true
        for (var w = 0; w < tokens.length - 1 && ok; w++) {
          ok = false
          for (var d = 0; d < dirs.length - 1; d++) {
            var parts = dirs[d].split(/[\s\-_.]+/)
            for (var q = 0; q < parts.length; q++) if (parts[q].indexOf(tokens[w]) === 0) { ok = true; break }
            if (ok) break
          }
        }
        var own = ok ? nameScore(name, [last]) : -1
        if (own >= 0) sc = 200 + own / 5
      }
      if (sc < 0) continue
      var depth = path.substring(homeDir.length).split("/").length - 1
      sc += (folder ? 60 : 0) - depth * 6 + zoxideBonus(path, folder)
      scored.push({ sc: sc, row: { path: path, name: name, folder: folder, bytes: -1, partial: false,
                                   pinned: false, recording: /\.(mp4|mkv|webm|mov)$/i.test(name), section: "result" } })
    }
    scored.sort(function(a, b) { return b.sc - a.sc || a.row.path.length - b.row.path.length })
    var out = []
    for (var j = 0; j < scored.length && j < 60; j++) out.push(scored[j].row)
    results = out
    selected = 0
    flick.contentY = 0
  }
  // Contenu du dossier courant : dossiers puis fichiers, par nom.
  function listFolder(text) {
    var lines = String(text || "").split("\n")
    var rowsOut = []
    for (var i = 0; i < lines.length; i++) {
      var line = lines[i]
      if (!line) continue
      var folder = line.charAt(line.length - 1) === "/"
      var path = folder ? line.substring(0, line.length - 1) : line
      var name = path.substring(path.lastIndexOf("/") + 1)
      rowsOut.push({ path: path, name: name, folder: folder, bytes: -1, partial: false, pinned: false,
                     recording: /\.(mp4|mkv|webm|mov)$/i.test(name), section: "result" })
    }
    rowsOut.sort(function(a, b) {
      if (a.folder !== b.folder) return a.folder ? -1 : 1
      return a.name.toLowerCase() < b.name.toLowerCase() ? -1 : 1
    })
    results = rowsOut.slice(0, 300)
    selected = 0
    flick.contentY = 0
  }
  // Même classement que le lanceur : préfixe, début de mot, sous-séquence.
  function fuzzy(text, q) {
    text = String(text || "").toLowerCase()
    if (!q) return 0
    if (text.indexOf(q) === 0) return 1000 - text.length
    var idx = text.indexOf(q)
    if (idx > 0) return 700 - idx + (/[\s\-_.]/.test(text.charAt(idx - 1)) ? 150 : 0)
    var score = 0, ti = 0, run = 0
    for (var qi = 0; qi < q.length; qi++) {
      var found = text.indexOf(q.charAt(qi), ti)
      if (found === -1) return -1
      run = found === ti ? run + 1 : 0
      score += 10 + run * 8 + ((found === 0 || /[\s\-_.]/.test(text.charAt(found - 1))) ? 20 : 0)
      ti = found + 1
    }
    return score
  }
  readonly property var thumbs: shelf ? shelf.thumbs : ({})
  readonly property var run: doing ? doing.run : Run.idle()

  property int selected: 0
  property var chosen: ({})
  property int anchorIndex: -1
  readonly property int chosenCount: Model.chosenCount(chosen, rows)

  // Destination en cours de choix : "move", "copy" ou "send".
  property string pending: ""
  property var flyoutRows: []
  property int flyoutIndex: 0

  property string notice: ""
  property bool noticeError: false
  function say(text, error) {
    notice = text || ""
    noticeError = !!error
    noticeTimer.restart()
  }
  Timer { id: noticeTimer; interval: 4000; onTriggered: view.notice = "" }

  readonly property real rowHeight: 48
  readonly property real captionHeight: 26
  readonly property real headerHeight: 30
  readonly property real footerHeight: 40
  readonly property real chrome: headerHeight + 8 + footerHeight + 2
  readonly property real contentHeight: pending !== ""
    ? (flyoutRows.length + 1) * (rowHeight + 4) + (pending === "localsend" ? 20 : 0)
    : rows.length === 0 ? 132 : layout.total
  readonly property real listHeight: Math.min(Math.max(rowHeight, maxHeight - chrome), contentHeight)
  implicitHeight: Math.max(headerHeight + 8 + listHeight,
    actionsPanel.shown ? headerHeight + 8 + actionsPanel.height + 8 : 0) + footerHeight + 2

  // ---------------------------------------------------------------- actions panel (Ctrl+B)
  // Comme le lanceur : la liste des actions de la ligne, avec leur touche.
  property bool actionsOpen: false
  property int actionIndex: 0
  readonly property var currentActions: {
    var row = rows[selected]
    if (!row) return []
    var many = chosenCount > 0
    if (row.section === "result") {
      var res = [{ id: "open", label: row.folder && !many ? "Open in Flea" : "Open", key: "↵", glyph: "󰏌" }]
      if (row.folder && !many) res.push({ id: "enter", label: "Browse inside", key: "Tab", glyph: "󰉖" })
      res = res.concat([
                 { id: "reveal", label: "Show in folder", key: "Alt F", glyph: "󰉋" },
                 { id: "terminal", label: "Open terminal here", key: "Alt T", glyph: "󰆍" },
                 { id: "clipboard", label: "Copy to clipboard", key: "Ctrl C", glyph: "󰅍" },
                 { id: "paths", label: many ? "Copy paths" : "Copy path", key: "Alt P", glyph: "󰴠" },
                 { id: "add", label: "Add to shelf", key: "Alt +", glyph: "󰐕" },
                 { id: "move", label: "Move to…", key: "Alt M", glyph: "󰪹" },
                 { id: "copy", label: "Copy to…", key: "Alt C", glyph: "󰆏" },
                 { id: "zip", label: "Zip", key: "Alt A", glyph: "󰗄" },
                 { id: "localsend", label: "Send with LocalSend", key: "Alt L", glyph: "󰁔" }])
      return res
    }
    var out = [{ id: "open", label: "Open", key: "↵", glyph: "󰏌" },
               { id: "clipboard", label: "Copy to clipboard", key: "Ctrl C", glyph: "󰅍" },
               { id: "move", label: "Move to…", key: "Alt M", glyph: "󰪹" },
               { id: "copy", label: "Copy to…", key: "Alt C", glyph: "󰆏" },
               { id: "zip", label: "Zip", key: "Alt A", glyph: "󰗄" },
               { id: "paths", label: many ? "Copy paths" : "Copy path", key: "Alt P", glyph: "󰴠" }]
    if (row.section === Model.CAPTURE && !many) out.push({ id: "add", label: "Add to shelf", key: "Alt +", glyph: "󰐕" })
    out.push({ id: "localsend", label: "Send with LocalSend", key: "Alt L", glyph: "󰁔" })
    if (doing && doing.peerable) out.push({ id: "send", label: "Send with Taildrop", key: "Alt T", glyph: "󰒊" })
    if (row.section !== Model.CAPTURE || many) out.push({ id: "remove", label: "Remove from shelf", key: "Alt X", glyph: "󰅖" })
    return out
  }
  function runAction(id) {
    actionsOpen = false
    if (id === "open") openRow(selected)
    else if (id === "clipboard") copyContent()
    else if (id === "move" || id === "copy" || id === "zip" || id === "paths" || id === "send") act(id)
    else if (id === "pin") pin()
    else if (id === "add") { var added = actionPaths(); host.addToShelf(added); if (added.length) say(added.length === 1 ? "Added to the shelf" : "Added " + added.length + " items to the shelf", false) }
    else if (id === "enter") { if (rows[selected]) enterFolder(rows[selected].path) }
    else if (id === "localsend") startLocalSend()
    else if (id === "reveal") { if (rows[selected]) { Quickshell.execDetached(["flea", "--select", rows[selected].path]); closeRequested() } }
    else if (id === "terminal") {
      var r = rows[selected]
      if (r) { Quickshell.execDetached(["uwsm-app", "--", "xdg-terminal-exec", "--dir=" + (r.folder ? r.path : parentOf(r.path).replace(/^~/, homeDir))]); closeRequested() }
    }
    else if (id === "remove") forgetRow(selected)
    else if (id === "clear") { if (shelf) shelf.clear() }
    else if (id === "undo") { if (shelf) shelf.undo() }
  }


  onActiveChanged: {
    if (active) {
      selected = 0
      chosen = ({})
      anchorIndex = -1
      pending = ""
      notice = ""
      if (doing) doing.askPeers()
      actionsOpen = false
      scope = ""
      field.text = ""
      results = []
      zoxideProc.running = true
      Qt.callLater(function() { field.forceActiveFocus() })
    } else {
      pending = ""
      actionsOpen = false
    }
  }
  onRowsChanged: {
    if (selected >= rows.length) selected = Math.max(0, rows.length - 1)
  }

  // ---------------------------------------------------------------- layout
  // Position de chaque ligne (légendes de section comprises).
  readonly property var layout: {
    var ys = [], captions = [], y = 0, last = ""
    for (var i = 0; i < rows.length; i++) {
      var section = rows[i].section
      if (section !== last) {
        if (!(i === 0 && (section === Model.PILE || section === "result"))) {
          captions.push({ y: y, label: section === Model.PINNED ? "Pinned"
            : section === Model.CAPTURE ? Model.capturesCaption(shelf ? shelf.shelfSettings : null) : "Shelf" })
          y += captionHeight
        }
        last = section
      }
      ys.push(y)
      y += rowHeight + 4
    }
    return { ys: ys, captions: captions, total: Math.max(0, y - 4) }
  }
  function ensureVisible(i) {
    if (i < 0 || i >= layout.ys.length) return
    var top = layout.ys[i], bottom = top + rowHeight
    if (i === 0 || (rows[i].section !== rows[Math.max(0, i - 1)].section)) top -= captionHeight
    if (top < flick.contentY) flick.contentY = Math.max(0, top)
    else if (bottom > flick.contentY + flick.height) flick.contentY = bottom - flick.height
  }
  function moveTo(i) {
    selected = Math.max(0, Math.min(rows.length - 1, i))
    ensureVisible(selected)
  }

  // ---------------------------------------------------------------- LocalSend
  // Via le moteur de Flea (`flea --backend`, qui pilote localsend-cli) :
  // découverte des appareils, puis envoi. localsend-cli n'envoie que des
  // fichiers : un dossier est zippé d'abord (dossier temporaire, supprimé
  // après l'envoi). L'appli LocalSend ouverte garde le port : on le dit.
  property var lsPaths: []
  property string lsReason: ""
  property bool lsLooking: false
  property int lsId: 0
  property var lsQueue: []
  property string lsTmp: ""
  property string lsPeer: ""
  property string lsLabel: ""
  function startLocalSend() {
    var paths = actionPaths()
    if (!paths.length) { say("Nothing selected.", true); return }
    lsPaths = paths
    lsReason = ""
    flyoutRows = []
    flyoutIndex = 0
    pending = "localsend"
    lsAskPeers()
  }
  function lsWrite(obj) {
    lsQueue = lsQueue.concat([JSON.stringify(obj)])
    if (fleaBackend.running) lsFlush()
    else fleaBackend.running = true
  }
  function lsFlush() {
    var q = lsQueue
    lsQueue = []
    for (var i = 0; i < q.length; i++) fleaBackend.write(q[i] + "\n")
  }
  function lsAskPeers() {
    if (lsLooking) return
    lsLooking = true
    lsWrite({ c: "localsend", op: "peers", id: ++lsId })
  }
  // Tant que le choix est ouvert, on relance la découverte (Flea ne cherche
  // que 4 s d'affilée).
  Timer {
    interval: 800
    repeat: true
    running: view.pending === "localsend" && !view.lsLooking
    onTriggered: view.lsAskPeers()
  }
  function lsSend(peer) {
    lsPeer = peer
    var paths = lsPaths
    lsLabel = paths.length === 1 ? paths[0].substring(paths[0].lastIndexOf("/") + 1) : paths.length + " items"
    pending = ""
    chosen = ({})
    say("Preparing " + lsLabel + "…", false)
    zipProc.command = ["sh", "-c",
      'tmp=$(mktemp -d "${XDG_RUNTIME_DIR:-/tmp}/poste-localsend.XXXXXX") || exit 1; echo "TMP:$tmp"; '
      + 'for p in "$@"; do if [ -d "$p" ]; then n=$(basename "$p"); '
      + '(cd "$(dirname "$p")" && zip -qr "$tmp/$n.zip" "$n") && echo "FILE:$tmp/$n.zip"; '
      + 'elif [ -f "$p" ]; then echo "FILE:$p"; fi; done', "poste-localsend"].concat(paths)
    zipProc.running = true
  }
  Process {
    id: zipProc
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        var files = []
        var lines = String(text || "").split("\n")
        for (var i = 0; i < lines.length; i++) {
          if (lines[i].indexOf("TMP:") === 0) view.lsTmp = lines[i].substring(4)
          else if (lines[i].indexOf("FILE:") === 0) files.push(lines[i].substring(5))
        }
        if (!files.length) { view.say("Nothing could be sent.", true); view.lsCleanup(); return }
        view.say("Sending " + view.lsLabel + " to " + view.lsPeer + " — accept it on the device…", false)
        view.lsWrite({ c: "localsend", op: "send", peer: view.lsPeer, paths: files, id: ++view.lsId })
      }
    }
  }
  function lsCleanup() {
    if (lsTmp !== "") Quickshell.execDetached(["rm", "-rf", lsTmp])
    lsTmp = ""
  }
  function backendLine(line) {
    var msg
    try { msg = JSON.parse(line) } catch (e) { return }
    if (msg.t === "localsendpeers") {
      lsLooking = false
      var names = (msg.peers || []).map(function(p) { return String(p.name) })
      // Une recherche vide ne fait pas disparaître un appareil déjà vu.
      if (pending === "localsend" && names.length) {
        var merged = flyoutRows.slice()
        for (var i = 0; i < names.length; i++) if (merged.indexOf(names[i]) < 0) merged.push(names[i])
        flyoutRows = merged
      }
      lsReason = names.length ? "" : String(msg.reason || "")
    } else if (msg.t === "localsendsent") {
      lsCleanup()
      var text = msg.ok ? "Sent " + lsLabel + " to " + lsPeer : "LocalSend: " + String(msg.reason || "sending failed")
      say(text, !msg.ok)
      Quickshell.execDetached(["notify-send", "-a", "LocalSend", "LocalSend", text])
    }
  }
  Process {
    id: fleaBackend
    command: ["flea", "--backend"]
    stdinEnabled: true
    onStarted: view.lsFlush()
    onExited: view.lsLooking = false
    stdout: SplitParser { onRead: function(data) { view.backendLine(data) } }
  }

  // ---------------------------------------------------------------- actions
  // Les lignes cochées, sinon celle du curseur (comme le lanceur) ; Ctrl+A
  // coche toute la liste.
  function actionPaths() {
    if (chosenCount > 0) return Model.actionPaths(chosen, rows)
    return rows[selected] ? [rows[selected].path] : []
  }
  function toggleChosen(i) {
    if (!rows[i]) return
    chosen = Model.toggleChosen(chosen, rows[i].path)
    anchorIndex = i
  }
  function chooseRange(to) {
    var from = anchorIndex >= 0 ? anchorIndex : selected
    chosen = Model.chooseRange(chosen, rows, from, to)
  }
  // Ctrl+C : le contenu dans le presse-papiers. Une image ou un texte seul
  // part tel quel (collable dans un chat, un éditeur…) ; sinon les fichiers
  // partent comme fichiers (text/uri-list, collables dans Flea/Nautilus).
  function copyContent() {
    var paths = actionPaths()
    if (paths.length === 0) { say("Nothing selected.", true); return }
    var script = 'if [ "$#" -eq 1 ] && [ -f "$1" ]; then '
      + 'm=$(file -b --mime-type -- "$1"); '
      + 'case "$m" in image/*) exec wl-copy --type "$m" < "$1";; text/*|application/json|application/x-sh*) exec wl-copy --type text/plain < "$1";; esac; fi; '
      + 'for p in "$@"; do printf "file://%s\r\n" "$p"; done | wl-copy --type text/uri-list'
    Quickshell.execDetached(["sh", "-c", script, "shelf-copy"].concat(paths))
    say(paths.length === 1 ? "Copied to clipboard: " + shortPath(paths[0]).split("/").pop()
      : "Copied " + paths.length + " files to clipboard", false)
  }
  function openRow(i) {
    if (!rows[i] || !shelf) return
    if (rows[i].folder) Quickshell.execDetached(["zoxide", "add", rows[i].path])
    shelf.open(rows[i].path)
    closeRequested()
  }
  function forgetRow(i) {
    var row = rows[i]
    if (!row || row.section === Model.CAPTURE || row.section === "result" || !shelf) return
    // Flea garde les éléments épinglés : on désépingle avant d'oublier.
    if (row.pinned) shelf.pinAll([row.path], false)
    shelf.forget(row.path)
  }
  function pin() {
    if (!shelf) return
    if (chosenCount === 0) {
      var row = rows[selected]
      if (row) shelf.pin(row.path, !row.pinned)
      return
    }
    var paths = actionPaths()
    shelf.pinAll(paths, !Model.allPinned(paths, rows))
  }
  function act(id) {
    if (!doing) return
    var paths = actionPaths()
    if (paths.length === 0) { say("Nothing selected.", true); return }
    if (id === "paths") {
      Quickshell.clipboardText = doing.yank(paths)
      say(paths.length === 1 ? "Copied path: " + shortPath(paths[0]) : "Copied " + paths.length + " paths", false)
      return
    }
    if (id === "zip") { doing.zip(Model.today(), paths); return }
    if (id === "send") {
      if (!doing.peerable) { say("Taildrop is not available here.", true); return }
      doing.askPeers()
      pending = "send"
      flyoutRows = doing.peers
    } else {
      doing.askPlaces()
      pending = id
      flyoutRows = doing.places
    }
    flyoutIndex = 0
  }
  function runChosen(dest) {
    var paths = actionPaths()
    if (pending === "send") doing.send(dest, paths)
    else if (pending !== "") doing.transfer(pending === "move", dest, paths)
    pending = ""
    chosen = ({})
  }
  function pickFlyout(i) {
    if (pending === "localsend") { if (flyoutRows[i]) lsSend(flyoutRows[i]); return }
    if (i === flyoutRows.length && (pending === "move" || pending === "copy")) {
      doing.choose(Run.flyoutTitle(pending, actionPaths().length), flyoutRows.length ? flyoutRows[0] : "")
      return
    }
    if (flyoutRows[i]) runChosen(flyoutRows[i])
  }
  function shortPath(p) {
    var home = String(Quickshell.env("HOME") || "")
    var s = String(p || "")
    return home && s.indexOf(home) === 0 ? "~" + s.substring(home.length) : s
  }
  function parentOf(p) {
    var s = String(p || "").replace(/\/+$/, "")
    var cut = s.lastIndexOf("/")
    return cut > 0 ? shortPath(s.substring(0, cut)) : "/"
  }

  Connections {
    target: view.doing
    ignoreUnknownSignals: true
    function onPlacesChanged() { if ((view.pending === "move" || view.pending === "copy") && view.flyoutRows.length === 0) view.flyoutRows = view.doing.places }
    function onPeersChanged() { if (view.pending === "send" && view.flyoutRows.length === 0) view.flyoutRows = view.doing.peers }
    function onLanded(sentence) { view.say(sentence, false) }
    function onRefused(why) { view.say(why, true) }
    function onBrowsed(dest) { view.runChosen(dest) }
  }
  Connections {
    target: view.shelf
    ignoreUnknownSignals: true
    function onFailed(why) { view.say(why, true) }
    function onCleared(count) { view.say(Model.clearedText(count), false) }
    function onUndone(kind, count) { view.say(Model.undoneText(kind, count), false) }
  }

  // Pas de glisser-déposer (il bloquait le pointeur) : tout se fait au clavier
  // ou aux boutons de ligne.

  // ---------------------------------------------------------------- keys
  function handleKey(event) {
      var shift = (event.modifiers & Qt.ShiftModifier) !== 0
      var ctrl = (event.modifiers & Qt.ControlModifier) !== 0
      var k = event.key
      event.accepted = true
      if (view.actionsOpen) {
        var an = view.currentActions.length
        if (k === Qt.Key_Escape || (ctrl && k === Qt.Key_B)) view.actionsOpen = false
        else if (k === Qt.Key_Down || k === Qt.Key_J || k === Qt.Key_Tab) view.actionIndex = Math.min(an - 1, view.actionIndex + 1)
        else if (k === Qt.Key_Up || k === Qt.Key_K || k === Qt.Key_Backtab) view.actionIndex = Math.max(0, view.actionIndex - 1)
        else if (k === Qt.Key_Return || k === Qt.Key_Enter) { if (view.currentActions[view.actionIndex]) view.runAction(view.currentActions[view.actionIndex].id) }
        else {
          // La lettre d'une action la lance directement.
          var typed = "Alt " + (shift ? "Shift " : "") + String.fromCharCode(k).toUpperCase()
          var hit = null
          for (var ai = 0; ai < an; ai++) if (view.currentActions[ai].key === typed) hit = view.currentActions[ai]
          if (hit) { view.runAction(hit.id); return }
          view.actionsOpen = false
        }
        // Une autre touche ferme le panneau et garde son effet habituel.
        if (view.actionsOpen || k === Qt.Key_Escape || k === Qt.Key_Return || k === Qt.Key_Enter
            || (ctrl && k === Qt.Key_B) || k === Qt.Key_Tab || k === Qt.Key_Backtab) return
      }
      if (k === Qt.Key_Escape) {
        if (view.run.running) view.doing.cancelRun()
        else if (view.pending !== "") view.pending = ""
        else if (view.query !== "") field.text = ""
        else if (view.scope !== "") view.scope = "", view.runSearch()
        else view.closeRequested()
        return
      }
      if (view.pending !== "") {
        var n = view.flyoutRows.length + 1
        if (k === Qt.Key_Down || k === Qt.Key_J) view.flyoutIndex = Math.min(n - 1, view.flyoutIndex + 1)
        else if (k === Qt.Key_Up || k === Qt.Key_K) view.flyoutIndex = Math.max(0, view.flyoutIndex - 1)
        else if (k === Qt.Key_Return || k === Qt.Key_Enter) view.pickFlyout(view.flyoutIndex)
        else if (k === Qt.Key_B && (view.pending === "move" || view.pending === "copy")) view.pickFlyout(view.flyoutRows.length)
        else if (k >= Qt.Key_1 && k <= Qt.Key_9 && k - Qt.Key_1 < view.flyoutRows.length) view.pickFlyout(k - Qt.Key_1)
        else event.accepted = false
        return
      }
      // Alt+lettre : l'action directement, même en tapant (la touche est
      // affichée dans le panneau Ctrl+B).
      if ((event.modifiers & Qt.AltModifier) && !ctrl) {
        var letter = k === Qt.Key_Plus || k === Qt.Key_Equal ? "+" : String.fromCharCode(k).toUpperCase()
        // « + » demande Maj sur certains claviers : Maj n'y compte pas.
        var want = "Alt " + (shift && letter !== "+" ? "Shift " : "") + letter
        var acts = view.currentActions
        for (var ci = 0; ci < acts.length; ci++) if (acts[ci].key === want) { view.runAction(acts[ci].id); return }
        event.accepted = false
        return
      }
      // Ctrl+A reste au champ (sélectionner le texte) ; Ctrl+Maj+A coche tout.
      if (ctrl && shift && k === Qt.Key_A) { view.chosen = Model.chooseAll(view.chosen, view.rows); return }
      if (ctrl && k === Qt.Key_C) { view.copyContent(); return }
      if (ctrl && k === Qt.Key_B) { if (view.currentActions.length) { view.actionIndex = 0; view.actionsOpen = true }; return }
      if (ctrl) { event.accepted = false; return }
      if (k === Qt.Key_Down || k === Qt.Key_Up) {
        var step = k === Qt.Key_Down ? 1 : -1
        view.moveTo(view.selected + step)
        if (shift) view.chooseRange(view.selected)
      }
      else if (k === Qt.Key_Return || k === Qt.Key_Enter) view.openRow(view.selected)
      else if ((k === Qt.Key_Tab || (k === Qt.Key_Right && field.cursorPosition === field.text.length))
               && view.rows[view.selected] && (view.rows[view.selected].folder || view.pathMode)) {
        if (view.pathMode) { if (!view.rows[view.selected].self) view.completePath(view.rows[view.selected].path, view.rows[view.selected].folder) }
        else view.enterFolder(view.rows[view.selected].path)
      }
      else if ((k === Qt.Key_Backspace || (k === Qt.Key_Left && field.cursorPosition === 0)) && field.text === "" && view.scope !== "") view.leaveFolder()
      else if (k === Qt.Key_Tab) {}
      else if (k === Qt.Key_Space && view.query === "" && view.scope === "") view.toggleChosen(view.selected)
      else if (k === Qt.Key_Delete && view.query === "") view.forgetRow(view.selected)
      else event.accepted = false
  }

  // ---------------------------------------------------------------- header
  Item {
    id: header
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.top: parent.top
    height: view.headerHeight
    Glyph {
      id: searchGlyph
      anchors.left: parent.left
      anchors.leftMargin: 4
      anchors.verticalCenter: parent.verticalCenter
      glyph: view.pending !== "" ? "󰁍" : "󰍉"
      color: view.host.colorMuted
      fontFamily: view.host.fontFamily
      pixelSize: 16
    }
    Text {
      anchors.left: searchGlyph.right
      anchors.leftMargin: 10
      anchors.verticalCenter: parent.verticalCenter
      visible: view.pending !== ""
      text: view.pending === "localsend"
        ? "Send " + (view.lsPaths.length === 1 ? view.lsPaths[0].substring(view.lsPaths[0].lastIndexOf("/") + 1) : view.lsPaths.length + " items") + " with LocalSend"
        : Run.flyoutTitle(view.pending, view.actionPaths().length)
      width: parent.width - 60
      elide: Text.ElideMiddle
      color: view.host.colorText
      font.family: view.host.uiFont
      font.pixelSize: 15
      font.weight: Font.DemiBold
    }
    Text {
      id: crumb
      anchors.left: searchGlyph.right
      anchors.leftMargin: visible ? 10 : 0
      anchors.verticalCenter: parent.verticalCenter
      visible: view.pending === "" && view.scope !== ""
      width: visible ? Math.min(implicitWidth, 260) : 0
      elide: Text.ElideLeft
      text: view.scope !== "" ? view.shortPath(view.scope) + "  ›" : ""
      color: view.host.colorText
      font.family: view.host.uiFont
      font.pixelSize: 15
      font.weight: Font.DemiBold
      MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: view.leaveFolder() }
    }
    TextInput {
      id: field
      anchors.left: crumb.right
      anchors.leftMargin: 10
      anchors.right: countText.left
      anchors.rightMargin: 10
      anchors.verticalCenter: parent.verticalCenter
      visible: view.pending === ""
      focus: view.active
      color: view.host.colorText
      selectionColor: view.host.withAlpha(view.host.colorAccent, 0.4)
      font.family: view.host.uiFont
      font.pixelSize: 17
      clip: true
      onTextChanged: view.query = text
      Keys.onPressed: function(event) { view.handleKey(event) }
      Text {
        anchors.fill: parent
        visible: !field.text
        text: view.scope !== "" ? "Search in this folder…" : "Search files and folders…"
        color: view.host.colorMuted
        font: field.font
      }
    }
    Text {
      id: countText
      anchors.right: parent.right
      anchors.rightMargin: 8
      anchors.verticalCenter: parent.verticalCenter
      visible: view.pending === ""
      text: view.chosenCount > 0 ? view.chosenCount + " selected"
        : view.searching ? (view.searchBusy && view.results.length === 0 ? "Searching…"
          : view.query.trim() === "" ? view.results.length + " items" : view.results.length + (view.results.length >= 60 ? "+" : "") + " found")
        : (view.shelf ? view.shelf.pile.items.length : 0) + " on the shelf"
      color: view.chosenCount > 0 ? view.host.colorAccent : view.host.colorMuted
      font.family: view.host.uiFont
      font.pixelSize: 12
    }
  }

  // ---------------------------------------------------------------- list
  Flickable {
    id: flick
    anchors.top: header.bottom
    anchors.topMargin: 8
    anchors.left: parent.left
    anchors.right: parent.right
    height: view.listHeight
    visible: view.pending === "" && view.rows.length > 0
    clip: true
    contentHeight: view.layout.total
    boundsBehavior: Flickable.StopAtBounds

    // Surlignage du curseur, comme le lanceur.
    Rectangle {
      width: flick.width
      height: view.rowHeight
      y: view.layout.ys[view.selected] !== undefined ? view.layout.ys[view.selected] : 0
      Behavior on y { NumberAnimation { duration: 140; easing.type: Easing.OutCubic } }
      radius: 14
      color: view.host.withAlpha(view.host.colorText, 0.09)
      visible: view.rows.length > 0
      Rectangle {
        anchors.left: parent.left
        anchors.leftMargin: 6
        anchors.verticalCenter: parent.verticalCenter
        width: 3; height: 18; radius: 1.5
        color: view.host.colorAccent
      }
    }

    Repeater {
      model: view.layout.captions
      delegate: Text {
        required property var modelData
        x: 16
        y: modelData.y + 8
        text: String(modelData.label).toUpperCase()
        color: view.host.colorMuted
        font.family: view.host.uiFont
        font.pixelSize: 10
        font.weight: Font.DemiBold
        font.letterSpacing: 0.8
      }
    }

    Repeater {
      model: view.rows
      delegate: Item {
        id: row
        required property var modelData
        required property int index
        readonly property bool isChosen: view.chosen[modelData.path] === true
        readonly property bool capture: modelData.section === Model.CAPTURE
        readonly property bool result: modelData.section === "result"
        readonly property bool image: /\.(png|jpe?g|gif|webp|svg|bmp|avif)$/i.test(modelData.name)
        readonly property string thumb: Model.thumbFor(view.thumbs, modelData)
        width: flick.width
        height: view.rowHeight
        y: view.layout.ys[index] !== undefined ? view.layout.ys[index] : 0

        MouseArea {
          id: rowMouse
          anchors.fill: parent
          hoverEnabled: true
          acceptedButtons: Qt.LeftButton | Qt.RightButton
          onPositionChanged: if (containsMouse && !view.actionsOpen) view.selected = row.index
          onClicked: function(mouse) {
            if (mouse.button === Qt.RightButton) { view.selected = row.index; view.actionIndex = 0; view.actionsOpen = true; return }
            view.actionsOpen = false
            if (mouse.modifiers & Qt.ShiftModifier) { view.selected = row.index; view.chooseRange(row.index) }
            else if (mouse.modifiers & Qt.ControlModifier) view.toggleChosen(row.index)
            else { view.selected = row.index; view.anchorIndex = row.index }
          }
          onDoubleClicked: view.openRow(row.index)
        }

        // Vignette (ou icône), qui sert aussi de case à cocher.
        Item {
          id: tile
          anchors.left: parent.left
          anchors.leftMargin: 16
          anchors.verticalCenter: parent.verticalCenter
          width: 32; height: 32
          Image {
            id: tileImage
            anchors.fill: parent
            source: row.thumb !== "" ? "file://" + row.thumb : ""
            sourceSize.width: 64
            sourceSize.height: 64
            fillMode: Image.PreserveAspectCrop
            asynchronous: true
            visible: status === Image.Ready
            layer.enabled: true
          }
          Rectangle {
            anchors.fill: parent
            radius: 9
            visible: tileImage.status !== Image.Ready
            color: view.host.withAlpha(view.host.colorText, 0.07)
            Glyph {
              anchors.centerIn: parent
              glyph: row.modelData.folder ? "󰉋" : row.modelData.recording ? "󰕧" : row.capture || row.image ? "󰋩" : "󰈔"
              color: view.host.colorText
              fontFamily: view.host.fontFamily
              pixelSize: 16
            }
          }
          // Coche : pastille d'accent sur la vignette.
          Rectangle {
            anchors.fill: parent
            radius: 9
            color: "transparent"
            border.width: row.isChosen ? 2 : 0
            border.color: view.host.colorAccent
          }
          Rectangle {
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            anchors.margins: -4
            width: 16; height: 16; radius: 8
            visible: row.isChosen || tileMouse.containsMouse
            color: row.isChosen ? view.host.colorAccent : Qt.lighter(view.host.colorIsland, 2.2)
            border.width: row.isChosen ? 0 : 1
            border.color: view.host.withAlpha(view.host.colorText, 0.3)
            Glyph {
              anchors.centerIn: parent
              visible: row.isChosen
              glyph: "󰄬"
              color: view.host.colorAccentText
              fontFamily: view.host.fontFamily
              pixelSize: 11
            }
          }
          MouseArea {
            id: tileMouse
            anchors.fill: parent
            anchors.margins: -4
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: { view.selected = row.index; view.toggleChosen(row.index) }
          }
        }

        // Boutons de ligne (survol ou curseur).
        Row {
          id: rowButtons
          anchors.right: parent.right
          anchors.rightMargin: 12
          anchors.verticalCenter: parent.verticalCenter
          spacing: 2
          opacity: rowMouse.containsMouse || tileMouse.containsMouse || buttonsHover.hovered || view.selected === row.index ? 1 : 0
          Behavior on opacity { NumberAnimation { duration: 120 } }
          HoverHandler { id: buttonsHover }
          component RowButton: Rectangle {
            id: rb
            property string glyph: ""
            property string tip: ""
            signal activated()
            width: 26; height: 26; radius: 8
            color: rbMouse.containsMouse ? view.host.withAlpha(view.host.colorText, 0.1) : "transparent"
            Glyph { anchors.centerIn: parent; glyph: rb.glyph; color: rbMouse.containsMouse ? view.host.colorText : view.host.colorMuted; fontFamily: view.host.fontFamily; pixelSize: 13 }
            MouseArea { id: rbMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: rb.activated() }
          }
          RowButton { visible: row.capture || row.result; glyph: "󰐕"; onActivated: { view.host.addToShelf([row.modelData.path]); view.say("Added to the shelf", false) } }
          RowButton { visible: !row.capture && !row.result; glyph: "󰅖"; onActivated: view.forgetRow(row.index) }
        }

        Column {
          anchors.left: tile.right
          anchors.leftMargin: 12
          anchors.right: rowButtons.left
          anchors.rightMargin: 10
          anchors.verticalCenter: parent.verticalCenter
          spacing: 1
          Row {
            width: parent.width
            spacing: 6
            Text {
              id: nameText
              width: Math.min(implicitWidth, parent.width)
              text: row.modelData.name
              textFormat: Text.PlainText
              elide: Text.ElideMiddle
              maximumLineCount: 1
              color: view.host.colorText
              font.family: view.host.uiFont
              font.pixelSize: 14
              font.weight: Font.DemiBold
            }
            Glyph {
              anchors.verticalCenter: nameText.verticalCenter
              visible: false
              glyph: "󰐃"
              color: view.host.colorMuted
              fontFamily: view.host.fontFamily
              pixelSize: 11
            }
          }
          Text {
            width: parent.width
            readonly property string sizeLabel: row.modelData.bytes >= 0 ? Model.sizeText(row.modelData) : ""
            text: (sizeLabel !== "" ? sizeLabel + "  ·  " : "") + view.parentOf(row.modelData.path)
            textFormat: Text.PlainText
            elide: Text.ElideMiddle
            color: view.host.colorMuted
            font.family: view.host.uiFont
            font.pixelSize: 12
          }
        }
      }
    }
  }

  // ---------------------------------------------------------------- empty
  Item {
    anchors.top: header.bottom
    anchors.topMargin: 8
    anchors.left: parent.left
    anchors.right: parent.right
    height: view.listHeight
    visible: view.pending === "" && view.rows.length === 0
    Column {
      anchors.centerIn: parent
      spacing: 6
      Glyph { anchors.horizontalCenter: parent.horizontalCenter; glyph: "󰉍"; color: view.host.colorMuted; fontFamily: view.host.fontFamily; pixelSize: 26 }
      Text { anchors.horizontalCenter: parent.horizontalCenter; text: view.searching ? (view.searchBusy ? "Searching…" : "No results") : "The shelf is empty"; color: view.host.colorText; font.family: view.host.uiFont; font.pixelSize: 13; font.weight: Font.DemiBold }
      Text { anchors.horizontalCenter: parent.horizontalCenter; visible: !view.searching; text: "Type to search your files, or use “Add to shelf” in Flea"; color: view.host.colorMuted; font.family: view.host.uiFont; font.pixelSize: 12 }
    }
  }

  // ---------------------------------------------------------------- destination picker
  Column {
    anchors.top: header.bottom
    anchors.topMargin: 8
    anchors.left: parent.left
    anchors.right: parent.right
    visible: view.pending !== ""
    spacing: 4
    Repeater {
      model: view.pending === "move" || view.pending === "copy" ? view.flyoutRows.concat([""])
        : view.pending !== "" ? view.flyoutRows : []
      delegate: Rectangle {
        id: dest
        required property var modelData
        required property int index
        readonly property bool browse: index === view.flyoutRows.length && (view.pending === "move" || view.pending === "copy")
        width: parent.width
        height: view.rowHeight
        radius: 14
        color: view.flyoutIndex === index ? view.host.withAlpha(view.host.colorText, 0.09) : "transparent"
        Rectangle {
          visible: view.flyoutIndex === dest.index
          anchors.left: parent.left
          anchors.leftMargin: 6
          anchors.verticalCenter: parent.verticalCenter
          width: 3; height: 18; radius: 1.5
          color: view.host.colorAccent
        }
        Rectangle {
          id: numTile
          anchors.left: parent.left
          anchors.leftMargin: 16
          anchors.verticalCenter: parent.verticalCenter
          width: 32; height: 32; radius: 9
          color: view.host.withAlpha(view.host.colorText, 0.07)
          Text {
            anchors.centerIn: parent
            text: dest.browse ? "󰉖" : (dest.index < 9 ? String(dest.index + 1) : (view.pending === "send" ? "󰒊" : "󰉋"))
            color: view.host.colorText
            font.family: dest.browse ? view.host.fontFamily : view.host.uiFont
            font.pixelSize: dest.browse ? 16 : 13
            font.weight: Font.DemiBold
          }
        }
        Text {
          anchors.left: numTile.right
          anchors.leftMargin: 12
          anchors.right: parent.right
          anchors.rightMargin: 14
          anchors.verticalCenter: parent.verticalCenter
          text: dest.browse ? "Browse…" : (view.pending === "send" || view.pending === "localsend" ? String(dest.modelData) : view.shortPath(dest.modelData))
          elide: Text.ElideMiddle
          color: view.host.colorText
          font.family: view.host.uiFont
          font.pixelSize: 14
          font.weight: Font.DemiBold
        }
        MouseArea {
          anchors.fill: parent
          hoverEnabled: true
          cursorShape: Qt.PointingHandCursor
          onEntered: view.flyoutIndex = dest.index
          onClicked: view.pickFlyout(dest.index)
        }
      }
    }
    Text {
      visible: view.pending === "localsend"
      width: parent.width
      leftPadding: 16
      topPadding: 12
      wrapMode: Text.WordWrap
      text: view.lsReason.indexOf("already running") >= 0 ? "The LocalSend app is open and holds its port: close it, or send from the app."
        : view.flyoutRows.length ? "Still looking for other devices…"
        : "Looking for devices… open LocalSend on the other device, on the same network."
      color: view.lsReason.indexOf("already running") >= 0 ? "#e5484d" : view.host.colorMuted
      font.family: view.host.uiFont
      font.pixelSize: 12
    }
  }

  // ---------------------------------------------------------------- actions panel
  Rectangle {
    id: actionsPanel
    readonly property bool shown: view.actionsOpen && view.currentActions.length > 0
    z: 5
    anchors.right: parent.right
    anchors.bottom: footer.top
    anchors.bottomMargin: 8
    width: 280
    height: actionsColumn.implicitHeight + 12
    radius: 16
    color: Qt.lighter(view.host.colorIsland, 2.4)
    border.width: 1
    border.color: view.host.withAlpha(view.host.colorText, 0.1)
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
        model: actionsPanel.shown ? view.currentActions : []
        delegate: Rectangle {
          required property var modelData
          required property int index
          width: actionsColumn.width
          height: 34
          radius: 10
          color: view.actionIndex === index ? view.host.withAlpha(view.host.colorText, 0.1) : "transparent"
          Behavior on color { ColorAnimation { duration: 100 } }
          Row {
            anchors.left: parent.left
            anchors.leftMargin: 10
            anchors.verticalCenter: parent.verticalCenter
            spacing: 10
            Glyph { anchors.verticalCenter: parent.verticalCenter; glyph: modelData.glyph; color: view.actionIndex === index ? view.host.colorAccent : view.host.colorMuted; fontFamily: view.host.fontFamily; pixelSize: 14 }
            Text { anchors.verticalCenter: parent.verticalCenter; text: modelData.label; color: view.host.colorText; font.family: view.host.uiFont; font.pixelSize: 13 }
          }
          // La touche directe, pour s'en souvenir.
          Rectangle {
            anchors.right: parent.right
            anchors.rightMargin: 10
            anchors.verticalCenter: parent.verticalCenter
            width: keyLabel.implicitWidth + 10; height: 16; radius: 4
            color: view.host.withAlpha(view.host.colorText, 0.08)
            Text { id: keyLabel; anchors.centerIn: parent; text: modelData.key; color: view.host.colorMuted; font.family: view.host.uiFont; font.pixelSize: 10; font.weight: Font.DemiBold }
          }
          MouseArea {
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onEntered: view.actionIndex = index
            onClicked: view.runAction(modelData.id)
          }
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
    height: view.footerHeight
    Rectangle { anchors.top: parent.top; width: parent.width; height: 1; color: view.host.withAlpha(view.host.colorText, 0.06) }
    // Progression d'un déplacement / d'une copie.
    Rectangle {
      anchors.top: parent.top
      height: 1
      width: parent.width * Run.runFraction(view.run)
      color: view.host.colorAccent
      visible: view.run.running
    }
    Text {
      anchors.left: parent.left
      anchors.leftMargin: 14
      anchors.right: hints.visible ? hints.left : parent.right
      anchors.rightMargin: hints.visible ? 12 : 14
      anchors.verticalCenter: parent.verticalCenter
      readonly property string runLine: Run.runFooter(view.run)
      text: runLine !== "" ? runLine : view.notice
      visible: text !== ""
      elide: Text.ElideMiddle
      color: view.noticeError && runLine === "" ? "#e5484d" : view.host.colorMuted
      font.family: view.host.uiFont
      font.pixelSize: 11
    }
    Row {
      id: hints
      anchors.right: parent.right
      anchors.rightMargin: 14
      anchors.verticalCenter: parent.verticalCenter
      spacing: 12
      visible: view.notice === "" && !view.run.running
      component Hint: Row {
        property string keys: ""
        property string label: ""
        spacing: 5
        Rectangle {
          anchors.verticalCenter: parent.verticalCenter
          width: keyText.implicitWidth + 10; height: 16; radius: 4
          color: view.host.withAlpha(view.host.colorText, 0.08)
          Text { id: keyText; anchors.centerIn: parent; text: keys; color: view.host.colorMuted; font.family: view.host.uiFont; font.pixelSize: 10; font.weight: Font.DemiBold }
        }
        Text { anchors.verticalCenter: parent.verticalCenter; text: label; color: view.host.colorMuted; font.family: view.host.uiFont; font.pixelSize: 11 }
      }
      Hint { visible: view.pending !== ""; keys: "1–9"; label: "Pick" }
      Hint { visible: view.pending === "move" || view.pending === "copy"; keys: "B"; label: "Browse" }
      Hint { visible: view.pending !== ""; keys: "Esc"; label: "Back" }
      Hint { visible: view.pending === ""; keys: "↵"; label: "Open" }
      Hint { visible: view.pending === "" && !view.searching; keys: "Space"; label: "Select" }
      Hint { visible: view.pending === "" && !!view.rows[view.selected] && view.rows[view.selected].folder === true && view.rows[view.selected].section === "result"; keys: "Tab"; label: "Browse" }
      Hint { visible: view.pending === ""; keys: "Ctrl C"; label: "Clipboard" }
      Hint { visible: view.pending === ""; keys: "Ctrl B"; label: "Actions" }
    }
  }
}
