# Rules of the usage module

- Read-only: never write into `~/.local/state/omarchy/agents/usage/` (it is Omarchy's collector
  that writes it) nor touch Omarchy's scripts, which would be overwritten on its update.
- Never read or show the agents' login tokens (`~/.claude/.credentials.json`…):
  the collector takes care of them.
- Rust crate (`Cargo.toml`, listed in the `members` of the root `Cargo.toml`): no heavy
  dependency (serde, serde_json, libc).
- The history (`~/.local/state/poste/usage/history.jsonl`) has a single writer, `poste usage
  record` (the timer): a line only when an agent's limits change, and never anything
  but the time, the agent and the limits (label, percentage, reset).
- The timer is declared in `units` of `module.toml`: `poste sys setup` enables it; do not run
  `systemctl enable` by hand from the module.
- Agent logs: read-only, and never anything of their content in an output (neither message
  text, nor token, nor account identifier): times, sessions, models, efforts, folder
  names, token counts. A missing effort is `null`, never guessed.
- Tests: invented logs, collector and history in a temporary folder (test `HOME`, fake
  `omarchy-agent-usage-update` at the head of the `PATH`), never the real ones. A check on the
  real logs only shows aggregates, once in a while, never in a loop.
