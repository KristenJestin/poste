# Rules of the files module

- The JSON of `search --json` and `home --json` follows `docs/launcher-format.md`. Test it
  (`item.rs`) and do not put `path` in it (the launcher would add its own "Copy path" a second
  time).
- With `--json`, a failure gives a single launcher entry and exit code 0.
- Nothing typed (`home`): the recent files, or an empty list. Never a hint line ("Type a file
  name…"), never counts or dates nobody asked for.
- No daemon and no process left behind: `refresh --background` ends after its walk.
  `home` runs at each opening of the extension: it reads no index.
- The index is replaced in one go (written beside, then renamed); one walk at a time (lock
  `files.lock`).
- Only our rules exclude (`default.ignore` then `files.ignore`, .gitignore syntax): do not turn
  the reading of `.gitignore`, `.ignore` or git's global excludes back on. An excluded folder is
  never opened: drop it in `filter_entry`, not afterwards.
- "Show in file manager" goes through D-Bus `org.freedesktop.FileManager1`: never a file manager
  by name.
- Launcher texts in English, like the rest of the launcher.
- Nothing personal in the code (roots, exclusions of a machine): everything goes in
  `~/.config/poste/files.toml` and `files.ignore` (or the machine's `poste-data`), never
  overwritten by the module.
- Tests: never the real `$HOME`, the D-Bus session bus or the clipboard; the end-to-end test works
  in a temporary home folder with a fake `xdg-open` in its `PATH`.
- Measuring latency on the real home folder is done with temporary `XDG_CACHE_HOME` and
  `XDG_STATE_HOME`, deleted right after; a few queries by hand, never a loop under load.
