# poste record

Screen recording with `gpu-screen-recorder`, driven by the island. It is a Rust crate
(`poste-record`, listed in the `members` of the root `Cargo.toml`): `poste sys setup` builds the
binary in release and installs it in `~/.local/share/poste/modules/record/poste-record`, and
`poste record …` runs it. The original Python script stays in the repository (`poste-record.py`,
`poste-share-picker.py`) while the validation for real goes on; it is not installed.

## Commands

- `poste record start --preset ID`: a preset (see below).
- `poste record start --window ADDR [--audio A]`: a window (Hyprland address).
- `poste record start --monitor NAME [--audio A]`: a whole screen.
- `poste record start --region [--audio A]`: a region picked with the mouse.
- `poste record stop | pause | resume | toggle`: stop (the file is finished cleanly),
  pause, resume, or toggle pause/resume.
- `poste record status`: state as JSON (`{"t": "status", "recording": false}` at rest).
- `poste record watch`: one JSON line per state change (read by the island).
- `poste record items`: the list of the launcher's "Record" extension (JSON).
- `poste record hotkey`: SHIFT+PRINT. Stops the recording in progress, otherwise opens the island's
  launcher on "Record".
- `poste record open-dir | open-last [--preset ID]`: the folder, or the last recording.

Sound (`--audio`): `none`, `app` (the sound of the window's app), `desktop`, `mic`,
`desktop+mic`, `app+mic`, or raw `gpu-screen-recorder` sources separated by `|`.

## Presets (user config)

`~/.config/poste/record/presets.json` (`XDG_CONFIG_HOME` honoured), versioned in
`config/.config/poste/record/presets.json` and put in place as a link. It used to be in
`~/.config/poste-island/record/presets.json`: moved without changing its content; the old link
is spotted as an orphan and removed by `poste sys setup`. The format is described in the
`"//"` field of the file (`defaults`, `presets`, `audioSource`, `audioAppName`, `cursor`, `dir`…).

## What the module brings to the machine

| Where | File of the module | Role |
| --- | --- | --- |
| Hyprland | `hypr/record.lua` | SHIFT+PRINT to `poste record hotkey`, `hl.unbind("ALT + PRINT")` |
| Island launcher | `island/launcher.json` | extension `record` (keywords `rec`, `record`), with `"highlight": true`: the island frames the selected target on screen |
| Island (slot `capture`) | `island/island.json`, `island/RecordState.qml` | recording state for the capture pill and its card (shared with OBS, which stays in the island) |
| `xdph.conf` | line of `module.toml` (`[[line]]`) | `screencopy:custom_picker_binary` to `bin/poste-share-picker` |
| Auxiliary program | `bin/poste-share-picker` | screen-share picker of xdg-desktop-portal-hyprland: short script (stable path for the `xdph.conf` line) that runs `poste-record share-picker`. The latter answers alone with the window requested by `poste record` (file `pick.json` less than 20 s old in `$XDG_RUNTIME_DIR/poste-record`), otherwise hands over to `hyprland-preview-share-picker` (`exec`, same arguments) |

Without the module, none of these traces exists (no shortcut, no launcher entry, no `xdph.conf` line).
`xdg-desktop-portal-hyprland` only reads its config again at startup: after putting the
line in place or removing it, restart the portal (`systemctl --user restart xdg-desktop-portal-hyprland`).

## Files

- `$XDG_RUNTIME_DIR/poste-record/`: `state.json` (recording in progress), `pick.json`, `last.log`.
- Notifications: "Recording saved" (a click opens the folder) or the error.

## Development

- `poste-record share-picker` is a hidden subcommand (called by `bin/poste-share-picker`),
  like `poste-record --supervise '<json>'`, the detached supervisor launched by `start`
  (`setsid`, it keeps gpu-screen-recorder, writes `state.json`, cleans up and notifies at the end).
- Environment variables RESERVED FOR TESTS: `POSTE_RECORD_GSR` (replaces `gpu-screen-recorder`),
  `POSTE_RECORD_HYPRCTL` (replaces `hyprctl`), `POSTE_RECORD_NOTIFY` (replaces
  `omarchy-notification-send`), `POSTE_RECORD_SLURP` (replaces `slurp`).
- Tests (`cargo test -p poste-record`):
  - `tests/parity.rs` and `tests/calc.rs` play the same scenarios with `poste-record.py` and with the
    binary, on the same fake programs (`tests/fakes/fake.sh`), and compare JSON outputs (byte
    for byte), gpu-screen-recorder arguments, notifications and created files. Without `python3`,
    they are skipped.
  - `tests/cycle.rs`: start, pause, resume, toggle, stop, notification, open-last, hotkey, failure of
    gpu-screen-recorder, share picker, with a fake gpu-screen-recorder (never the real one).
- Deliberate differences with the Python script (all without effect on its uses): a missing or zero pid in
  `state.json` is never signalled (the Python would have targeted its process group); a failure to
  launch gpu-screen-recorder notifies "Recording failed" (the Python stopped silently);
  an invalid preset regular expression matches no window (the Python
  crashed); a malformed entry of `presets.json` (preset without `id`, element that is not an
  object) is ignored instead of making `items` fail; a flag without a value (`--preset` last)
  gives a message and code 1. The regular expressions are those of the `regex` crate
  (no backtracking or lookaround assertions, unlike `re`).
