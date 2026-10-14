# usage

Quotas des agents IA (Claude Code, Codex…) : pourcentage de chaque limite (session de 5 h, semaine…)
et heure de remise à zéro, pour les agents comme pour Kris.

- `poste usage [agent]` : un agent, `claude` par défaut.
- `poste usage --all` : tous les agents relevés.
- `poste usage --refresh` : refait le relevé avant d'afficher.
- `poste usage --json` : l'enregistrement brut du collecteur.
- `poste usage history [agent…] [--since <date>] [--json]` : l'historique des relevés, 24 dernières
  heures par défaut (`--since` accepte ce que comprend `date -d` : `"3 days ago"`, `2026-10-01`…).
  `--json` : les lignes de l'historique telles quelles (une par ligne, pour `jq`).
- `poste usage record` : un relevé de tous les agents, ajouté à l'historique s'il a changé.

## D'où viennent les chiffres

Du collecteur d'Omarchy, pas de poste : `omarchy-agent-usage-update` appelle un collecteur par agent
(`omarchy-agent-usage-claude`…), qui interroge le service de l'agent avec la connexion de son outil, et
écrit `~/.local/state/omarchy/agents/usage/<agent>.json`. C'est la même source que le centre de
contrôle de l'île. `poste usage` refait le relevé (`--limits-only`) s'il date de plus de 5 minutes.

Code de sortie 1 si un agent n'a pas de relevé ou si son relevé signale une erreur (connexion
expirée : le message du collecteur dit quoi faire).

## Historique

Le collecteur d'Omarchy ne garde que le dernier relevé. Le minuteur `poste-usage-record.timer`
(posé par `poste sys setup`, toutes les 5 minutes, service `poste-usage-record.service`) lance
`poste usage record` : relevé de tous les agents (`--limits-only`), puis une ligne par agent dans
`~/.local/state/poste/usage/history.jsonl`, seulement si ses limites ont changé depuis sa ligne
précédente (le fichier reste petit) :

```json
{"t":"2026-10-14T16:20:00Z","agent":"claude","limits":[{"label":"Session (5-hour)","percent":11,"resetsAt":"2026-10-14T21:00:00Z"}]}
```

- `t` : heure du relevé (UTC) ; `percent` : 0 à 100, une décimale ; `resetsAt` : remise à zéro
  (UTC) arrondie à la minute (le serveur la donne à la microseconde près et elle bouge d'un relevé à
  l'autre : sans arrondi, chaque relevé serait un changement).
- Un agent en erreur (connexion expirée…) ou sans limite n'écrit rien.
- Rien d'autre : ni jeton, ni identifiant de compte, ni statistique locale.

Vérifier le minuteur : `systemctl --user list-timers poste-usage-record.timer`.

Module-script (shell, `jq`), pas de caisse Rust.
