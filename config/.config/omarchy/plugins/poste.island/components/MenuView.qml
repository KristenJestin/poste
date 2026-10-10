import QtQuick
import Quickshell
import Quickshell.Io
import "file:///usr/share/omarchy/shell/plugins/menu/MenuModel.js" as MenuModel

// The Omarchy menu inside the island (SUPER+ALT+SPACE). The tree comes from
// Omarchy's own definition — the default omarchy-menu.jsonc merged with
// ~/.config/omarchy/extensions/omarchy-menu.jsonc — parsed, guarded (when /
// checked) and filtered with Omarchy's MenuModel.js, and every action runs
// the exact command the native menu runs. Layout inspired by Island's
// OmarchyMenu view (https://github.com/Guilhermerisu/island, MIT).
//
// Exceptions: "Apps" opens the island launcher, and provider-backed submenus
// (fonts, power profiles) are handed to the native menu at that route.
// Keys: ↑↓ move, Enter / → open, ← / Backspace (empty search) go back,
// Esc goes back then closes; typing searches the whole menu.
Item {
  id: menu
  required property var host
  property bool active: false
  signal closeRequested()
  signal openLauncher()

  property real maxHeight: 600
  readonly property real rowHeight: 44
  readonly property real chrome: header.height + 12
  readonly property real listHeight: Math.min(Math.max(rowHeight + 4, maxHeight - chrome),
    Math.max(1, rows.count) * (rowHeight + 4))
  implicitHeight: Math.min(maxHeight, chrome + listHeight)

  // ---------------------------------------------------------------- data
  readonly property string omarchyPath: host.omarchyPath || "/usr/share/omarchy"
  property var defaultItems: []
  property var userItems: []
  property var items: ({})
  property var itemOrder: []
  property var whenResults: ({})
  property var checkedResults: ({})

  FileView {
    path: menu.omarchyPath + "/default/omarchy/omarchy-menu.jsonc"
    watchChanges: true
    printErrors: false
    onFileChanged: reload()
    onLoaded: { menu.defaultItems = MenuModel.parseMenuJsonc(text()); menu.merge() }
  }
  FileView {
    path: Quickshell.env("HOME") + "/.config/omarchy/extensions/omarchy-menu.jsonc"
    watchChanges: true
    printErrors: false
    onFileChanged: reload()
    onLoaded: { menu.userItems = MenuModel.parseMenuJsonc(text()); menu.merge() }
    onLoadFailed: { menu.userItems = []; menu.merge() }
  }
  function merge() {
    var merged = MenuModel.mergeMenuSources(defaultItems, userItems)
    items = merged.items
    itemOrder = merged.itemOrder
    evaluateGuards()
    rebuild()
  }

  // `when` / `checked` are shell conditions, batched in one bash run exactly
  // like the native menu (same script, same output format).
  Process {
    id: guardProc
    property string collected: ""
    stdout: SplitParser { onRead: function(line) { guardProc.collected += line + "\n" } }
    onExited: function(code, status) {
      if (code !== 0 || status !== 0) return
      var w = {}, c = {}
      var lines = guardProc.collected.split("\n")
      for (var i = 0; i < lines.length; i++) {
        var line = lines[i].trim()
        var colon = line.lastIndexOf(":")
        if (colon < 0) continue
        var value = line.substring(colon + 1) === "1"
        var rest = line.substring(0, colon)
        var tagAt = rest.lastIndexOf(":")
        if (tagAt < 0) continue
        var id = rest.substring(0, tagAt)
        if (rest.substring(tagAt + 1) === "w") w[id] = value
        else c[id] = value
      }
      menu.whenResults = w
      menu.checkedResults = c
      menu.rebuild()
    }
  }
  function evaluateGuards() {
    if (guardProc.running) return
    var script = MenuModel.guardScript(items)
    if (!script) return
    guardProc.collected = ""
    guardProc.command = ["bash", "-lc", script]
    guardProc.running = true
  }

  // ---------------------------------------------------------------- navigation
  property string activeMenu: "root"
  property var navStack: []
  property string query: ""
  property int selected: 0
  property int slideDirection: 1

  onActiveChanged: {
    if (!active) return
    activeMenu = "root"
    navStack = []
    field.text = ""
    query = ""
    selected = 0
    evaluateGuards()
    rebuild()
    Qt.callLater(function() { field.forceActiveFocus() })
  }
  onQueryChanged: { selected = 0; rebuild() }

  function visible_(entry) { return MenuModel.isVisible(items, itemOrder, whenResults, entry) }
  function isSubmenu(entry) { return entry.kind === "menu" || entry.kind === "link" }

  ListModel { id: rows }

  function fuzzy(text, q) {
    text = String(text || "").toLowerCase()
    if (!q) return 0
    var idx = text.indexOf(q)
    if (idx === 0) return 1000 - text.length
    if (idx > 0) return 600 - idx
    var ti = 0, score = 0
    for (var qi = 0; qi < q.length; qi++) {
      var found = text.indexOf(q.charAt(qi), ti)
      if (found === -1) return -1
      score += found === ti ? 12 : 4
      ti = found + 1
    }
    return score
  }

  function rowFor(entry, detail) {
    return {
      itemId: entry.id,
      label: MenuModel.labelFor(entry, checkedResults),
      icon: String(entry.icon || ""),
      detail: detail || "",
      chevron: isSubmenu(entry) && entry.id !== "apps"
    }
  }

  function rebuild() {
    var q = query.trim().toLowerCase()
    var out = []
    if (q) {
      var scored = []
      for (var i = 0; i < itemOrder.length; i++) {
        var e = items[itemOrder[i]]
        if (!e || e.id === "root" || !visible_(e)) continue
        if (activeMenu !== "root" && e.id !== activeMenu && !MenuModel.isDescendantOf(items, e.id, activeMenu)) continue
        var s = Math.max(fuzzy(e.label, q), fuzzy((e.aliases || []).join(" "), q) - 100, fuzzy(e.description, q) - 200)
        if (s < 0) continue
        scored.push({ e: e, s: s - MenuModel.depthFor(items, e.id) * 5 })
      }
      scored.sort(function(a, b) { return b.s - a.s })
      for (var j = 0; j < Math.min(scored.length, 40); j++)
        out.push(rowFor(scored[j].e, MenuModel.parentPathFor(items, scored[j].e.id)))
    } else {
      for (var k = 0; k < itemOrder.length; k++) {
        var child = items[itemOrder[k]]
        if (!child || child.parent !== activeMenu || !visible_(child)) continue
        out.push(rowFor(child, ""))
      }
    }
    rows.clear()
    for (var n = 0; n < out.length; n++) rows.append(out[n])
    selected = Math.min(selected, Math.max(0, rows.count - 1))
  }

  function enter(id) {
    slideDirection = 1
    navStack = navStack.concat([activeMenu])
    activeMenu = id
    field.text = ""
    query = ""
    selected = 0
    rebuild()
    slide.restart()
  }
  function back() {
    if (activeMenu === "root") return false
    slideDirection = -1
    var prev = navStack.length ? navStack[navStack.length - 1] : ((items[activeMenu] || {}).parent || "root")
    navStack = navStack.slice(0, Math.max(0, navStack.length - 1))
    activeMenu = prev
    selected = 0
    rebuild()
    slide.restart()
    return true
  }

  function activate(index) {
    if (index < 0 || index >= rows.count) return
    var entry = items[rows.get(index).itemId]
    if (!entry) return
    if (entry.id === "apps") { openLauncher(); return }
    if (entry.provider) {
      // Dynamic submenu (fonts, power profiles): the native menu owns it.
      closeRequested()
      Quickshell.execDetached(["omarchy-menu", "summon", entry.id])
      return
    }
    if (isSubmenu(entry)) { enter(entry.target || entry.id); return }
    closeRequested()
    // Same as the native menu's runAction (Util.execDetached of the string).
    if (entry.action) Quickshell.execDetached(["bash", "-c", entry.action])
  }

  readonly property string title: activeMenu === "root" ? "" : MenuModel.pathFor(items, activeMenu)

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
        id: backGlyph
        anchors.left: parent.left
        anchors.leftMargin: 4
        anchors.verticalCenter: parent.verticalCenter
        glyph: menu.activeMenu === "root" ? "󰍉" : "󰁍"
        color: menu.host.colorMuted
        fontFamily: menu.host.fontFamily
        pixelSize: 16
        MouseArea {
          anchors.fill: parent
          anchors.margins: -6
          enabled: menu.activeMenu !== "root"
          cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
          onClicked: menu.back()
        }
      }
      TextInput {
        id: field
        anchors.left: backGlyph.right
        anchors.leftMargin: 10
        anchors.right: crumb.left
        anchors.rightMargin: 10
        anchors.verticalCenter: parent.verticalCenter
        color: menu.host.colorText
        selectionColor: menu.host.withAlpha(menu.host.colorAccent, 0.4)
        font.family: menu.host.uiFont
        font.pixelSize: 17
        clip: true
        onTextChanged: menu.query = text
        Text {
          anchors.fill: parent
          visible: !field.text
          text: "Search…"
          color: menu.host.colorMuted
          font: field.font
        }
        Keys.onPressed: function(event) {
          if (event.key === Qt.Key_Escape) { if (field.text) field.text = ""; else if (!menu.back()) menu.closeRequested(); event.accepted = true }
          else if (event.key === Qt.Key_Down) { menu.selected = Math.min(rows.count - 1, menu.selected + 1); list.positionViewAtIndex(menu.selected, ListView.Contain); event.accepted = true }
          else if (event.key === Qt.Key_Up) { menu.selected = Math.max(0, menu.selected - 1); list.positionViewAtIndex(menu.selected, ListView.Contain); event.accepted = true }
          else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) { menu.activate(menu.selected); event.accepted = true }
          else if (event.key === Qt.Key_Right && !field.text) { var r = rows.count ? rows.get(menu.selected) : null; if (r && r.chevron) menu.activate(menu.selected); event.accepted = true }
          else if ((event.key === Qt.Key_Left || event.key === Qt.Key_Backspace) && !field.text) { menu.back(); event.accepted = true }
        }
      }
      Text {
        id: crumb
        anchors.right: parent.right
        anchors.rightMargin: 6
        anchors.verticalCenter: parent.verticalCenter
        width: Math.min(implicitWidth, 220)
        elide: Text.ElideLeft
        text: menu.title
        color: menu.host.colorMuted
        font.family: menu.host.uiFont
        font.pixelSize: 12
        font.weight: Font.DemiBold
      }
    }
    Rectangle { width: parent.width; height: 1; color: menu.host.withAlpha(menu.host.colorText, 0.08) }
  }

  Text {
    anchors.top: header.bottom
    anchors.topMargin: 16
    anchors.horizontalCenter: parent.horizontalCenter
    visible: rows.count === 0
    text: "No results"
    color: menu.host.colorMuted
    font.family: menu.host.uiFont
    font.pixelSize: 13
  }

  ListView {
    WheelScroll {}
    id: list
    anchors.top: header.bottom
    anchors.topMargin: 12
    anchors.left: parent.left
    anchors.right: parent.right
    height: menu.listHeight
    clip: true
    spacing: 4
    model: rows
    currentIndex: menu.selected
    boundsBehavior: Flickable.StopAtBounds
    highlightMoveDuration: 140
    highlight: Rectangle {
      width: list.width
      radius: 14
      color: menu.host.withAlpha(menu.host.colorText, 0.09)
      Rectangle {
        anchors.left: parent.left
        anchors.leftMargin: 6
        anchors.verticalCenter: parent.verticalCenter
        width: 3; height: 18; radius: 1.5
        color: menu.host.colorAccent
      }
    }

    // Submenus slide in from the side they were entered from.
    transform: Translate { id: slideShift }
    SequentialAnimation {
      id: slide
      ParallelAnimation {
        NumberAnimation { target: slideShift; property: "x"; from: 36 * menu.slideDirection; to: 0; duration: 260; easing.type: Easing.OutQuint }
        NumberAnimation { target: list; property: "opacity"; from: 0; to: 1; duration: 200; easing.type: Easing.OutCubic }
      }
    }

    delegate: Item {
      id: row
      required property int index
      required property string itemId
      required property string label
      required property string icon
      required property string detail
      required property bool chevron
      width: list.width
      height: menu.rowHeight

      MouseArea {
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onEntered: menu.selected = row.index
        onClicked: menu.activate(row.index)
      }
      Glyph {
        id: glyph
        anchors.left: parent.left
        anchors.leftMargin: 18
        anchors.verticalCenter: parent.verticalCenter
        width: 22
        horizontalAlignment: Text.AlignHCenter
        glyph: row.icon
        color: menu.host.colorText
        fontFamily: menu.host.fontFamily
        pixelSize: 16
      }
      Column {
        anchors.left: glyph.right
        anchors.leftMargin: 14
        anchors.right: chevronGlyph.left
        anchors.rightMargin: 8
        anchors.verticalCenter: parent.verticalCenter
        spacing: 1
        Text {
          width: parent.width
          text: row.label
          elide: Text.ElideRight
          color: menu.host.colorText
          font.family: menu.host.uiFont
          font.pixelSize: 14
          font.weight: Font.DemiBold
        }
        Text {
          width: parent.width
          visible: row.detail !== ""
          text: row.detail
          elide: Text.ElideRight
          color: menu.host.colorMuted
          font.family: menu.host.uiFont
          font.pixelSize: 11
        }
      }
      Glyph {
        id: chevronGlyph
        anchors.right: parent.right
        anchors.rightMargin: 16
        anchors.verticalCenter: parent.verticalCenter
        visible: row.chevron
        glyph: "󰅂"
        color: menu.host.colorMuted
        fontFamily: menu.host.fontFamily
        pixelSize: 13
      }
    }
  }
}
