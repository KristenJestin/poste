# Rules of the record module

- NEVER stop, pause or disturb a recording in progress. Before any
  handling (test, install, restart of the island or of the portal), run
  `poste record status`: if `"recording": true`, apply nothing and wait.
- Never run `poste record start` to test (it really records the screen); only test the
  config reading, `items` and `status`. The automatic tests never use the real
  gpu-screen-recorder: `POSTE_RECORD_GSR`, `POSTE_RECORD_HYPRCTL` and `POSTE_RECORD_NOTIFY` (reserved
  for tests) replace it with `tests/fakes/fake.sh`, with a temporary HOME, `XDG_RUNTIME_DIR` and PATH.
  Never write into the real `$XDG_RUNTIME_DIR/poste-record` from a test.
- It is a Rust crate (`poste-record`, binary and library, in the root `members`), like `music`.
  `poste-record.py` and `poste-share-picker.py` are the old Python version, kept only as a
  reference for the parity tests (`tests/parity.rs`, `tests/calc.rs`) and to be able to go back; they are not
  installed. Delete them once the Rust version is validated for real (and the parity tests with them).
- Any change of format (`status`, `watch`, `items`, `state.json`, `pick.json`) must stay readable
  by the island (`island/RecordState.qml`, `island/launcher.json`): the parity tests compare the outputs
  byte for byte with the Python, do not "fix" them without a decision.
- `bin/poste-share-picker` stays a minimal shell script: it is the stable path of the `xdph.conf` line,
  the work is in `poste-record share-picker`. No tokio; minimal dependencies (serde, serde_json,
  libc, regex).
- The module only knows the island through its `island/` files (slot `capture`,
  launcher extension) and the command `omarchy-shell poste.island launcherIn record`.
- Nothing of the module lives outside it: shortcuts in `hypr/record.lua`, `xdph.conf` line in
  `module.toml`; put nothing back in `config/` (bindings.lua, xdph.conf, launcher.json).
- The presets are user config (`~/.config/poste/record/presets.json`): never overwrite them.
