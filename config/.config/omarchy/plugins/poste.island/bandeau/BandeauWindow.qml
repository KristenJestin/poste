import QtQuick
import Quickshell
import Quickshell.Wayland

// La fenêtre du bandeau sur un écran : tout l'écran, taille fixe (une pièce qui arrive ou
// part ne change que le contenu, le compositeur ne retaille jamais la surface), au-dessus de
// tout, sans clavier. La souris traverse partout sauf sur les cartes de notification.
// L'OSD et la dictée vont sur l'écran actif ; une notification reste sur l'écran où elle est
// apparue (règle du service). Chaque pièce garde la place qu'elle prend (`space`, animée),
// celles du dessus glissent quand elle arrive ou part.
PanelWindow {
  id: win
  required property var bandeau

  readonly property string screenName: screen ? String(screen.name) : ""
  readonly property bool focused: bandeau.focusedScreen === "" || bandeau.focusedScreen === screenName

  visible: dictation.live || osdView.live || notifs.live
  color: "transparent"
  surfaceFormat.opaque: false
  exclusionMode: ExclusionMode.Ignore
  anchors { top: true; bottom: true; left: true; right: true }
  WlrLayershell.namespace: "poste-bandeau"
  WlrLayershell.layer: WlrLayer.Overlay
  WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
  mask: Region { item: hitArea }

  // Seule zone qui prend la souris : les cartes de notification. Enfant direct de la fenêtre,
  // pour que le masque suive quand la pile monte ou descend.
  Item {
    id: hitArea
    x: notifs.x
    y: notifs.y + notifs.height - height
    width: notifs.width
    height: notifs.cardsHeight
  }

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

  NotificationView {
    id: notifs
    bandeau: win.bandeau
    screenName: win.screenName
    anchors.horizontalCenter: parent.horizontalCenter
    anchors.bottom: parent.bottom
    anchors.bottomMargin: win.bandeau.bottomMargin + dictation.space + osdView.space
    // Hauteur fixe : seule la marge du bas bouge quand la dictée ou l'OSD arrivent.
    height: Math.round(win.height * 0.6)
  }
}
