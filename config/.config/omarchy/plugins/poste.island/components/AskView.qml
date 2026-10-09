import QtQuick
import Quickshell
import Quickshell.Io

// Question à l'IA, dans l'île (« ai » + espace dans le lanceur, la pastille
// IA, ou omarchy-shell poste.island ask "…"). Façon Quick AI de Raycast : la
// réponse arrive au fil de l'eau sous la question ; une relance garde le
// contexte mais n'affiche que le dernier échange. La conversation vit dans
// AskService (host.ai), commune aux écrans : fermer la vue ne l'arrête pas.
//
// Clavier : ↵ envoie, Échap ferme (l'IA continue en arrière-plan), Ctrl+.
// arrête la réponse, Ctrl+N nouvelle conversation, Tab change de
// fournisseur, Ctrl+C (sans texte sélectionné dans le champ) copie la
// dernière réponse.
Item {
  id: ask
  required property var host
  property bool active: false
  property real maxHeight: 600
  signal closeRequested()
  readonly property bool pinned: c.pinned
  // Épingler : la conversation part dans la pastille de droite, la vue se ferme.
  // Déjà épinglée : on la désépingle et on reste dans la vue.
  function pinRequested() {
    if (c.pinned) { c.pinned = false; return }
    if (messages.count === 0) return
    c.pinned = true
    closeRequested()
  }

  readonly property var ai: host.ai
  readonly property var c: ai.current   // la conversation affichée (AiConversation)
  readonly property var messages: c.messages
  readonly property bool busy: c.busy

  // Ouverture : avec une question à envoyer, c'est une nouvelle conversation ;
  // sans, on retrouve la conversation en cours (et sa réponse, même arrivée
  // pendant que la vue était fermée).
  // Ouverture (lanceur, commande ask) : toujours une conversation neuve ; les
  // épinglées et celles qui travaillent restent dans leurs pastilles.
  function start(text, sendNow) {
    ai.newConversation()
    if (sendNow && String(text || "").trim() !== "") {
      ai.current.send(text)
      field.text = ""
    } else {
      field.text = text || ""
      field.cursorPosition = field.text.length
    }
    scrollDown()
    Qt.callLater(function() { field.forceActiveFocus() })
  }
  // Rouvrir une conversation existante (clic sur sa pastille).
  function resume(conversation) {
    ai.show(conversation)
    field.text = ""
    scrollDown()
    Qt.callLater(function() { field.forceActiveFocus() })
  }
  function send() {
    if (c.send(field.text)) { field.text = ""; scrollDown() }
  }
  function scrollDown() { Qt.callLater(function() { flick.contentY = Math.max(0, flick.contentHeight - flick.height) }) }
  Connections {
    target: ask.c
    function onRevisionChanged() { if (ask.active) ask.scrollDown() }
  }

  onActiveChanged: {
    ai.openViews += active ? 1 : -1
    if (active) { scrollDown(); Qt.callLater(function() { field.forceActiveFocus() }) }
  }
  Component.onDestruction: if (active) ai.openViews -= 1

  // ---------------------------------------------------------------- layout
  readonly property real headerHeight: 30
  readonly property real footerHeight: 40
  readonly property real chrome: headerHeight + 12 + footerHeight + 2
  readonly property real transcriptHeight: messages.count === 0 ? 0
    : Math.min(Math.max(60, maxHeight - chrome), column.implicitHeight)
  implicitHeight: headerHeight + (messages.count ? 12 + transcriptHeight : 0) + footerHeight + 2

  Item {
    id: header
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.top: parent.top
    height: ask.headerHeight
    Glyph {
      id: glyph
      anchors.left: parent.left
      anchors.leftMargin: 4
      anchors.verticalCenter: parent.verticalCenter
      glyph: "󰚩"
      color: ask.busy ? ask.host.colorAccent : ask.host.colorMuted
      fontFamily: ask.host.fontFamily
      pixelSize: 16
    }
    Text {
      id: crumb
      anchors.left: glyph.right
      anchors.leftMargin: 10
      anchors.verticalCenter: parent.verticalCenter
      text: ask.c.providerName + "  ›"
      color: ask.host.colorText
      font.family: ask.host.uiFont
      font.pixelSize: 15
      font.weight: Font.DemiBold
      MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: ask.c.cycleProvider() }
    }
    // Épingle : la vue reste ouverte pendant qu'on travaille ailleurs.
    Rectangle {
      id: pin
      anchors.right: parent.right
      anchors.rightMargin: 4
      anchors.verticalCenter: parent.verticalCenter
      width: 26; height: 26; radius: 8
      color: ask.pinned ? ask.host.withAlpha(ask.host.colorAccent, 0.18)
        : (pinMouse.containsMouse ? ask.host.withAlpha(ask.host.colorText, 0.08) : "transparent")
      Glyph {
        anchors.centerIn: parent
        glyph: ask.pinned ? "󰐃" : "󰤱"
        color: ask.pinned ? ask.host.colorAccent : ask.host.colorMuted
        fontFamily: ask.host.fontFamily
        pixelSize: 14
      }
      MouseArea { id: pinMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: ask.pinRequested() }
    }
    TextInput {
      id: field
      anchors.left: crumb.right
      anchors.leftMargin: 10
      anchors.right: pin.left
      anchors.rightMargin: 8
      anchors.verticalCenter: parent.verticalCenter
      focus: ask.active
      color: ask.host.colorText
      selectionColor: ask.host.withAlpha(ask.host.colorAccent, 0.4)
      font.family: ask.host.uiFont
      font.pixelSize: 17
      clip: true
      Text {
        anchors.fill: parent
        visible: !field.text
        text: messages.count ? "Follow up…" : "Ask anything…"
        color: ask.host.colorMuted
        font: field.font
      }
      Keys.onPressed: function(event) {
        var ctrl = (event.modifiers & Qt.ControlModifier) !== 0
        if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) { ask.send(); event.accepted = true }
        else if (event.key === Qt.Key_Escape) { ask.closeRequested(); event.accepted = true }
        else if (ctrl && event.key === Qt.Key_Period) { ask.c.stop(); event.accepted = true }
        else if (ctrl && event.key === Qt.Key_P) { ask.pinRequested(); event.accepted = true }
        else if (event.key === Qt.Key_Tab || event.key === Qt.Key_Backtab) { ask.c.cycleProvider(); event.accepted = true }
        else if (ctrl && event.key === Qt.Key_N) { ask.ai.newConversation(); field.text = ""; event.accepted = true }
        else if (ctrl && event.key === Qt.Key_C && field.selectedText === "" && ask.c.lastAnswer !== "") {
          Quickshell.execDetached(["wl-copy", "--", ask.c.lastAnswer]); event.accepted = true
        }
        else if (event.key === Qt.Key_PageUp) { flick.contentY = Math.max(0, flick.contentY - flick.height * 0.8); event.accepted = true }
        else if (event.key === Qt.Key_PageDown) { flick.contentY = Math.min(flick.contentHeight - flick.height, flick.contentY + flick.height * 0.8); event.accepted = true }
      }
    }
  }

  // Dernier échange seulement (comme le Quick AI de Raycast) : la question en
  // petit, la réponse en dessous. Le contexte reste pour les relances.
  readonly property int lastIndex: c.lastIndex
  readonly property string answerText: c.answerText
  readonly property bool answerError: c.lastError
  readonly property string questionText: c.question

  Flickable {
    id: flick
    anchors.top: header.bottom
    anchors.topMargin: 12
    anchors.left: parent.left
    anchors.right: parent.right
    height: ask.transcriptHeight
    visible: messages.count > 0
    clip: true
    contentHeight: column.implicitHeight
    boundsBehavior: Flickable.StopAtBounds

    Column {
      id: column
      width: flick.width
      spacing: 8
      Text {
        width: parent.width
        leftPadding: 8
        rightPadding: 8
        text: ask.questionText
        elide: Text.ElideRight
        maximumLineCount: 2
        wrapMode: Text.Wrap
        color: ask.host.colorMuted
        font.family: ask.host.uiFont
        font.pixelSize: 12
      }
      // Chargement : lignes fantômes balayées par un reflet (Shimmer.qml), et ce
      // que l'IA fait en ce moment (outil en cours) ou « Thinking… ».
      Shimmer {
        x: 8
        width: parent.width - 16
        visible: ask.busy && answer.text === ""
        host: ask.host
        label: ask.ai.toolLabel(ask.c.currentTool)
        lines: [0.94, 0.8, 0.52]
      }
      TextEdit {
        id: answer
        width: parent.width
        visible: text !== ""
        leftPadding: 8
        rightPadding: 8
        readOnly: true
        selectByMouse: true
        wrapMode: TextEdit.Wrap
        textFormat: ask.answerError ? TextEdit.PlainText : TextEdit.MarkdownText
        text: ask.answerText
        color: ask.answerError ? "#e5484d" : ask.host.colorText
        selectionColor: ask.host.withAlpha(ask.host.colorAccent, 0.4)
        font.family: ask.host.uiFont
        font.pixelSize: 14
        onLinkActivated: function(link) { Quickshell.execDetached(["xdg-open", link]) }
      }
    }
  }

  // ---------------------------------------------------------------- footer
  Item {
    id: footer
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.bottom: parent.bottom
    height: ask.footerHeight
    Rectangle { anchors.top: parent.top; width: parent.width; height: 1; color: ask.host.withAlpha(ask.host.colorText, 0.06) }
    Row {
      anchors.right: parent.right
      anchors.rightMargin: 14
      anchors.verticalCenter: parent.verticalCenter
      spacing: 14
      component Hint: Row {
        property string keys: ""
        property string label: ""
        spacing: 6
        Rectangle {
          anchors.verticalCenter: parent.verticalCenter
          width: keyText.implicitWidth + 10; height: 16; radius: 4
          color: ask.host.withAlpha(ask.host.colorText, 0.08)
          Text { id: keyText; anchors.centerIn: parent; text: keys; color: ask.host.colorMuted; font.family: ask.host.uiFont; font.pixelSize: 10; font.weight: Font.DemiBold }
        }
        Text { anchors.verticalCenter: parent.verticalCenter; text: label; color: ask.host.colorMuted; font.family: ask.host.uiFont; font.pixelSize: 11 }
      }
      Hint { visible: ask.busy; keys: "Ctrl ."; label: "Stop" }
      Hint { visible: messages.count > 0; keys: "Ctrl P"; label: ask.pinned ? "Unpin" : "Pin right" }
      Hint { keys: ask.busy ? "Esc" : "↵"; label: ask.busy ? "Hide" : "Send" }
      Hint { visible: ask.c.lastAnswer !== ""; keys: "Ctrl C"; label: "Copy answer" }
      Hint { visible: messages.count > 0; keys: "Ctrl N"; label: "New" }
      Hint {
        visible: messages.count === 0
        readonly property string next: ask.ai.providerIds[(ask.ai.providerIds.indexOf(ask.c.provider) + 1) % ask.ai.providerIds.length] || ""
        keys: "Tab"; label: next ? (ask.ai.providerNames[next] || next) : ""
      }
    }
  }
}
