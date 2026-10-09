import QtQuick

// Pastille d'une conversation IA, en haut à droite de l'écran (miroir des
// minuteurs à gauche ; une par conversation, voir AskService.docked) : l'IA travaille (« Thinking… »,
// « Searching the web… », avec le reflet du chargement), ou sa réponse
// attend (« Answer ready », point d'accent), ou elle y est épinglée (sa
// question). Clic : rouvre la conversation. Le ✕ au survol arrête l'IA si
// elle travaille, sinon retire l'épingle.
Rectangle {
  id: pill
  required property var host
  required property var conv          // l'AiConversation qu'elle résume
  readonly property bool ready: conv.unseen && !conv.busy
  // Compacte (place manquante, voir IslandWindow) : l'icône seule, l'état en
  // couleur ; au survol, son libellé dans une bulle dessous (la largeur ne
  // change pas : la rangée ne doit pas glisser sur l'île).
  property bool compact: false
  readonly property bool small: compact
  // Bulle affichée : la pastille ou la bulle elle-même est survolée.
  readonly property bool tipShown: small && (hover.hovered || tipHover.hovered)
  signal openRequested()

  height: host.restHeight
  width: small ? height : row.implicitWidth + 24
  radius: height / 2
  color: host.colorIsland
  Behavior on width { NumberAnimation { duration: 260 * pill.host.motionScale; easing.type: Easing.OutQuint } }

  HoverHandler { id: hover }
  MouseArea {
    anchors.fill: parent
    cursorShape: Qt.PointingHandCursor
    onClicked: pill.openRequested()
  }

  // Compacte, réponse prête : badge d'accent dans le coin.
  Rectangle {
    visible: pill.small && pill.ready
    anchors.right: parent.right
    anchors.top: parent.top
    anchors.rightMargin: 3
    anchors.topMargin: 3
    width: 8; height: 8; radius: 4
    color: pill.conv.lastError ? "#e5484d" : pill.host.colorAccent
    z: 2
  }
  Row {
    id: row
    anchors.centerIn: parent
    spacing: 8
    Glyph {
      id: glyphText
      anchors.verticalCenter: parent.verticalCenter
      glyph: "󰚩"
      color: pill.ready || (pill.small && pill.conv.busy) ? pill.host.colorAccent : pill.host.colorText
      fontFamily: pill.host.fontFamily
      pixelSize: 14
      // Compacte et au travail : l'icône respire.
      SequentialAnimation on opacity {
        running: pill.small && pill.conv.busy
        loops: Animation.Infinite
        onRunningChanged: if (!running) glyphText.opacity = 1
        NumberAnimation { to: 0.35; duration: 700; easing.type: Easing.InOutSine }
        NumberAnimation { to: 1; duration: 700; easing.type: Easing.InOutSine }
      }
    }
    // En cours : le libellé traversé par le reflet du chargement (Shimmer.qml).
    Item {
      anchors.verticalCenter: parent.verticalCenter
      visible: pill.conv.busy && !pill.small
      width: label.implicitWidth
      height: label.implicitHeight
      property real phase: 0
      NumberAnimation on phase { running: pill.conv.busy && pill.visible; from: 0; to: 1; duration: 1500; loops: Animation.Infinite }
      ShimmerText {
        id: label
        host: pill.host
        text: pill.host.ai.toolLabel(pill.conv.currentTool)
        band: 60
        sweepX: -60 + parent.phase * (implicitWidth + 120)
      }
    }
    // Épinglée, au repos : la question, pour savoir de quelle conversation il s'agit.
    Text {
      anchors.verticalCenter: parent.verticalCenter
      visible: pill.conv.pinned && !pill.conv.busy && !pill.ready && !pill.small
      width: Math.min(implicitWidth, 220)
      // Coupé au milieu : début et fin de la question restent lisibles (deux
      // questions qui commencent pareil se distinguent par leur fin).
      elide: Text.ElideMiddle
      text: pill.conv.question
      color: pill.host.colorText
      font.family: pill.host.uiFont
      font.pixelSize: 12
    }
    Text {
      anchors.verticalCenter: parent.verticalCenter
      visible: pill.ready && !pill.small
      text: pill.conv.lastError ? "Answer failed" : "Answer ready"
      color: pill.host.colorText
      font.family: pill.host.uiFont
      font.pixelSize: 12
      font.weight: Font.DemiBold
    }
    Rectangle {
      anchors.verticalCenter: parent.verticalCenter
      visible: pill.ready && !pill.small
      width: 7; height: 7; radius: 3.5
      color: pill.conv.lastError ? "#e5484d" : pill.host.colorAccent
    }
    // Au survol : ✕ arrête l'IA si elle travaille, sinon retire l'épingle.
    Rectangle {
      anchors.verticalCenter: parent.verticalCenter
      visible: (pill.conv.busy || pill.conv.pinned) && hover.hovered && !pill.small
      width: 18; height: 18; radius: 9
      color: pill.host.withAlpha(pill.host.colorText, stopMouse.containsMouse ? 0.2 : 0.1)
      Glyph { anchors.centerIn: parent; glyph: "󰅖"; color: pill.host.colorText; fontFamily: pill.host.fontFamily; pixelSize: 11 }
      MouseArea {
        id: stopMouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: { if (pill.conv.busy) pill.conv.stop(); else { pill.conv.pinned = false; pill.conv.unseen = false } }
      }
    }
  }

  // Bulle du mode compact : libellé, et ✕ (arrêter ou désépingler).
  Rectangle {
    visible: pill.tipShown
    anchors.top: parent.bottom
    anchors.topMargin: 2
    anchors.right: parent.right
    z: 5
    height: 26
    width: tipRow.implicitWidth + 20
    radius: 13
    color: pill.host.colorIsland
    border.width: 1
    border.color: pill.host.withAlpha(pill.host.colorText, 0.1)
    HoverHandler { id: tipHover }
    Row {
      id: tipRow
      anchors.centerIn: parent
      spacing: 8
      Text {
        anchors.verticalCenter: parent.verticalCenter
        width: Math.min(implicitWidth, 260)
        elide: Text.ElideMiddle
        text: pill.conv.busy ? pill.host.ai.toolLabel(pill.conv.currentTool)
          : (pill.ready ? (pill.conv.lastError ? "Answer failed · " : "Answer ready · ") + pill.conv.question : pill.conv.question)
        color: pill.host.colorText
        font.family: pill.host.uiFont
        font.pixelSize: 12
      }
      Glyph {
        anchors.verticalCenter: parent.verticalCenter
        visible: pill.conv.busy || pill.conv.pinned
        glyph: "󰅖"
        color: tipStop.containsMouse ? pill.host.colorText : pill.host.colorMuted
        fontFamily: pill.host.fontFamily
        pixelSize: 12
        MouseArea {
          id: tipStop
          anchors.fill: parent
          anchors.margins: -4
          hoverEnabled: true
          cursorShape: Qt.PointingHandCursor
          onClicked: { if (pill.conv.busy) pill.conv.stop(); else { pill.conv.pinned = false; pill.conv.unseen = false } }
        }
      }
    }
  }
}
