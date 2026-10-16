# poste desktop: useful commands

This file (AGENTS.md) is loaded only by the island's AI (poste-ask starts it from this
folder) and by the Claude Code skill `poste-desktop`. Arch + Omarchy 4,
Hyprland 0.56, Quickshell shell with the personal island `poste.island`.

## Island (`omarchy-shell poste.island <command>`)
- `timer "10m pasta"`: timer (formats: `25`, `5m30`, `1h30`, `90s`, `12:30`, label after). `timerStop`: stops all poste timers.
- Omarchy's reminders (`omarchy-reminder`, SUPER+CTRL+R) also show as timers (read from systemd every 5 s, never written to `timers.json`): without pause or +1 min, they disappear on time (Omarchy's notification is the alert). `timerCancel "omarchy:<unit>"` cancels one; `timerStop` only touches poste's timers.
- `ask "question"`: opens the AI view (empty = reopens the current conversation). `askPin`: puts the conversation in the right-hand pill, or removes it from there.
- `askBg "question"` (or `askBgWith "question" codex`): starts an AI in the background, in a pinned pill, without opening the view. `askUnpinAll`: removes all idle AI pills.
- `devices`: batteries of wireless devices as JSON (keyboard, mouse, headset). `deviceSim "mouse 12"` / `batterySim "charging 75"`: simulations (`off` to stop).
- Power profile (power-profiles-daemon): "Power" tile of the control center, on the settings row with Sound and Silence (profile icon and name, a click goes to the next one, also on a desktop PC), profile icon in the battery bubble 6 s after unplugging, OSD in the banner at each change. `profile`: current profile and the machine's profiles (JSON). `profileSim "power-saver"`: simulation (`off` to stop). To change it: `omarchy-powerprofiles-set autodetect <profile>` (remembers it for mains or battery), only if the user asks; never `powerprofilesctl set`.
- `usage` / `usageOf codex`: detail of Claude Code or Codex consumption (sessions, agents, models; script `bin/poste-agent-usage [session|week] [claude|codex]` as JSON).
- `lock`: sound then lock. `launcher`, `menu`, `notifications` (a click on one opens its picture, else the file its toast opened (a Taildrop file), else focuses its app; no other stored command is run), `player`, `controls`, `close`.
- `module wifi|bluetooth|audio|vpn`: opens that panel. `unread`: number of unread notifications.

## Sounds
- `~/.config/omarchy/plugins/poste.island/bin/poste-sfx <event>`: timer-done, dictation-start/stop/paste, screenshot, lock, unlock, ai-done, notification. Settings: `~/.config/poste-island/sounds/sounds.json`.

## Windows and apps (Hyprland 0.56, dispatch in Lua)
- List: `hyprctl clients -j`. Focus: `hyprctl dispatch 'hl.dsp.focus({ window = "address:0x…" })'`.
- Start a detached graphical app: `setsid -f uwsm-app -- <app> >/dev/null 2>&1`.
- Workspaces: 1-10 on the first screen of `workspace_monitors` (monitors.lua), 11-20 on the second, etc.

## Music (`melodie`, Navidrome server)
Background player (MPRIS 'Mélodie'), also shown by the island. Playback commands act on what the user is listening to: run one only when asked.
- Search: `melodie search daft punk --json` (artists, albums, tracks, playlists; `id` is `kind:id`). `melodie playlists --json`, `melodie mixes --json`.
- Play: `melodie play playlist|album|artist|track <id> [--shuffle]` (replaces the queue). Add to the queue: `melodie queue [--next] album <id>`.
- Mixes: `melodie mix me`, `mix rediscover`, `mix artist "Daft Punk"`, `mix genre rock`; `--print` lists without playing.
- Controls: `melodie toggle|pause|resume|next|prev|stop`, `melodie volume +5` (or `30`).
- State: `melodie now --json` (title, artist, position, quality, `starred`). Without a player running, control commands answer 'nothing playing', exit code 1.
- Likes and playlists: `melodie like` (current track, `--toggle` to flip), `unlike`, `melodie playlist add <playlistId> current`, `playlist create "Name" current`.
- Login: `melodie login` (interactive, for the user to run in a terminal), `melodie status`.
- Rely on exit codes and JSON, never on message text.
- In the island launcher: keyword `play` or `music`.

## Files
- File manager: Strata, always by its full path (`~/.local/bin/strata <folder>`; a `strata` in `/usr/bin` may be an old package). SUPER+E opens it on the active terminal's folder. A single instance: never kill it, never start it to test something.
- Its context menu has 'Send with Taildrop' (files to a device of the tailnet), from `~/.config/strata/actions/taildrop/`.

## AI agent quotas
- `poste usage`: Claude Code quotas (5 h session, week), as a percentage, with the reset time. `poste usage codex`, `poste usage --all`, `--refresh` to redo the reading, `--json` for the raw data.
- The source is Omarchy's collector (`~/.local/state/omarchy/agents/usage/<agent>.json`, the same as the control center), never `~/.claude`: Claude Code's logs say nothing about quotas.
- Any question about consumption (why the 5 h quota goes faster, which model or effort costs, sub-agents, parallel sessions): these commands, never a manual reread of the logs (duplicates, sub-agents, reasoning: already handled). All have `--json` (readable with `jq`) and `--agent claude|codex`; dates: ISO or what `date -d` understands.
  - `poste usage history [--since "3 days ago"]`: the quota history (read every 5 min, one line when it changes).
  - `poste usage calls --since … [--until …]`: one call per line, duplicates removed (model, effort, sub-agent, input tokens, cache read and written, output, reasoning).
  - `poste usage windows [--since …]`: each 5 h window (peak, time until the ceiling, calls, parallel sessions, share of each effort and model, tokens per quota point); `estimated`: window guessed from the calls, without a reading.
  - `poste usage compare "14 days ago..7 days ago" "7 days ago.."`: the same indicators for two periods, side by side.

## Rules
- Never sudo: give the command to the user.
- Delete, overwrite or push nothing without a clear request.
- Launcher settings: `~/.config/poste-island/launcher/launcher.json`. Island code: `~/.config/omarchy/plugins/poste.island/` (link to `~/.local/share/poste/current`: in dev mode it is the repository, a change is already there, `poste sys git commit` then `push` remain; in release mode it is a read-only installed version, `poste sys use dev` first, `poste sys versions` to find out).
