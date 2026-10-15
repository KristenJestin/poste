# usage

AI agent quotas (Claude Code, Codex…): percentage of each limit (5 h session, week…)
and reset time, their history, and the consumption read back from the agents' logs,
to answer "why do I reach the 5 h quota faster than before" without reading the logs by
hand.

## Commands

- `poste usage [agent]`: quotas of an agent, `claude` by default; `--all` all the agents recorded,
  `--refresh` redoes the reading before showing, `--json` the collector's raw record.
- `poste usage history [agent…] [--since <date>] [--json]`: the history of the readings (last 24
  hours by default).
- `poste usage calls [--since <date>] [--until <date>] [--agent claude|codex] [--json]`: one call per
  line, duplicates removed (last 24 hours by default).
- `poste usage windows [--since <date>] [--until <date>] [--agent …] [--json]`: each 5 h window
  and its indicators (last 7 days by default).
- `poste usage compare <period A> <period B> [--agent …] [--json]`: the same indicators side by side.
  A period is written `<start>..<end>`, end absent: now
  (`poste usage compare "14 days ago..7 days ago" "7 days ago.."`).
- `poste usage record`: a reading of all the agents, added to the history if it changed (what the
  timer runs).

Dates: ISO 8601, or anything `date -d` understands (`"3 days ago"`, `"yesterday 14:00"`,
`2026-10-01`…). `--json`: one JSON object per line (`compare`: a single object `{a, b}`), for `jq`.

## Quotas: Omarchy's collector

`omarchy-agent-usage-update` calls one collector per agent (`omarchy-agent-usage-claude`…), which
queries the agent's service with the login of its tool, and writes
`~/.local/state/omarchy/agents/usage/<agent>.json`. It is the same source as the island's
control center. `poste usage` redoes the reading (`--limits-only`) if it is older than 5 minutes.

Exit code 1 if an agent has no reading or if its reading reports an error (expired login: the
collector's message says what to do).

## History

Omarchy's collector only keeps the last reading. The timer `poste-usage-record.timer`
(put in place by `poste sys setup`, every 5 minutes, service `poste-usage-record.service`) runs
`poste usage record`: reading of all the agents (`--limits-only`), then one line per agent in
`~/.local/state/poste/usage/history.jsonl`, only if its limits changed since its
previous line (the file stays small):

```json
{"t":"2026-10-14T16:20:00Z","agent":"claude","limits":[{"label":"Session (5-hour)","percent":11,"resetsAt":"2026-10-14T21:00:00Z"}]}
```

- `t`: time of the reading (UTC); `percent`: 0 to 100, one decimal; `resetsAt`: reset
  (UTC) rounded to the minute (the server gives it to the microsecond and it moves from one reading to
  the next: without rounding, every reading would be a change).
- An agent in error (expired login…) or without a limit writes nothing.
- Nothing else: no token, no account identifier, no local statistic.

Check the timer: `systemctl --user list-timers poste-usage-record.timer`.

## Calls: the agents' logs

A line of `calls`: time, agent, session, project (name of the working folder), model, effort,
sub-agent or not, and the separate tokens: `input` (input outside the cache), `cache_read`, `cache_write`,
`output`, `reasoning` (included in `output`). Never the content of the messages.

Claude Code (`~/.claude/projects/`, or `$CLAUDE_CONFIG_DIR/projects/`):

- One `assistant` line per response block: the same call (`requestId`) comes back several times
  (nearly half of the lines). It counts once, in its most complete version. A resumed session
  copies calls of the previous one into another file: the deduplication applies to all
  the files.
- Sub-agents: in `<project>/<session>/subagents/` (and `subagents/workflows/<id>/`), `isSidechain`
  lines. They are counted, and marked `subagent`.
- Effort: `perTurnEffort` of the line, otherwise `effort`; absent (a few old messages):
  `null`, never guessed.
- Reasoning: `usage.output_tokens_details.thinking_tokens`. Cache: `cache_read_input_tokens`
  (read) and `cache_creation_input_tokens` (written), separate. The `<synthetic>` messages (made up by
  the tool, without a call) are ignored.

Codex (`~/.codex/sessions/`, or `$CODEX_HOME/sessions/`):

- One call per `token_usage_record` (`response_id`). The older files do not have any: one
  call per `token_count` event whose cumulative total moved (the same event is repeated).
- Model and effort: the `turn_context` of the turn. Sub-agent: `session_meta.source.subagent`.
- Codex's `input_tokens` also counts the cache read: `input` is the input outside the cache.

Speed: all the logs (2.4 GB) are read again in 2 to 3 s (5 s when they are not in the memory
cache); a file that has not moved since `--since` is not opened. No cache on disk:
useless at this speed, and nothing to invalidate.

## 5 h windows

- From the history: the session limit (`Session (5-hour)`, `5h window`) gives the reset
  R; the window is `[R - 5 h, R)`. Peak percentage, and time to the first reading at 100%
  (`capped_after_minutes`).
- Without a reading (before the history, or during a gap): window estimated from the calls, marked
  `estimated`: it starts at the first call that no window covers (at the hour sharp below
  for Claude, whose resets fall on the hour) and lasts 5 h, without a percentage.
- Indicators: calls (including sub-agents), sessions, parallel sessions (the most sessions that
  made a call in the same 5 min slice), share of each effort and of each model (in
  calls and in output tokens), tokens, and tokens per quota point (recorded windows only:
  tokens / peak).
- `compare` gathers the windows that start in each period: number, capped windows and
  average time to the cap, calls per window, average of parallel sessions, shares, tokens,
  tokens per point.
- Limit: only this machine's logs are read. A quota started elsewhere counts in the
  percentage, not in the calls.

## Why Rust

The module was a shell script (`jq`). Reading 2.4 GB of logs again with `jq` at each question takes
tens of seconds, and the deduplication, the windows and the comparisons would be
unreadable and untested there. The Rust crate (`serde_json`, `libc` for the local time, nothing heavy)
reads everything again in 2 to 3 s, without a cache.
