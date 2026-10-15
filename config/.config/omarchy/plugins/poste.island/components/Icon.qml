import QtQuick
import Quickshell.Io

// Tabler icon (assets/tabler, MIT) in the island's colors:
// the SVG is read again, its color (currentColor) and the stroke width
// replaced, then displayed by an Image.
Item {
  id: icon
  property string name: ""
  property color color: "white"
  property real size: 18
  property real stroke: 2.5
  property bool filled: false     // closed shapes filled (play, pause…)

  implicitWidth: size
  implicitHeight: size

  // name: path under assets/, without .svg ('tabler/microphone').
  readonly property string dir: String(Qt.resolvedUrl("../assets/")).replace(/^file:\/\//, "")
  property string raw: ""
  FileView {
    path: icon.name !== "" ? icon.dir + icon.name + ".svg" : ""
    printErrors: false
    onLoaded: icon.raw = text()
    onLoadFailed: icon.raw = ""
  }
  function hex(v) { var h = Math.round(v * 255).toString(16); return h.length < 2 ? "0" + h : h }
  readonly property string svg: {
    var text = raw
    if (text === "") return ""
    var rgb = "#" + hex(color.r) + hex(color.g) + hex(color.b)
    text = text.replace(/<!--[\s\S]*?-->/g, "")
      .replace(/currentColor/g, rgb)
      // The invisible 24×24 square that Tabler puts in each icon: without this,
      // 'filled' would fill it (a solid square instead of the icon).
      .replace(/<path[^>]*d="M0 0h24v24H0z"[^>]*\/>/g, "")
    if (filled) text = text.replace(/fill="none"/, 'fill="' + rgb + '" fill-opacity="' + color.a + '"')
    text = text
      .replace(/stroke-width="[^"]*"/, 'stroke-width="' + stroke + '" stroke-opacity="' + color.a + '"')
    return "data:image/svg+xml;charset=utf-8," + encodeURIComponent(text)
  }

  Image {
    anchors.fill: parent
    source: icon.svg
    sourceSize.width: Math.ceil(icon.size)
    sourceSize.height: Math.ceil(icon.size)
    smooth: true
    asynchronous: false
  }
}
