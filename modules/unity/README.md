# Module unity

The Unity projects of the machine in the island launcher: "Unity Projects" (or `open` / `proj`
followed by a space), Enter opens the project in the editor of its version (`unity open`, the
Unity CLI). Ctrl+B: open the folder, a terminal in it, copy the path.

## Where the projects come from

- **Unity Hub**: the projects it knows (`~/.config/unityhub/projects-v1.json`), with their title,
  version, last opening and favorite mark.
- **The roots**: any folder holding `ProjectSettings/ProjectVersion.txt` under `~/Projects` and
  `~/Unity`, at any depth up to 4 levels (a worktree such as `~/Projects/game/_worktrees/kit` is
  found). A project is not searched inside; a symbolic link to a folder is followed; hidden
  folders and build or dependency folders (`Library`, `Temp`, `Logs`, `obj`, `node_modules`,
  `target`, `build`, `dist`, `vendor`) are never opened.

Favorites first, then the newest (last opening for Hub, last write of the project otherwise). The
launcher keeps the list 5 minutes.

The editors Unity Hub installs as desktop entries (`unityhub-unity-editor*`) are hidden from the
launcher while the extension is shown (`hiddenApps` in `island/launcher.json`): an editor opened alone only shows a project
picker, the projects are here.

## Commands

| Command | Role |
| --- | --- |
| `poste unity projects [--json]` | the projects; `--json`: launcher format |

## Settings

`~/.config/poste/unity.toml` (`XDG_CONFIG_HOME` respected), optional:

```toml
roots = ["~/Projects", "~/Unity"]   # folders searched for projects
depth = 4                           # folder levels under a root (1 to 8)
```

## Requires

The `unity` command (Unity CLI, AUR `unity-cli-bin`): without it `poste sys setup` leaves the
module out.

## Removal

Disable the module (`disabled` in `~/.config/poste/config.toml`): the extension leaves the
launcher and the editors come back in the app list. The module writes nothing.
