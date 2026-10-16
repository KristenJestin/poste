# Rules of the navette module

- Never run `navette mode start`, `mode stop`, `mic` or `pair` to test: a mode takes the user's
  screens, keyboard and mic. `watch --json --once`, `status` and `mode list` are harmless. To see
  the bubble, replay an invented stream on a socket of a temporary `XDG_RUNTIME_DIR`, in a
  windowless Quickshell.
- Navette stays a separate program: nothing of the island, theme or Hyprland goes into its
  repository, and this module reimplements nothing of Navette (it passes the commands as is).
  poste never reads `paired.conf` nor decides when the service runs: Navette does.
- The stream contract is `docs/events.md` of the Navette repository: an unknown field or value is
  ignored, and `home` is read as `desk`.
- No address, label or monitor of a real machine in the module: `mode.conf` and the firewall rule
  come from the machine's data repository.
- The module knows the island only through `island/` (slot `remote`).
- `poste-navette` stays a short shell script, without dependency.
