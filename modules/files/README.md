# Module files

Fast file search in the island launcher: SUPER+D opens the launcher straight on "Files", the recent
files show first, you type a piece of a name or path, Enter opens. In the main launcher
(SUPER+SPACE), `find`, `file` or `fichier` followed by a space enters it too.

## How it works

No daemon: nothing runs while you are not searching.

- **Index**: a file, `~/.cache/poste/files.idx` (`XDG_CACHE_HOME` respected), the list of the files
  and folders of the roots. SUPER+D runs `poste files refresh --background`, which returns at once,
  walks the roots, replaces the index in one go and stops. Searches read the previous index in the
  meantime: a file created since the last opening shows up as soon as the walk is done. An index
  under 10 s old is not rebuilt. Opened by a keyword (without SUPER+D), the first search starts the
  walk in the background when the index is over a minute old. No index at all (very first
  search): the search does the walk itself and waits.
- **Walk** with the `ignore` crate (ripgrep's walker, also used by fff), filtered by our own rules
  only (see "Exclusions"): the `.gitignore` files met are not read, so a file ignored by git is
  found. An excluded folder is never opened.
- **Search**, fuzzy, with neo_frizbee, fff's engine (Smith-Waterman in SIMD): each word of the query
  must be found, letters in order, in the path (`poste bind` finds
  `~/Projects/poste/config/.config/hypr/bindings.lua`); a matching file name comes before a mere
  matching path; at equal score, the shortest path wins. For a word of 6 letters or more that
  gives almost nothing, one typo is allowed on the names (`invocie` finds `invoice`). Case is always ignored
  (`Huk19` finds `HUk19e.jpg`); the same case still adds to the score. Each keystroke is a
  `poste files search --json` process that reads the index again.
- **Recent files**, shown before anything is typed: the ones opened from the launcher (Enter goes
  through `poste files open`, which remembers them in `~/.local/state/poste/files-recent`), merged
  with the desktop's own list, `~/.local/share/recently-used.xbel` (freedesktop, filled by GTK
  applications). Newest first, only what still exists; nothing recent, an empty list.

Measured on a real home folder (377,000 entries, 39 MB index): full walk in 0.3 s when the folders
are in the system cache (longer right after the machine starts), search between 34 and 40 ms per
keystroke, process included.

Why not a daemon (in-memory index kept up to date by inotify, tried first): 25 ms per keystroke
instead of 35, but 150 MB and 44,000 inotify watches held all the time, and a first walk to wait
for at each restart of the daemon. Why not `fff-search`: the library is made for a program that
stays open, and offers no way to add exclusions. Why not `plocate`: substring search only,
exclusions by name only, system database updated once a day.

## Commands

| Command | Role |
| --- | --- |
| `poste files search <query…> [--json] [--limit N]` | search. `--json`: launcher format |
| `poste files home [--json]` | recent files (the launcher shows them before anything is typed) |
| `poste files refresh [--background] [--force]` | rebuild the index; `--background` returns at once (SUPER+D) |
| `poste files open <path>` | open with the default application, remember as recent |
| `poste files copy <path>` | copy the file itself to the clipboard |
| `poste files reveal <path>` | show the file in the default file manager |
| `poste files status` | roots, entries, age of the index, settings files |

## In the launcher

Each result: name, parent folder (with `~`), a Nerd Font glyph by type (folder, image, video, audio, PDF, archive, document, table, slides, code, other file). Enter = **Open** (`xdg-open`: the
default application, the file manager for a folder). Secondary actions:

- **Copy file**: the file itself, as `text/uri-list` (`wl-copy`): file managers, browsers and chat
  applications paste it as a file; an application that only takes text gets its `file://` address;
- **Copy path** (`copy` action of the launcher);
- **Open terminal here** (in the folder, or in the file's folder);
- **Show in file manager**: `poste files reveal`, through D-Bus
  `org.freedesktop.FileManager1.ShowItems`, so the file manager owning that name on the bus
  (Nautilus, Flea, Strata… all implement it); none is named. When nobody answers, the parent
  folder is opened with `xdg-open`.

Keyboard shortcuts for these actions are not there yet: the launcher only has Ctrl+B (list of
actions) for now.

Files of the module for the machine: [`island/launcher.json`](island/launcher.json) (extension
`poste-files`, `rootItems: false`) and [`hypr/files.lua`](hypr/files.lua) (SUPER+D).

## Exclusions

In .gitignore syntax, anchored on each root, in two layers read in order; the last matching line
wins:

1. [`default.ignore`](default.ignore), shipped with the module: hidden files and folders (`.*`),
   `.git`, `.cache`, node_modules, Unity's Library, target, dist, build, node, .venv,
   `__pycache__`, .next, .svelte-kit;
2. `~/.config/poste/files.ignore`, the user's (optional).

```gitignore
# This folder, at any depth
Videos/rushes/
# Only the one at the root
/Downloads/
*.iso
# Lift default exclusions (including a hidden folder to keep)
!build/
!.config/
```

A `#` only starts a comment at the beginning of a line, as in git. As in git, nothing comes back
under an excluded folder (`!secret.txt` does not bring a file out of an excluded `.cache/`), and
`!.*` would lift `.git/` and `.cache/` too. A symbolic link to a folder counts as a folder and is
never followed. Changes count from the next walk (SUPER+D, or `poste files refresh --force`).

## Settings

`~/.config/poste/files.toml` (optional):

```toml
roots = ["~", "/srv/data"]      # indexed folders (default: ["~"])
limit = 30                      # results given to the launcher
```

### From the machine's data repository

`files.toml` and `files.ignore` are files of `~/.config/poste/` like any other: for a machine to
keep its own, put them in its `poste-data` repository, which `poste sys setup` places in `~`:

```text
poste-data/config/.config/poste/files.ignore
poste-data/config/.config/poste/files.toml
```

## Known limits

- Paths that are not UTF-8 or that hold a line break are not indexed (the count is written in
  `~/.local/state/poste/files.log`, log of the last background walk).
- What changes while the launcher is open only shows up after a new walk.

## Removal

Disable or delete the module: `poste sys setup` removes the program, `island/` and `hypr/`
(SUPER+D goes with it), then `hyprctl reload`. `~/.cache/poste/files.idx` and
`~/.local/state/poste/files-recent` can be deleted; `files.toml` and `files.ignore` stay the user's.
