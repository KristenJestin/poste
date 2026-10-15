# poste melodie

Plugs [Mélodie](https://github.com/KristenJestin/melodie), a Navidrome music player, into the
desktop. Mélodie itself (player daemon, local copy of the library, search, mixes, likes,
playlists, MPRIS) is not in poste: this module only declares where the island finds it.
It is installed only when `melodie` is in the `PATH` (`requires` in `module.toml`).

## What it brings

- **Launcher** (`island/launcher.json`): the « Music » extension, keyword `play` or `music`.
  Without typing, `melodie home --json` (mixes, then playlists); while typing,
  `melodie search --json`. The entries and their actions (play, shuffle, queue, mix, like, add to
  a playlist) come as they are from Mélodie, in the launcher's JSON format.
- **Player card** (`island/island.json`, `player` block): for the MPRIS player
  `org.mpris.MediaPlayer2.melodie`, the stream quality under the artist (`melodie:quality`),
  the heart lit for a liked track (`melodie:starred`, toggled by `melodie like --toggle`), and
  the sound source pill opens the launcher on the extension (Mélodie has no window of its own).
  Everything else on the card (title, cover, progress, controls) is the island's generic card.
- **Command**: `poste melodie <command>` passes the command as is to `melodie`.

## Install and login

Installing Mélodie is a choice of the machine, so it lives in the machine's data repository
(a step that downloads a pinned release and runs its `install.sh`), not in poste. Mélodie needs
`libmpv` (`mpv` package) and a Secret Service. Then, once per machine, in a terminal:
`melodie login`. The first copy of the library runs in the background.

## Contract

Mélodie's README and `docs/events.md` are the reference: launcher JSON, exit codes, MPRIS keys.
The desktop relies on exit codes and JSON, never on message text.
