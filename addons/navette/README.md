# poste navette

Plugs [Navette](https://github.com/KristenJestin/navette), the work-from-home mode between two
Hyprland machines, into the desktop: a bubble left of the island when the other machine is there,
its mode and its mic. Navette itself (the link, the pictures, the keyboard, the sound) is not in
poste: this add-on installs it, keeps its service running and shows what it says.

Written for Navette 0.2.0 and later (`install.sh`, `navette run`, `navette.service`, `events.sock`).

## How it works

- **The same add-on on both machines.** The desk is the machine with the screens, the remote the
  one it drives. The role is fixed at pairing; poste never reads it.
- **The service**: `navette.service` (Navette's own unit, `navette run --audio`) is enabled and
  started by `setup`. It runs `serve` on the desk and `agent` on the remote. poste never decides
  when it runs: away from the desk's network, the agent sleeps on its own (`away`).
- **The bubble** (`island/NavetteState.qml`, slot `remote` of the island) reads Navette's event
  socket, `$XDG_RUNTIME_DIR/navette/events.sock`: the current state on connection, then one line
  per change, nothing in between (no polling). Navette absent: the island checks every 5 seconds
  whether the socket is back.
  - On the desk, the other machine is there (`linked`): the bubble shows its name. A click starts
    the mode at once when `mode.conf` has a single one; otherwise the card lists them.
  - A mode starting, running or stopping: on the desk, the bubble shows where the mic goes, and
    its card toggles the mic or stops the mode; on the remote, it says what drives it.
  - The two machines run versions that do not talk (`incompatible`): the bubble says to update.
  - The actions run `navette mode start|stop` and `navette mic toggle`.
- **Updates**: `poste sys update` moves Navette, never while a mode starts, runs or stops
  (`navette watch --json --once`), then restarts the service on the new version. Update both
  machines together: they must run the same minor version.
- **Hyprland**: nothing here. Navette sets its own mic key (`SUPER + CTRL + M`) during a mode and
  removes it at the stop.

## Commands

`poste navette <command>` passes the command as is to Navette (`mode start`, `mode stop`,
`mic toggle`, `status`, `pair`…: see `navette --help`).

## Machine configuration

In the machine's data repository, never in poste:

- `~/.config/navette/mode.conf` on the desk: its modes and the machines' labels. Put
  `here_notice = off` in it: the bubble replaces Navette's `<remote_label> is here` notification.
- The pairing (`identity.key`, `paired.conf`) stays on each machine and is never versioned.

## Choose it on a machine

`addons = ["navette"]` in the `poste.toml` of both data repositories, then `poste sys setup`:

1. Navette is installed from its latest release (archive checked, its `install.sh` run).
2. The pairing, once (`[[manual]]` step `navette-pair`):
   - on the desk: `navette pair --listen <desk LAN address>:<port>` shows a PIN;
   - on the remote: `navette pair <desk address>:<port>` asks for it;
   - then, on both: `systemctl --user restart navette` (the service exits while not paired).
3. On the desk only, the firewall lets the remote reach that port (`[[manual]]` step
   `navette-firewall`; on the remote, `poste sys skip add navette-firewall`). With Omarchy's ufw:

   ```
   sudo ufw allow in proto tcp from <remote address> to <desk address> port <port> comment navette
   ```

   then `poste sys done navette-firewall`.

Removing it from the list removes the bubble; Navette and its service stay installed.
