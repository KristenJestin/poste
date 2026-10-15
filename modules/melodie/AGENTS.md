# Rules of the melodie module

- Mélodie stays a separate program: nothing of the island, theme or Hyprland goes into its
  repository, and this module reimplements nothing of Mélodie. It is configuration only
  (`island/`), plus the one-line `poste-melodie` relay.
- Never play sound on the real output and never stop, signal or query the user's running player
  to test. To try Mélodie, use temporary `XDG_CONFIG_HOME`, `XDG_CACHE_HOME`, `XDG_STATE_HOME`
  and `XDG_RUNTIME_DIR`, with `MELODIE_AO=null` and `MELODIE_MPRIS_NAME=<test name>`, and never
  send a request to the real server.
- Never touch `~/.config/melodie`, `~/.cache/melodie` nor the real keyring.
- Nothing specific to a machine in the module (server address, user, install path): that is
  Mélodie's own config, written by `melodie login`, and the machine's data repository.
