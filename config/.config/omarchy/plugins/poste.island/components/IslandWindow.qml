// One island per screen. Adapted in part from Island
// (https://github.com/Guilhermerisu/island), MIT License,
// Copyright (c) 2026 Guilherme Pimenta. See ../LICENSE-island.
import QtQuick
import Quickshell
import Quickshell.Hyprland
import Quickshell.Wayland
import Quickshell.Widgets

PanelWindow {
  id: window
  required property var host
  // Fenêtre plein écran, transparente, qui ne réserve aucune place : sans ces
  // trois lignes, elle couvre les écrans de blanc et descend sous la barre.
  color: "transparent"
  surfaceFormat.opaque: false
  exclusionMode: ExclusionMode.Ignore

  readonly property string screenName: screen ? String(screen.name) : ""
  readonly property bool focusedScreen: host.focusedScreen === screenName
  readonly property int offset: host.workspaceOffset(screenName)

  // "rest", "player" or "controls"; per screen.
  property string view: "rest"
  readonly property bool playerOpen: view === "player"
  readonly property bool controlsOpen: view === "controls"
  readonly property bool notificationsOpen: view === "notifications"
  readonly property bool launcherOpen: view === "launcher"
  readonly property bool menuOpen: view === "menu"
  readonly property bool wifiOpen: view === "wifi"
  readonly property bool bluetoothOpen: view === "bluetooth"
  readonly property bool audioOpen: view === "audio"
  readonly property bool vpnOpen: view === "vpn"
  readonly property bool tailscaleOpen: view === "tailscale"
  readonly property bool shelfOpen: view === "shelf"
  readonly property bool askOpen: view === "ask"
  readonly property bool usageOpen: view === "usage"
  // Control-center modules grown inside the island (level 2).
  readonly property bool moduleOpen: wifiOpen || bluetoothOpen || audioOpen || vpnOpen || tailscaleOpen
  readonly property bool surfaceOpen: playerOpen || controlsOpen || notificationsOpen || launcherOpen || menuOpen || moduleOpen || shelfOpen || askOpen || usageOpen
  readonly property bool restPill: !surfaceOpen

  Connections {
    target: window.host
    function onCloseAll() { window.view = "rest" }
    function onOpenPlayerOn(name) { if (name === window.screenName && window.host.player) window.view = "player" }
    function onToggleControlsOn(name) { if (name === window.screenName) window.view = window.controlsOpen ? "rest" : "controls" }
    function onOpenModuleOn(name, module) { if (name === window.screenName) window.view = window.view === module ? "rest" : module }
    function onToggleMenuOn(name) { if (name === window.screenName) window.view = window.menuOpen ? "rest" : "menu" }
    function onOpenAskOn(name, text, sendNow) {
      if (name !== window.screenName) return
      // Conversation neuve d'abord, vue ensuite : sinon la vue affiche un
      // instant l'ancienne conversation courante et la marque « lue » (une
      // réponse épinglée perdait son « Answer ready »).
      askView.start(text, sendNow)
      window.view = "ask"
    }
    function onToggleLauncherOn(name) { if (name === window.screenName) window.view = window.launcherOpen ? "rest" : "launcher" }
    function onOpenLauncherExtOn(name, ext) {
      if (name !== window.screenName) return
      window.view = "launcher"
      // Après la remise à zéro du lanceur à son ouverture.
      Qt.callLater(function() { launcherView.enterExtension(ext) })
    }
    function onToggleNotificationsOn(name) { if (name === window.screenName) window.view = window.notificationsOpen ? "rest" : "notifications" }
  }

  // « Advanced settings… » of a module: Omarchy's own panel, under the island.
  function openNative(id) {
    view = "rest"
    host.toggleNativePanel(id, screenName)
  }

  // Tell the notification service where this island ends and what is open.
  readonly property real reportedBottom: island.y + island.targetHeight
  function report() { host.reportState(screenName, view, Math.max(host.restTop + host.restHeight, reportedBottom)) }
  onViewChanged: report()
  onReportedBottomChanged: report()
  Component.onCompleted: report()

  // Tout l'écran, transparent : d'ordinaire seule la bulle prend la souris
  // (mask), mais une vue au clavier exclusif capte tout l'écran (voir catcher).
  anchors { top: true; bottom: true; left: true; right: true }
  WlrLayershell.namespace: "poste-island"
  // Overlay, above the dim (Top) whatever order the two surfaces mapped in.
  WlrLayershell.layer: WlrLayer.Overlay
  // The launcher needs the keyboard the moment it opens (a mapped surface
  // switched to OnDemand isn't focused by Hyprland), so it takes it exclusively.
  // Exclusif seulement à l'ouverture (le temps que Hyprland donne le clavier),
  // puis « à la demande » sous le focus grab : il garde le clavier sur l'île
  // et referme au clic extérieur sur n'importe quel écran (l'exclusif bloque
  // les clics partout et désactive le grab).
  WlrLayershell.keyboardFocus: exclusiveOpen && !focusGrab.armed ? WlrKeyboardFocus.Exclusive
    : surfaceOpen ? WlrKeyboardFocus.OnDemand : WlrKeyboardFocus.None
  // Vues au clavier exclusif (pour taper dès l'ouverture) : Hyprland ne
  // donne alors la souris à aucune autre surface, ni fenêtre ni voile, et ne
  // signale pas le clic extérieur au focus grab. L'île capte donc elle-même
  // tout l'écran, et un clic hors de la bulle la referme.
  readonly property bool exclusiveOpen: launcherOpen || menuOpen || moduleOpen || controlsOpen || shelfOpen || askOpen || usageOpen
  readonly property bool dimWanted: surfaceOpen
  // Zones souris : l'île et la bande des activités ; plus la carte dépliée
  // quand une activité l'est.
  // Zones souris : l'île, les deux bandes d'activités et leur carte dépliée
  // (un élément vide quand rien n'est déplié).
  Region {
    id: islandRegion
    item: island
    Region { item: activities }
    Region { item: ambient }
    Region { item: capture }
    Region { x: capture.x + recCard.x; y: capture.y + recCard.y; width: recCard.visible ? recCard.width : 0; height: recCard.visible ? recCard.height : 0 }
    Region { x: activities.x + activities.cardRect.x; y: activities.y + activities.cardRect.y; width: activities.cardRect.width; height: activities.cardRect.height }
    Region { x: ambient.x + ambient.cardRect.x; y: ambient.y + ambient.cardRect.y; width: ambient.cardRect.width; height: ambient.cardRect.height }
  }
  Region { id: screenRegion; item: catcher }
  mask: exclusiveOpen ? screenRegion : islandRegion
  MouseArea {
    id: catcher
    anchors.fill: parent
    z: -1
    enabled: window.exclusiveOpen
    acceptedButtons: Qt.AllButtons
    onPressed: window.view = "rest"
  }

  // Clic ailleurs, sur n'importe quel écran : l'île se referme.
  HyprlandFocusGrab {
    id: focusGrab
    windows: [window]
    property bool armed: false
    active: window.surfaceOpen && armed
    onCleared: window.view = "rest"
  }
  Timer {
    interval: 120
    running: window.surfaceOpen
    onTriggered: focusGrab.armed = true
  }
  onSurfaceOpenChanged: if (!surfaceOpen) focusGrab.armed = false

  // Enregistrement d'écran (OBS) : une
  // pastille à part, juste à gauche de l'île, dans la hauteur de la barre (ne
  // déborde pas sur les fenêtres). Les états qui durent (musique, batterie)
  // viennent après elle, plus à gauche. Survol de la partie OBS : sa carte.
  Item {
    id: capture
    readonly property bool vox: false   // la dictée a sa pastille sous l'île (voxDock)
    readonly property bool rec: !!window.host.obs && window.host.obs.active
    readonly property bool shown: (vox || rec) && !window.surfaceOpen && !island.tucked
    readonly property real overlap: 0
    x: island.x - width - (shown ? 8 : 0)
    y: island.y
    height: window.host.restHeight
    width: shown ? captureRow.implicitWidth + 26 : 0
    opacity: shown ? 1 : 0
    scale: shown ? 1 : 0.6
    Behavior on opacity { NumberAnimation { duration: 200 } }
    Behavior on scale { NumberAnimation { duration: 380 * window.host.motionScale; easing.type: Easing.OutBack; easing.overshoot: 1.4 } }
    Behavior on width { NumberAnimation { duration: 320 * window.host.motionScale; easing.type: Easing.OutQuint } }
    visible: opacity > 0.01

    Rectangle {
      anchors.fill: parent
      radius: height / 2
      color: window.host.colorIsland
      border.width: 1
      border.color: window.host.withAlpha(window.host.colorText, 0.07)
    }
    Row {
      id: captureRow
      x: 13
      anchors.verticalCenter: parent.verticalCenter
      spacing: 10
      opacity: capture.shown ? 1 : 0
      Behavior on opacity { NumberAnimation { duration: 180 } }
      VoxPill {
        anchors.verticalCenter: parent.verticalCenter
        visible: capture.vox
        host: window.host
      }
      Rectangle {
        anchors.verticalCenter: parent.verticalCenter
        visible: capture.vox && capture.rec
        width: 1; height: 14
        color: window.host.withAlpha(window.host.colorText, 0.15)
      }
      Row {
        id: recPart
        anchors.verticalCenter: parent.verticalCenter
        visible: capture.rec
        spacing: 7
        Rectangle {
          id: capDot
          anchors.verticalCenter: parent.verticalCenter
          width: 9; height: 9; radius: 4.5
          color: window.host.obs && window.host.obs.paused ? "#ffd60a" : "#ff453a"
          SequentialAnimation on opacity {
            running: capture.rec && !(window.host.obs && window.host.obs.paused)
            loops: Animation.Infinite
            onRunningChanged: if (!running) capDot.opacity = 1
            NumberAnimation { to: 0.3; duration: 650; easing.type: Easing.InOutSine }
            NumberAnimation { to: 1; duration: 650; easing.type: Easing.InOutSine }
          }
        }
        Text {
          anchors.verticalCenter: parent.verticalCenter
          text: !window.host.obs ? "" : window.host.obs.recording
            ? (window.host.obs.paused ? "Paused" : window.host.formatLeft(window.host.obs.recordElapsed))
            : "LIVE " + window.host.formatLeft(window.host.obs.streamElapsed)
          color: window.host.obs && window.host.obs.paused ? "#ffd60a" : "#ff453a"
          font.family: window.host.uiFont
          font.pixelSize: 13
          font.weight: Font.DemiBold
          font.features: { "tnum": 1 }
        }
      }
    }
    // Carte OBS au survol (un instant) de la pastille ; reste tant que la
    // souris est sur la pastille ou la carte.
    HoverHandler { id: captureHover }
    property bool cardOpen: false
    Timer { interval: 320; running: captureHover.hovered && capture.rec && !capture.cardOpen; onTriggered: capture.cardOpen = true }
    Timer { interval: 380; running: capture.cardOpen && !captureHover.hovered && !recCard.hovered; onTriggered: capture.cardOpen = false }
    onRecChanged: if (!rec) cardOpen = false
    ActivityCard {
      id: recCard
      host: window.host
      activity: ({ kind: "obs" })
      x: Math.max(-capture.x + 8, (capture.width - width) / 2)
      y: capture.height + 8
      readonly property bool shownNow: capture.cardOpen && capture.shown
      opacity: shownNow ? 1 : 0
      scale: shownNow ? 1 : 0.94
      transformOrigin: Item.Top
      visible: opacity > 0.01
      Behavior on opacity { NumberAnimation { duration: recCard.shownNow ? 180 : 130; easing.type: Easing.OutCubic } }
      Behavior on scale { NumberAnimation { duration: recCard.shownNow ? 320 * window.host.motionScale : 140; easing.type: recCard.shownNow ? Easing.OutBack : Easing.InCubic; easing.overshoot: 1.2 } }
      onDone: capture.cardOpen = false
    }
  }

  Rectangle {
    id: island
    x: (parent.width - width) / 2
    // Hidden (slid up) when the bar is toggled off, or over a fullscreen
    // window on this screen, unless it has something transient to show.
    readonly property bool tucked: window.restPill && !window.host.voxActive
      && (window.host.barHidden || window.host.fullscreenScreens[window.screenName] === true)
    y: tucked ? -height - 12 : window.host.restTop
    opacity: tucked ? 0 : 1
    Behavior on opacity { NumberAnimation { duration: 260 * window.host.motionScale; easing.type: Easing.OutCubic } }
    Behavior on y { NumberAnimation { duration: 300 * window.host.motionScale; easing.type: Easing.OutCubic } }

    readonly property real targetWidth: window.menuOpen ? 520
      : window.shelfOpen ? 580
      : window.askOpen ? 620
      : window.usageOpen ? 600
      : window.launcherOpen ? 580
      : window.notificationsOpen ? 440
      : window.controlsOpen ? (controlCenter.tileCount > 5 ? 400 : 360)
      : window.wifiOpen || window.bluetoothOpen || window.vpnOpen || window.tailscaleOpen ? 400
      : window.audioOpen ? 420
      : window.playerOpen ? 420
      : restRow.implicitWidth + 30
    // 16 px above the launcher, 12 px below its footer.
    readonly property real targetHeight: window.menuOpen ? menuView.implicitHeight + 32
      : window.shelfOpen ? shelfView.implicitHeight + 16
      : window.askOpen ? askView.implicitHeight + 16
      : window.usageOpen ? usageView.implicitHeight + 32
      : window.launcherOpen ? launcherView.implicitHeight + 16
      : window.notificationsOpen ? notificationList.implicitHeight + 36
      : window.controlsOpen ? controlCenter.implicitHeight + 40
      : window.wifiOpen ? wifiView.implicitHeight + 36
      : window.bluetoothOpen ? bluetoothView.implicitHeight + 36
      : window.audioOpen ? audioView.implicitHeight + 36
      : window.vpnOpen ? vpnView.implicitHeight + 36
      : window.tailscaleOpen ? tailscaleView.implicitHeight + 36
      : window.playerOpen ? playerView.implicitHeight + 44
      : window.host.restHeight

    readonly property real springStiffness: 6.5 / window.host.motionScale
    property real springWidth: targetWidth
    property real springHeight: targetHeight
    Behavior on springWidth { SpringAnimation { spring: island.springStiffness; damping: 0.4; mass: 1; epsilon: 0.2 } }
    Behavior on springHeight { SpringAnimation { spring: island.springStiffness; damping: 0.4; mass: 1; epsilon: 0.2 } }
    width: Math.max(40, springWidth)
    height: Math.max(26, springHeight)

    property real radiusCap: window.surfaceOpen ? 28 : 36
    Behavior on radiusCap { NumberAnimation { duration: 390 * window.host.motionScale; easing.type: Easing.OutQuint } }
    radius: Math.min(height / 2, radiusCap)

    color: window.host.colorIsland
    border.width: 1
    border.color: window.host.withAlpha(window.host.colorText, 0.07)
    clip: true

    scale: window.restPill && hover.hovered ? 1.04 : 1
    Behavior on scale { NumberAnimation { duration: 240 * window.host.motionScale; easing.type: Easing.OutBack; easing.overshoot: 1.6 } }
    HoverHandler { id: hover }

    // ---------------------------------------------------------------- rest
    Row {
      id: restRow
      // Pinned to the resting strip (not the island's center) and faded out
      // in every unfolded view, so it never shows through a panel.
      anchors.horizontalCenter: parent.horizontalCenter
      y: (window.host.restHeight - height) / 2
      spacing: 14
      opacity: window.restPill ? 1 : 0
      visible: opacity > 0.01
      Behavior on opacity { NumberAnimation { duration: window.restPill ? 220 * window.host.motionScale : 90; easing.type: Easing.InOutQuad } }

      WorkspaceStrip {
        anchors.verticalCenter: parent.verticalCenter
        host: window.host
        screenName: window.screenName
        offset: window.offset
      }

      Rectangle {
        anchors.verticalCenter: parent.verticalCenter
        width: 1; height: 14
        color: window.host.withAlpha(window.host.colorText, 0.12)
      }

      Row {
        anchors.verticalCenter: parent.verticalCenter
        spacing: 5
      // Only while a VPN tunnel is up.
      Glyph {
        anchors.verticalCenter: parent.verticalCenter
        visible: window.host.vpnConnected
        glyph: "󰌾"
        color: window.host.withAlpha(window.host.colorText, 0.55)
        fontFamily: window.host.fontFamily
        pixelSize: 12
      }
      Text {
        id: clockLabel
        anchors.verticalCenter: parent.verticalCenter
        MouseArea {
          anchors.fill: parent
          anchors.margins: -6
          cursorShape: Qt.PointingHandCursor
          onClicked: window.view = "controls"
        }
        text: Qt.formatDateTime(window.host.clockDate, "HH:mm")
        color: window.host.colorText
        font.family: window.host.uiFont
        font.pixelSize: 14
        font.weight: Font.DemiBold
        font.features: { "tnum": 1, "case": 1 }
        font.letterSpacing: -0.3
      }
      }

      // Notifications dot: orange while something is unread (red if one of them
      // is critical), a discreet grey
      // once everything is read but the history still holds some, gone when
      // the history is empty. Click opens the list in the island.
      Item {
        id: unreadDot
        readonly property bool unread: window.host.unreadCount > 0
        readonly property bool shown: unread || window.host.notificationList.length > 0
        anchors.verticalCenter: parent.verticalCenter
        width: shown ? 8 : 0
        height: 8
        visible: width > 0.5
        Behavior on width { NumberAnimation { duration: 260 * window.host.motionScale; easing.type: Easing.OutQuint } }
        Rectangle {
          id: dotShape
          anchors.centerIn: parent
          width: 8; height: 8; radius: 4
          color: unreadDot.unread && window.host.unreadImportant ? "#ff453a"
            : unreadDot.unread ? window.host.colorAccent : window.host.withAlpha(window.host.colorText, 0.32)
          Behavior on color { ColorAnimation { duration: 320 * window.host.motionScale; easing.type: Easing.InOutQuad } }
          scale: unreadDot.shown ? 1 : 0
          Behavior on scale { NumberAnimation { duration: 340 * window.host.motionScale; easing.type: Easing.OutBack; easing.overshoot: 2.2 } }
        }
        // Pop again when it turns orange.
        onUnreadChanged: if (unread && shown) popAnim.restart()
        SequentialAnimation {
          id: popAnim
          NumberAnimation { target: dotShape; property: "scale"; to: 1.6; duration: 140; easing.type: Easing.OutQuad }
          NumberAnimation { target: dotShape; property: "scale"; to: 1; duration: 320; easing.type: Easing.OutBack; easing.overshoot: 2.2 }
        }
        MouseArea {
          anchors.fill: parent
          anchors.margins: -8
          cursorShape: Qt.PointingHandCursor
          onClicked: window.view = "notifications"
        }
      }

      // Dictée : pastille sous l'île (voxDock) ; OBS : pastille à gauche (capture). Musique et autres
      // activités : de part et d'autre de l'île (ActivityStrip.qml).
    }

    // ---------------------------------------------------------------- controls
    ControlCenter {
      id: controlCenter
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.top: parent.top
      anchors.margins: 20
      host: window.host
      screenName: window.screenName
      active: window.controlsOpen
      opacity: window.controlsOpen ? 1 : 0
      visible: opacity > 0.01
      Behavior on opacity { NumberAnimation { duration: opacity > 0.5 ? 70 : 180 * window.host.motionScale; easing.type: Easing.InOutQuad } }
      transformOrigin: Item.Top
      scale: window.controlsOpen || window.moduleOpen ? 1 : 0.96
      Behavior on scale { NumberAnimation { duration: 320 * window.host.motionScale; easing.type: Easing.OutQuint } }
      // Slides out to the left while a module (Wi-Fi) grows in its place.
      transform: Translate {
        x: window.moduleOpen ? -40 : 0
        Behavior on x { NumberAnimation { duration: 300 * window.host.motionScale; easing.type: Easing.OutQuint } }
      }
      onCloseRequested: window.view = "rest"
      onModuleRequested: function(name) { window.view = name }
    }

    // ---------------------------------------------------------------- modules (from the control center)
    // Each view is clipped to the island while it morphs, so nothing hangs
    // out of the shape during the spring.
    WifiView {
      id: wifiView
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.top: parent.top
      anchors.margins: 18
      height: Math.max(0, island.height - 36)
      clip: true
      maxListHeight: (window.screen ? window.screen.height * 0.45 : 360)
      host: window.host
      active: window.wifiOpen
      opacity: window.wifiOpen ? 1 : 0
      visible: opacity > 0.01
      Behavior on opacity { NumberAnimation { duration: opacity > 0.5 ? 90 : 200 * window.host.motionScale; easing.type: Easing.InOutQuad } }
      transform: Translate {
        x: window.wifiOpen ? 0 : 40
        Behavior on x { NumberAnimation { duration: 300 * window.host.motionScale; easing.type: Easing.OutQuint } }
      }
      onBackRequested: window.view = "controls"
      onAdvancedRequested: window.openNative("omarchy.network")
    }
    BluetoothView {
      id: bluetoothView
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.top: parent.top
      anchors.margins: 18
      height: Math.max(0, island.height - 36)
      clip: true
      maxListHeight: (window.screen ? window.screen.height * 0.45 : 360)
      host: window.host
      active: window.bluetoothOpen
      opacity: window.bluetoothOpen ? 1 : 0
      visible: opacity > 0.01
      Behavior on opacity { NumberAnimation { duration: opacity > 0.5 ? 90 : 200 * window.host.motionScale; easing.type: Easing.InOutQuad } }
      transform: Translate {
        x: window.bluetoothOpen ? 0 : 40
        Behavior on x { NumberAnimation { duration: 300 * window.host.motionScale; easing.type: Easing.OutQuint } }
      }
      onBackRequested: window.view = "controls"
      onAdvancedRequested: window.openNative("omarchy.bluetooth")
    }
    AudioView {
      id: audioView
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.top: parent.top
      anchors.margins: 18
      height: Math.max(0, island.height - 36)
      clip: true
      maxListHeight: (window.screen ? window.screen.height * 0.5 : 420)
      host: window.host
      active: window.audioOpen
      opacity: window.audioOpen ? 1 : 0
      visible: opacity > 0.01
      Behavior on opacity { NumberAnimation { duration: opacity > 0.5 ? 90 : 200 * window.host.motionScale; easing.type: Easing.InOutQuad } }
      transform: Translate {
        x: window.audioOpen ? 0 : 40
        Behavior on x { NumberAnimation { duration: 300 * window.host.motionScale; easing.type: Easing.OutQuint } }
      }
      onBackRequested: window.view = "controls"
      onAdvancedRequested: window.openNative("omarchy.audio")
    }
    VpnView {
      id: vpnView
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.top: parent.top
      anchors.margins: 18
      height: Math.max(0, island.height - 36)
      clip: true
      maxListHeight: (window.screen ? window.screen.height * 0.35 : 300)
      host: window.host
      active: window.vpnOpen
      opacity: window.vpnOpen ? 1 : 0
      visible: opacity > 0.01
      Behavior on opacity { NumberAnimation { duration: opacity > 0.5 ? 90 : 200 * window.host.motionScale; easing.type: Easing.InOutQuad } }
      transform: Translate {
        x: window.vpnOpen ? 0 : 40
        Behavior on x { NumberAnimation { duration: 300 * window.host.motionScale; easing.type: Easing.OutQuint } }
      }
      onBackRequested: window.view = "controls"
      onAdvancedRequested: window.openNative(window.host.vpnId)
    }
    TailscaleView {
      id: tailscaleView
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.top: parent.top
      anchors.margins: 18
      height: Math.max(0, island.height - 36)
      clip: true
      maxListHeight: (window.screen ? window.screen.height * 0.35 : 300)
      host: window.host
      active: window.tailscaleOpen
      opacity: window.tailscaleOpen ? 1 : 0
      visible: opacity > 0.01
      Behavior on opacity { NumberAnimation { duration: opacity > 0.5 ? 90 : 200 * window.host.motionScale; easing.type: Easing.InOutQuad } }
      transform: Translate {
        x: window.tailscaleOpen ? 0 : 40
        Behavior on x { NumberAnimation { duration: 300 * window.host.motionScale; easing.type: Easing.OutQuint } }
      }
      onBackRequested: window.view = "controls"
      onCloseRequested: window.view = "rest"
      onAdvancedRequested: window.openNative(window.host.tsId)
    }

    // ---------------------------------------------------------------- Omarchy menu
    MenuView {
      id: menuView
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.top: parent.top
      anchors.margins: 16
      height: implicitHeight
      maxHeight: (window.screen ? window.screen.height * 0.62 : 600) - 32
      host: window.host
      active: window.menuOpen
      opacity: window.menuOpen ? 1 : 0
      visible: opacity > 0.01
      Behavior on opacity { NumberAnimation { duration: opacity > 0.5 ? 70 : 180 * window.host.motionScale; easing.type: Easing.InOutQuad } }
      transformOrigin: Item.Top
      scale: window.menuOpen ? 1 : 0.96
      Behavior on scale { NumberAnimation { duration: 320 * window.host.motionScale; easing.type: Easing.OutQuint } }
      onCloseRequested: window.view = "rest"
      onOpenLauncher: window.view = "launcher"
    }

    // ---------------------------------------------------------------- launcher
    Launcher {
      id: launcherView
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.top: parent.top
      anchors.margins: 16
      height: implicitHeight
      // ~62 % of the screen height, minus the island's top offset and padding.
      maxHeight: (window.screen ? window.screen.height * 0.62 : 600) - 16
      host: window.host
      active: window.launcherOpen
      opacity: window.launcherOpen ? 1 : 0
      visible: opacity > 0.01
      Behavior on opacity { NumberAnimation { duration: opacity > 0.5 ? 70 : 180 * window.host.motionScale; easing.type: Easing.InOutQuad } }
      transformOrigin: Item.Top
      scale: window.launcherOpen ? 1 : 0.96
      Behavior on scale { NumberAnimation { duration: 320 * window.host.motionScale; easing.type: Easing.OutQuint } }
      onCloseRequested: window.view = "rest"
    }

    // ---------------------------------------------------------------- ask (IA)
    AskView {
      id: askView
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.top: parent.top
      anchors.margins: 16
      height: implicitHeight
      maxHeight: (window.screen ? window.screen.height * 0.7 : 640) - 16
      host: window.host
      active: window.askOpen
      opacity: window.askOpen ? 1 : 0
      visible: opacity > 0.01
      Behavior on opacity { NumberAnimation { duration: opacity > 0.5 ? 70 : 180 * window.host.motionScale; easing.type: Easing.InOutQuad } }
      transformOrigin: Item.Top
      scale: window.askOpen ? 1 : 0.96
      Behavior on scale { NumberAnimation { duration: 320 * window.host.motionScale; easing.type: Easing.OutQuint } }
      onCloseRequested: window.view = "rest"
    }

    // ---------------------------------------------------------------- usage (Claude Code)
    UsageView {
      id: usageView
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.top: parent.top
      anchors.margins: 16
      height: implicitHeight
      maxHeight: (window.screen ? window.screen.height * 0.75 : 700) - 32
      host: window.host
      active: window.usageOpen
      opacity: window.usageOpen ? 1 : 0
      visible: opacity > 0.01
      Behavior on opacity { NumberAnimation { duration: opacity > 0.5 ? 70 : 180 * window.host.motionScale; easing.type: Easing.InOutQuad } }
      transformOrigin: Item.Top
      scale: window.usageOpen ? 1 : 0.96
      Behavior on scale { NumberAnimation { duration: 320 * window.host.motionScale; easing.type: Easing.OutQuint } }
      onCloseRequested: window.view = "rest"
    }

    // ---------------------------------------------------------------- shelf (Flea)
    ShelfView {
      id: shelfView
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.top: parent.top
      anchors.margins: 16
      height: implicitHeight
      maxHeight: (window.screen ? window.screen.height * 0.62 : 600) - 16
      host: window.host
      active: window.shelfOpen
      opacity: window.shelfOpen ? 1 : 0
      visible: opacity > 0.01
      Behavior on opacity { NumberAnimation { duration: opacity > 0.5 ? 70 : 180 * window.host.motionScale; easing.type: Easing.InOutQuad } }
      transformOrigin: Item.Top
      scale: window.shelfOpen ? 1 : 0.96
      Behavior on scale { NumberAnimation { duration: 320 * window.host.motionScale; easing.type: Easing.OutQuint } }
      onCloseRequested: window.view = "rest"
    }

    // ---------------------------------------------------------------- notifications
    NotificationList {
      id: notificationList
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.top: parent.top
      anchors.margins: 18
      host: window.host
      active: window.notificationsOpen
      opacity: window.notificationsOpen ? 1 : 0
      visible: opacity > 0.01
      Behavior on opacity { NumberAnimation { duration: opacity > 0.5 ? 70 : 180 * window.host.motionScale; easing.type: Easing.InOutQuad } }
      transformOrigin: Item.Top
      scale: window.notificationsOpen ? 1 : 0.96
      Behavior on scale { NumberAnimation { duration: 320 * window.host.motionScale; easing.type: Easing.OutQuint } }
      onCloseRequested: window.view = "rest"
    }

    // ---------------------------------------------------------------- player
    PlayerView {
      id: playerView
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.top: parent.top
      anchors.margins: 22
      host: window.host
      active: window.playerOpen
      opacity: window.playerOpen ? 1 : 0
      visible: opacity > 0.01
      Behavior on opacity { NumberAnimation { duration: opacity > 0.5 ? 70 : 180 * window.host.motionScale; easing.type: Easing.InOutQuad } }
      transformOrigin: Item.Top
      scale: window.playerOpen ? 1 : 0.96
      Behavior on scale { NumberAnimation { duration: 320 * window.host.motionScale; easing.type: Easing.OutQuint } }
      onCloseRequested: window.view = "rest"
    }
  }

  // Dictée (voxtype) : pastille détachée sous l'île, sur l'écran actif : le
  // micro et l'onde de la voix, puis les points de la transcription (VoxPill).
  Rectangle {
    id: voxDock
    readonly property bool shown: window.host.voxActive && window.focusedScreen && !window.surfaceOpen
    x: island.x + (island.width - width) / 2
    y: island.y + window.host.restHeight + 8
    height: window.host.restHeight
    width: voxRow.implicitWidth + 28
    radius: height / 2
    color: window.host.colorIsland
    opacity: shown ? 1 : 0
    scale: shown ? 1 : 0.7
    transformOrigin: Item.Top
    visible: opacity > 0.01
    Behavior on opacity { NumberAnimation { duration: voxDock.shown ? 180 : 140 } }
    Behavior on scale { NumberAnimation { duration: voxDock.shown ? 360 * window.host.motionScale : 160; easing.type: voxDock.shown ? Easing.OutBack : Easing.InCubic; easing.overshoot: 1.3 } }
    Behavior on width { NumberAnimation { duration: 220 * window.host.motionScale; easing.type: Easing.OutQuint } }
    Row {
      id: voxRow
      x: 14
      anchors.verticalCenter: parent.verticalCenter
      spacing: 12
      VoxPill {
        anchors.verticalCenter: parent.verticalCenter
        host: window.host
      }
    }
  }


  // Activités en cours (ActivityStrip.qml), de part et d'autre de l'île : à
  // droite ce qui travaille en fond (minuteurs, IA), à gauche ce qui dure
  // (musique, batterie). La dictée est dans l'île même.
  ActivityStrip {
    id: activities
    host: window.host
    kinds: ["timer", "ai"]
    side: "right"
    x: island.x + island.width + 8
    y: island.y
    screenWidth: window.width
    roomRight: Math.max(0, window.width - x - 12)
    hidden: window.surfaceOpen || island.tucked
    onOpenConversation: function(c) { askView.resume(c); window.view = "ask" }
    onOpenPlayer: if (window.host.player) window.view = "player"
  }
  ActivityStrip {
    id: ambient
    host: window.host
    kinds: ["music", "battery", "device"]
    side: "left"
    x: (capture.visible ? capture.x : island.x) - width - 8
    y: island.y
    screenWidth: window.width
    roomRight: Math.max(0, island.x - 20)
    hidden: window.surfaceOpen || island.tucked
    onOpenConversation: function(c) { askView.resume(c); window.view = "ask" }
    onOpenPlayer: if (window.host.player) window.view = "player"
  }
}
