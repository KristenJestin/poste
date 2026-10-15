import QtQuick
import Quickshell.Io
import "../poste.island/bandeau"

// poste OSD: clone of omarchy.osd (volume, brightness, microphone, music, power).
// Omarchy sends there everything aimed at omarchy.osd: shell.summon("omarchy.osd", payload) calls
// open(payload) of an active clone, and omarchy-osd goes through `omarchy-shell osd show <json>`
// (same 'osd' IPC target as Omarchy). The reading of the payload stays Omarchy's
// (OsdModel.js, imported by the banner); only the drawing changes: the OSD goes in the banner.
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
