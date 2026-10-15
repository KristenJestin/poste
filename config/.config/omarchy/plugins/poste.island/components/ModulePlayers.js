// Player card extras declared by modules: the `player` block of a module's island/island.json
// (see ModuleRegistry.qml). Pure functions, tested in tests/island/tst_module_players.qml.
.pragma library

function str(v) { return typeof v === "string" ? v : "" }

// The declaration of one module, or null when it is missing or has no bus name.
function declaration(module, block) {
  if (!block || typeof block !== "object" || str(block.bus) === "") return null
  return {
    module: module,
    bus: block.bus,
    quality: str(block.quality),
    starred: str(block.starred),
    like: Array.isArray(block.like) ? block.like.map(String) : [],
    launcher: str(block.launcher),
    app: str(block.app)
  }
}

// What the card shows for an MPRIS player (its bus name and metadata), given the declarations.
// Without a matching declaration: no detail, no heart, no launcher.
function extras(players, bus, metadata) {
  var meta = metadata || {}
  var decl = null
  // The exact name, or the same name with an instance suffix (MPRIS allows `<name>.<instance>`).
  for (var i = 0; i < (players || []).length; i++) {
    var b = players[i].bus
    if (str(bus) !== "" && (bus === b || bus.indexOf(b + ".") === 0)) { decl = players[i]; break }
  }
  if (!decl) return { detail: "", hasAction: false, actionOn: false, like: [], launcher: "", app: "" }
  var hasAction = decl.starred !== "" && decl.like.length > 0
  return {
    detail: decl.quality !== "" ? String(meta[decl.quality] || "") : "",
    hasAction: hasAction,
    actionOn: hasAction && !!meta[decl.starred],
    like: hasAction ? decl.like : [],
    launcher: decl.launcher,
    app: decl.app
  }
}
