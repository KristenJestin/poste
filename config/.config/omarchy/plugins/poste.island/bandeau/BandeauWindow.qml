import QtQuick
import Quickshell
import Quickshell.Wayland

// La fenêtre du bandeau sur un écran : tout l'écran, taille fixe (une pièce qui arrive ou
// part ne change que le contenu, le compositeur ne retaille jamais la surface), au-dessus de
// tout, sans clavier, et la souris la traverse partout (rien n'y est cliquable). L'OSD et la
// dictée vont sur l'écran actif. Chaque pièce garde la place qu'elle prend (`space`, animée),
// celle du dessus glisse quand elle arrive ou part.
PanelWindow {
  id: win
  required property var bandeau

  readonly property string screenName: screen ? String(screen.name) : ""
  readonly property bool focused: bandeau.focusedScreen === "" || bandeau.focusedScreen === screenName

  visible: dictation.live || osdView.live
  color: "transparent"
  surfaceFormat.opaque: false
  exclusionMode: ExclusionMode.Ignore
  anchors { top: true; bottom: true; left: true; right: true }
  WlrLayershell.namespace: "poste-bandeau"
  WlrLayershell.layer: WlrLayer.Overlay
  WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
  // Région vide : aucune zone ne prend la souris.
  mask: Region {}

  DictationView {
    id: dictation
    bandeau: win.bandeau
    active: win.focused && win.bandeau.dictShown
    anchors.horizontalCenter: parent.horizontalCenter
    y: win.height - win.bandeau.bottomMargin - height
    maxTextWidth: Math.max(240, Math.min(900, win.width * 0.6))
  }

  OsdView {
    id: osdView
    bandeau: win.bandeau
    active: win.focused && win.bandeau.osdOpen
    anchors.horizontalCenter: parent.horizontalCenter
    y: win.height - win.bandeau.bottomMargin - dictation.space - height
  }
}
