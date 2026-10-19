# Rules of the unity module

- The JSON of `projects --json` follows `docs/launcher-format.md`: `verb` is "Open" and the
  name is the project's (the launcher writes "Open <name>" in its main list). Do not add a
  "Copy path" action: `path` makes the launcher add its own.
- With `--json`, a failure gives a single launcher entry and exit code 0.
- Nothing personal in the code: the roots are settings (`~/.config/poste/unity.toml`).
- The walk stays cheap (it runs at each opening of the extension once the 5 min cache is over):
  depth bound, never inside a project, never `Library` or a hidden folder.
- The module never writes Unity Hub's files.
- Tests: never the real `$HOME`; a temporary home folder.
