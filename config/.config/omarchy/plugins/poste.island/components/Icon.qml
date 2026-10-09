import QtQuick
import Quickshell.Io

// Icône Tabler (assets/tabler, MIT) aux couleurs de l'île :
// le SVG est relu, sa couleur (currentColor) et l'épaisseur du trait
// remplacées, puis affiché par une Image.
Item {
  id: icon
  property string name: ""
  property color color: "white"
  property real size: 18
  property real stroke: 2.5
  property bool filled: false     // formes fermées remplies (lecture, pause…)

  implicitWidth: size
  implicitHeight: size

  // name : chemin sous assets/, sans .svg (« tabler/microphone »).
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
      // Le carré invisible de 24×24 que Tabler met dans chaque icône : sans ça,
      // « filled » le remplirait (un carré plein au lieu de l'icône).
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
