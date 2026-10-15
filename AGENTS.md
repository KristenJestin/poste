# Rules for agents (poste repository)

- Everything in English: code, identifiers, documentation, comments, program messages and commit messages. No em dash.
- Each module is a separate application, with its own `README.md` and `AGENTS.md` (its own rules). What belongs to a module (launcher entries, island QML, Hyprland shortcuts, lines for files without includes) lives in the module (`island/`, `hypr/`, `[[line]]`), never in `config/`: the island and Hyprland know no module.
- `poste` (the dispatcher) hard-codes no module: it looks for `poste-<name>`, that is all.
- `poste sys setup` must never overwrite or delete what it did not put in place; before replacing a file it put in place, it keeps a copy in `~/.local/state/poste/backup/`.
- No secret in the repository or in the logs: credentials go in the system keyring (Secret Service).
- The repository is meant to be public: no personal data in the files or in the commit messages (email addresses, senders or contents of real emails, names of services or people, IPs and names of the local network, serial numbers, disk UUIDs, details of the work or of the work PC). Examples are invented (`example.com`, `192.0.2.10`, `<connection>`). What belongs to one machine lives in its local unversioned config (`~/.config/poste/`, `config.local`, `monitors.lua` of the work PC), never in the repository.
- No `sudo` (root steps are printed, never run).
- Test before committing: `cargo test` and `cargo clippy -- -D warnings`.
- Never stop a screen recording in progress or the music playing: check `poste record status` before restarting the island or the portal.
- Do not change what does not belong to poste (existing `~/.config/*`, chezmoi, Hyprland, the Quickshell island) until the project has decided to.
- Never load-test the machine (loops that occupy the processor, dozens of parallel processes): it is the user's real workstation. For an unstable test, look for the cause by reading and by simple repeats (`--test-threads=1`, ten runs at most).
