import QtQuick
import Quickshell
import Quickshell.Bluetooth
import "file:///usr/share/omarchy/shell/plugins/panels/bluetooth/Model.js" as BtModel

// Bluetooth, grown inside the island from the control center's tile. Device
// state comes from Quickshell.Bluetooth; actions go through Omarchy's own
// helpers (omarchy-bluetooth-device / omarchy-bluetooth-power), like its
// panel. Discovery runs only while this view is open.
// Keys: ↑↓ move, Enter connects / pairs, Ctrl+B actions, Esc / ← back.
Item {
  id: bt
  required property var host
  property bool active: false
  signal backRequested()
  signal advancedRequested()

  implicitHeight: column.implicitHeight
  property real maxListHeight: 360
  readonly property real rowHeight: 46

  readonly property var adapter: Bluetooth.defaultAdapter
  readonly property bool powered: !!adapter && adapter.enabled
  readonly property var lists: BtModel.deviceLists(Bluetooth.devices ? Bluetooth.devices.values : [])

  property int selected: 0
  property bool actionsOpen: false
  property int actionIndex: 0
  property bool confirmForget: false
  property var pending: ({})   // address -> "connecting" | "disconnecting" | "forgetting"
  property string notice: ""
  property bool noticeError: false

  onActiveChanged: {
    if (!active) { actionsOpen = false; return }
    selected = 0
    confirmForget = false
    sync()
    Qt.callLater(function() { bt.forceActiveFocus() })
  }
  onListsChanged: Qt.callLater(sync)

  // Snapshots, keyed by address (BlueZ objects can vanish under a delegate).
  ListModel { id: rows }
  property int rev: 0
  function sync() {
    var flat = []
    function add(list, group) {
      for (var i = 0; i < list.length; i++) {
        var d = list[i]
        if (!d || !d.address) continue
        flat.push({ address: String(d.address), name: BtModel.deviceLabel(d), group: group,
                    connected: !!d.connected, paired: !!(d.paired || d.bonded || d.trusted),
                    battery: d.batteryAvailable ? Math.round((d.battery || 0) * 100) : -1,
                    icon: String(d.icon || ""), busy: !!d.pairing || d.state === 3 })
      }
    }
    add(lists.connected, "Connected")
    add(lists.known, "My devices")
    if (active && powered) add(lists.discovered, "Nearby")
    var keys = {}
    for (var a = 0; a < flat.length; a++) keys[flat[a].address] = true
    for (var j = rows.count - 1; j >= 0; j--) if (!keys[rows.get(j).address]) rows.remove(j)
    for (var k = 0; k < flat.length; k++) {
      var found = -1
      for (var m = k; m < rows.count; m++) if (rows.get(m).address === flat[k].address) { found = m; break }
      if (found === -1) rows.insert(k, flat[k])
      else { if (found !== k) rows.move(found, k, 1); rows.set(k, flat[k]) }
    }
    while (rows.count > flat.length) rows.remove(rows.count - 1)
    if (selected >= rows.count) selected = Math.max(0, rows.count - 1)
    rev++
    // Settle pending actions whose outcome is visible.
    var next = {}, changed = false
    for (var addr in pending) {
      var row = null
      for (var r = 0; r < rows.count; r++) if (rows.get(r).address === addr) row = rows.get(r)
      var act = pending[addr]
      var done = act === "connecting" ? (row && row.connected)
        : act === "disconnecting" ? (!row || !row.connected)
        : (!row || !row.paired)
      if (done) changed = true
      else next[addr] = act
    }
    if (changed) pending = next
  }
  readonly property var current: { rev; return selected < rows.count ? rows.get(selected) : null }

  // ---------------------------------------------------------------- discovery
  property bool owesStop: false
  Timer {
    interval: 1000; repeat: true; triggeredOnStart: true
    running: bt.active && bt.powered && !bt.adapter.discovering
    onTriggered: { bt.owesStop = true; bt.adapter.discovering = true }
  }
  // Stop the scan we started once the view is closed (bound to the confirmed
  // state, see Omarchy's panel for why a write at close time is not enough).
  Timer {
    id: stopTimer
    interval: 1000; repeat: true
    property int attempts: 0
    running: !bt.active && bt.owesStop && !!bt.adapter && bt.adapter.discovering === true
    onRunningChanged: if (running) attempts = 0
    onTriggered: {
      attempts += 1
      if (attempts > 3) { bt.owesStop = false; return }
      bt.adapter.discovering = false
    }
  }
  Connections {
    target: bt.adapter
    function onDiscoveringChanged() { if (!bt.adapter.discovering && !bt.active) bt.owesStop = false }
  }
  Component.onDestruction: if (owesStop && adapter && adapter.discovering) adapter.discovering = false

  // ---------------------------------------------------------------- actions
  function setPending(address, action) {
    var next = {}
    for (var k in pending) next[k] = pending[k]
    if (action) next[address] = action
    else delete next[address]
    pending = next
    if (action) pendingTimeout.restart()
  }
  Timer { id: pendingTimeout; interval: 25000; onTriggered: bt.pending = ({}) }
  function device(address) {
    var list = Bluetooth.devices ? Bluetooth.devices.values : []
    for (var i = 0; i < list.length; i++) if (list[i] && String(list[i].address) === address) return list[i]
    return null
  }
  function say(text, error) { notice = text; noticeError = !!error; noticeClear.restart() }
  Timer { id: noticeClear; interval: 6000; onTriggered: bt.notice = "" }

  function activate(index) {
    var n = index < rows.count ? rows.get(index) : null
    if (!n || n.connected || pending[n.address]) return
    setPending(n.address, "connecting")
    Quickshell.execDetached(["omarchy-bluetooth-device", n.paired ? "connect" : "pair", n.address])
    say((n.paired ? "Connecting to " : "Pairing with ") + n.name + "…")
  }
  function togglePower() {
    Quickshell.execDetached(["omarchy-bluetooth-power", powered ? "off" : "on"])
  }
  readonly property var currentActions: {
    var n = current
    if (!n) return []
    var list = []
    if (n.connected) list.push({ id: "disconnect", label: "Disconnect", glyph: "󰂲" })
    else list.push({ id: "connect", label: n.paired ? "Connect" : "Pair", glyph: "󰂱" })
    if (n.paired) list.push({ id: "forget", label: confirmForget ? "Confirm forget" : "Forget", glyph: "󰆴", danger: confirmForget })
    list.push({ id: "copy", label: "Copy address", glyph: "󰆏" })
    return list
  }
  function runRowAction(id) {
    var n = current
    if (!n) return
    if (id === "connect") { actionsOpen = false; activate(selected) }
    else if (id === "disconnect") {
      actionsOpen = false
      setPending(n.address, "disconnecting")
      var d = device(n.address)
      if (d && d.disconnect) d.disconnect()
      Quickshell.execDetached(["omarchy-bluetooth-device", "disconnect", n.address])
      say("Disconnecting " + n.name + "…")
    } else if (id === "forget") {
      if (!confirmForget) { confirmForget = true; return }
      confirmForget = false; actionsOpen = false
      setPending(n.address, "forgetting")
      Quickshell.execDetached(["omarchy-bluetooth-device", "forget", n.address])
      say("Forgetting " + n.name + "…")
    } else if (id === "copy") {
      Quickshell.execDetached(["wl-copy", "--", n.address])
      actionsOpen = false
      say("Address copied")
    }
  }

  function deviceGlyph(icon) {
    if (/headset|headphone/.test(icon)) return "󰋋"
    if (/audio|speaker/.test(icon)) return "󰓃"
    if (/mouse/.test(icon)) return "󰍽"
    if (/keyboard/.test(icon)) return "󰌌"
    if (/phone/.test(icon)) return "󰏲"
    if (/gaming|joystick/.test(icon)) return "󰊗"
    if (/computer/.test(icon)) return "󰟀"
    return "󰂯"
  }
  function batteryText(p) {
    if (p < 0) return ""
    var g = p >= 90 ? "󰁹" : p >= 70 ? "󰂀" : p >= 50 ? "󰁾" : p >= 30 ? "󰁼" : p >= 10 ? "󰁺" : "󰂎"
    return g + " " + p + "%"
  }

  // ---------------------------------------------------------------- keys
  Keys.onPressed: function(event) {
    var ctrl = event.modifiers & Qt.ControlModifier
    if (actionsOpen) {
      if (event.key === Qt.Key_Escape || (ctrl && event.key === Qt.Key_B)) { actionsOpen = false; confirmForget = false }
      else if (event.key === Qt.Key_Left) { actionIndex = Math.max(0, actionIndex - 1); confirmForget = false }
      else if (event.key === Qt.Key_Right) { actionIndex = Math.min(currentActions.length - 1, actionIndex + 1); confirmForget = false }
      else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) { var act = currentActions[actionIndex]; if (act) runRowAction(act.id) }
      else return
      event.accepted = true
      return
    }
    if (event.key === Qt.Key_Escape || event.key === Qt.Key_Left || event.key === Qt.Key_Backspace) bt.backRequested()
    else if (event.key === Qt.Key_Down) { selected = Math.min(rows.count - 1, selected + 1); list.positionViewAtIndex(selected, ListView.Contain) }
    else if (event.key === Qt.Key_Up) { selected = Math.max(0, selected - 1); list.positionViewAtIndex(selected, ListView.Contain) }
    else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) activate(selected)
    else if (ctrl && event.key === Qt.Key_B) { if (current) { actionIndex = 0; confirmForget = false; actionsOpen = true } }
    else return
    event.accepted = true
  }

  // ---------------------------------------------------------------- view
  Column {
    id: column
    anchors.left: parent.left
    anchors.right: parent.right
    spacing: 12

    ModuleHeader {
      host: bt.host
      title: "Bluetooth"
      status: bt.powered && bt.adapter.discovering ? "Scanning…" : ""
      showSwitch: !!bt.adapter
      switchOn: bt.powered
      onBackClicked: bt.backRequested()
      onSwitchClicked: bt.togglePower()
    }

    Text {
      visible: !bt.adapter || !bt.powered || rows.count === 0
      width: parent.width
      horizontalAlignment: Text.AlignHCenter
      topPadding: 6
      bottomPadding: 6
      text: !bt.adapter ? "No Bluetooth adapter" : !bt.powered ? "Bluetooth is off" : "Looking for devices…"
      color: bt.host.colorMuted
      font.family: bt.host.uiFont
      font.pixelSize: 12
    }

    ListView {
      id: list
      visible: bt.powered && rows.count > 0
      width: parent.width
      height: Math.min(bt.maxListHeight, contentHeight)
      clip: true
      spacing: 4
      interactive: contentHeight > height
      boundsBehavior: Flickable.StopAtBounds
      model: rows
      section.property: "group"
      section.delegate: Text {
        required property string section
        width: list.width
        leftPadding: 6
        topPadding: 6
        bottomPadding: 4
        text: section
        color: bt.host.colorMuted
        font.family: bt.host.uiFont
        font.pixelSize: 11
        font.weight: Font.DemiBold
      }
      add: Transition {
        ParallelAnimation {
          NumberAnimation { property: "opacity"; from: 0; to: 1; duration: 200; easing.type: Easing.OutCubic }
          NumberAnimation { property: "x"; from: 8; to: 0; duration: 240; easing.type: Easing.OutQuint }
        }
      }
      remove: Transition { NumberAnimation { property: "opacity"; to: 0; duration: 150 } }
      displaced: Transition { NumberAnimation { properties: "x,y"; duration: 260; easing.type: Easing.OutQuint } }

      delegate: ModuleRow {
        id: row
        required property int index
        required property string address
        required property string name
        required property bool connected
        required property bool paired
        required property int battery
        required property string icon
        required property bool busy
        readonly property string action: bt.pending[address] || ""
        readonly property bool showActions: bt.actionsOpen && isSelected
        host: bt.host
        rowHeight: bt.rowHeight
        width: list.width
        isSelected: bt.selected === index
        glyph: bt.deviceGlyph(icon)
        title: name
        strong: connected
        subtitle: action === "connecting" || busy ? "Connecting…" : action === "disconnecting" ? "Disconnecting…"
          : action === "forgetting" ? "Forgetting…" : connected ? "Connected" : paired ? "Not connected" : "Not paired"
        subtitleAccent: connected || action !== "" || busy
        trailing: bt.batteryText(battery)
        extraHeight: showActions ? 40 : 0
        onHovered: if (!bt.actionsOpen) bt.selected = index
        onClicked: function(button) {
          bt.selected = index
          if (button === Qt.RightButton) { bt.actionIndex = 0; bt.confirmForget = false; bt.actionsOpen = true }
          else bt.activate(index)
        }
        Row {
          visible: row.showActions
          y: 4
          spacing: 6
          Repeater {
            model: row.showActions ? bt.currentActions : []
            delegate: PillButton {
              required property var modelData
              required property int index
              host: bt.host
              glyph: modelData.glyph
              label: modelData.label
              danger: modelData.danger === true
              highlighted: bt.actionIndex === index
              onClicked: { bt.actionIndex = index; bt.runRowAction(modelData.id) }
            }
          }
        }
      }
    }

    ModuleFooter {
      host: bt.host
      notice: bt.notice
      noticeError: bt.noticeError
      onAdvancedClicked: bt.advancedRequested()
    }
  }
}
