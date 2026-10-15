# poste power

Battery and disk alerts through a small user daemon (`poste-power watch`, started by the systemd
unit `poste-power.service` that `poste sys setup` enables). It replaces the hasty waybar scripts of
the old work laptop (`waybar-battery-ac-warn`, `waybar-disk-warn`, `omarchy-battery-monitor-plus` and
its systemd drop-in). Meant for the laptop; on a machine without a battery it only does the disk
alert (see "Not installing it on a machine" below).

## Alerts

Each one is sent ONCE per threshold crossing, and re-armed when the condition goes away. Notification
through `omarchy-notification-send` (island style, application 'Power').

| Alert | Condition | Urgency | Re-arming |
| --- | --- | --- | --- |
| Low battery | level <= 20 % and the battery is not charging | normal | it charges, or level > 20 % |
| Critical battery | level <= 10 % and the battery is not charging | urgent | it charges, or level > 10 % |
| Plugged in, but the battery is draining | mains plugged in AND (status 'Discharging' OR real loss > 12 W) for 2 passes in a row | urgent | the loss stops |
| Disk almost full | usage >= 90 % on `/` and `/home` (ignored if `/home` is on the same disk as `/`) | normal | usage < threshold - 2 points (hysteresis) |

- 'Not charging' = status other than `Charging` and `Full` (`Not charging` counts: a laptop's USB-C
  charger may not keep up).
- The real loss is measured on `energy_now` (or `charge_now` x `voltage_now`) between two passes: the
  status and `power_now` sometimes lie (lesson from the old laptop, unstable USB-C charger). A
  measure after a long sleep is ignored.
- A direct jump below 10 % only triggers 'critical'. An alert whose sending fails is retried at the
  next pass.
- Only the system's supplies count (`scope` other than `Device`: a mouse battery
  `hidpp_battery_*` is never 'the battery'). Mains = type `Mains` online.
- Without a battery: only the disk alert.
- No plug / unplug alert: the references had none (the noise is useless).

## Commands

- `poste power status`: battery, mains, disks, thresholds, alerts already sent.
- `poste power test`: sends a sample notification.
- `poste power check`: one pass (what the daemon does every minute).
- `poste power watch`: the daemon.

## Settings: `~/.config/poste/power.toml`

Optional; each missing key keeps its default value. Read again at each pass (no restart).

```toml
interval = 60            # seconds between two passes (10 at least)

[battery]
low = 20
critical = 10
drain_w = 12.0           # loss (W) that counts as 'draining on mains'
drain_samples = 2        # consecutive passes before alerting

[disk]
percent = 90
free_gb = 0              # also alert below this number of free GB (0: no)
paths = ["/", "/home"]
```

## Light daemon

One pass every 60 s: a few reads of `/sys/class/power_supply/*` and one `statvfs` per disk, then
`sleep`. No udev (the 60 s polling is enough and stays simple). The 'already alerted' state is in
`$XDG_RUNTIME_DIR/poste-power/state.json` (lost at the machine's restart, kept when the daemon
restarts). The unit (`systemd/poste-power.service`) runs with `Nice=19` and `MemoryMax=64M`.

## Not installing it on a machine

In this machine's `~/.config/poste/config.toml`: `poste sys module disable power` (the next
`poste sys setup` then removes files and unit, with a preview).

## Development

Variables RESERVED FOR TESTS: `POSTE_POWER_SUPPLY_DIR` (fake `power_supply` in a temporary folder) and
`POSTE_POWER_NOTIFY` (replaces `omarchy-notification-send`). The tests (`cargo test -p poste-power`):
thresholds and re-arming (`src/engine.rs`), reading of a fake tree (`src/supply.rs`), successive
passes (`src/lib.rs`), complete binary with fake notifications (`tests/cli.rs`).
