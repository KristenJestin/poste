# bootstrap: the machine described as data

Read by `poste sys setup` (preview), `poste sys status` and `poste sys done`. Nothing here is a script.

| File | Content |
| --- | --- |
| `packages.toml` | expected packages (`name = "what it is for"`), compared with `pacman -Qq` |
| `steps.toml` | third-party plugins, apps (GitHub releases), files, user steps, system steps, manual steps |
| `system/` | contents of root files (`@USER@`, `@UID@`, `@GID@`, `@HOME@` are replaced in them) |
| `files/` | contents of user files put in place by poste (cursor-accent config, Strata's desktop entry, D-Bus service and Taildrop action, the handler of `.url` shortcuts); `@HOME@`… are replaced in them |
| `patches/` | local patches of third-party plugins, applied by `git apply` |
| `icons/` | Tabler icons in place of Nerd Font glyphs in the shell: table of correspondences (see [`icons/README.md`](icons/README.md)) |

Step families (details and comments in `steps.toml`):

- `[[plugin]]`: Omarchy shell plugin, not pinned. Absent: `omarchy plugin add`, then its latest
  release tag (a pre-release tag such as `-rc1` only if there is nothing else; no tag: its default
  branch). Present: only its optional patch is checked (already applied =
  `git apply --reverse --check` succeeds); a patch that no longer applies is left alone, a note
  says so. Moving it is `poste sys update` (see below).
- `[[app]]`: program published as a GitHub release asset (`github`, `asset` with `{version}` and
  `*`, `prerelease`). Exactly one way to install it: `package` (a pacman package: poste downloads
  and checks it, then PRINTS `sudo pacman -U`), `bin` (that file of the archive into
  `~/.local/bin`) or `script` (the archive extracted, that script of it run). `version`: command
  whose output gives the installed version (not needed with `package`: `pacman -Q`); `busy`:
  command that succeeds while it must not be replaced. Installed by `setup` only if absent.
- `[[file]]`: user file put in place if it is absent; if it exists and differs, it is left and reported.
  `mode = "755"` (octal): permissions of the written file, put back if only they differ.
- `[[user]]`: without sudo. `check` (code 0 = already done), otherwise `run`, started by poste after confirmation.
  `requires` = required binary; `trigger` = run only if one of the named steps has just been done.
- `[[system]]`: with sudo. Poste checks (file content, line present, `check` command) and
  PRINTS the commands in a copyable block. It never runs `sudo`. A file unreadable without
  root (polkit) stays 'to check' until `poste sys done <id>`.
- `[[manual]]`: by hand; optional `check`, otherwise `poste sys done <id>` (`--undo` to cancel).

## Versions

No version is written anywhere, neither here nor on the machine: the installed one is read on
the system each time (`pacman -Q`, the `version` command, the plugin's git `HEAD`), the available
one from its source. An update made by hand or by `yay` is therefore always seen.

- `poste sys outdated`: apps, plugins, then the packages of the repositories (`checkupdates`,
  Omarchy included) and of the AUR (`yay -Qua`), with the command to run for those.
- `poste sys update [<id>…] [--yes] [--dry-run]`: moves the apps and plugins that are behind (all, or
  the cited ones) to their latest version, after a preview (for a plugin: its new commits) and a
  confirmation. An app is downloaded through `gh` and checked against the sha256 GitHub publishes
  (no checksum, no install). A plugin's patch is taken off, the plugin moved, the patch put back;
  if it no longer applies, the plugin is put back where it was and the update fails. A plugin
  already past its latest tag is up to date (never a step back). The packages of the
  repositories are never updated by poste: `omarchy update` (or `yay -Syu`).
