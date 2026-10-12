import QtQuick
import Quickshell
import Quickshell.Wayland

// Texte dicté en direct, en bas de l'écran actif, pendant une dictée de la source du slot
// « dictation » (voir Island.qml) : les phrases figées en couleur normale, puis la phrase en
// cours en gris. Trois lignes au plus : trop long, c'est la fin qui reste visible. Ne prend ni
// la souris ni le clavier. Le dernier texte reste affiché pendant le fondu de sortie.
PanelWindow {
  id: win
  required property var host
  required property bool here

  readonly property bool wanted: here && host.voxActive && (host.dictationFinals !== "" || host.dictationPartial !== "")
  property string finals: ""
  property string partial: ""
  onWantedChanged: if (wanted) keep()
  Connections {
    target: win.host
    function onDictationFinalsChanged() { if (win.wanted) win.keep() }
    function onDictationPartialChanged() { if (win.wanted) win.keep() }
  }
  function keep() {
    finals = host.dictationFinals
    partial = host.dictationPartial
  }

  function escapeHtml(s) {
    return String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;")
  }
  readonly property string plain: finals + (finals !== "" && partial !== "" ? " " : "") + partial
  // Le gris de la phrase en cours : le texte mêlé au fond (couleur opaque, #rrggbb).
  readonly property color partialColor: Qt.rgba(
    host.colorText.r * 0.45 + host.colorIsland.r * 0.55,
    host.colorText.g * 0.45 + host.colorIsland.g * 0.55,
    host.colorText.b * 0.45 + host.colorIsland.b * 0.55, 1)
  readonly property string styled: escapeHtml(finals) + (finals !== "" && partial !== "" ? " " : "")
    + (partial !== "" ? "<font color=\"" + partialColor + "\">" + escapeHtml(partial) + "</font>" : "")

  readonly property int fontSize: 16
  readonly property int lines: 3
  readonly property int padX: 22
  readonly property int padY: 12
  readonly property real maxTextWidth: Math.max(240, Math.min(900, (screen ? screen.width : 1920) * 0.6))

  visible: box.opacity > 0.01
  color: "transparent"
  surfaceFormat.opaque: false
  exclusionMode: ExclusionMode.Ignore
  anchors.bottom: true
  margins.bottom: 56
  implicitWidth: Math.ceil(box.width)
  implicitHeight: Math.ceil(box.height)
  WlrLayershell.namespace: "poste-island-dictation"
  WlrLayershell.layer: WlrLayer.Overlay
  WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
  mask: Region {}

  TextMetrics {
    id: oneLine
    font.family: win.host.fontFamily
    font.pixelSize: win.fontSize
    text: win.plain
  }

  Rectangle {
    id: box
    width: Math.min(win.maxTextWidth, Math.ceil(oneLine.advanceWidth) + 2) + 2 * win.padX
    height: clipper.height + 2 * win.padY
    radius: Math.min(height / 2, 22)
    color: win.host.colorIsland
    opacity: win.wanted ? 1 : 0
    Behavior on opacity { NumberAnimation { duration: win.wanted ? 160 : 220 } }
    Behavior on width { NumberAnimation { duration: 160 * win.host.motionScale; easing.type: Easing.OutQuint } }

    Item {
      id: clipper
      x: win.padX
      y: win.padY
      width: box.width - 2 * win.padX
      height: Math.min(label.implicitHeight, Math.ceil(oneLine.height) * win.lines)
      clip: true
      Text {
        id: label
        anchors.bottom: parent.bottom
        width: parent.width
        text: win.styled
        textFormat: Text.StyledText
        wrapMode: Text.WordWrap
        horizontalAlignment: lineCount > 1 ? Text.AlignLeft : Text.AlignHCenter
        color: win.host.colorText
        font.family: win.host.fontFamily
        font.pixelSize: win.fontSize
      }
    }
  }
}
