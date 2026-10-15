import QtQuick
import QtTest
import "../../config/.config/omarchy/plugins/poste.island/components/ModulePlayers.js" as ModulePlayers

// Player card extras declared by a module, with an invented player.
TestCase {
  name: "ModulePlayers"

  readonly property var block: ({
    bus: "org.mpris.MediaPlayer2.example",
    quality: "example:quality",
    starred: "example:starred",
    like: ["example", "like", "--toggle"],
    launcher: "example",
    app: "example-app"
  })

  function test_declaration_needs_a_bus() {
    compare(ModulePlayers.declaration("m", null), null)
    compare(ModulePlayers.declaration("m", {}), null)
    compare(ModulePlayers.declaration("m", { bus: "" }), null)
    var d = ModulePlayers.declaration("m", { bus: "org.mpris.MediaPlayer2.example", like: "not a list" })
    compare(d.module, "m")
    compare(d.quality, "")
    compare(d.like, [])
  }

  function test_matching_player() {
    var players = [ModulePlayers.declaration("m", block)]
    var e = ModulePlayers.extras(players, "org.mpris.MediaPlayer2.example",
      { "example:quality": "FLAC · 16 bit / 44.1 kHz", "example:starred": true })
    compare(e.detail, "FLAC · 16 bit / 44.1 kHz")
    verify(e.hasAction)
    verify(e.actionOn)
    compare(e.like, ["example", "like", "--toggle"])
    compare(e.launcher, "example")
    compare(e.app, "example-app")

    e = ModulePlayers.extras(players, "org.mpris.MediaPlayer2.example", {})
    compare(e.detail, "")
    verify(e.hasAction)
    verify(!e.actionOn)
  }

  function test_instance_suffix() {
    var players = [ModulePlayers.declaration("m", block)]
    verify(ModulePlayers.extras(players, "org.mpris.MediaPlayer2.example.instance42", {}).hasAction)
    verify(!ModulePlayers.extras(players, "org.mpris.MediaPlayer2.examples", {}).hasAction)
  }

  function test_other_player_stays_generic() {
    var players = [ModulePlayers.declaration("m", block)]
    var e = ModulePlayers.extras(players, "org.mpris.MediaPlayer2.other", { "example:starred": true })
    compare(e.detail, "")
    verify(!e.hasAction)
    verify(!e.actionOn)
    compare(e.launcher, "")
    compare(e.app, "")
    verify(!ModulePlayers.extras([], "", null).hasAction)
  }

  function test_no_heart_without_command() {
    var players = [ModulePlayers.declaration("m", { bus: "b", starred: "k" })]
    var e = ModulePlayers.extras(players, "b", { k: true })
    verify(!e.hasAction)
    verify(!e.actionOn)
  }
}
