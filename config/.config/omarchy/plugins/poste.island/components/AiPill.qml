import QtQuick

// Pill of an AI conversation, at the top right of the screen (mirror of the
// timers on the left; one per conversation, see AskService.docked): the AI is working ('Thinking…',
// 'Searching the web…', with the loading reflection), or its answer
// is waiting ('Answer ready', accent dot), or it is pinned there (its
// question). Click: reopens the conversation. The ✕ on hover stops the AI if
// it is working, otherwise removes the pin.
Rectangle {
  id: pill
  required property var host
  required property var conv          // the AiConversation it summarizes
  readonly property bool ready: conv.unseen && !conv.busy
  // Compact (room missing, see IslandWindow): the icon alone, the state in
  // color; on hover, its label in a bubble below (the width does not
  // change: the row must not slide on the island).
  property bool compact: false
  readonly property bool small: compact
  // Bubble shown: the pill or the bubble itself is hovered.
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

  // Compact, answer ready: accent badge in the corner.
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
      // Compact and working: the icon breathes.
      SequentialAnimation on opacity {
        running: pill.small && pill.conv.busy
        loops: Animation.Infinite
        onRunningChanged: if (!running) glyphText.opacity = 1
        NumberAnimation { to: 0.35; duration: 700; easing.type: Easing.InOutSine }
        NumberAnimation { to: 1; duration: 700; easing.type: Easing.InOutSine }
      }
    }
    // Running: the label crossed by the loading reflection (Shimmer.qml).
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
    // Pinned, idle: the question, to know which conversation it is.
    Text {
      anchors.verticalCenter: parent.verticalCenter
      visible: pill.conv.pinned && !pill.conv.busy && !pill.ready && !pill.small
      width: Math.min(implicitWidth, 220)
      // Cut in the middle: start and end of the question stay readable (two
      // questions that start the same are told apart by their end).
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
    // On hover: ✕ stops the AI if it is working, otherwise removes the pin.
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

  // Compact mode bubble: label, and ✕ (stop or unpin).
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
