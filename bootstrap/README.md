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
  says so. Moving it is `poste sys update` (see below). poste keeps a copy of the patch it
  applied (`~/.local/state/poste/patches/<id>.patch`): when the repository's patch changes (or is
  removed from the steps), the plugin is seen carrying the earlier one, and `poste sys update`
  swaps them (or takes it off).
- `[[app]]`: program published as a GitHub release asset (`github`, `asset` with `{version}` and
  `*`, `prerelease`). Exactly one way to install it: `package` (a pacman package: poste downloads
  and checks it, then PRINTS `sudo pacman -U`), `bin` (that file of the archive into
  `~/.local/bin`) or `script` (the archive extracted, that script of it run). `version`: command
  whose output gives the installed version (not needed with `package`: `pacman -Q`); `busy`:
  command that succeeds while it must not be replaced. Installed by `setup` only if absent.
- A `[[user]]` step with both `check` and `trigger` runs when its `check` fails, and again when
  one of its triggers has just been done (Omamail's backend after its plugin moved).
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
- `poste sys update [--yes] [--dry-run]`: everything at once, after one preview (for a plugin:
  its new commits) and one confirmation: the apps and plugins that are behind, then, in the
  terminal (your password), the `sudo pacman -U` of the downloaded packages and `omarchy update`
  (or `yay -Syu`) when the packages of the repositories or the AUR are behind. The only command of
  poste that runs `sudo`.
- `poste sys update <id>…`: only these apps and plugins (no system packages).
- An app is downloaded through `gh` and checked against the sha256 GitHub publishes (no
  checksum, no install). A plugin's patch is taken off (the version it carries, from that copy),
  the plugin moved, the repository's patch put on; if it no longer applies, the plugin is put
  back where it was, earlier patch included, and the update fails.
- Versions are compared, never moved back: pacman's order (`0.1.2` < `0.1.2.r3.g<hash>` <
  `0.1.3`), a release above its pre-releases (`0.1.1-beta.9` < `0.1.1`), and the order of the
  releases for tags without an order (`1.0.0-local.<hash>`). The available version is the one
  the asset's name carries (`{version}`), not the tag: a rolling pre-release such as
  `dev-latest` works. A plugin already past its latest tag is up to date.
- Stable or pre-release: `prerelease = true` on an `[[app]]` or a `[[plugin]]`, in the steps file.
  Without it, only releases (for a plugin, tags without `-rc1`, `-beta`).

