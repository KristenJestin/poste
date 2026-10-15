# Rules of the voxtype add-on

- Never run `press`, `release` nor `voxtype record` to test: that opens the mic. `status` is safe.
- voxtype stays a separate program: this add-on reimplements nothing of it. It only holds the key,
  the island source, the paste focus and the default config.
- Nothing specific to a machine in the add-on (GPU build, model, device): a machine with other
  needs keeps its voxtype config in its data repository.
- No dictated text in a file, a log nor a commit message.
- `poste-voxtype` stays a short shell script, with no dependency beyond voxtype, hyprctl and jq.
