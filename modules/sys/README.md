# poste sys

Module that manages the `poste` repository. Always installed. The clone of the repository is found by the
`repo` key of `~/.config/poste/config.toml` (written by `./setup`); what `setup` puts in place is read from the
root `~/.local/share/poste/current`, which points to the clone (dev mode) or to an installed version
(release mode): see "Versions and root".

## Commands

- `poste sys setup [--yes] [--dry-run] [--no-config] [--take-repo <path>]… [--take-home <path>]…`:
  puts `current` in place if it is absent (dev: to the clone; release: to the most recent installed
  version), runs `cargo build --release` if the root is the clone (an installed version arrives
  already built), installs `~/.local/bin/poste` and, for each
  `modules/*/` holding a `module.toml`, `~/.local/share/poste/modules/<name>/`
  (`poste-<name>`, `module.toml`, `island/`, `hypr/`, `bin/`), then puts the config and the module
  lines in place and removes the orphans (see below). The program of a module is the built binary
  if it has a `Cargo.toml`, otherwise the script `poste-<name>` of the module folder.
  Exit code 2 if config conflicts remain. Unless `--no-config` is given, goes on with the "machine"
  part (see below).
- `poste sys outdated`: what is behind its source: apps (GitHub releases), Omarchy plugins, packages
  of the repositories and of the AUR (see [`bootstrap/README.md`](../../bootstrap/README.md), "Versions").
- `poste sys update [<id>…] [--yes] [--dry-run]`: moves the apps and plugins that are behind to
  their latest version, after a preview and a confirmation; prints the `sudo` left to run.
- `poste sys done [<id>] [--undo]`: marks a manual step (or a system step that cannot be checked) done;
  without id, lists them. State in `~/.local/state/poste/done/<id>`.
- `poste sys status`: source (dev or release) and root read; branch of the clone, uncommitted changes, unpushed commits; installed
  modules (installed / repository fingerprint); files put in place that are missing or edited by hand;
  config (linked, missing, broken links, links replaced by an ordinary file, conflicts);
  module lines (in place, conflicts); orphans;
  missing packages, pending system, user and manual steps.
- `poste sys diff [<path>]`: differences between `config/` and `~` (unified diff).
- `poste sys add <path> [--dry-run]`: puts a file (or folder) of `~` under management:
  copied into `config/` (after a backup), replaced by a link. Refuses what matches
  `config/.poste/ignore` (secrets: `.ssh/**`, `.claude/.credentials.json`, `**/.env*`, `.npmrc`…).
- `poste sys forget <path>`: puts an ordinary file back in place of the link; the file stays
  in the repository until your `git rm` and your commit.
- `poste sys sync [--yes] [--dry-run]`: in dev, `git pull --ff-only` in the clone and `current` put back on the clone; in release, the latest published version, downloaded and verified if it is not already installed, and `current` on it (see "Versions and root"). In both modes, `git pull --ff-only` in the machine's data repository (`data`, see the root README), then `setup` (preview included), then pruning of the old versions (release). Then, depending on the files changed by the pull, by the version change (the two roots compared) or put in place by `setup`, `omarchy restart shell` (island: `config/.config/omarchy/`, `poste-island/`, `fontconfig/`, `island/` of the modules) and `hyprctl reload` (`config/.config/hypr/`, `hypr/` and `module.toml` of the modules), after confirmation unless `--yes` is given. The island is not restarted if `gpu-screen-recorder` is running (the message says to do it later).
- `poste sys import-chezmoi [--source <folder>] [--dry-run]`: see "Import from chezmoi".
- `poste sys local [list|add <path>|remove <path>]`: files of `config/` that THIS machine keeps
  for itself (see "Machine-specific settings").
- `poste sys module [list|enable <name>|disable <name>]`: modules of the repository not installed here.
- `poste sys addons`: the add-ons of the repository, chosen here or not (key `addons`), program installed or not, plugged in or not.
- `poste sys skip [list|add <id>…|remove <id>…]`: bootstrap packages and steps ignored here.
- `poste sys override [list|set <path> <key> <value>|unset <path> <key>]`: JSON overrides per machine.
- `poste sys use dev|release|<commit>` `[--yes] [--dry-run]`: switches the machine (see "Versions and root").
- `poste sys versions`: source, root (`current`) and installed versions (`*`: the one in place).
- `poste sys git <args…>`: `git -C <clone> <args…>`.
- `poste sys cd`: prints the path of the clone (`cd "$(poste sys cd)"`).

## Versions and root

- Root: the link `~/.local/share/poste/current`. `config/`, `modules/`, `bootstrap/` and the binaries
  (`target/release/`) are read from it; the config links in `~` point to `current/config/<path>`, so
  changing `current` changes the config without putting the links back. A version has the layout
  of the repository plus a `VERSION` file (`commit=…`, `date=…`) and `ARCHIVE_SHA256` (checksum of the archive
  it comes from); it has no `Cargo.toml`.
- Key `source` of `config.toml`: `dev` (default when it is missing) or `release`. Only written by
  `poste sys use` (toml_edit, the rest of the file is kept), never by `setup`; specific to the
  machine (ignored in the `poste.toml` of the data repository).
- `poste sys use dev`: `source = "dev"`, `current` to the clone, `setup` (which builds).
  `poste sys use release`: `source = "release"`, `current` to the most recent installed version
  (downloaded if there is none), `setup`. `poste sys use <start of a commit>`: one precise installed
  version (rollback; the next `sync` goes back to the latest). Then,
  like `sync`: reload of the island and Hyprland depending on the files that differ between
  the old and the new root, after confirmation unless `--yes`, never the island during a screen recording.
  `--dry-run` shows the switch and the planned reload without writing anything or running `setup`.
- Download (`sync` in release, `use release` with no installed version): `curl` first reads
  `poste-x86_64-linux.tar.gz.sha256` at the published address (`POSTE_SYS_RELEASE_URL` replaces it, for
  example for a fork); if an installed version comes from the same archive, nothing is downloaded.
  Otherwise the archive goes to `~/.cache/poste/download/`, its checksum is verified BEFORE extracting
  (different: refused, nothing is installed), then it is extracted by `tar` into a temporary folder
  of `versions/`, renamed to `versions/<commit>/`. `--dry-run` only reads the checksum.
- Pruning: after a `sync` in release, only the 3 most recent versions (build date)
  stay, plus the one `current` points to if it is older; never the one `current` points to.
- Hand-over (`sync` and `use`, except `--dry-run`): once `current` is set, if the `poste-sys` of the
  new root (`target/release/poste-sys`, built first for the clone) differs from the one
  that is running, this one hands over to it (`exec`) through the hidden command `poste-sys finish`
  (`--yes`, `--prune`, `--skip-build`, `--from <old root>` so it compares the roots
  itself again, `--reload-shell`, `--reload-hypr`): `setup`, pruning and
  reload are done by the new program, which knows what is new in its version (for
  example the systemd timers). `finish` never hands over again. A version from before the
  hand-over (rollback) does not know `finish` (checked by `finish --help`): the rest is
  then done by the running program, as if the launch failed. The options of `finish`
  do not change from one version to the next: they are only added to. The `sync` that installs the
  first version with hand-over is still done by the old program: run `poste sys setup` again
  afterwards if this version brings something new to `poste sys`.
- Read-only in release: `add` and `setup --take-home` write into the repository, they are refused
  as long as `current` points to a version (switch to dev). `import-chezmoi`, `git` and `cd` always work
  on the clone.
- Migration of an existing machine: a link of `~` to `config/<path>` of another poste root (the clone, a version, `current` itself) instead of `current/config/<path>` is
  "to be re-linked". `setup` puts it back to `current` (old link backed up in `backup/`) only if it
  is noted in `installed.json`; otherwise it is a conflict like any other foreign link, never touched.

## Rules of `setup`

- Can be run again: only does the difference (sha256 comparison), "Everything is up to date" otherwise.
- Shows the plan (created / changed / unchanged, then the config) then asks for confirmation, unless `--yes` is given.
- `--dry-run` writes nothing (the build only produces `target/`).
- Never replaces a file it did not put in place: it refuses, without writing anything at all,
  and says which one is in the way. A file already identical to the source is simply taken into account.
- Before replacing a file it put in place, copies the old one to
  `~/.local/state/poste/backup/<flattened path>.<YYYYMMDD-HHMMSS>.bak`.
- Only deletes orphans (see below), after a preview and a confirmation: a file removed from the repository stays in place unless it is a link or a module file we put in place and that is intact.
- Keeps `~/.local/state/poste/installed.json`: path, sha256, owner module (`config` for the config), date.

## Machine-specific settings

The repository is the same on every PC (no per-machine profiles). What differs from one machine to
the next lives in `~/.config/poste/config.toml`, LOCAL to the machine and never in the repository. Four keys:

```toml
local = [".config/hypr/monitors.lua", ".config/hypr/input.lua"]   # files of this machine
disabled = ["record", "power"]                                      # modules not installed here
skip = ["gimp", "io.github.grichard99.omaproton-vpn"]               # bootstrap packages / steps ignored here

[[override]]                                                        # JSON: repository + local values
path = ".config/omarchy/shell.json"
set = { "idle.lock" = 3600 }
```

All four are edited without opening the file (comments and other keys are kept).

### `local`: files of the machine

- `setup` does not link these paths, does not report them as conflicts, never touches them (no linking,
  no replacing, no permissions, no removal as an orphan). A path can be a folder of `config/`
  (everything it holds is local); a file inside a folder linked as a block (`link-dirs`) is
  declared through that folder.
- `status` lists them under "Local files" with their state against the repository (absent, identical,
  different, still linked); `diff <path>` still shows the difference (`diff` without argument skips them).
- `local add <path>` refuses a path that `config/` does not know. If it is still a link to the
  repository, it is left as is (editing it would change the repository): `poste sys forget <path>` makes it
  an ordinary file. `local remove` puts the path back under management (a conflict to decide if it differs).

### `skip`: bootstrap packages and steps ignored

- An identifier is a package name of `bootstrap/packages.toml` or the `id` of a step of
  `bootstrap/steps.toml` (`[[plugin]]`, `[[app]]`, `[[file]]`, `[[user]]`, `[[system]]`, `[[manual]]`).
- What is ignored is neither required, nor offered, nor counted as missing in `setup`,
  `setup --dry-run` and `status`, which add a line
  `ignored on this machine (config.toml, skip): …`.
- Dependencies: a user step with an ignored `trigger` is ignored too (for example
  `cursor-accent-recolor` when `io.github.smoothpixels.cursor-accent` or `cursor-accent-config`
  is in `skip`), in cascade. `requires` designates a binary, not a step: not concerned.
- An unknown identifier (typo) gives a warning, not an error; `skip add` adds it
  anyway. `skip remove` fails if an identifier is not there (the others are removed).
- Nothing is uninstalled: poste never ran pacman, and a plugin or file already in place stays.
  `poste sys done` can still be used on an ignored step.

### `disabled`: modules not installed

- `setup` does not install these modules; if they were installed, it removes them cleanly through the
  orphan mechanism, with a preview and a confirmation: files (`poste-<name>`, `module.toml`, `island/`,
  `hypr/`, `bin/`, `systemd/`), lines put in files without includes, systemd units
  (disabled first). A file edited by hand is kept. `sys` cannot be disabled.
- `module enable` enables again: the next `setup` reinstalls. A name unknown to the repository is reported by `module list`.

### `[[override]]`: JSON per machine

For a JSON file of `config/` whose rest must follow the repository (real case: `idle.lock` of Omarchy's
`shell.json`, the bar and the plugins coming from the repository). `set`: dotted keys
(`"idle.lock" = 3600`, or as a nested table `[override.set.idle] lock = 3600`, flattened; no
array indices). A value is a number, boolean, string or list.

- A file that has overrides is NOT linked: `setup` writes an ordinary file equal to the content of the
  repository plus the overrides, JSON rewritten cleanly (indentation 2, key order of the repository kept).
  It is noted in `installed.json` like the other config entries (fingerprint of the written text) and
  regenerated when the repository or the overrides change ("regenerated" in the preview, old one backed up).
- An old link to the repository, or a file that already holds the same values (same JSON, other
  formatting, case of the work PC set by hand), is replaced without conflict (backup made).
- Edited by hand since we wrote it: CONFLICT (code 2, nothing is touched; `status`: "generated,
  edited by hand"; `diff <path>` compares with what `setup` would write).
  - `--take-repo <path>` regenerates (the old one is backed up).
  - `--take-home <path>` copies the file of `~` back into the repository WHILE PUTTING BACK, for each
    overridden key, the value of the repository (or removing the key if the repository did not have it), then regenerates. The
    JSON of the repository is rewritten with indentation 2. Choice: taking it back is REFUSED, without writing anything, if an
    overridden value was changed by hand in `~` (or disappeared) or if the file is not valid JSON: we cannot guess
    whether the override or the repository was meant to change. Fix the override (`override set`) or use `--take-repo`.
- `override set <path> <key> <value>`: the value is JSON (`3600`, `true`, `"text"`, `[1,2]`),
  otherwise a string; refused if the key goes through a value that is not an object. `override unset`
  removes the key; the last one removed deletes the entry and the file becomes a link again at the next
  `setup` (a generated file that then differs from the repository is a conflict to decide). `override` alone lists.
- A path that is both `local` and overridden stays local.

## Systemd units

A module declares `units = ["poste-<name>.service"]` in `module.toml` and puts the unit file in
its `systemd/` folder (copied with the rest to `~/.local/share/poste/modules/<name>/systemd/`).

- `setup` enables it with `systemctl --user enable --now <path of the installed file>` (systemd creates
  the link in `~/.config/systemd/user/` itself; nothing is copied elsewhere) and notes it in
  `installed.json` (`units`). When the files of the module are updated by this `setup`, the unit is
  reloaded and restarted (`daemon-reload`, `try-restart`).
- Module removed, disabled (`disabled`) or unit removed from `module.toml`: `systemctl --user disable
  --now` BEFORE the files are removed, then the entry is forgotten.
- Timer: `units = ["poste-<name>.timer"]`, and the service it triggers (`poste-<name>.service`,
  same name) next to it in `systemd/` without being declared. `setup` first links the service
  (`systemctl --user link`, so that systemd finds it), then enables the timer; on removal, it
  disables the timer then unlinks the service. A link of the service that is not ours is a conflict.
- A unit with the same name that we did not put in place (ordinary file, link elsewhere) is a conflict (code 2),
  never touched. `--dry-run` enables nothing (only `is-enabled` reads).
- Test: `POSTE_SYS_TEST_SYSTEMCTL` (RESERVED FOR TESTS) replaces `systemctl`.

## Module lines

Some files have no include mechanism (real case: `~/.config/hypr/xdph.conf`,
where the `record` module puts `screencopy:custom_picker_binary = …`). A module declares in its
`module.toml`:

```toml
[[line]]
file = "~/.config/hypr/xdph.conf"            # under ~ only
line = "screencopy:custom_picker_binary = {modules}/record/bin/poste-share-picker"
```

`{home}` (home folder) and `{modules}` (`~/.local/share/poste/modules`) are replaced.
`config/.poste/lines.toml` (same format, owner `config`) carries the base lines of the repository.

- `setup` adds the line at the end of the file if it is missing (file created if it does not exist) and
  notes `{file, line, module}` in `installed.json` (`lines`). The existing file is backed up first.
- If the module no longer declares it or disappears, the line is removed; if it had already disappeared from the file,
  the entry is only forgotten. If the value changes, the line we had put in place is replaced.
- Never touched: a line we did not put in place (the same line already present is left, not tracked).
- Conflict (nothing is written, exit code 2): the key (what precedes the first `=`) is asked for by
  two modules, or already exists in the file with another value that we did not put in place.
- A file that is a symbolic link is never modified (we would write into its target).

## Orphans

`setup` and `status` spot what poste had put in place and whose source has disappeared:

- a link of `~` that points to `config/…` of a poste root (`current`, the clone, an installed
  version, the data repository) whose target no longer exists (file renamed or removed from the repository):
  only links, never an ordinary file, never a foreign link;
- a module file noted in `installed.json` that the repository no longer produces (module deleted, file
  renamed) and left as installed (a file edited by hand is kept). It is backed up before
  removal, and module folders that became empty are removed.

`setup` shows the preview (with the module lines) and asks for a confirmation (unless `--yes` is given);
`--dry-run` removes nothing.

## The config

The `config/` folder of the repository mirrors `~` with the real names (`config/.config/hypr/bindings.lua`
for `~/.config/hypr/bindings.lua`), without prefixes. `setup` puts each entry in place as a symbolic
link to the root (`current/config/…`): in dev, editing the file in `~` edits the
clone; in release, the installed version is read-only (do not edit it: the next
`sync` replaces it).

- A symbolic link of `config/` is recreated as is (same target) in `~`.
- The executable bit is carried by git. Restricted permissions (0600, 0700) are listed in
  `config/.poste/modes`, one line `path mode` (for example `.config/foot/foot.ini 0600`), and
  applied again to the real target of the link (the file of the repository; for a plain folder, the folder in `~`).
- Granularity: one link per file. A folder listed in `config/.poste/link-dirs` (one path per
  line) is linked as a block; to keep for folders whose whole content belongs to the repository and where
  no tool writes files of its own. `__pycache__` is ignored in comparisons and in git.
- `config/.poste/ignore`: patterns (relative to `~`) that `add` refuses to add.

State of each target and action of `setup`:

| State of the target | Action |
| --- | --- |
| already linked as planned | nothing |
| absent | creates the link |
| ordinary file identical to the repository | backup in `backup/`, then link |
| ordinary file that differs | CONFLICT: nothing is done; `poste sys diff <path>` to see |
| link to the same path in another poste root (clone, version), noted in `installed.json` | backup in `backup/`, then link to `current` |
| link elsewhere, folder in place of a file or the reverse, parent that is not a folder | CONFLICT |

Conflicts do not stop the rest; they are listed at the end of the preview and the exit code is 2.
To decide them one by one: `--take-repo <path>` (the repository wins, the old one is backed up) or
`--take-home <path>` (the real file is copied into the repository, backed up, then linked; for a
file with restricted permissions, the mode is noted in `modes`). A real folder replaced by a link
is moved as is into `backup/`.

## The machine: packages and steps (`bootstrap/`)

After the binaries and the config, `setup` reads `bootstrap/packages.toml` and `bootstrap/steps.toml`
(format: [`bootstrap/README.md`](../../bootstrap/README.md)):

1. Packages: compared with `pacman -Qq`. If some are missing, the `yay -S --needed …` command is PRINTED.
2. System steps (root): checked, then the commands are PRINTED in a copyable block.
   `setup` never runs `sudo`.
3. User steps (shell plugins + patches, absent apps, files, services, theme…): preview of the
   commands, then confirmation (unless `--yes` is given); `--dry-run` runs none. Idempotent: a
   state that is already right does not show up. What is ambiguous or risky (plugin folder without git,
   patch that no longer applies, user file that differs) is never touched, a note says so.
   If a step was done, the config is planned again (`omarchy plugin add` may rewrite
   `shell.json`) to put the links of the repository back.
4. Manual steps not done, listed at the end of `setup`.

Test: the variable `POSTE_TEST_PACMAN_QQ` (reserved for tests) replaces `pacman -Qq` with the
given program (without arguments).

## Import from chezmoi

`poste sys import-chezmoi` lists the entries of `chezmoi managed` and copies into `config/` the
REAL content of the machine (not that of the chezmoi source), with executable bit, restricted permissions
and links. It only writes in the poste repository and in `~/.local/state/poste/import-chezmoi.md`
(report: differences with the source, missing files, `DA` entries, folders that are candidates for a folder
link, secrets refused without copying their value). chezmoi's `README.md`, `bootstrap.sh` and
`bootstrap-patches/` would go into `bootstrap/`: the import is done once and for all and these
files were taken over into `bootstrap/*.toml`; do not run the import again (it would recreate them). A file that holds an obvious secret is not imported. Can be run again.

## Coming

`install <module>`.
