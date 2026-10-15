import QtQuick
import Quickshell
import Quickshell.Io
import "NameColor.js" as NameColor

// Detail of Claude Code consumption (opened from the Claude Code block
// of the control center, or `omarchy-shell poste.island usage`). The figures
// come from bin/poste-agent-usage (Claude Code's logs): total of the
// window, share per model, tokens per hour, then the most
// greedy sessions; Enter or click on a session unfolds its agents.
// Keys: Tab 5 h session / week, ↑↓ choose, Enter unfold, Escape close.
Item {
  id: view
  required property var host
  property bool active: false
  property real maxHeight: 600
  signal closeRequested()

  property string kind: "session"
  readonly property string provider: host.usageProvider || "claude"
  readonly property string providerName: provider === "codex" ? "Codex" : "Claude Code"
  property var usage: null
  property bool loading: false
  property int selected: 0
  property var expanded: ({})
  property var agentOpen: ({})

  readonly property string bin: Quickshell.env("HOME") + "/.config/omarchy/plugins/poste.island/bin/poste-agent-usage"
  readonly property var sessions: usage ? usage.sessions || [] : []
  readonly property real totalTokens: usage ? Number(usage.total) || 0 : 0
  implicitHeight: Math.min(maxHeight, content.implicitHeight)

  function refresh() {
    loading = true
    proc.command = [bin, kind, provider]
    proc.running = true
  }
  onActiveChanged: if (active) { selected = 0; expanded = ({}); agentOpen = ({}); refresh(); Qt.callLater(function() { view.forceActiveFocus() }) }
  onKindChanged: if (active) { selected = 0; expanded = ({}); refresh() }
  onProviderChanged: { usage = null; if (active) refresh() }

  Process {
    id: proc
    stdout: StdioCollector {
      onStreamFinished: {
        view.loading = false
        try { view.usage = JSON.parse(String(this.text)) } catch (e) { view.usage = null }
        // The most greedy session opens unfolded (its agents).
        var top = view.sessions[0]
        if (top && !Object.keys(view.expanded).length) { var e = {}; e[top.id] = true; view.expanded = e }
      }
    }
  }

  function fmt(n) {
    n = Number(n) || 0
    if (n >= 1e9) return (n / 1e9).toFixed(1) + " G"
    if (n >= 1e6) return Math.round(n / 1e6) + " M"
    if (n >= 1e3) return Math.round(n / 1e3) + " k"
    return String(n)
  }
  function share(n) { return totalTokens > 0 ? Math.round(n / totalTokens * 100) + " %" : "" }
  // Color of a model: drawn from its name alone (NameColor.js), hence the same
  // everywhere (session, week, agents), whatever the other models.
  function colorFor(model) { return NameColor.color(model) }
  // Local time of an ISO date: '14:00', or 'Sun 14:00' over the week.
  function clock(iso) {
    var d = new Date(String(iso || ""))
    if (isNaN(d.getTime())) return ""
    var hm = String(d.getHours()).padStart(2, "0") + ":" + String(d.getMinutes()).padStart(2, "0")
    return kind === "week" ? ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"][d.getDay()] + " " + hm : hm
  }
  function since(iso) {
    var ms = new Date(String(iso || "")).getTime()
    if (!isFinite(ms)) return ""
    var m = Math.max(0, Math.round((host.clockDate.getTime() - ms) / 60000))
    return m < 2 ? "now" : m < 60 ? m + " min ago" : Math.floor(m / 60) + " h ago"
  }
  function toggleAgent(key) {
    var next = {}
    for (var k in agentOpen) next[k] = agentOpen[k]
    next[key] = !next[key]
    agentOpen = next
  }
  // '2 h 14', '35 min': from `from` to `to` (or now).
  function duration(from, to) {
    var a = new Date(String(from || "")).getTime()
    var b = to ? new Date(String(to)).getTime() : host.clockDate.getTime()
    if (!isFinite(a) || !isFinite(b)) return ""
    var m = Math.max(0, Math.round((b - a) / 60000))
    return m < 60 ? m + " min" : Math.floor(m / 60) + " h " + String(m % 60).padStart(2, "0")
  }
  function toggle(i) {
    var s = sessions[i]
    if (!s) return
    var next = {}
    for (var k in expanded) next[k] = expanded[k]
    next[s.id] = !next[s.id]
    expanded = next
  }

  Keys.onPressed: function(event) {
    var k = event.key
    if (k === Qt.Key_Escape) { view.closeRequested(); event.accepted = true }
    else if (k === Qt.Key_Tab || k === Qt.Key_Backtab) { kind = kind === "session" ? "week" : "session"; event.accepted = true }
    else if (k === Qt.Key_Down) { selected = Math.min(sessions.length - 1, selected + 1); list.positionViewAtIndex(selected, ListView.Contain); event.accepted = true }
    else if (k === Qt.Key_Up) { selected = Math.max(0, selected - 1); list.positionViewAtIndex(selected, ListView.Contain); event.accepted = true }
    else if (k === Qt.Key_Return || k === Qt.Key_Enter || k === Qt.Key_Space) { toggle(selected); event.accepted = true }
    else if (k === Qt.Key_R) { refresh(); event.accepted = true }
  }

  component Label: Text {
    color: view.host.colorMuted
    font.family: view.host.uiFont
    font.pixelSize: 11
    font.features: { "tnum": 1 }
  }

  Column {
    id: content
    width: parent.width
    spacing: 14

    // ------------------------------------------------ header: window and quota
    Item {
      width: parent.width
      height: 26
      Text {
        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        text: view.providerName + " usage"
        color: view.host.colorText
        font.family: view.host.uiFont
        font.pixelSize: 15
        font.weight: Font.DemiBold
      }
      Row {
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        spacing: 4
        Repeater {
          model: [{ id: "session", label: "5-hour session" }, { id: "week", label: "Weekly" }]
          delegate: Rectangle {
            required property var modelData
            readonly property bool on: view.kind === modelData.id
            width: tabText.implicitWidth + 18
            height: 24
            radius: 12
            color: on ? view.host.withAlpha(view.host.colorText, 0.12) : (tabMouse.containsMouse ? view.host.withAlpha(view.host.colorText, 0.06) : "transparent")
            Text {
              id: tabText
              anchors.centerIn: parent
              text: modelData.label
              color: on ? view.host.colorText : view.host.colorMuted
              font.family: view.host.uiFont
              font.pixelSize: 12
              font.weight: on ? Font.DemiBold : Font.Normal
            }
            MouseArea { id: tabMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: view.kind = modelData.id }
          }
        }
      }
    }

    // ------------------------------------------------ chargement (Shimmer.qml)
    Shimmer {
      width: parent.width
      visible: !view.usage
      host: view.host
      label: "Reading " + view.providerName + "'s logs…"
      lines: [0.28, 1, 0.5, 1, 0.92, 0.84, 0.66]
    }

    // ------------------------------------------------ total, share per model
    Column {
      width: parent.width
      spacing: 8
      visible: !!view.usage
      Row {
        spacing: 10
        Text {
          text: view.usage ? view.fmt(view.totalTokens) : (view.loading ? "…" : "-")
          color: view.host.colorText
          font.family: view.host.uiFont
          font.pixelSize: 26
          font.weight: Font.Bold
          font.features: { "tnum": 1 }
        }
        Column {
          anchors.verticalCenter: parent.verticalCenter
          Label { text: "tokens · " + (view.usage ? view.usage.req : 0) + " requests" }
          Label {
            text: view.usage && view.usage.percent !== null && view.usage.percent !== undefined
              ? Math.round(view.usage.percent * 100) + " % of the limit" : ""
            visible: text !== ""
          }
        }
      }
      // Stacked bar per model.
      Row {
        width: parent.width
        height: 6
        Repeater {
          model: view.usage ? view.usage.models : []
          delegate: Rectangle {
            required property var modelData
            required property int index
            width: view.totalTokens > 0 ? parent.width * modelData.tokens / view.totalTokens : 0
            height: 6
            radius: 3
            color: view.colorFor(modelData.name)
          }
        }
      }
      Row {
        spacing: 14
        Repeater {
          model: view.usage ? view.usage.models : []
          delegate: Row {
            required property var modelData
            spacing: 5
            Rectangle { anchors.verticalCenter: parent.verticalCenter; width: 7; height: 7; radius: 3.5; color: view.colorFor(modelData.name) }
            Label { text: modelData.name + "  " + view.fmt(modelData.tokens) + " · " + view.share(modelData.tokens) }
          }
        }
      }
    }

    // ------------------------------------------------ tokens per hour
    Column {
      id: hoursCol
      width: parent.width
      spacing: 4
      visible: !!view.usage && view.usage.hours.length > 1
      readonly property real peak: {
        var m = 0, h = view.usage ? view.usage.hours : []
        for (var i = 0; i < h.length; i++) m = Math.max(m, h[i].tokens)
        return m
      }
      Row {
        id: hourBars
        width: parent.width
        height: 34
        spacing: 3
        readonly property int count: view.usage ? view.usage.hours.length : 0
        Repeater {
          model: view.usage ? view.usage.hours : []
          delegate: Item {
            required property var modelData
            width: (hourBars.width - hourBars.spacing * (hourBars.count - 1)) / Math.max(1, hourBars.count)
            height: hourBars.height
            Rectangle {
              anchors.bottom: parent.bottom
              width: parent.width
              height: Math.max(2, parent.height * modelData.tokens / Math.max(1, hoursCol.peak))
              radius: 2
              color: view.host.withAlpha(view.host.colorText, 0.35)
            }
          }
        }
      }
      Item {
        width: parent.width
        height: 14
        Label { anchors.left: parent.left; text: view.usage && view.usage.hours.length ? view.usage.hours[0].hour.slice(11) + " h" : "" }
        Label { anchors.right: parent.right; text: view.usage && view.usage.hours.length ? view.usage.hours[view.usage.hours.length - 1].hour.slice(11) + " h" : "" }
      }
    }

    // ------------------------------------------------ timeline: who ran when
    // One row per session (the 8 most greedy), the window in 48 cells:
    // a full cell = this session consumed at that moment, more opaque
    // when it consumed more, in the color of the model that dominates the cell.
    Column {
      id: timelineCol
      width: parent.width
      spacing: 3
      visible: rows.length > 0
      readonly property var rows: view.sessions.filter(function(s) { return (s.timeline || []).length > 0 }).slice(0, 8)
      readonly property int slots: view.usage ? Number(view.usage.slots) || 48 : 48
      readonly property real peak: {
        var m = 0
        for (var i = 0; i < rows.length; i++)
          for (var j = 0; j < rows[i].timeline.length; j++) m = Math.max(m, rows[i].timeline[j][1])
        return m
      }
      // Position of 'now' in the window (the week runs until its reset).
      readonly property real nowAt: {
        if (!view.usage) return 1
        var a = new Date(view.usage.start).getTime(), b = new Date(view.usage.end).getTime()
        var n = new Date(view.usage.now || Date.now()).getTime()
        return b > a ? Math.max(0, Math.min(1, (n - a) / (b - a))) : 1
      }
      readonly property real labelWidth: 96
      Repeater {
        model: timelineCol.rows
        delegate: Item {
          id: tlRow
          required property var modelData
          width: timelineCol.width
          height: 12
          Label {
            width: timelineCol.labelWidth - 8
            elide: Text.ElideRight
            anchors.verticalCenter: parent.verticalCenter
            text: tlRow.modelData.project
          }
          Item {
            id: strip
            x: timelineCol.labelWidth
            width: parent.width - x
            height: parent.height
            Rectangle {
              anchors.fill: parent
              radius: 3
              color: view.host.withAlpha(view.host.colorText, 0.04)
            }
            Repeater {
              model: tlRow.modelData.timeline
              delegate: Rectangle {
                required property var modelData
                readonly property real cell: strip.width / timelineCol.slots
                x: modelData[0] * cell
                width: Math.max(2, cell - 1)
                height: strip.height
                radius: 2
                color: view.colorFor(modelData[2])
                opacity: 0.25 + 0.75 * Math.sqrt(modelData[1] / Math.max(1, timelineCol.peak))
              }
            }
          }
        }
      }
      // Marks: start, now, end of the window.
      Item {
        width: parent.width
        height: 14
        Label {
          x: timelineCol.labelWidth
          text: view.usage ? view.clock(view.usage.start) : ""
        }
        Label {
          x: Math.min(parent.width - width, Math.max(timelineCol.labelWidth, timelineCol.labelWidth + (parent.width - timelineCol.labelWidth) * timelineCol.nowAt - width / 2))
          visible: timelineCol.nowAt < 0.97 && timelineCol.nowAt > 0.08
          text: "now"
        }
        Label {
          anchors.right: parent.right
          text: view.usage ? view.clock(view.usage.end) : ""
        }
      }
    }

    Rectangle { visible: !!view.usage; width: parent.width; height: 1; color: view.host.withAlpha(view.host.colorText, 0.08) }

    // ------------------------------------------------ sessions
    ListView {
      WheelScroll {}
      id: list
      width: parent.width
      height: Math.min(contentHeight, view.maxHeight - y - 30)
      interactive: contentHeight > height
      clip: true
      spacing: 4
      model: view.sessions
      currentIndex: view.selected
      delegate: Rectangle {
        id: row
        required property var modelData
        required property int index
        readonly property bool open: !!view.expanded[modelData.id]
        readonly property bool current: index === view.selected
        width: list.width
        height: rowCol.implicitHeight + 16
        radius: 10
        color: current ? view.host.withAlpha(view.host.colorText, 0.08) : (rowMouse.containsMouse ? view.host.withAlpha(view.host.colorText, 0.04) : "transparent")
        MouseArea {
          id: rowMouse
          anchors.fill: parent
          hoverEnabled: true
          cursorShape: Qt.PointingHandCursor
          onClicked: { view.selected = row.index; view.toggle(row.index) }
        }
        Column {
          id: rowCol
          x: 10
          y: 8
          width: parent.width - 20
          spacing: 4
          Item {
            width: parent.width
            height: 18
            Row {
              anchors.left: parent.left
              anchors.verticalCenter: parent.verticalCenter
              width: parent.width - tokens.width - 12
              spacing: 6
              Text {
                id: proj
                text: row.modelData.project
                color: view.host.colorText
                font.family: view.host.uiFont
                font.pixelSize: 13
                font.weight: Font.DemiBold
              }
              Text {
                width: parent.width - proj.width - 6
                text: row.modelData.title
                elide: Text.ElideRight
                color: view.host.colorMuted
                font.family: view.host.uiFont
                font.pixelSize: 12
              }
            }
            Text {
              id: tokens
              anchors.right: parent.right
              anchors.verticalCenter: parent.verticalCenter
              text: view.fmt(row.modelData.tokens) + "  ·  " + view.share(row.modelData.tokens)
              color: view.host.colorText
              font.family: view.host.uiFont
              font.pixelSize: 12
              font.weight: Font.DemiBold
              font.features: { "tnum": 1 }
            }
          }
          Label {
            text: (row.open ? "▾ " : "▸ ") + "context " + view.fmt(row.modelData.mainContext) + " avg, " + view.fmt(row.modelData.maxContext) + " max"
              + "  ·  " + row.modelData.agentCount + " agents  ·  " + row.modelData.req + " requests  ·  " + view.since(row.modelData.last)
          }
          // Agents of the session, unfolded.
          Item {
            width: parent.width
            height: row.open ? agentList.implicitHeight : 0
            clip: true
            visible: height > 0.5
            opacity: row.open ? 1 : 0
            Behavior on height { NumberAnimation { duration: 280 * view.host.motionScale; easing.type: Easing.OutCubic } }
            Behavior on opacity { NumberAnimation { duration: 220 * view.host.motionScale; easing.type: Easing.OutCubic } }
          Column {
            id: agentList
            width: parent.width
            spacing: 0
            topPadding: 4
            Repeater {
              model: row.modelData.agents
              // Explorer-style tree: the main conversation first,
              // lines that link it to its agents, and each agent to its own.
              delegate: Item {
                id: agentRow
                required property var modelData
                readonly property int depth: modelData.depth || 0
                readonly property bool open: !!view.agentOpen[row.modelData.id + "/" + (modelData.id || "main")]
                // X of a level's line (1: agents of the conversation).
                function railX(level) { return 12 + (level - 1) * 18 }
                readonly property real indent: depth === 0 ? 0 : railX(depth) + 12
                readonly property color line: view.host.withAlpha(view.host.colorText, 0.2)
                width: rowCol.width
                height: agentCol.implicitHeight

                // Lines of the levels above that continue further down.
                Repeater {
                  model: agentRow.modelData.rails || []
                  delegate: Rectangle {
                    required property var modelData
                    required property int index
                    visible: modelData === true
                    x: agentRow.railX(index + 1)
                    width: 1
                    height: agentRow.height
                    color: agentRow.line
                  }
                }
                // Its own elbow: from the top down to the middle of the row, then to the right;
                // the line continues if it still has siblings further down.
                Rectangle {
                  visible: agentRow.depth > 0
                  x: agentRow.railX(agentRow.depth)
                  width: 1
                  height: agentRow.modelData.lastSibling ? head.height / 2 : agentRow.height
                  color: agentRow.line
                }
                Rectangle {
                  visible: agentRow.depth > 0
                  x: agentRow.railX(agentRow.depth)
                  y: head.height / 2
                  width: 8
                  height: 1
                  color: agentRow.line
                }
                // Start of the line toward its own agents (under its detail if it is open).
                Rectangle {
                  visible: !!agentRow.modelData.hasChildren
                  x: agentRow.railX(agentRow.depth + 1)
                  y: head.height - (agentRow.depth === 0 ? 4 : 2)
                  width: 1
                  height: agentRow.height - y
                  color: agentRow.line
                }

                Column {
                  id: agentCol
                  width: parent.width
                  Item {
                    id: head
                    width: parent.width
                    height: agentRow.depth === 0 ? 26 : 21
                    // The main conversation: a separate banner.
                    Rectangle {
                      visible: agentRow.depth === 0
                      anchors.fill: parent
                      anchors.bottomMargin: 4
                      radius: 6
                      color: view.host.withAlpha(view.host.colorAccent, 0.10)
                      border.width: 1
                      border.color: view.host.withAlpha(view.host.colorAccent, 0.22)
                    }
                    MouseArea {
                      anchors.fill: parent
                      cursorShape: Qt.PointingHandCursor
                      onClicked: view.toggleAgent(row.modelData.id + "/" + (agentRow.modelData.id || "main"))
                    }
                    // Invisible anchor: the model pill always starts from the same x.
                    Item {
                      id: live
                      x: agentRow.indent + (agentRow.depth === 0 ? 6 : 0)
                      anchors.verticalCenter: parent.verticalCenter
                      anchors.verticalCenterOffset: agentRow.depth === 0 ? -2 : 0
                      width: 0
                      height: 6
                    }
                    // Running: green dot on the right, before the duration.
                    Rectangle {
                      id: liveDot
                      visible: !!agentRow.modelData.running
                      anchors.right: agentTokens.left
                      anchors.rightMargin: 6
                      anchors.verticalCenter: live.verticalCenter
                      width: 6
                      height: 6
                      radius: 3
                      color: "#30a46c"
                      SequentialAnimation on opacity {
                        running: agentRow.modelData.running && view.active
                        loops: Animation.Infinite
                        NumberAnimation { to: 0.35; duration: 700; easing.type: Easing.InOutSine }
                        NumberAnimation { to: 1; duration: 700; easing.type: Easing.InOutSine }
                      }
                    }
                    Rectangle {
                      id: chip
                      anchors.left: live.right
                      anchors.verticalCenter: live.verticalCenter
                      width: chipText.implicitWidth + 10
                      height: 15
                      radius: 4
                      color: view.host.withAlpha(view.colorFor(agentRow.modelData.model), 0.18)
                      Text {
                        id: chipText
                        anchors.centerIn: parent
                        text: agentRow.modelData.model
                        color: view.colorFor(agentRow.modelData.model)
                        font.family: view.host.uiFont
                        font.pixelSize: 10
                        font.weight: Font.DemiBold
                      }
                    }
                    Text {
                      anchors.left: chip.right
                      anchors.leftMargin: 8
                      anchors.right: liveDot.visible ? liveDot.left : agentTokens.left
                      anchors.rightMargin: 8
                      anchors.verticalCenter: live.verticalCenter
                      text: agentRow.modelData.description
                      elide: Text.ElideRight
                      color: view.host.colorText
                      font.family: view.host.uiFont
                      font.pixelSize: 12
                      font.weight: agentRow.depth === 0 ? Font.DemiBold : Font.Normal
                    }
                    Label {
                      id: agentTokens
                      anchors.right: parent.right
                      anchors.rightMargin: agentRow.depth === 0 ? 8 : 0
                      anchors.verticalCenter: live.verticalCenter
                      text: (agentRow.modelData.running && agentRow.modelData.first ? view.duration(agentRow.modelData.first) + "  ·  " : "")
                        + view.fmt(agentRow.modelData.tokens) + "  ·  ctx " + view.fmt(agentRow.modelData.avgContext)
                    }
                  }
                  // Detail of an agent: who started it, how, what it does, its instruction.
                  Item {
                    width: parent.width
                    height: agentRow.open ? detailBox.height + 6 : 0
                    clip: true
                    visible: height > 0.5
                    opacity: agentRow.open ? 1 : 0
                    Behavior on height { NumberAnimation { duration: 240 * view.host.motionScale; easing.type: Easing.OutCubic } }
                    Behavior on opacity { NumberAnimation { duration: 200 * view.host.motionScale; easing.type: Easing.OutCubic } }
                    Rectangle {
                      id: detailBox
                      x: agentRow.indent + 4
                      width: parent.width - x
                      height: detail.implicitHeight + 16
                      radius: 8
                      color: view.host.withAlpha(view.host.colorText, 0.04)
                      Column {
                        id: detail
                        x: 10
                        y: 8
                        width: parent.width - 20
                        spacing: 5
                        Label {
                          width: parent.width
                          wrapMode: Text.Wrap
                          visible: !agentRow.modelData.main
                          text: "Launched by " + (agentRow.modelData.parent || "?")
                            + "  ·  " + (agentRow.modelData.type || "agent") + (agentRow.modelData.background ? ", background" : "")
                            + "  ·  " + (agentRow.modelData.askedModel ? agentRow.modelData.askedModel + " asked" : "session model")
                            + (agentRow.modelData.first ? "  ·  " + (agentRow.modelData.running ? "running for " : "ran ") + view.duration(agentRow.modelData.first, agentRow.modelData.running ? "" : agentRow.modelData.last) : "")
                        }
                        Label {
                          width: parent.width
                          elide: Text.ElideRight
                          visible: !!agentRow.modelData.lastTool
                          text: (agentRow.modelData.running ? "Now: " : "Last: ") + agentRow.modelData.lastTool
                          color: view.host.colorText
                        }
                        Text {
                          width: parent.width
                          visible: !!agentRow.modelData.prompt
                          text: String(agentRow.modelData.prompt || "")
                          textFormat: Text.PlainText
                          wrapMode: Text.Wrap
                          maximumLineCount: 9
                          elide: Text.ElideRight
                          color: view.host.withAlpha(view.host.colorText, 0.7)
                          font.family: view.host.uiFont
                          font.pixelSize: 11
                          lineHeight: 1.15
                        }
                      }
                    }
                  }
                }
              }
            }
            Label {
              visible: row.modelData.agentCount === 0
              leftPadding: 12
              text: "No agents in this window"
            }
          }
          }
        }
      }
    }

    Label {
      visible: !!view.usage && view.sessions.length === 0
      text: "Nothing used in this window" + (view.kind === "session" ? ". Tab: weekly." : ".")
      color: view.host.colorText
      font.pixelSize: 12
    }

    Label {
      width: parent.width
      wrapMode: Text.Wrap
      text: "From " + view.providerName + "'s logs. Tokens = input + cache writes + cache reads + output; a long context is re-read on every request. Tab: window  ·  ↑↓ Enter: agents  ·  R: refresh"
      opacity: 0.8
    }
  }
}
