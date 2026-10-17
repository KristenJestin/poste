# poste bobine

Plugs [Bobine](https://github.com/KristenJestin/bobine), a native film and series player
(Jellyfin first), into the desktop. Bobine itself (its window, the libraries, the player, Seerr
requests, MPRIS) is not in poste: this add-on installs it and keeps it up to date.
It is an add-on: installed only on a machine that lists `bobine` in `addons` (`poste.toml` of
its data repository), and plugged in once `bobine` is in the `PATH` (`requires`).

Written for Bobine 0.2.0 and later (`install.sh` in its release archive, and its command line for the launcher).

## What it brings

- **Launcher** (`island/launcher.json`): the 'Films' extension, keyword `watch` or `film`.
  Without typing, `bobine home --json` (Continue Watching, then Next Up); while typing,
  `bobine search --json` (films and series). The entries and their actions (resume, play from the
  start, open the page) come as they are from Bobine, in the launcher's JSON format; `open` and
  `play` go to the Bobine window already running.
- **App**: Bobine's `install.sh` puts 'Bobine' (its window) among the launcher's apps, with its
  icon.
- **Player card**: nothing to declare. Bobine publishes the film playing on MPRIS
  (`org.mpris.MediaPlayer2.bobine`, DesktopEntry `bobine`), so the island's generic card shows
  it (title, cover, progress, controls) and its source pill brings Bobine's window up.
- **Command**: `poste bobine <arguments>` passes them as is to `bobine` (`poste bobine --version`).
- **Hyprland**: nothing here. Bobine handles its own fullscreen and the black curtain on the
  other monitors.

## Install and login

`bootstrap/steps.toml` downloads its latest release, the beta included (sha256 checked), and
runs its `install.sh`, which puts `bobine` in `~/.local/bin`; `poste sys update bobine` moves it
later. While a film plays, nothing is replaced (it fails and says so): run the update again
after the film. `bootstrap/packages.toml` asks for `mpv` (libmpv). Bobine also needs a Secret
Service. Then, once per machine: sign in to the server in Bobine's window (manual step
`bobine-login`).

## Contract

The launcher format is poste's `docs/launcher-format.md`. The desktop relies on `bobine --version`
(last word: the version), `home --json` and `search --json`, the archive's `install.sh`
and the MPRIS name; Bobine's `docs/versions.md` is the reference for what a version promises.
