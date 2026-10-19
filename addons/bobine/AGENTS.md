# Rules of the bobine module

- Bobine stays a separate program: nothing of the island, theme or Hyprland goes into its
  repository, and this module reimplements nothing of Bobine. It is the install steps, plus the
  one-line `poste-bobine` relay and the launcher extension (`island/launcher.json`), which only
  calls `bobine home --json` and `bobine search --json`.
- Never open Bobine's window on the user's screens, never play a film on the real output and
  never stop, signal or query the user's running Bobine to test. A window or playback test runs in
  a headless nested compositor, with temporary `XDG_CONFIG_HOME`, `XDG_DATA_HOME`,
  `XDG_CACHE_HOME`, without `DBUS_SESSION_BUS_ADDRESS` (so that the real keyring and account are
  never read), and never sends a request to the real server.
- Never touch `~/.config/bobine` nor the real keyring.
- Nothing specific to a machine in the module (server address, user, install path): that is
  Bobine's own config, written at its sign-in, and the machine's data repository.
