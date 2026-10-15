# icons: Tabler icons in place of Nerd Font glyphs

The Omarchy shell draws its icons (menu, bar, panels, third-party plugins) with Nerd Font
glyphs of the `monospace` family, that is FiraCode Nerd Font. The `poste.island` island
shows Tabler SVGs instead. The 'Poste Nerd Font' font gives the same Tabler drawing to the
rest of the shell.

| File | Role |
| --- | --- |
| `glyphs.tsv` | the table: Nerd code point, Tabler icon, filled variant, name for the island, Nerd name, where it is used |
| `config/.local/share/fonts/poste/` | the font, versioned as is (six weights of FiraCode Nerd Font 3.5.1) |
| `config/.config/fontconfig/conf.d/60-poste-shell-icons.conf` | fontconfig rule that gives the font to the shell only |

Both are linked by `poste sys setup` like the rest of `config/`: nothing to generate or
install on the machine.

## The font

A copy of each weight of FiraCode Nerd Font (Light to Bold), families renamed. The
code points of `glyphs.tsv` point to the outlines of the `@tabler/icons-webfont` webfont
3.49.0 (MIT, license next to the TTFs). The text, the other glyphs and the metrics are those
of FiraCode (SIL Open Font License 1.1, no reserved name: `LICENSE-firacode` next to the TTFs). Light weight: Tabler with a thin stroke. A 24 Tabler grid is one em, centered
in the cell like the Nerd Font patcher does.

## The fontconfig rule

It replaces `monospace` with `Poste Nerd Font` for the `quickshell` process only (the shell).
Terminals keep FiraCode. It lives in `conf.d`, never in `fonts.conf` which
`omarchy-font-set` rewrites. To go back: delete the link
`~/.config/fontconfig/conf.d/60-poste-shell-icons.conf`, then `omarchy restart shell`.

## Changing an icon

The font is a frozen file: changing an icon means regenerating it away from the PCs, then
committing the TTFs. `glyphs.tsv` keeps the correspondence, and the island's `Glyphs.js` follows the same
table (to change in both places).

## Limits

- The shell's Nerd glyph picker (emojis) also goes through `monospace`: it shows the Tabler
  icon for the code points of the table.
- If `omarchy font set` picks another font, the shell keeps Poste Nerd Font (FiraCode)
  as long as the rule is in place.
- A glyph added by an Omarchy update keeps its Nerd drawing until the font is
  regenerated.
