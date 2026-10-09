// Claude Code / Codex brand tiles, adapted from Island
// (https://github.com/Guilhermerisu/island), MIT License,
// Copyright (c) 2026 Guilherme Pimenta.
.pragma library
.import "NameColor.js" as NameColor

var brands = {
  claude: { glyph: "", tile: "#d97757", ink: "#ffffff" },
  codex: { glyph: "", tile: "#f2f2f2", ink: "#000000" }
}

function agent(summary, app, appIcon) {
  summary = String(summary || "")
  if (summary === "Claude Code" || String(app || "") === "Claude Code") return "claude"
  var fromTerminal = /ghostty|kitty|alacritty|foot|wezterm/i.test(String(appIcon || "") + " " + String(app || ""))
  if (summary === "Codex" || String(app || "") === "Codex"
      || (fromTerminal && /^(Ghostty|kitty|Alacritty|foot|WezTerm)$/.test(summary))) return "codex"
  return ""
}

function brand(summary, app, appIcon) {
  var a = agent(summary, app, appIcon)
  return a ? brands[a] : null
}

// Un agent sans tuile attitrée (« fireworks », un nouveau venu) : son
// initiale sur une couleur tirée de son nom.
function brandOrName(name, summary, app, appIcon) {
  return brand(summary, app, appIcon)
    || { glyph: NameColor.initial(name), tile: NameColor.color(name, 0.55, 0.5), ink: "#ffffff" }
}
