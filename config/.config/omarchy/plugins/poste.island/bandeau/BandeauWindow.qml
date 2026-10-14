import QtQuick
import Quickshell
import Quickshell.Wayland

// La fenêtre du bandeau sur un écran : tout l'écran, taille fixe (une pièce qui arrive ou
// part ne change que le contenu, le compositeur ne retaille jamais la surface), au-dessus de
// tout, sans clavier. La souris traverse partout sauf sur les cartes de notification.
//
// En haut, la pile, juste sous le bord bas de l'île telle qu'elle est dessinée : elle suit son
// ressort image par image, donc descend sous une vue qui s'ouvre et remonte quand elle se
// referme, sans jamais la recouvrir. Du haut vers le bas : le texte dicté, l'OSD, les
// notifications. Chaque pièce garde la place qu'elle prend (`space`, animée), celles du
// dessous glissent quand elle arrive ou part. L'OSD et le texte dicté vont sur l'écran actif ;
// une notification reste sur l'écran où elle est apparue (règle du service).
//
// En bas de l'écran actif, seule : la pastille de la dictée.
PanelWindow {
  id: win
  required property var bandeau

  readonly property string screenName: screen ? String(screen.name) : ""
  readonly property bool focused: bandeau.focusedScreen === "" || bandeau.focusedScreen === screenName

  // Haut de la pile : un écart sous l'île de cet écran (le haut de l'écran sans île).
  readonly property real stackTop: (bandeau.islandEdges[screenName] || 0) + bandeau.islandGap

  visible: dictation.live || osdView.live || notifs.live || pill.live
  color: "transparent"
  surfaceFormat.opaque: false
  exclusionMode: ExclusionMode.Ignore
  anchors { top: true; bottom: true; left: true; right: true }
  WlrLayershell.namespace: "poste-bandeau"
  WlrLayershell.layer: WlrLayer.Overlay
  WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
  mask: Region { item: hitArea }

  // Seule zone qui prend la souris : les cartes de notification, plus l'écart au-dessus (une
  // carte qui entre descend de là). Enfant direct de la fenêtre, pour que le masque suive quand
  // la pile monte ou descend.
  Item {
    id: hitArea
    x: notifs.x
    y: notifs.y - win.bandeau.gap
    width: notifs.width
    height: notifs.cardsHeight > 0 ? notifs.cardsHeight + win.bandeau.gap : 0
  }

  DictationView {
    id: dictation
    bandeau: win.bandeau
    active: win.focused && win.bandeau.dictTextShown
    anchors.horizontalCenter: parent.horizontalCenter
    y: win.stackTop
    maxTextWidth: Math.max(240, Math.min(900, win.width * 0.6))
  }

  OsdView {
    id: osdView
    bandeau: win.bandeau
    active: win.focused && win.bandeau.osdOpen
    anchors.horizontalCenter: parent.horizontalCenter
    y: win.stackTop + dictation.space
  }

  NotificationView {
    id: notifs
    bandeau: win.bandeau
    screenName: win.screenName
    anchors.horizontalCenter: parent.horizontalCenter
    y: win.stackTop + dictation.space + osdView.space
  }

  DictationPill {
    id: pill
    bandeau: win.bandeau
    active: win.focused && win.bandeau.dictActive
    anchors.horizontalCenter: parent.horizontalCenter
    y: win.height - win.bandeau.bottomMargin - height
  }
}
