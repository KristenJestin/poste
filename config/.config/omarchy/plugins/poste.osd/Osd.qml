import QtQuick
import Quickshell.Io
import "../poste.island/bandeau"

// OSD de poste : clone de omarchy.osd (volume, luminosité, micro, musique, alimentation).
// Omarchy y envoie tout ce qui vise omarchy.osd : shell.summon("omarchy.osd", payload) appelle
// open(payload) d'un clone actif, et omarchy-osd passe par `omarchy-shell osd show <json>`
// (même cible IPC « osd » qu'Omarchy). La lecture du payload reste celle d'Omarchy
// (OsdModel.js, importé par le bandeau) ; seul le dessin change : l'OSD va dans le bandeau.
Item {
  id: root

  readonly property bool opened: Bandeau.osdOpen

  function open(payloadJson) { Bandeau.showOsdPayload(payloadJson) }
  function close() { Bandeau.hideOsd() }

  IpcHandler {
    target: "osd"
    function show(payloadJson: string): string {
      root.open(payloadJson)
      return "ok"
    }
    function close(): string { root.close(); return "ok" }
    function state(): string { return root.opened ? "open" : "closed" }
    function ping(): string { return "ok" }
  }
}
