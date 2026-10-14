# usage

Quotas des agents IA (Claude Code, Codex…) : pourcentage de chaque limite (session de 5 h, semaine…)
et heure de remise à zéro, leur historique, et la consommation relue dans les journaux des agents,
pour répondre à « pourquoi j'atteins le quota de 5 h plus vite qu'avant » sans relire les journaux à
la main.

## Commandes

- `poste usage [agent]` : quotas d'un agent, `claude` par défaut ; `--all` tous les agents relevés,
  `--refresh` refait le relevé avant d'afficher, `--json` l'enregistrement brut du collecteur.
- `poste usage history [agent…] [--since <date>] [--json]` : l'historique des relevés (24 dernières
  heures par défaut).
- `poste usage calls [--since <date>] [--until <date>] [--agent claude|codex] [--json]` : un appel par
  ligne, doublons retirés (24 dernières heures par défaut).
- `poste usage windows [--since <date>] [--until <date>] [--agent …] [--json]` : chaque fenêtre de 5 h
  et ses indicateurs (7 derniers jours par défaut).
- `poste usage compare <période A> <période B> [--agent …] [--json]` : les mêmes indicateurs côte à côte.
  Une période s'écrit `<début>..<fin>`, fin absente : maintenant
  (`poste usage compare "14 days ago..7 days ago" "7 days ago.."`).
- `poste usage record` : un relevé de tous les agents, ajouté à l'historique s'il a changé (ce que
  lance le minuteur).

Dates : ISO 8601, ou tout ce que comprend `date -d` (`"3 days ago"`, `"yesterday 14:00"`,
`2026-10-01`…). `--json` : un objet JSON par ligne (`compare` : un seul objet `{a, b}`), pour `jq`.

## Quotas : le collecteur d'Omarchy

`omarchy-agent-usage-update` appelle un collecteur par agent (`omarchy-agent-usage-claude`…), qui
interroge le service de l'agent avec la connexion de son outil, et écrit
`~/.local/state/omarchy/agents/usage/<agent>.json`. C'est la même source que le centre de contrôle
de l'île. `poste usage` refait le relevé (`--limits-only`) s'il date de plus de 5 minutes.

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

## Appels : les journaux des agents

Une ligne de `calls` : heure, agent, session, projet (nom du dossier de travail), modèle, effort,
sous-agent ou non, et les tokens séparés : `input` (entrée hors cache), `cache_read`, `cache_write`,
`output`, `reasoning` (compris dans `output`). Jamais le contenu des messages.

Claude Code (`~/.claude/projects/`, ou `$CLAUDE_CONFIG_DIR/projects/`) :

- Une ligne `assistant` par bloc de réponse : le même appel (`requestId`) revient plusieurs fois
  (près de la moitié des lignes). Il compte une fois, dans sa version la plus complète. Une session
  reprise recopie des appels de la précédente dans un autre fichier : le dédoublonnage vaut pour tous
  les fichiers.
- Sous-agents : dans `<projet>/<session>/subagents/` (et `subagents/workflows/<id>/`), lignes
  `isSidechain`. Ils sont comptés, et marqués `subagent`.
- Effort : `perTurnEffort` de la ligne, sinon `effort` ; absent (quelques messages anciens) :
  `null`, jamais deviné.
- Raisonnement : `usage.output_tokens_details.thinking_tokens`. Cache : `cache_read_input_tokens`
  (lu) et `cache_creation_input_tokens` (écrit), séparés. Les messages `<synthetic>` (fabriqués par
  l'outil, sans appel) sont ignorés.

Codex (`~/.codex/sessions/`, ou `$CODEX_HOME/sessions/`) :

- Un appel par `token_usage_record` (`response_id`). Les fichiers plus anciens n'en ont pas : un
  appel par événement `token_count` dont le total cumulé a bougé (le même événement est répété).
- Modèle et effort : le `turn_context` du tour. Sous-agent : `session_meta.source.subagent`.
- `input_tokens` de Codex compte aussi le cache lu : `input` est l'entrée hors cache.

Rapidité : tous les journaux (2,4 Go) se relisent en 2 à 3 s (5 s quand ils ne sont pas en cache
mémoire) ; un fichier qui n'a pas bougé depuis `--since` n'est pas ouvert. Pas de cache sur disque :
inutile à cette vitesse, et rien à invalider.

## Fenêtres de 5 h

- D'après l'historique : la limite de session (`Session (5-hour)`, `5h window`) donne la remise à zéro
  R ; la fenêtre est `[R - 5 h, R)`. Pic de pourcentage, et temps jusqu'au premier relevé à 100 %
  (`capped_after_minutes`).
- Sans relevé (avant l'historique, ou pendant un trou) : fenêtre estimée d'après les appels, marquée
  `estimated` : elle commence au premier appel qu'aucune fenêtre ne couvre (à l'heure pile en dessous
  pour Claude, dont les remises tombent à l'heure) et dure 5 h, sans pourcentage.
- Indicateurs : appels (dont sous-agents), sessions, sessions en parallèle (le plus de sessions qui
  ont fait un appel dans la même tranche de 5 min), part de chaque effort et de chaque modèle (en
  appels et en tokens de sortie), tokens, et tokens par point de quota (fenêtres relevées seulement :
  tokens / pic).
- `compare` réunit les fenêtres qui commencent dans chaque période : nombre, fenêtres plafonnées et
  temps moyen jusqu'au plafond, appels par fenêtre, moyenne des sessions en parallèle, parts, tokens,
  tokens par point.
- Limite : seuls les journaux de cette machine sont lus. Un quota entamé ailleurs compte dans le
  pourcentage, pas dans les appels.

## Pourquoi Rust

Le module était un script shell (`jq`). Relire 2,4 Go de journaux avec `jq` à chaque question prend
des dizaines de secondes, et le dédoublonnage, les fenêtres et les comparaisons y seraient
illisibles et sans tests. La caisse Rust (`serde_json`, `libc` pour l'heure locale, rien de lourd)
relit tout en 2 à 3 s, sans cache.
