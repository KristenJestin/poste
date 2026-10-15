# Rules for the power module

- Light daemon: one pass every 60 s, never a busy loop, no heavy dependency (serde, toml, libc).
- No test reads the real `/sys/class/power_supply` or sends a real notification: `POSTE_POWER_SUPPLY_DIR` and `POSTE_POWER_NOTIFY` (reserved for tests) replace them; the state goes in a temporary `XDG_RUNTIME_DIR`. Never a load test.
- The alert logic is in `engine::step` (pure function, tested): do not put any input/output in it. One alert per threshold crossing, re-armed when the condition goes away.
- Peripheral batteries (`scope` = `Device`) are never the system battery.
- The systemd unit is in `systemd/` and declared by `units` in `module.toml`: `poste sys setup` enables it; do not run `systemctl enable` by hand from the module's code.
- Thresholds = user config (`~/.config/poste/power.toml`): never overwrite it.
