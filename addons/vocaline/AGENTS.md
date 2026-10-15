# Rules of the vocaline module

- Never run `press`, `release`, `start`, `toggle` or `stop` to test: that opens the microphone.
  `watch` and `status` are harmless. To see real events, ask for a dictation; otherwise,
  replay an invented stream.
- Vocaline stays a separate program: nothing of the island, theme, sound or Hyprland goes into its
  repository, and this module reimplements nothing of Vocaline (it passes the commands as is).
- The stream contract is `docs/events.md` of the Vocaline repository: an unknown type or field is ignored.
- No path specific to a machine in the module (neither the path of a trial version, nor the config
  of Vocaline): `~/.config/vocaline/` comes from the machine's data repository.
- No dictated text in a file, a log or a commit message.
- The module knows the island only through `island/` (slot `dictation`); the key is in
  `hypr/vocaline.lua`, never in `config/`.
- `poste-vocaline` stays a short shell script, without dependency.
