import QtQuick
import Quickshell
import Quickshell.Services.SystemTray
import Quickshell.Widgets
import "file:///usr/share/omarchy/shell/plugins/panels/tailscale" as TsPanel
import "AgentBrand.js" as AgentBrand

// Opened from the clock: a month calendar, shortcuts to Omarchy's own
// network / bluetooth / audio panels, and the system tray.
Item {
  id: cc
  required property var host
  required property string screenName
  property bool active: false
  signal closeRequested()
  signal moduleRequested(string name)

  implicitHeight: column.implicitHeight

  readonly property var locale: Qt.locale("en_GB")
  // Réseau / Bluetooth / Audio, plus VPN, Tailscale and Mail when present.
  readonly property int tileCount: 3 + (host.mailAvailable ? 1 : 0) + ((host.vpnAvailable || host.vpnConnected) ? 1 : 0)
    + (host.tsAvailable ? 1 : 0)
  readonly property real tileWidth: Math.min(74, Math.floor((width - 8 * (tileCount - 1)) / tileCount))
  property int monthOffset: 0

  // Tray menu shown in the panel (see trayMenuView). Each submenu level keeps its own
  // opener: an entry belongs to its parent's model, so the stack is torn down deepest first.
  property var trayMenuItem: null
  property var trayMenuStack: []
  QsMenuOpener {
    id: trayMenuOpener
    menu: cc.trayMenuItem ? cc.trayMenuItem.menu : null
  }
  Component {
    id: trayOpenerComponent
    QsMenuOpener {}
  }
  function resetTraySubmenus() {
    var stack = trayMenuStack
    trayMenuStack = []
    for (var i = stack.length - 1; i >= 0; i--) stack[i].opener.destroy()
  }
  function openTrayMenu(item) {
    var same = trayMenuItem === item
    resetTraySubmenus()
    trayMenuItem = same ? null : item
  }
  function closeTrayMenu() {
    resetTraySubmenus()
    trayMenuItem = null
  }
  function enterTraySubmenu(entry, title) {
    var opener = trayOpenerComponent.createObject(cc, { menu: entry })
    if (!opener) return
    trayMenuStack = trayMenuStack.concat([{ opener: opener, title: title }])
  }
  function leaveTraySubmenu() {
    if (trayMenuStack.length === 0) return
    var stack = trayMenuStack.slice()
    var top = stack.pop()
    trayMenuStack = stack
    top.opener.destroy()
  }

  onActiveChanged: {
    if (!active) closeTrayMenu()
    if (active) host.runAgentUpdate("limits")
    if (!active) return
    monthOffset = 0
    Qt.callLater(function() { cc.forceActiveFocus() })
  }
  Keys.onPressed: function(event) {
    if (event.key === Qt.Key_Escape) {
      if (cc.trayMenuStack.length > 0) cc.leaveTraySubmenu()
      else if (cc.trayMenuItem) cc.closeTrayMenu()
      else cc.closeRequested()
      event.accepted = true
    }
    else if (event.key === Qt.Key_Left) { cc.monthOffset--; event.accepted = true }
    else if (event.key === Qt.Key_Right) { cc.monthOffset++; event.accepted = true }
  }

  readonly property date today: host.clockDate
  readonly property date shownMonth: new Date(today.getFullYear(), today.getMonth() + monthOffset, 1)
  function capitalize(text) { return text.charAt(0).toUpperCase() + text.slice(1) }

  // 42 cells, weeks starting on Monday.
  readonly property var cells: {
    var y = shownMonth.getFullYear(), m = shownMonth.getMonth()
    var lead = (new Date(y, m, 1).getDay() + 6) % 7
    var list = []
    for (var i = 0; i < 42; i++) {
      var d = new Date(y, m, 1 - lead + i)
      list.push({
        day: d.getDate(),
        inMonth: d.getMonth() === m,
        isToday: d.getFullYear() === today.getFullYear() && d.getMonth() === today.getMonth() && d.getDate() === today.getDate()
      })
    }
    return list
  }

  component IconButton: Item {
    id: button
    property string glyph: ""
    property string label: ""
    property int badge: 0
    property bool tailscaleMark: false   // Tailscale's own mark instead of a glyph
    property bool markOn: false
    signal activated()
    width: label ? cc.tileWidth : 28
    height: label ? 56 : 28
    Rectangle {
      anchors.fill: parent
      radius: button.label ? 14 : 14
      color: mouse.containsMouse ? cc.host.withAlpha(cc.host.colorText, 0.1)
        : button.label ? cc.host.withAlpha(cc.host.colorText, 0.05) : "transparent"
      Behavior on color { ColorAnimation { duration: 120 } }
    }
    Column {
      anchors.centerIn: parent
      spacing: 3
      Glyph {
        anchors.horizontalCenter: parent.horizontalCenter
        visible: !button.tailscaleMark
        width: implicitWidth; height: 22
        glyph: button.glyph
        color: cc.host.colorText
        fontFamily: cc.host.fontFamily
        pixelSize: button.label ? 19 : 16
      }
      Item {
        anchors.horizontalCenter: parent.horizontalCenter
        visible: button.tailscaleMark
        width: 22; height: 22
        TsPanel.TailscaleIcon {
          anchors.centerIn: parent
          iconSize: 16
          color: button.markOn ? cc.host.colorText : cc.host.withAlpha(cc.host.colorText, 0.45)
        }
      }
      Text {
        anchors.horizontalCenter: parent.horizontalCenter
        visible: button.label !== ""
        text: button.label
        color: cc.host.withAlpha(cc.host.colorText, 0.7)
        font.family: cc.host.uiFont
        font.pixelSize: 11
      }
    }
    Rectangle {
      visible: button.badge > 0
      anchors.top: parent.top
      anchors.right: parent.right
      anchors.margins: 6
      height: 15
      width: Math.max(15, badgeText.implicitWidth + 8)
      radius: 7.5
      color: cc.host.withAlpha(cc.host.colorText, 0.16)
      Text {
        id: badgeText
        anchors.centerIn: parent
        text: button.badge > 99 ? "99+" : button.badge
        color: cc.host.colorText
        font.family: cc.host.uiFont
        font.pixelSize: 9
        font.weight: Font.DemiBold
      }
    }
    MouseArea {
      id: mouse
      anchors.fill: parent
      hoverEnabled: true
      cursorShape: Qt.PointingHandCursor
      onClicked: button.activated()
    }
  }

  Column {
    id: column
    anchors.left: parent.left
    anchors.right: parent.right
    spacing: 14

    // ---------------------------------------------------------------- calendar
    Item {
      width: parent.width
      height: 28
      IconButton {
        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        glyph: "󰅁"
        onActivated: cc.monthOffset--
      }
      Text {
        anchors.centerIn: parent
        text: cc.capitalize(cc.locale.standaloneMonthName(cc.shownMonth.getMonth(), Locale.LongFormat)) + " " + cc.shownMonth.getFullYear()
        color: cc.host.colorText
        font.family: cc.host.uiFont
        font.pixelSize: 15
        font.weight: Font.DemiBold
        MouseArea {
          anchors.fill: parent
          enabled: cc.monthOffset !== 0
          cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
          onClicked: cc.monthOffset = 0
        }
      }
      IconButton {
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        glyph: "󰅂"
        onActivated: cc.monthOffset++
      }
    }

    Grid {
      id: grid
      anchors.horizontalCenter: parent.horizontalCenter
      columns: 7
      columnSpacing: 4
      rowSpacing: 2
      readonly property int cell: Math.floor((column.width - 6 * columnSpacing) / 7)

      Repeater {
        model: 7
        delegate: Text {
          required property int index
          width: grid.cell
          height: 22
          horizontalAlignment: Text.AlignHCenter
          verticalAlignment: Text.AlignVCenter
          // Monday first: Qt's dayName takes 0 = Sunday.
          text: cc.capitalize(cc.locale.dayName((index + 1) % 7, Locale.ShortFormat)).replace(".", "").slice(0, 2)
          color: cc.host.colorMuted
          font.family: cc.host.uiFont
          font.pixelSize: 11
          font.weight: Font.DemiBold
        }
      }
      Repeater {
        model: cc.cells
        delegate: Item {
          required property var modelData
          width: grid.cell
          height: 30
          Rectangle {
            anchors.centerIn: parent
            width: 28; height: 28; radius: 14
            color: modelData.isToday ? cc.host.colorAccent : "transparent"
          }
          Text {
            anchors.centerIn: parent
            text: modelData.day
            color: modelData.isToday ? cc.host.colorAccentText
              : modelData.inMonth ? cc.host.colorText : cc.host.withAlpha(cc.host.colorText, 0.25)
            font.family: cc.host.uiFont
            font.pixelSize: 13
            font.weight: modelData.isToday ? Font.Bold : Font.Normal
            font.features: { "tnum": 1 }
          }
        }
      }
    }

    // ---------------------------------------------------------------- battery
    // Portables seulement (BatteryState.qml) : niveau, état, temps restant. Dessous, le
    // profil d'alimentation (PowerProfileState.qml) dès que power-profiles-daemon est là,
    // même sans batterie (PC fixe) : la section n'a alors que lui.
    Column {
      id: batterySection
      readonly property var bat: cc.host.battery
      readonly property var pp: cc.host.powerProfile
      width: parent.width
      spacing: 8
      visible: bat.present || pp.present
      Rectangle { width: parent.width; height: 1; color: cc.host.withAlpha(cc.host.colorText, 0.08) }
      Item {
        visible: batterySection.bat.present
        width: parent.width
        height: 20
        Glyph {
          id: batGlyph
          anchors.left: parent.left
          anchors.verticalCenter: parent.verticalCenter
          glyph: batterySection.bat.glyph
          color: batterySection.bat.charging || batterySection.bat.level !== "ok" ? batterySection.bat.color : cc.host.colorText
          fontFamily: cc.host.fontFamily
          pixelSize: 16
        }
        Text {
          anchors.left: batGlyph.right
          anchors.leftMargin: 8
          anchors.verticalCenter: parent.verticalCenter
          text: batterySection.bat.charging ? "Charging" : "Battery"
          color: cc.host.colorText
          font.family: cc.host.uiFont
          font.pixelSize: 13
          font.weight: Font.DemiBold
        }
        Text {
          anchors.right: parent.right
          anchors.verticalCenter: parent.verticalCenter
          text: Math.round(batterySection.bat.percent * 100) + " %" + (batterySection.bat.timeText() ? "  ·  " + batterySection.bat.timeText() : "")
          color: cc.host.colorMuted
          font.family: cc.host.uiFont
          font.pixelSize: 11
          font.features: { "tnum": 1 }
        }
      }
      Rectangle {
        visible: batterySection.bat.present
        width: parent.width
        height: 4
        radius: 2
        color: cc.host.withAlpha(cc.host.colorText, 0.1)
        Rectangle {
          width: Math.max(parent.height, parent.width * Math.max(0, Math.min(1, batterySection.bat.percent)))
          height: parent.height
          radius: 2
          color: batterySection.bat.charging || batterySection.bat.level !== "ok" ? batterySection.bat.color : cc.host.withAlpha(cc.host.colorText, 0.55)
          Behavior on width { NumberAnimation { duration: 400; easing.type: Easing.OutCubic } }
        }
      }
      // Profil d'alimentation : un choix à trois (deux si la machine n'a pas « performance »),
      // le profil en place à l'accent. Un clic le retient pour la source en cours (secteur ou
      // batterie), comme le panneau d'Omarchy.
      Item { visible: batterySection.pp.present && batterySection.bat.present; width: 1; height: 2 }
      Rectangle {
        id: profileChoice
        visible: batterySection.pp.present
        readonly property var names: batterySection.pp.shownProfiles
        // Chaque segment a la largeur de son contenu, plus une part égale de la place restante
        // (« Performance » est bien plus long que « Saver »).
        FontMetrics { id: segMetrics; font.family: cc.host.uiFont; font.pixelSize: 12; font.weight: Font.DemiBold }
        function contentWidth(name) { return 14 + 6 + Math.ceil(segMetrics.advanceWidth(batterySection.pp.shortLabel(name))) }
        readonly property real spare: {
          var used = 0
          for (var i = 0; i < names.length; i++) used += contentWidth(names[i])
          return Math.max(0, width - 4 - used) / Math.max(1, names.length)
        }
        width: parent.width
        height: 32
        radius: 16
        color: cc.host.withAlpha(cc.host.colorText, 0.05)
        Row {
          anchors.fill: parent
          anchors.margins: 2
          Repeater {
            model: profileChoice.names
            delegate: Rectangle {
              id: segment
              required property var modelData
              readonly property bool on: batterySection.pp.active === modelData
              width: profileChoice.contentWidth(modelData) + profileChoice.spare
              height: profileChoice.height - 4
              radius: height / 2
              color: on ? cc.host.colorAccent : segMouse.containsMouse ? cc.host.withAlpha(cc.host.colorText, 0.08) : "transparent"
              Behavior on color { ColorAnimation { duration: 160 } }
              Row {
                anchors.centerIn: parent
                spacing: 6
                Glyph {
                  anchors.verticalCenter: parent.verticalCenter
                  glyph: batterySection.pp.glyphOf(segment.modelData)
                  color: segment.on ? cc.host.colorAccentText : cc.host.colorText
                  fontFamily: cc.host.fontFamily
                  pixelSize: 14
                }
                Text {
                  anchors.verticalCenter: parent.verticalCenter
                  text: batterySection.pp.shortLabel(segment.modelData)
                  color: segment.on ? cc.host.colorAccentText : cc.host.withAlpha(cc.host.colorText, 0.7)
                  font.family: cc.host.uiFont
                  font.pixelSize: 12
                  font.weight: segment.on ? Font.DemiBold : Font.Normal
                }
              }
              MouseArea {
                id: segMouse
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: segment.on ? Qt.ArrowCursor : Qt.PointingHandCursor
                onClicked: batterySection.pp.set(segment.modelData)
              }
            }
          }
        }
      }
    }

    // ---------------------------------------------------------------- devices
    // Périphériques sans fil (DevicesState.qml) : une ligne chacun.
    Column {
      width: parent.width
      spacing: 6
      visible: deviceRepeater.count > 0
      Rectangle { width: parent.width; height: 1; color: cc.host.withAlpha(cc.host.colorText, 0.08) }
      Repeater {
        id: deviceRepeater
        model: cc.host.devices ? cc.host.devices.list : []
        delegate: Item {
          required property var modelData
          width: parent.width
          height: 20
          Glyph {
            id: devGlyph
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            width: 18
            glyph: modelData.glyph
            color: modelData.charging || modelData.low ? modelData.color : cc.host.colorText
            fontFamily: cc.host.fontFamily
            pixelSize: 15
          }
          Text {
            anchors.left: devGlyph.right
            anchors.leftMargin: 8
            anchors.right: devPct.left
            anchors.rightMargin: 10
            anchors.verticalCenter: parent.verticalCenter
            text: modelData.name
            elide: Text.ElideRight
            color: cc.host.colorText
            font.family: cc.host.uiFont
            font.pixelSize: 12
          }
          Text {
            id: devPct
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            text: (modelData.charging ? "󱐋 " : "") + Math.round(modelData.percent * 100) + " %"
            color: modelData.charging || modelData.low ? modelData.color : cc.host.colorMuted
            font.family: cc.host.uiFont
            font.pixelSize: 11
            font.features: { "tnum": 1 }
          }
        }
      }
    }

    // ---------------------------------------------------------------- AI usage
    // Claude Code / Codex limits (Session 5 h, weekly): thin bars, grey by
    // default, orange above 75 %, red above 90 %.
    Column {
      id: usage
      width: parent.width
      spacing: 10
      visible: usageRepeater.count > 0

      function windowKind(limit) {
        if (limit.title) return ""   // model-scoped limits (e.g. "Fable Weekly") are skipped
        var text = String(limit.label || "").toLowerCase()
        if (text.indexOf("week") >= 0 || text.indexOf("7-day") >= 0 || text.indexOf("7 day") >= 0) return "week"
        if (text.indexOf("session") >= 0 || /\d+\s*h/.test(text)) return "session"
        return ""
      }
      function pick(record, kind) {
        var list = record.limits || []
        for (var i = 0; i < list.length; i++)
          if (Number(list[i].percent) >= 0 && windowKind(list[i]) === kind) return list[i]
        return null
      }
      function resetText(iso) {
        var ms = new Date(String(iso || "")).getTime()
        if (!isFinite(ms)) return ""
        var left = ms - cc.host.clockDate.getTime()
        if (left <= 60000) return "resets now"
        var minutes = Math.floor(left / 60000), hours = Math.floor(minutes / 60)
        if (hours < 24) return "resets in " + (hours > 0 ? hours + " h " + String(minutes % 60).padStart(2, "0") : (minutes % 60) + " min")
        return "resets " + cc.locale.toString(new Date(ms), "ddd HH:mm")
      }
      function barColor(p) { return p > 0.9 ? "#e5484d" : p > 0.75 ? cc.host.colorAccent : cc.host.withAlpha(cc.host.colorText, 0.55) }

      Rectangle { width: parent.width; height: 1; color: cc.host.withAlpha(cc.host.colorText, 0.08) }

      Repeater {
        id: usageRepeater
        model: cc.host.agentUsage
        delegate: Column {
          id: agentBlock
          required property var modelData
          readonly property var brand: AgentBrand.brandOrName(modelData.name, modelData.name, modelData.id === "codex" ? "Codex" : "", "")
          readonly property var session: usage.pick(modelData, "session")
          readonly property var week: usage.pick(modelData, "week")
          width: usage.width
          spacing: 6

          // En-tête ; pour Claude Code, un clic ouvre la vue « Usage » (qui consomme quoi).
          Item {
            width: parent.width
            height: 20
            Row {
              spacing: 8
              Rectangle {
                anchors.verticalCenter: parent.verticalCenter
                width: 20; height: 20; radius: 6
                color: agentBlock.brand ? agentBlock.brand.tile : cc.host.withAlpha(cc.host.colorText, 0.1)
                // Logos Claude / Codex : glyphes de police, gardés tels quels (pas d'équivalent Tabler).
                Text {
                  anchors.centerIn: parent
                  text: agentBlock.brand ? agentBlock.brand.glyph : "󰚩"
                  color: agentBlock.brand ? agentBlock.brand.ink : cc.host.colorText
                  font.family: cc.host.fontFamily
                  font.pixelSize: 12
                }
              }
              Text {
                anchors.verticalCenter: parent.verticalCenter
                text: agentBlock.modelData.name
                color: cc.host.colorText
                font.family: cc.host.uiFont
                font.pixelSize: 13
                font.weight: Font.DemiBold
              }
              Text {
                anchors.verticalCenter: parent.verticalCenter
                visible: text !== ""
                text: String(agentBlock.modelData.tierLabel || "")
                color: cc.host.colorMuted
                font.family: cc.host.uiFont
                font.pixelSize: 11
              }
            }
            MouseArea {
              id: detailsMouse
              anchors.fill: parent
              enabled: agentBlock.modelData.id === "claude" || agentBlock.modelData.id === "codex"
              hoverEnabled: true
              cursorShape: Qt.PointingHandCursor
              onClicked: { cc.host.usageProvider = agentBlock.modelData.id; cc.moduleRequested("usage") }
            }
            Text {
              anchors.right: parent.right
              anchors.verticalCenter: parent.verticalCenter
              visible: detailsMouse.enabled
              text: "Details  󰅂"
              color: detailsMouse.containsMouse ? cc.host.colorText : cc.host.colorMuted
              font.family: cc.host.uiFont
              font.pixelSize: 11
            }
          }

          Repeater {
            model: [{ name: "5-hour session", limit: agentBlock.session }, { name: "Weekly", limit: agentBlock.week }]
            delegate: Item {
              required property var modelData
              readonly property real pct: modelData.limit ? Math.max(0, Math.min(1, Number(modelData.limit.percent))) : 0
              visible: !!modelData.limit
              width: agentBlock.width
              height: visible ? 26 : 0
              Text {
                anchors.left: parent.left
                anchors.top: parent.top
                text: modelData.name
                color: cc.host.colorMuted
                font.family: cc.host.uiFont
                font.pixelSize: 11
              }
              Text {
                anchors.right: parent.right
                anchors.top: parent.top
                text: Math.round(parent.pct * 100) + " %  ·  " + (modelData.limit ? usage.resetText(modelData.limit.resetsAt) : "")
                color: cc.host.colorMuted
                font.family: cc.host.uiFont
                font.pixelSize: 11
                font.features: { "tnum": 1 }
              }
              Rectangle {
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                height: 4
                radius: 2
                color: cc.host.withAlpha(cc.host.colorText, 0.1)
                Rectangle {
                  width: Math.max(parent.height, parent.width * pct)
                  height: parent.height
                  radius: 2
                  color: usage.barColor(pct)
                  visible: pct > 0
                  Behavior on width { NumberAnimation { duration: 400; easing.type: Easing.OutCubic } }
                }
              }
            }
          }
        }
      }
    }

    Rectangle { width: parent.width; height: 1; color: cc.host.withAlpha(cc.host.colorText, 0.08) }

    // ---------------------------------------------------------------- Omarchy panels
    Row {
      anchors.horizontalCenter: parent.horizontalCenter
      spacing: 8
      IconButton {
        glyph: "󰤨"; label: "Network"
        // Grows the Wi-Fi module inside the island; its « Réglages avancés… »
        // still opens Omarchy's panel.
        onActivated: cc.moduleRequested("wifi")
      }
      IconButton {
        glyph: "󰂯"; label: "Bluetooth"
        onActivated: cc.moduleRequested("bluetooth")
      }
      IconButton {
        glyph: "󰕾"; label: "Sound"
        onActivated: cc.moduleRequested("audio")
      }
      IconButton {
        visible: cc.host.vpnAvailable || cc.host.vpnConnected
        glyph: cc.host.vpnConnected ? "󰌾" : "󰦝"; label: "VPN"
        onActivated: cc.moduleRequested("vpn")
      }
      IconButton {
        visible: cc.host.tsAvailable
        tailscaleMark: true
        markOn: cc.host.tsConnected
        label: "Tailscale"
        onActivated: cc.moduleRequested("tailscale")
      }
      IconButton {
        visible: cc.host.mailAvailable
        glyph: "󰇮"; label: "Mail"
        badge: cc.host.mailUnread
        onActivated: { cc.closeRequested(); cc.host.openMail() }
      }
    }

    // ---------------------------------------------------------------- tray
    Flow {
      id: tray
      width: parent.width
      spacing: 6
      visible: trayRepeater.count > 0
      Repeater {
        id: trayRepeater
        model: SystemTray.items.values.filter(function(item) { return item.status !== Status.Passive })
        delegate: Item {
          id: trayItem
          required property var modelData
          width: 32; height: 32
          Rectangle {
            anchors.fill: parent
            radius: 10
            color: trayMouse.containsMouse ? cc.host.withAlpha(cc.host.colorText, 0.1) : "transparent"
          }
          IconImage {
            anchors.centerIn: parent
            implicitSize: 18
            source: String(trayItem.modelData.icon || "")
          }
          MouseArea {
            id: trayMouse
            anchors.fill: parent
            hoverEnabled: true
            acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
            cursorShape: Qt.PointingHandCursor
            onClicked: function(mouse) {
              var item = trayItem.modelData
              if (mouse.button === Qt.MiddleButton) { item.secondaryActivate(); return }
              if (mouse.button === Qt.RightButton || item.onlyMenu) {
                if (item.hasMenu) cc.openTrayMenu(item)
                return
              }
              item.activate()
            }
          }
        }
      }
    }

    // Menu of a tray item, drawn here: item.display() needs a platform menu, which
    // Quickshell refuses because Omarchy's shell root has no `UseQApplication` pragma.
    // Submenus drill down in place (same approach as Omarchy's bar tray).
    Column {
      id: trayMenuView
      width: parent.width
      visible: cc.trayMenuItem !== null
      spacing: 0

      Item {
        visible: cc.trayMenuStack.length > 0
        width: parent.width
        height: visible ? 28 : 0
        Rectangle {
          anchors.fill: parent
          radius: 8
          color: backMouse.containsMouse ? cc.host.withAlpha(cc.host.colorText, 0.1) : "transparent"
        }
        Text {
          anchors.verticalCenter: parent.verticalCenter
          x: 8
          width: parent.width - 16
          text: "‹  " + (cc.trayMenuStack.length > 0 ? cc.trayMenuStack[cc.trayMenuStack.length - 1].title : "")
          textFormat: Text.PlainText
          elide: Text.ElideRight
          color: cc.host.colorText
          font.family: cc.host.uiFont
          font.pixelSize: 12
        }
        MouseArea {
          id: backMouse
          anchors.fill: parent
          hoverEnabled: true
          cursorShape: Qt.PointingHandCursor
          onClicked: cc.leaveTraySubmenu()
        }
      }

      Flickable {
        width: parent.width
        height: Math.min(trayMenuRows.implicitHeight, 320)
        contentHeight: trayMenuRows.implicitHeight
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        interactive: contentHeight > height

        Column {
          id: trayMenuRows
          width: parent.width
          Repeater {
            model: cc.trayMenuStack.length > 0
              ? cc.trayMenuStack[cc.trayMenuStack.length - 1].opener.children
              : trayMenuOpener.children
            delegate: Item {
              id: row
              required property var modelData
              readonly property bool separator: modelData.isSeparator
              width: trayMenuRows.width
              height: separator ? 9 : 28
              opacity: modelData.enabled ? 1 : 0.45

              Rectangle {
                visible: row.separator
                anchors.verticalCenter: parent.verticalCenter
                x: 8; width: parent.width - 16; height: 1
                color: cc.host.withAlpha(cc.host.colorText, 0.12)
              }
              Rectangle {
                visible: !row.separator
                anchors.fill: parent
                radius: 8
                color: rowMouse.containsMouse && row.modelData.enabled ? cc.host.withAlpha(cc.host.colorText, 0.1) : "transparent"
              }
              Text {
                id: check
                visible: !row.separator && row.modelData.buttonType !== QsMenuButtonType.None
                anchors.verticalCenter: parent.verticalCenter
                x: 6; width: 14
                horizontalAlignment: Text.AlignHCenter
                text: row.modelData.checkState === Qt.Checked ? "✓" : ""
                color: cc.host.colorText
                font.family: cc.host.uiFont
                font.pixelSize: 12
              }
              IconImage {
                id: rowIcon
                visible: !row.separator && String(row.modelData.icon || "") !== ""
                anchors.verticalCenter: parent.verticalCenter
                x: 24
                implicitSize: 16
                source: String(row.modelData.icon || "")
              }
              Text {
                visible: !row.separator
                anchors.verticalCenter: parent.verticalCenter
                x: rowIcon.visible ? 46 : 24
                width: parent.width - x - 24
                text: String(row.modelData.text || "")
                textFormat: Text.PlainText
                elide: Text.ElideRight
                color: cc.host.colorText
                font.family: cc.host.uiFont
                font.pixelSize: 12
              }
              Text {
                visible: !row.separator && row.modelData.hasChildren
                anchors.verticalCenter: parent.verticalCenter
                anchors.right: parent.right
                anchors.rightMargin: 10
                text: "›"
                color: cc.host.colorMuted
                font.family: cc.host.uiFont
                font.pixelSize: 12
              }
              MouseArea {
                id: rowMouse
                anchors.fill: parent
                hoverEnabled: true
                enabled: !row.separator && row.modelData.enabled
                cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
                onClicked: {
                  if (row.modelData.hasChildren) {
                    cc.enterTraySubmenu(row.modelData, String(row.modelData.text || ""))
                  } else {
                    row.modelData.triggered()
                    cc.closeTrayMenu()
                  }
                }
              }
            }
          }
        }
      }
    }
  }
}
