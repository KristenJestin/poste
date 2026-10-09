import QtQuick
import Quickshell
import Quickshell.Io

// Wi-Fi, grown inside the island from the control center's « Réseau » tile
// (macOS Control Center style). Data and actions go through bin/poste-wifi
// (nmcli). Keys: ↑↓ move, Enter connects, Ctrl+B row actions, Esc / ← back
// to the control center. « Réglages avancés… » opens Omarchy's own panel.
Item {
  id: wifi
  required property var host
  property bool active: false
  signal backRequested()
  signal advancedRequested()

  implicitHeight: column.implicitHeight

  readonly property string script: Quickshell.env("HOME") + "/.config/omarchy/plugins/poste.island/bin/poste-wifi"
  readonly property real rowHeight: 46
  property real maxListHeight: 360

  // ---------------------------------------------------------------- state
  property bool radio: true
  property string ip: ""
  property var ethernet: null
  property bool loaded: false
  property bool scanning: false
  property int selected: 0
  property bool actionsOpen: false
  property int actionIndex: 0
  property bool confirmForget: false
  property string passwordFor: ""
  property string busySsid: ""
  property string notice: ""
  property bool noticeError: false

  // Bumped after every sync: ListModel.get() alone does not notify bindings.
  property int rev: 0
  readonly property var current: { rev; return selected < rows.count ? rows.get(selected) : null }
  readonly property var connected: {
    rev
    for (var i = 0; i < rows.count; i++) if (rows.get(i).inUse) return rows.get(i)
    return null
  }

  onActiveChanged: {
    if (active) {
      selected = 0
      actionsOpen = false
      confirmForget = false
      passwordFor = ""
      refresh(false)
      rescanSoon.restart()
      Qt.callLater(function() { wifi.forceActiveFocus() })
    }
  }

  ListModel { id: rows }
  // Keyed by SSID so a refresh moves/updates rows instead of rebuilding them.
  function sync(list) {
    var keys = {}
    for (var i = 0; i < list.length; i++) keys[list[i].ssid] = true
    for (var j = rows.count - 1; j >= 0; j--) if (!keys[rows.get(j).ssid]) rows.remove(j)
    for (var k = 0; k < list.length; k++) {
      var n = list[k]
      var item = { ssid: n.ssid, strength: n.signal, security: n.security || "", inUse: n.inUse === true,
                   known: n.known === true, uuid: n.uuid || "" }
      var found = -1
      for (var m = k; m < rows.count; m++) if (rows.get(m).ssid === n.ssid) { found = m; break }
      if (found === -1) rows.insert(k, item)
      else {
        if (found !== k) rows.move(found, k, 1)
        rows.set(k, item)
      }
    }
    while (rows.count > list.length) rows.remove(rows.count - 1)
    if (selected >= rows.count) selected = Math.max(0, rows.count - 1)
    rev++
  }

  property bool pendingRescan: false
  function refresh(rescan) {
    if (stateProc.running) { if (rescan) pendingRescan = true; return }
    scanning = rescan
    stateProc.command = [script, "state", rescan ? "rescan" : "no"]
    stateProc.running = true
  }
  Process {
    id: stateProc
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        try {
          var st = JSON.parse(text)
          wifi.radio = st.radio === true
          wifi.ip = st.ip || ""
          wifi.ethernet = st.ethernet
          wifi.sync(st.networks || [])
          wifi.loaded = true
        } catch (e) {}
      }
    }
    onExited: {
      wifi.scanning = false
      if (wifi.pendingRescan) { wifi.pendingRescan = false; wifi.refresh(true) }
    }
  }
  // A rescan takes ~2 s: show the cached list first, then the fresh one.
  Timer { id: rescanSoon; interval: 150; onTriggered: wifi.refresh(true) }
  Timer { interval: 10000; repeat: true; running: wifi.active; onTriggered: wifi.refresh(true) }

  // ---------------------------------------------------------------- actions
  property string pendingSecret: ""
  property string actionKind: ""
  property string actionSsid: ""
  function runAction(kind, ssid, args, secret) {
    if (actionProc.running) return
    actionKind = kind
    actionSsid = ssid
    busySsid = kind === "up" || kind === "connect" ? ssid : ""
    notice = kind === "up" || kind === "connect" ? "Connecting to " + ssid + "…"
      : kind === "down" ? "Disconnecting…" : kind === "forget" ? "Forgetting…" : ""
    noticeError = false
    pendingSecret = secret || ""
    actionProc.command = [script].concat(args)
    actionProc.running = true
  }
  Process {
    id: actionProc
    stdinEnabled: true
    onStarted: {
      write(wifi.pendingSecret + "\n")
      wifi.pendingSecret = ""
    }
    stdout: StdioCollector { id: actionOut; waitForEnd: true }
    onExited: function(code) {
      var k = wifi.actionKind, s = wifi.actionSsid
      wifi.busySsid = ""
      wifi.noticeError = code !== 0
      if (code === 0) {
        wifi.notice = k === "up" || k === "connect" ? "Connected to " + s
          : k === "down" ? "Disconnected from " + s : k === "forget" ? "Forgot " + s : ""
      } else {
        var last = String(actionOut.text || "").trim().split("\n").pop()
        wifi.notice = "Failed: " + (last.replace(/^Error:\s*/, "") || "unknown error")
      }
      noticeClear.restart()
      wifi.refresh(false)
    }
  }
  Timer { id: noticeClear; interval: 6000; onTriggered: if (!actionProc.running) wifi.notice = "" }

  function activate(index) {
    var n = index < rows.count ? rows.get(index) : null
    if (!n || n.inUse || actionProc.running) return
    if (n.known) runAction("up", n.ssid, ["up", n.uuid])
    else if (n.security !== "" && n.security !== "--") { passwordFor = n.ssid; actionsOpen = false }
    else runAction("connect", n.ssid, ["connect", n.ssid, ""])
  }
  function submitPassword(text) {
    var n = current
    if (!n || text === "") return
    passwordFor = ""
    runAction("connect", n.ssid, ["connect", n.ssid, n.security], text)
    wifi.forceActiveFocus()
  }
  function toggleRadio() {
    if (actionProc.running) return
    runAction("radio", "", ["radio", radio ? "off" : "on"])
    radio = !radio
  }

  readonly property var currentActions: {
    var n = current
    if (!n) return []
    var list = []
    if (n.inUse && n.known) list.push({ id: "down", label: "Disconnect", glyph: "󰖪" })
    if (n.known) list.push({ id: "forget", label: confirmForget ? "Confirm forget" : "Forget", glyph: "󰆴", danger: confirmForget })
    list.push({ id: "copy", label: "Copy SSID", glyph: "󰆏" })
    return list
  }
  function runRowAction(id) {
    var n = current
    if (!n) return
    if (id === "down") { actionsOpen = false; runAction("down", n.ssid, ["down", n.uuid]) }
    else if (id === "forget") {
      if (!confirmForget) { confirmForget = true; return }
      confirmForget = false; actionsOpen = false
      runAction("forget", n.ssid, ["forget", n.uuid])
    } else if (id === "copy") {
      Quickshell.execDetached(["wl-copy", "--", n.ssid])
      actionsOpen = false
      notice = "SSID copied"; noticeError = false; noticeClear.restart()
    }
  }

  function signalGlyph(s) {
    return s >= 80 ? "󰤨" : s >= 60 ? "󰤥" : s >= 40 ? "󰤢" : s >= 20 ? "󰤟" : "󰤯"
  }
  function secured(n) { return n && n.security !== "" && n.security !== "--" }

  // ---------------------------------------------------------------- keys
  Keys.onPressed: function(event) {
    var ctrl = event.modifiers & Qt.ControlModifier
    if (actionsOpen) {
      if (event.key === Qt.Key_Escape || (ctrl && event.key === Qt.Key_B)) { actionsOpen = false; confirmForget = false }
      else if (event.key === Qt.Key_Left) { actionIndex = Math.max(0, actionIndex - 1); confirmForget = false }
      else if (event.key === Qt.Key_Right) { actionIndex = Math.min(currentActions.length - 1, actionIndex + 1); confirmForget = false }
      else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
        var act = currentActions[actionIndex]
        if (act) runRowAction(act.id)
      } else return
      event.accepted = true
      return
    }
    if (event.key === Qt.Key_Escape || event.key === Qt.Key_Left || event.key === Qt.Key_Backspace) wifi.backRequested()
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
      host: wifi.host
      title: "Wi-Fi"
      status: wifi.scanning ? "Scanning…" : ""
      showSwitch: true
      switchOn: wifi.radio
      onBackClicked: wifi.backRequested()
      onSwitchClicked: wifi.toggleRadio()
    }

    // Current connection.
    Item {
      width: parent.width
      height: 40
      Glyph {
        id: bigGlyph
        anchors.left: parent.left
        anchors.leftMargin: 4
        anchors.verticalCenter: parent.verticalCenter
        glyph: !wifi.radio ? "󰤮" : wifi.connected ? wifi.signalGlyph(wifi.connected.strength) : "󰤯"
        color: wifi.connected ? wifi.host.colorText : wifi.host.colorMuted
        fontFamily: wifi.host.fontFamily
        pixelSize: 22
      }
      Column {
        anchors.left: bigGlyph.right
        anchors.leftMargin: 14
        anchors.verticalCenter: parent.verticalCenter
        spacing: 2
        Text {
          text: !wifi.radio ? "Wi-Fi is off" : wifi.connected ? wifi.connected.ssid : wifi.loaded ? "Not connected" : "…"
          color: wifi.host.colorText
          font.family: wifi.host.uiFont
          font.pixelSize: 14
          font.weight: Font.DemiBold
        }
        Text {
          visible: wifi.radio && !!wifi.connected
          text: (wifi.connected ? wifi.connected.security.replace(/\s+/g, " ") + "  ·  " : "") + (wifi.ip !== "" ? wifi.ip : "no address")
          color: wifi.host.colorMuted
          font.family: wifi.host.uiFont
          font.pixelSize: 11
        }
      }
    }

    // Wired, when a cable is plugged in.
    Rectangle {
      visible: !!wifi.ethernet
      width: parent.width
      height: 36
      radius: 12
      color: wifi.host.withAlpha(wifi.host.colorText, 0.05)
      Row {
        anchors.left: parent.left
        anchors.leftMargin: 12
        anchors.verticalCenter: parent.verticalCenter
        spacing: 10
        Glyph { glyph: "󰈀"; color: wifi.host.colorText; fontFamily: wifi.host.fontFamily; pixelSize: 15; anchors.verticalCenter: parent.verticalCenter }
        Text {
          text: "Ethernet  ·  " + (wifi.ethernet ? wifi.ethernet.name : "")
          color: wifi.host.colorText; font.family: wifi.host.uiFont; font.pixelSize: 12
          anchors.verticalCenter: parent.verticalCenter
        }
      }
    }

    Rectangle { width: parent.width; height: 1; color: wifi.host.withAlpha(wifi.host.colorText, 0.08) }

    Text {
      visible: wifi.radio && wifi.loaded && rows.count === 0
      width: parent.width
      horizontalAlignment: Text.AlignHCenter
      text: "No networks found"
      color: wifi.host.colorMuted
      font.family: wifi.host.uiFont
      font.pixelSize: 12
    }

    ListView {
      id: list
      visible: wifi.radio
      width: parent.width
      height: Math.min(wifi.maxListHeight, contentHeight)
      clip: true
      spacing: 4
      interactive: contentHeight > height
      boundsBehavior: Flickable.StopAtBounds
      model: rows
      add: Transition {
        ParallelAnimation {
          NumberAnimation { property: "opacity"; from: 0; to: 1; duration: 200; easing.type: Easing.OutCubic }
          NumberAnimation { property: "x"; from: 8; to: 0; duration: 240; easing.type: Easing.OutQuint }
        }
      }
      remove: Transition { NumberAnimation { property: "opacity"; to: 0; duration: 150 } }
      displaced: Transition { NumberAnimation { properties: "x,y"; duration: 260; easing.type: Easing.OutQuint } }
      move: Transition { NumberAnimation { properties: "y"; duration: 260; easing.type: Easing.OutQuint } }

      delegate: ModuleRow {
        id: row
        required property int index
        required property string ssid
        required property int strength
        required property string security
        required property bool inUse
        required property bool known
        required property string uuid
        readonly property bool askPassword: wifi.passwordFor === ssid && isSelected
        readonly property bool showActions: wifi.actionsOpen && isSelected
        readonly property bool busy: wifi.busySsid === ssid
        host: wifi.host
        rowHeight: wifi.rowHeight
        width: list.width
        isSelected: wifi.selected === index
        glyph: wifi.signalGlyph(strength)
        title: ssid
        strong: inUse
        subtitle: busy ? "Connecting…" : inUse ? "Connected" : known ? "Saved" : wifi.secured(row) ? security : "Open"
        subtitleAccent: busy || inUse
        trailing: wifi.secured(row) ? "󰌾" : ""
        extraHeight: (askPassword ? 44 : 0) + (showActions ? 40 : 0)
        onHovered: if (!wifi.actionsOpen && wifi.passwordFor === "") wifi.selected = index
        onClicked: function(button) {
          wifi.selected = index
          if (button === Qt.RightButton) { wifi.actionIndex = 0; wifi.confirmForget = false; wifi.actionsOpen = true }
          else wifi.activate(index)
        }

        // Inline passphrase.
        Rectangle {
          visible: row.askPassword
          width: parent.width
          height: 34
          radius: 10
          color: wifi.host.withAlpha(wifi.host.colorText, 0.06)
          border.width: 1
          border.color: wifi.host.withAlpha(wifi.host.colorText, 0.1)
          TextInput {
            id: pass
            anchors.fill: parent
            anchors.leftMargin: 12
            anchors.rightMargin: 12
            verticalAlignment: TextInput.AlignVCenter
            echoMode: TextInput.Password
            passwordCharacter: "•"
            color: wifi.host.colorText
            selectionColor: wifi.host.withAlpha(wifi.host.colorAccent, 0.4)
            font.family: wifi.host.uiFont
            font.pixelSize: 13
            clip: true
            onVisibleChanged: { text = ""; if (visible) Qt.callLater(function() { pass.forceActiveFocus() }) }
            Keys.onPressed: function(event) {
              if (event.key === Qt.Key_Escape) { wifi.passwordFor = ""; text = ""; wifi.forceActiveFocus(); event.accepted = true }
              else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) { var t = text; text = ""; wifi.submitPassword(t); event.accepted = true }
            }
            Text {
              anchors.verticalCenter: parent.verticalCenter
              visible: pass.text === ""
              text: "Password  ·  Enter to connect"
              color: wifi.host.colorMuted
              font: pass.font
            }
          }
        }

        // Ctrl+B actions.
        Row {
          visible: row.showActions
          y: (row.askPassword ? 44 : 0) + 4
          spacing: 6
          Repeater {
            model: row.showActions ? wifi.currentActions : []
            delegate: PillButton {
              required property var modelData
              required property int index
              host: wifi.host
              glyph: modelData.glyph
              label: modelData.label
              danger: modelData.danger === true
              highlighted: wifi.actionIndex === index
              onClicked: { wifi.actionIndex = index; wifi.runRowAction(modelData.id) }
            }
          }
        }
      }
    }

    ModuleFooter {
      host: wifi.host
      notice: wifi.notice
      noticeError: wifi.noticeError
      onAdvancedClicked: wifi.advancedRequested()
    }
  }
}
