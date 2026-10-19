# Launcher format

The island launcher knows no program. A program plugs into it by declaring an extension (in a
module's or add-on's `island/launcher.json`, or in `~/.config/poste-island/launcher/launcher.json`)
whose commands print results in the format below. This document is the contract: a program that
prints this format works in the launcher, and a change to the format is a change to this file.
The reference implementation is `components/Launcher.qml` of the island.

## Declaring an extension

```json
{
  "extensions": [
    {
      "id": "example",
      "name": "Example",
      "subtitle": "what it searches",
      "icon": "󰝚",
      "keywords": ["words", "that", "find", "it"],
      "keyword": ["ex"],
      "ttl": 30,
      "rootItems": false,
      "requires": "example",
      "source": {
        "type": "script",
        "command": "example list --json",
        "search": "example search --json"
      }
    }
  ]
}
```

- `id`: unique. On the same `id`, the user's own `launcher.json` wins over a module's.
- `keyword`: typing it plus a space enters the extension.
- `source.command`: run with `bash -c` when the extension opens without typing; its results are
  cached `ttl` seconds (300 by default).
- `source.search` (optional): run at each keystroke with the typed text as one argument
  (`bash -c '<search> "$1"'`); its results replace the list.
- `rootItems: false`: the results only show inside the extension, not in the main list.
- `requires` (optional): a command that must be in the `PATH` (or `plugin:<id>`, an installed
  Omarchy plugin), one value or a list; otherwise the extension is hidden.
- `hiddenApps` (optional): desktop ids (without `.desktop`) the launcher no longer lists as apps
  while the extension is shown, `prefix*` for a family of ids. For entries a program installs and
  that the extension replaces (`"hiddenApps": ["unityhub-unity-editor*"]`).

### Nothing typed yet

What `source.command` shows when the extension opens is the same rule for every extension:
useful content when it has some (Music: playlists and mixes; Files: recent files), otherwise an
empty array. Never a hint line ("Type a file name…": the field's placeholder already says it),
and no counts or dates nobody asked for. A message line stays fine when something is wrong (see
"Errors").

## Results

Both commands print, on stdout, a JSON array of entries:

```json
[
  {
    "id": "album:a1",
    "name": "Album name",
    "subtitle": "Artist · 2024",
    "icon": "~/.cache/example/a1.png",
    "keywords": ["more", "words"],
    "verb": "Play",
    "action": { "type": "command", "command": "example play album a1" },
    "actions": [
      { "label": "Shuffle", "icon": "󰒟", "action": { "type": "command", "command": "example play album a1 --shuffle" } },
      { "label": "Add to a playlist…", "icon": "󰐒", "action": { "type": "submenu", "command": "example playlist pick --json album a1" } }
    ]
  }
]
```

| Field | Meaning |
| --- | --- |
| `id` | stable identifier inside the extension (pins and usage ranking use it); `name` when missing |
| `name` | main line |
| `subtitle` | second line, muted |
| `icon` | an icon name of the theme, a file path (`/…` or `~/…`), or a glyph / emoji |
| `keywords` | more words matched by the fuzzy filter |
| `verb` | label of the main action (`Open` when missing; `Play` shows a play glyph). When set, the main list (outside the extension) reads `<verb> <name>`, unless the name already starts with it |
| `action` | what Enter does (see below) |
| `actions` | secondary actions, shown with →: `{ "label", "icon", "action" }` |
| `highlight` | optional `{ x, y, w, h, window }`: a frame drawn on screen around the target while the line is selected (only when the extension has `"highlight": true`) |

An entry without `action` does nothing on Enter (a message line: 'Copying the library…').
Unknown fields are ignored, so a program may add its own.

## Actions

| `type` | Fields | Does |
| --- | --- | --- |
| `command` | `command`, `cwd`? | runs `bash -c <command>`, detached, in `cwd` (home by default) |
| `terminal` | `command`, `cwd`?, `title`? | runs the command in a new terminal |
| `url` | `url` | opens the address |
| `copy` | `text` | copies the text |
| `app` | `id`, `args`? | launches the desktop entry `<id>.desktop` |
| `submenu` | `command` | runs the command and shows its output (same JSON array) as a sub-list; Escape goes back |

## Errors

A source command that prints something other than a JSON array gives an empty list (a submenu
command shows 'list unavailable'), with a warning in the island's log. A program
that wants to say something (not logged in, server unreachable, still syncing) prints a one-entry
array without `action`, or with an action that fixes it (for example a `terminal` action running
its login), and exits 0.
