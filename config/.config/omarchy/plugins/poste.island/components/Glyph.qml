import QtQuick
import "Glyphs.js" as Glyphs

// An island glyph: the matching Tabler icon (Glyphs.js), otherwise
// the glyph of the original font. Same use as a Text with a glyph.
Item {
  id: g
  property string glyph: ""
  property color color: "white"
  property real pixelSize: 16
  property string fontFamily: ""
  property real stroke: 2.5
  property bool filled: Glyphs.prefersFilled(glyph)
  readonly property string icon: Glyphs.iconFor(glyph)
  // Text settings accepted to keep the same writings (no effect on an icon).
  property int horizontalAlignment: Text.AlignHCenter
  property int verticalAlignment: Text.AlignVCenter
  readonly property real contentWidth: implicitWidth
  readonly property real paintedWidth: implicitWidth

  implicitWidth: icon !== "" ? pixelSize : fallback.implicitWidth
  implicitHeight: icon !== "" ? pixelSize : fallback.implicitHeight

  Icon {
    anchors.centerIn: parent
    visible: g.icon !== ""
    // Tabler has filled variants (<name>-filled): taken for 'filled'.
    name: g.filled && g.icon.indexOf("tabler/") === 0 ? g.icon + "-filled" : g.icon
    color: g.color
    size: g.pixelSize
    stroke: g.stroke
    filled: g.filled
  }
  Text {
    id: fallback
    anchors.centerIn: parent
    visible: g.icon === ""
    horizontalAlignment: g.horizontalAlignment
    verticalAlignment: g.verticalAlignment
    text: g.glyph
    color: g.color
    font.family: g.fontFamily
    font.pixelSize: g.pixelSize
  }
}
