# bootstrap: the machine described as data

Read by `poste sys setup` (preview), `poste sys status` and `poste sys done`. Nothing here is a script.

| File | Content |
| --- | --- |
| `packages.toml` | expected packages (`name = "what it is for"`), compared with `pacman -Qq` |
| `steps.toml` | third-party plugins, files, user steps, system steps, manual steps |
| `system/` | contents of root files (`@USER@`, `@UID@`, `@GID@`, `@HOME@` are replaced in them) |
| `files/` | contents of user files put in place by poste (cursor-accent config, Strata's desktop entry, D-Bus service and Taildrop action); `@HOME@`… are replaced in them |
| `patches/` | local patches of third-party plugins, applied by `git apply` |
| `icons/` | Tabler icons in place of Nerd Font glyphs in the shell: table of correspondences (see [`icons/README.md`](icons/README.md)) |

Step families (details and comments in `steps.toml`):

- `[[plugin]]`: Omarchy shell plugin pinned to a commit (`omarchy plugin add`, `git checkout`),
  optional patch (already applied = `git apply --reverse --check` succeeds). A plugin at another
  version with modified files, or a patch that no longer applies, is left alone: a
  note says so.
- `[[file]]`: user file put in place if it is absent; if it exists and differs, it is left and reported.
  `mode = "755"` (octal): permissions of the written file, put back if only they differ.
- `[[user]]`: without sudo. `check` (code 0 = already done), otherwise `run`, started by poste after confirmation.
  `requires` = required binary; `trigger` = run only if one of the named steps has just been done.
- `[[system]]`: with sudo. Poste checks (file content, line present, `check` command) and
  PRINTS the commands in a copyable block. It never runs `sudo`. A file unreadable without
  root (polkit) stays 'to check' until `poste sys done <id>`.
- `[[manual]]`: by hand; optional `check`, otherwise `poste sys done <id>` (`--undo` to cancel).
