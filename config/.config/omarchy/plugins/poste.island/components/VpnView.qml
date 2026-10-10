import QtQuick
import Quickshell

// Proton VPN, grown inside the island from the control center's tile. No
// service of its own: it drives the OmaProton plugin's Service (host.vpnService,
// mounted once by the island), so state, notifications and the CLI queue stay
// in one place. Keys: type to search countries, ↑↓ move, Enter connects,
// Esc clears the search then goes back.
Item {
  id: vpn
  required property var host
  property bool active: false
  signal backRequested()
  signal advancedRequested()

  implicitHeight: column.implicitHeight
  property real maxListHeight: 300
  readonly property real rowHeight: 40

  readonly property var svc: host.vpnService
  readonly property bool installed: !!svc && svc.installed === true
  readonly property bool signedOut: installed && svc.accountProbed && !svc.signedIn && !svc.connected
  readonly property bool ready: installed && !signedOut
  readonly property bool connected: !!svc && svc.connected === true
  readonly property bool busy: !!svc && svc.busy === true

  property int selected: 0
  readonly property var countries: {
    var list = svc && svc.countries ? svc.countries : []
    var q = search.text.trim().toLowerCase()
    if (q === "") return list
    return list.filter(function(c) { return c.name.toLowerCase().indexOf(q) !== -1 || c.code.toLowerCase() === q })
  }
  onCountriesChanged: selected = 0

  onActiveChanged: {
    if (!active) return
    search.text = ""
    selected = 0
    if (svc) {
      svc.refresh()
      svc.loadCountries(false)
      svc.loadConfig()
    }
    Qt.callLater(function() { search.forceActiveFocus() })
  }

  readonly property string ipText: {
    var f = svc && svc.fields ? svc.fields : []
    for (var i = 0; i < f.length; i++) if (/\bip\b/i.test(String(f[i].label))) return String(f[i].value)
    return ""
  }
  function toggle() {
    if (!ready || busy) return
    if (connected) svc.disconnect()
    else svc.connectFastest()
  }
  function connectCountry(index) {
    var c = countries[index]
    if (c && ready && !busy) svc.connectCountry(c.code, c.name)
  }

  function handleKey(event) {
    if (event.key === Qt.Key_Escape) {
      if (search.text !== "") search.text = ""
      else vpn.backRequested()
    } else if (event.key === Qt.Key_Backspace && search.text === "") vpn.backRequested()
    else if (event.key === Qt.Key_Down) { selected = Math.min(countries.length - 1, selected + 1); list.positionViewAtIndex(selected, ListView.Contain) }
    else if (event.key === Qt.Key_Up) { selected = Math.max(0, selected - 1); list.positionViewAtIndex(selected, ListView.Contain) }
    else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) connectCountry(selected)
    else return
    event.accepted = true
  }
  Keys.onPressed: function(event) { handleKey(event) }

  Column {
    id: column
    anchors.left: parent.left
    anchors.right: parent.right
    spacing: 12

    ModuleHeader {
      host: vpn.host
      title: "VPN"
      status: vpn.busy ? (vpn.svc.pendingLabel || "Working…") : ""
      showSwitch: vpn.ready
      switchOn: vpn.connected
      onBackClicked: vpn.backRequested()
      onSwitchClicked: vpn.toggle()
    }

    // Status.
    Item {
      width: parent.width
      height: 44
      Glyph {
        id: shield
        anchors.left: parent.left
        anchors.leftMargin: 4
        anchors.verticalCenter: parent.verticalCenter
        glyph: vpn.connected ? "󰒘" : "󰦞"
        color: vpn.connected ? vpn.host.colorAccent : vpn.host.colorMuted
        fontFamily: vpn.host.fontFamily
        pixelSize: 22
      }
      Column {
        anchors.left: shield.right
        anchors.leftMargin: 14
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        spacing: 2
        Text {
          width: parent.width
          elide: Text.ElideRight
          text: !vpn.svc ? "Proton VPN" : !vpn.installed ? "Proton VPN is not installed"
            : vpn.connected ? "Protected" : vpn.signedOut ? "Signed out" : vpn.svc.displayStatus
          color: vpn.host.colorText
          font.family: vpn.host.uiFont
          font.pixelSize: 14
          font.weight: Font.DemiBold
        }
        Text {
          width: parent.width
          elide: Text.ElideRight
          visible: text !== ""
          text: vpn.connected ? [vpn.svc.displayServer, vpn.svc.location, vpn.ipText].filter(function(s) { return !!s }).join("  ·  ")
            : vpn.ready && vpn.svc.account !== "" ? vpn.svc.account + (vpn.svc.plan !== "" ? "  ·  " + vpn.svc.plan : "") : ""
          color: vpn.host.colorMuted
          font.family: vpn.host.uiFont
          font.pixelSize: 11
        }
      }
    }

    // Main actions.
    Row {
      spacing: 6
      PillButton {
        visible: !!vpn.svc && !vpn.installed
        host: vpn.host
        accent: true
        glyph: "󰇚"
        label: "Install Proton VPN"
        onClicked: vpn.svc.installCli()
      }
      PillButton {
        visible: vpn.signedOut
        host: vpn.host
        accent: true
        glyph: "󰍂"
        label: "Sign in"
        onClicked: vpn.svc.signIn("")
      }
      PillButton {
        visible: vpn.ready
        host: vpn.host
        accent: !vpn.connected
        glyph: vpn.connected ? "󰖂" : "󰓅"
        label: vpn.connected ? "Disconnect" : "Connect (fastest)"
        onClicked: vpn.toggle()
      }
      PillButton {
        visible: vpn.ready && vpn.svc.configLoaded
        host: vpn.host
        glyph: "󰒃"
        label: "Kill switch: " + (vpn.svc && vpn.svc.killSwitchOn ? "on" : "off")
        onClicked: vpn.svc.toggleKillSwitch()
      }
    }

    Rectangle { visible: vpn.ready; width: parent.width; height: 1; color: vpn.host.withAlpha(vpn.host.colorText, 0.08) }

    // Country search.
    Rectangle {
      visible: vpn.ready
      width: parent.width
      height: 34
      radius: 10
      color: vpn.host.withAlpha(vpn.host.colorText, 0.06)
      Glyph {
        id: searchGlyph
        anchors.left: parent.left
        anchors.leftMargin: 12
        anchors.verticalCenter: parent.verticalCenter
        glyph: "󰍉"
        color: vpn.host.colorMuted
        fontFamily: vpn.host.fontFamily
        pixelSize: 13
      }
      TextInput {
        id: search
        anchors.left: searchGlyph.right
        anchors.leftMargin: 10
        anchors.right: parent.right
        anchors.rightMargin: 12
        anchors.verticalCenter: parent.verticalCenter
        color: vpn.host.colorText
        selectionColor: vpn.host.withAlpha(vpn.host.colorAccent, 0.4)
        font.family: vpn.host.uiFont
        font.pixelSize: 13
        clip: true
        Keys.onPressed: function(event) { vpn.handleKey(event) }
        Text {
          anchors.verticalCenter: parent.verticalCenter
          visible: search.text === ""
          text: vpn.svc && vpn.svc.countriesLoaded ? "Search " + vpn.svc.countries.length + " countries" : "Loading countries…"
          color: vpn.host.colorMuted
          font: search.font
        }
      }
    }

    ListView {
      WheelScroll {}
      id: list
      visible: vpn.ready && vpn.countries.length > 0
      width: parent.width
      height: Math.min(vpn.maxListHeight, contentHeight)
      clip: true
      spacing: 2
      interactive: contentHeight > height
      boundsBehavior: Flickable.StopAtBounds
      model: vpn.countries
      delegate: ModuleRow {
        required property var modelData
        required property int index
        host: vpn.host
        rowHeight: vpn.rowHeight
        width: list.width
        isSelected: vpn.selected === index
        glyph: modelData.code
        title: modelData.name
        strong: vpn.connected && vpn.svc.location !== "" && vpn.svc.location.indexOf(modelData.name) !== -1
        trailing: "󰁔"
        onHovered: vpn.selected = index
        onClicked: vpn.connectCountry(index)
      }
    }

    ModuleFooter {
      host: vpn.host
      notice: vpn.svc ? (vpn.svc.lastError !== "" ? vpn.svc.lastError : vpn.svc.actionStatus) : ""
      noticeError: !!vpn.svc && vpn.svc.lastError !== ""
      onAdvancedClicked: vpn.advancedRequested()
    }
  }
}
