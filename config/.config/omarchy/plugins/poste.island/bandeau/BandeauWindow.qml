import QtQuick
import Quickshell
import Quickshell.Wayland

// The banner's window on a screen: the whole screen, fixed size (a piece that arrives or
// leaves only changes the content, the compositor never resizes the surface), above
// everything, without keyboard. The mouse goes through everywhere except on the notification cards.
//
// At the top, the stack, just under the bottom edge of the island as it is drawn: it follows its
// spring frame by frame, so it goes down under a view that opens and goes up when it
// closes, without ever covering it. From top to bottom: the dictated text, the OSD, the
// notifications. Each piece keeps the place it takes (`space`, animated), those
// below slide when it arrives or leaves. The OSD and the dictated text go to the active screen;
// a notification stays on the screen where it appeared (rule of the service).
//
// At the bottom of the active screen, alone: the dictation pill.
PanelWindow {
  id: win
  required property var bandeau

  readonly property string screenName: screen ? String(screen.name) : ""
  readonly property bool focused: bandeau.focusedScreen === "" || bandeau.focusedScreen === screenName

  // Top of the stack: a gap under this screen's island (the top of the screen without an island).
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

  // Only area that takes the mouse: the notification cards, plus the gap above (a
  // card that comes in goes down from there). Direct child of the window, so that the mask follows when
  // the stack goes up or down.
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
