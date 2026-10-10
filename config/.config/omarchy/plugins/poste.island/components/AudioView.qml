import QtQuick
import Quickshell
import Quickshell.Services.Pipewire
import "file:///usr/share/omarchy/shell/plugins/panels/audio/Model.js" as AudioModel

// Sound, grown inside the island from the control center's tile: output and
// input volume + mute, device choice (same calls as Omarchy's audio panel),
// and per-app volumes. Keys: ↑↓ move over devices, Enter makes the device
// the default, Ctrl+B actions, Esc / ← back.
Item {
  id: audio
  required property var host
  property bool active: false
  signal backRequested()
  signal advancedRequested()

  implicitHeight: column.implicitHeight
  property real maxListHeight: 420
  readonly property real rowHeight: 42

  readonly property var nodes: Pipewire.nodes ? Pipewire.nodes.values : []
  readonly property var sinks: nodes.filter(function(n) { return n && n.isSink && !n.isStream })
  readonly property var sources: nodes.filter(function(n) {
    return n && !n.isSink && !n.isStream && AudioModel.isAudioSource(n) && String(n.name || "") !== "quickshell"
  })
  readonly property var streams: nodes.filter(function(n) {
    return n && n.isStream && AudioModel.isPlaybackStream(n) && String(n.name || "").indexOf("omarchy_speaker_tuning") !== 0
  })
  readonly property var sink: Pipewire.defaultAudioSink
  readonly property var source: Pipewire.defaultAudioSource
  PwObjectTracker { objects: audio.sinks.concat(audio.sources).concat(audio.streams) }

  // Selectable device rows: outputs then inputs.
  readonly property var devices: sinks.map(function(n) { return { node: n, kind: "output" } })
    .concat(sources.map(function(n) { return { node: n, kind: "input" } }))
  property int selected: 0
  property bool actionsOpen: false
  property int actionIndex: 0
  readonly property var current: selected < devices.length ? devices[selected] : null

  onActiveChanged: {
    if (!active) { actionsOpen = false; return }
    selected = Math.max(0, sinks.indexOf(sink))
    Qt.callLater(function() { audio.forceActiveFocus() })
  }

  function label(n) { return AudioModel.nodeLabel(n) }
  function isDefault(entry) { return entry && (entry.kind === "output" ? entry.node === sink : entry.node === source) }
  function setDefault(entry) {
    if (!entry || isDefault(entry)) return
    var n = entry.node
    if (entry.kind === "output") {
      Pipewire.preferredDefaultAudioSink = n
      if (n.id !== undefined && n.name) Quickshell.execDetached(["omarchy-audio-output-set-default", String(n.id), String(n.name)])
    } else {
      Pipewire.preferredDefaultAudioSource = n
      if (n.id !== undefined && n.name) Quickshell.execDetached(["omarchy-audio-input-set-default", String(n.id), String(n.name)])
    }
  }
  function outputGlyph(n) {
    var t = (label(n) + " " + String(n.name || "")).toLowerCase()
    if (/headphone|headset|casque|pro x|bluez/.test(t)) return "󰋋"
    if (/hdmi|displayport|monitor|pl\d|screen/.test(t)) return "󰍹"
    return "󰓃"
  }
  readonly property var currentActions: {
    var e = current
    if (!e) return []
    var list = []
    if (!isDefault(e)) list.push({ id: "default", label: "Use as default", glyph: "󰄬" })
    if (e.node.audio) list.push({ id: "mute", label: e.node.audio.muted ? "Unmute" : "Mute", glyph: e.node.audio.muted ? "󰕾" : "󰝟" })
    return list
  }
  function runRowAction(id) {
    var e = current
    if (!e) return
    actionsOpen = false
    if (id === "default") setDefault(e)
    else if (id === "mute" && e.node.audio) e.node.audio.muted = !e.node.audio.muted
  }

  Keys.onPressed: function(event) {
    var ctrl = event.modifiers & Qt.ControlModifier
    if (actionsOpen) {
      if (event.key === Qt.Key_Escape || (ctrl && event.key === Qt.Key_B)) actionsOpen = false
      else if (event.key === Qt.Key_Left) actionIndex = Math.max(0, actionIndex - 1)
      else if (event.key === Qt.Key_Right) actionIndex = Math.min(currentActions.length - 1, actionIndex + 1)
      else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) { var act = currentActions[actionIndex]; if (act) runRowAction(act.id) }
      else return
      event.accepted = true
      return
    }
    if (event.key === Qt.Key_Escape || event.key === Qt.Key_Left || event.key === Qt.Key_Backspace) audio.backRequested()
    else if (event.key === Qt.Key_Down) selected = Math.min(devices.length - 1, selected + 1)
    else if (event.key === Qt.Key_Up) selected = Math.max(0, selected - 1)
    else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) setDefault(current)
    else if (ctrl && event.key === Qt.Key_B) { if (current) { actionIndex = 0; actionsOpen = true } }
    else return
    event.accepted = true
  }

  component SectionTitle: Text {
    leftPadding: 6
    color: audio.host.colorMuted
    font.family: audio.host.uiFont
    font.pixelSize: 11
    font.weight: Font.DemiBold
  }

  component DeviceRow: ModuleRow {
    id: drow
    required property var entry
    required property int flatIndex
    readonly property bool showActions: audio.actionsOpen && isSelected
    host: audio.host
    rowHeight: audio.rowHeight
    isSelected: audio.selected === flatIndex
    title: audio.label(entry.node)
    strong: audio.isDefault(entry)
    subtitle: audio.isDefault(entry) ? "Default" : ""
    subtitleAccent: true
    trailing: entry.node.audio && entry.node.audio.muted ? "󰝟" : ""
    extraHeight: showActions ? 40 : 0
    onHovered: if (!audio.actionsOpen) audio.selected = flatIndex
    onClicked: function(button) {
      audio.selected = flatIndex
      if (button === Qt.RightButton) { audio.actionIndex = 0; audio.actionsOpen = true }
      else audio.setDefault(entry)
    }
    Row {
      visible: drow.showActions
      y: 4
      spacing: 6
      Repeater {
        model: drow.showActions ? audio.currentActions : []
        delegate: PillButton {
          required property var modelData
          required property int index
          host: audio.host
          glyph: modelData.glyph
          label: modelData.label
          highlighted: audio.actionIndex === index
          onClicked: { audio.actionIndex = index; audio.runRowAction(modelData.id) }
        }
      }
    }
  }

  Column {
    id: column
    anchors.left: parent.left
    anchors.right: parent.right
    spacing: 12

    ModuleHeader {
      host: audio.host
      title: "Sound"
      onBackClicked: audio.backRequested()
    }

    Flickable {
      WheelScroll {}
      id: flick
      width: parent.width
      height: Math.min(audio.maxListHeight, body.implicitHeight)
      contentHeight: body.implicitHeight
      clip: true
      interactive: contentHeight > height
      boundsBehavior: Flickable.StopAtBounds

      Column {
        id: body
        width: flick.width
        spacing: 8

        SectionTitle { text: "Output" }
        LevelSlider {
          host: audio.host
          value: audio.sink && audio.sink.audio ? audio.sink.audio.volume : 0
          muted: audio.sink && audio.sink.audio ? audio.sink.audio.muted : false
          onMoved: function(v) { if (audio.sink && audio.sink.audio) audio.sink.audio.volume = v }
          onMuteClicked: if (audio.sink && audio.sink.audio) audio.sink.audio.muted = !audio.sink.audio.muted
        }
        Repeater {
          model: audio.sinks
          delegate: DeviceRow {
            required property var modelData
            required property int index
            width: body.width
            entry: ({ node: modelData, kind: "output" })
            flatIndex: index
            glyph: audio.outputGlyph(modelData)
          }
        }

        Item { width: 1; height: 4 }
        SectionTitle { text: "Input" }
        LevelSlider {
          host: audio.host
          glyph: "󰍬"
          mutedGlyph: "󰍭"
          value: audio.source && audio.source.audio ? audio.source.audio.volume : 0
          muted: audio.source && audio.source.audio ? audio.source.audio.muted : false
          onMoved: function(v) { if (audio.source && audio.source.audio) audio.source.audio.volume = v }
          onMuteClicked: if (audio.source && audio.source.audio) audio.source.audio.muted = !audio.source.audio.muted
        }
        Repeater {
          model: audio.sources
          delegate: DeviceRow {
            required property var modelData
            required property int index
            width: body.width
            entry: ({ node: modelData, kind: "input" })
            flatIndex: audio.sinks.length + index
            glyph: "󰍬"
          }
        }

        Item { width: 1; height: 4; visible: audio.streams.length > 0 }
        SectionTitle { text: "Apps"; visible: audio.streams.length > 0 }
        Repeater {
          model: audio.streams
          delegate: Column {
            id: streamItem
            required property var modelData
            readonly property var props: modelData.ready && modelData.properties ? modelData.properties : ({})
            width: body.width
            spacing: 0
            Text {
              leftPadding: 6
              width: parent.width
              elide: Text.ElideRight
              text: streamItem.props["application.name"] || modelData.description || modelData.name || "App"
              color: audio.host.colorText
              font.family: audio.host.uiFont
              font.pixelSize: 12
            }
            LevelSlider {
              host: audio.host
              glyph: "󰝚"
              value: streamItem.modelData.audio ? streamItem.modelData.audio.volume : 0
              muted: streamItem.modelData.audio ? streamItem.modelData.audio.muted : false
              onMoved: function(v) { if (streamItem.modelData.audio) streamItem.modelData.audio.volume = v }
              onMuteClicked: if (streamItem.modelData.audio) streamItem.modelData.audio.muted = !streamItem.modelData.audio.muted
            }
          }
        }
      }
    }

    ModuleFooter {
      host: audio.host
      onAdvancedClicked: audio.advancedRequested()
    }
  }
}
