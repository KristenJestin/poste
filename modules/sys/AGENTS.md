# Rules of the sys module

- Always installed: `poste sys setup` must stay usable alone, without any other module.
- Never replace a file that is absent from `installed.json` if it differs from the source; never delete anything.
- Every replaced file is first copied to a timestamped `.bak` in `~/.local/state/poste/backup/`.
- `--dry-run` writes nothing (outside `target/` for the build); the plan is always shown before writing.
- Atomic writes (temporary file then rename): `poste-sys` replaces itself.
- The folder name of a module of the repository must equal its `name` in `module.toml`.
- No `sudo`, no push, no secret in the output.
- Unit tests for any plan, ranking or manifest logic.
- Config: `setup` never replaces an ordinary file of `~` that differs from the repository without an explicit decision (`--take-repo`, `--take-home`); an identical file is only replaced after a backup. A conflict does not stop the rest but gives exit code 2.
- `add` refuses any path that matches `config/.poste/ignore` or the built-in secret rules (`.ssh`, `.env*`, `.npmrc`, `.claude/.credentials.json`…).
- The config and import tests work in a temporary HOME, never in the real `~`.

- Bootstrap: `setup` NEVER runs `sudo`; system steps are checked then printed. User steps go through a preview and a confirmation, are idempotent, and never touch what is ambiguous (a note is printed). The content of root files lives in `bootstrap/system/`, not in the code.
- The `provision` tests run no real step (no systemctl, vocaline, omarchy); the fake `pacman -Qq` goes through `POSTE_TEST_PACMAN_QQ`.
- Apps and plugins (`apps.rs`, `updates.rs`): no version is stored, in the repository or on the machine; the installed one is always read on the system. An asset is installed only after its sha256 (published by GitHub) is checked. A plugin is never moved back (target already in `HEAD`), never touched if modified by hand beyond its patch, and put back where it was if its patch no longer applies. `--dry-run` fetches nothing. The tests go through fake `gh`/`pacman` run by `sh` (`apps::Tools`), never the network.
- Module lines: never change or remove a line that is absent from `installed.json` (`lines`), never write through a symbolic link, a conflict is reported without writing anything.
- Orphans: only links to `<repository>/config/` whose target is gone, or module files of `installed.json` that are still intact (already gone: only forgotten from the manifest); never an ordinary file, always a preview and a confirmation.
- A script module has no `Cargo.toml` and is not in `members` of the root `Cargo.toml`.
- Machine config (`~/.config/poste/config.toml`: `local`, `disabled`, `[[override]]`): never written by `setup`, only by `local`/`module`/`override` (toml_edit: the rest of the file is kept). A `local` path is never linked, reported or touched (not even as an orphan).
- Generated file (`[[override]]`): never overwritten if it was edited by hand (conflict); `--take-home` refuses without writing anything if taking it back is ambiguous. The JSON goes through `ojson.rs` (key order kept); do not enable `preserve_order` of serde_json in the workspace (it would change the other modules).
- Systemd units: `setup` enables and disables them through `systemctl --user` (never `sudo`); the tests use a fake `systemctl` (`POSTE_SYS_TEST_SYSTEMCTL` or `units::Runner::with_program`), never the real one.
- Versions: everything `setup` puts in place is read from the root `~/.local/share/poste/current` (clone in dev, `versions/<commit>/` in release); never a hard-coded clone path for what gets installed (`util::clone` only for git, import, cd). `source` is only written by `use`. An archive is only extracted after its checksum is verified; pruning never removes the version `current` points to. A link to another poste root is only put back to `current` if it is in `installed.json` (backup first). The download tests go through a fake `curl` run by `sh` (`Fetcher::with`), never the network.
