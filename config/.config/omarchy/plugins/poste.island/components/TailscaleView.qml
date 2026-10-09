import QtQuick
import Quickshell
import "file:///usr/share/omarchy/shell/plugins/panels/tailscale" as TsPanel

// Tailscale, grown inside the island from the control center's tile. No
// service of its own: it drives the Service of Omarchy's Tailscale widget
// (host.tsService, mounted once by the island), like VpnView with Proton.
// Receiving Taildrop files is Omarchy's omarchy-tailscale-receive service;
// sending goes through omarchy-tailscale-send (file chooser, notification).
// Keys: ↑↓ move, Enter sends files to the machine, C copies its IP,
// T turns Tailscale on or off, Esc goes back.
Item {
  id: ts
  required property var host
  property bool active: false
  signal backRequested()
  signal advancedRequested()
  signal closeRequested()

  implicitHeight: column.implicitHeight
  property real maxListHeight: 300
  readonly property real rowHeight: 46

  readonly property var svc: host.tsService
  readonly property bool installed: !!svc && svc.installed === true
  readonly property bool needsLogin: installed && svc.needsLogin === true
  readonly property bool connected: !!svc && svc.active === true
  readonly property bool busy: !!svc && svc.busy === true
  readonly property var peers: connected && svc.peers ? svc.peers : []

  property int selected: 0
  property string notice: ""
  onPeersChanged: if (selected >= peers.length) selected = Math.max(0, peers.length - 1)

  onActiveChanged: {
    if (!active) return
    selected = 0
    notice = ""
    if (svc) svc.refresh(false)
    Qt.callLater(function() { ts.forceActiveFocus() })
  }

  function peerIp(peer) {
    var ips = peer && peer.TailscaleIPs ? peer.TailscaleIPs : []
    return ips.length ? String(ips[0]) : ""
  }
  function canSend(peer) { return !!svc && svc.canSendFiles(peer) }
  // The machine's name in the admin console (its MagicDNS name), not the
  // hostname the device reports ("home", "Kris").
  function peerName(peer) { return peer ? (svc.shortDnsName(peer.DNSName) || peer.DisplayName) : "" }
  function sendTo(index) {
    var peer = peers[index]
    if (!peer) return
    if (!canSend(peer)) { say(svc.fileSharing ? peerName(peer) + " can't receive files" : "Taildrop is off for this tailnet"); return }
    svc.sendFile(peer)
    closeRequested()
  }
  function copyIp(index) {
    var peer = peers[index]
    if (!peer || peerIp(peer) === "") return
    svc.copyPeerIp(peer)
    say("Copied " + peerIp(peer))
  }
  function toggle() {
    if (installed && !busy) svc.toggleTailscale()
  }
  function say(text) {
    notice = text
    noticeTimer.restart()
  }
  Timer { id: noticeTimer; interval: 3000; onTriggered: ts.notice = "" }

  Keys.onPressed: function(event) {
    var k = event.key
    if (k === Qt.Key_Escape || k === Qt.Key_Backspace) ts.backRequested()
    else if (k === Qt.Key_Down || k === Qt.Key_J) { selected = Math.min(peers.length - 1, selected + 1); list.positionViewAtIndex(selected, ListView.Contain) }
    else if (k === Qt.Key_Up || k === Qt.Key_K) { selected = Math.max(0, selected - 1); list.positionViewAtIndex(selected, ListView.Contain) }
    else if (k === Qt.Key_Return || k === Qt.Key_Enter) sendTo(selected)
    else if (k === Qt.Key_C) copyIp(selected)
    else if (k === Qt.Key_T) toggle()
    else return
    event.accepted = true
  }

  Column {
    id: column
    anchors.left: parent.left
    anchors.right: parent.right
    spacing: 12

    ModuleHeader {
      host: ts.host
      title: "Tailscale"
      status: ts.busy && ts.svc.actionStatus !== "" ? ts.svc.actionStatus : ""
      showSwitch: ts.installed && !ts.needsLogin
      switchOn: ts.connected
      onBackClicked: ts.backRequested()
      onSwitchClicked: ts.toggle()
    }

    // Status: this machine.
    Item {
      width: parent.width
      height: 44
      TsPanel.TailscaleIcon {
        id: mark
        anchors.left: parent.left
        anchors.leftMargin: 5
        anchors.verticalCenter: parent.verticalCenter
        iconSize: 20
        color: ts.connected ? ts.host.colorAccent : ts.host.colorMuted
      }
      Column {
        anchors.left: mark.right
        anchors.leftMargin: 14
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        spacing: 2
        Text {
          width: parent.width
          elide: Text.ElideRight
          text: !ts.svc ? "Tailscale" : !ts.installed ? "Tailscale is not installed"
            : ts.needsLogin ? "Signed out" : ts.connected ? "Connected" : "Off"
          color: ts.host.colorText
          font.family: ts.host.uiFont
          font.pixelSize: 14
          font.weight: Font.DemiBold
        }
        Text {
          width: parent.width
          elide: Text.ElideRight
          visible: text !== ""
          text: ts.connected ? [ts.svc.shortDnsName(ts.svc.selfDnsName) || ts.svc.selfName, ts.svc.selfIp, ts.svc.selectedAccountLabel].filter(function(s) { return !!s }).join("  ·  ")
            : ts.needsLogin ? "Authorize this PC to join the tailnet" : ""
          color: ts.host.colorMuted
          font.family: ts.host.uiFont
          font.pixelSize: 11
        }
      }
    }

    Row {
      visible: ts.needsLogin
      spacing: 6
      PillButton {
        host: ts.host
        accent: true
        glyph: "󰍂"
        label: "Sign in"
        onClicked: ts.svc.loginOrUp()
      }
    }

    Rectangle { visible: ts.connected; width: parent.width; height: 1; color: ts.host.withAlpha(ts.host.colorText, 0.08) }

    // Machines online. Enter / click: send files; right click / C: copy the IP.
    Text {
      visible: ts.connected && ts.peers.length === 0
      text: "No other machine online"
      color: ts.host.colorMuted
      font.family: ts.host.uiFont
      font.pixelSize: 12
    }
    ListView {
      id: list
      visible: ts.peers.length > 0
      width: parent.width
      height: Math.min(ts.maxListHeight, contentHeight)
      clip: true
      spacing: 2
      interactive: contentHeight > height
      boundsBehavior: Flickable.StopAtBounds
      model: ts.peers
      delegate: ModuleRow {
        required property var modelData
        required property int index
        host: ts.host
        rowHeight: ts.rowHeight
        width: list.width
        isSelected: ts.selected === index
        glyph: ts.svc ? ts.svc.osIcon(modelData.OS) : ""
        title: ts.peerName(modelData)
        subtitle: ts.peerIp(modelData) + (modelData.ExitNode ? "  ·  exit node" : "")
        trailing: ts.canSend(modelData) ? "󰒊" : ""
        onHovered: ts.selected = index
        onClicked: function(button) {
          ts.selected = index
          if (button === Qt.RightButton) ts.copyIp(index)
          else ts.sendTo(index)
        }
      }
    }

    ModuleFooter {
      host: ts.host
      notice: ts.notice !== "" ? ts.notice : ts.svc ? (ts.svc.lastError !== "" ? ts.svc.lastError : ts.svc.actionStatus) : ""
      noticeError: ts.notice === "" && !!ts.svc && ts.svc.lastError !== ""
      onAdvancedClicked: ts.advancedRequested()
    }
  }
}
