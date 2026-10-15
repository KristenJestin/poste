# poste voxtype

Plugs the [voxtype](https://github.com/peteonrails/voxtype) dictation into the desktop: the SUPER+A
key and the dictation pill at the bottom of the active screen (mic and wave). voxtype itself
(daemon, mic, engine, paste) is not in poste: this add-on only links it to the island and
Hyprland.

## How it works

- SUPER+A (`hypr/voxtype.lua`): `poste voxtype press` on the press, `poste voxtype release` on
  the release. A short tap starts the recording and another tap stops it; holding the key is
  push-to-talk.
- voxtype pastes the text with Ctrl+V once the transcription is over. Through the config, it runs
  `poste voxtype focus save` when recording starts and `poste voxtype focus restore` before the
  paste: the text goes to the window that was focused at the start, after SUPER is released.
- The island (`island/VoxtypeState.qml`, slot `dictation`) reads the daemon's state file
  (`$XDG_RUNTIME_DIR/voxtype/state`) and, while recording, the mic level from
  `voxtype-audio-bridge`: the pill shows the mic and the wave, then the dots while transcribing.
  voxtype gives no text while one speaks, so nothing shows under the island.

## Commands

`poste voxtype press|release` (the key), `poste voxtype focus save|restore` (voxtype's
`pre_recording_command` and `pre_output_command`); anything else goes to voxtype as is
(`poste voxtype status`…).

## Setup of the machine

`poste sys setup` installs `voxtype-bin`, writes `~/.config/voxtype/config.toml` when it is
missing (Whisper `small`, then left as it is), downloads the model, then installs, enables and
starts the `voxtype.service` user service. A machine with its own voxtype config keeps it in its
data repository and skips the file step (`skip = ["voxtype-config"]`); GPU builds (CUDA drop-in of
the service) are a machine choice too.

## Choose it on a machine

This is an add-on: it is installed only on a machine that lists it in the `poste.toml` of its
data repository (`addons = ["voxtype"]`), and plugged in once `voxtype` is in the `PATH`
(`requires`). Choose a single dictation add-on (`voxtype` or `vocaline`): both take SUPER+A.
Removing it from the list removes the key and the island part.
