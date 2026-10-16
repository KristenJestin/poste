# poste vocaline

Plugs [Vocaline](https://github.com/KristenJestin/vocaline) dictation into the desktop: the SUPER+A
key, the dictation pill at the bottom of the active screen (microphone and wave) and the dictated
text live just under the island. Vocaline itself (daemon, microphone, engine, paste) is not in
poste: this module only links it to the island and to Hyprland.

## How it works

- SUPER+A (`hypr/vocaline.lua`): `poste vocaline press` on the press, `poste vocaline release` on
  the release. Hold to talk, or tap to start then tap again to stop.
- Nothing is typed in the window while you talk: at the stop, Vocaline pastes the whole dictation
  in one block into the window you started from.
- The island reads the `watch` stream directly on Vocaline's socket (`$XDG_RUNTIME_DIR/vocaline.sock`,
  `island/VocalineState.qml`, slot `dictation`):
  - everything is shown in the banner (`bandeau`), on the active screen: the pill at the bottom, the text under the island;
  - `loading` and `listening`: the pill with the microphone and the wave (`level`), and the listening sound;
    the module only gives the current level (`level`, scaled to the island's wave
    and smoothed between two events, since the daemon sends 10 per second),
    and the banner samples it at 100 per second;
  - `stopping`: the end dots, and the end sound;
  - under the island: the frozen sentences (`final`), then the sentence in progress (`partial`) in grey,
    three lines at most (the end stays visible);
  - `idle`: everything fades.
- Daemon absent or stream cut: the island checks every 5 seconds whether the socket is back, and only
  reconnects then (nothing in the log while it is absent).

## Commands

`poste vocaline <command>` passes the command as is to Vocaline (`press`, `release`,
`status`, `watch`…: see Vocaline's help).

## Machine configuration

The program is `vocaline` from the `PATH` (or `~/.local/bin`): the add-on is plugged in only
when it is there. To try another build, put it first in the `PATH` under the name `vocaline`.

Vocaline's own config (`~/.config/vocaline/`) is also kept in the machine's data repository,
never in poste.

## Choose it on a machine

This is an add-on: it is installed only on a machine that lists it in the `poste.toml` of its
data repository (`addons = ["vocaline"]`), and plugged in once `vocaline` is in the `PATH`
(`requires`). The `vocaline` package comes from its GitHub release: `poste sys setup` (if absent) or
`poste sys update` downloads and checks it, then runs the `sudo pacman -U` in the terminal
(`setup` only prints it). Its pre-releases (the `dev` builds) are followed.
Removing it from the list removes the key and the island part.
