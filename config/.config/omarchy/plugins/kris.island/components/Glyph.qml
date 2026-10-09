import QtQuick
import "Glyphs.js" as Glyphs

// Un glyphe de l'île : l'icône Tabler qui lui correspond (Glyphs.js), sinon
// le glyphe de la police d'origine. Même usage qu'un Text à glyphe.
Item {
  id: g
  property string glyph: ""
  property color color: "white"
  property real pixelSize: 16
  property string fontFamily: ""
  property real stroke: 2.5
  property bool filled: Glyphs.prefersFilled(glyph)
  readonly property string icon: Glyphs.iconFor(glyph)
  // Réglages de Text acceptés pour garder les mêmes écritures (sans effet sur une icône).
  property int horizontalAlignment: Text.AlignHCenter
  property int verticalAlignment: Text.AlignVCenter
  readonly property real contentWidth: implicitWidth
  readonly property real paintedWidth: implicitWidth

  implicitWidth: icon !== "" ? pixelSize : fallback.implicitWidth
  implicitHeight: icon !== "" ? pixelSize : fallback.implicitHeight

  Icon {
    anchors.centerIn: parent
    visible: g.icon !== ""
    // Tabler a des variantes pleines (<nom>-filled) : prises pour « filled ».
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
