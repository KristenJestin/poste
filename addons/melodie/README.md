# poste melodie

Plugs [Mélodie](https://github.com/KristenJestin/melodie), a Navidrome music player, into the
desktop. Mélodie itself (player daemon, local copy of the library, search, mixes, likes,
playlists, MPRIS) is not in poste: this module only declares where the island finds it.
It is an add-on: installed only on a machine that lists `melodie` in `addons` (`poste.toml` of
its data repository), and plugged in once `melodie` is in the `PATH` (`requires`).

## What it brings

- **Launcher** (`island/launcher.json`): the « Music » extension, keyword `play` or `music`.
  Without typing, `melodie home --json` (mixes, then playlists); while typing,
  `melodie search --json`. The entries and their actions (play, shuffle, queue, mix, like, add to
  a playlist) come as they are from Mélodie, in the launcher's JSON format.
- **Player card** (`island/island.json`, `player` block): for the MPRIS player
  `org.mpris.MediaPlayer2.melodie`, the stream quality under the artist (`melodie:quality`),
  the heart lit for a liked track (`melodie:starred`, toggled by `melodie like --toggle`), and
  the sound source pill brings Mélodie's window up, or opens it (through the DesktopEntry its
  MPRIS gives).
  Everything else on the card (title, cover, progress, controls) is the island's generic card.
- **Command**: `poste melodie <command>` passes the command as is to `melodie`.
- **App**: Mélodie's `install.sh` puts « Mélodie » (its window, `melodie-desktop`) among the
  launcher's apps; `bootstrap/files/melodie-open.desktop` opens `melodie:` addresses
  (`xdg-open melodie:album/<id>`) with `melodie open`, which Mélodie does not install.

## Install and login

`bootstrap/steps.toml` downloads the pinned release (sha256 checked) and runs its `install.sh`,
which puts `melodie` in `~/.local/bin`; to update, change the version there. The install stops the
running player: while Mélodie plays, the step is skipped instead (it fails and says so). `bootstrap/packages.toml`
asks for `mpv` (libmpv). Mélodie also needs a Secret Service. Then, once per machine, in a
terminal: `melodie login` (manual step `melodie-login`). The first copy of the library runs in
the background.

## Contract

The launcher format is poste's `docs/launcher-format.md`; Mélodie's README and `docs/events.md` are the reference for its exit codes and MPRIS keys.
The desktop relies on exit codes and JSON, never on message text.
