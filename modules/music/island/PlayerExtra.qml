import QtQuick
import Quickshell

// Partie « île » du module music : ce qui est propre à notre lecteur dans la carte du lecteur
// de l'île (slot « player-extra », voir island.json et components/ModuleRegistry.qml).
// L'île injecte `player` (lecteur MPRIS courant) et lit `active`, `detail`, `actionGlyph`,
// `actionOn`, `action()` et `launcherExt` ; le reste de la carte (titre, pochette, commandes)
// reste générique dans l'île.
Item {
  id: extra
  property var player: null

  readonly property var meta: player && player.metadata ? player.metadata : ({})
  // Notre lecteur : nom de bus MPRIS, ou identité « Musique » faute de nom.
  readonly property bool active: !!player && (String(player.dbusName || "") === "org.mpris.MediaPlayer2.poste_music"
    || (String(player.dbusName || "") === "" && String(player.identity || "") === "Musique"))

  // Qualité du flux (`poste:quality`), petite et discrète sous l'artiste.
  readonly property string detail: active ? String(meta["poste:quality"] || "") : ""

  // Cœur : ajoute ou retire le morceau des favoris. Le démon met à jour `poste:starred` après
  // le serveur : le cœur suit tout seul.
  readonly property string actionGlyph: active ? (actionOn ? "󰋑" : "󰋕") : ""
  readonly property bool actionOn: active && !!meta["poste:starred"]
  function action() { Quickshell.execDetached(["poste", "music", "like", "--toggle"]) }

  // Notre lecteur n'a pas de fenêtre : la pastille de la source du son ouvre le lanceur ici.
  readonly property string launcherExt: "poste-music"
}
