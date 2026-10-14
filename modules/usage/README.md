# usage

Quotas des agents IA (Claude Code, Codex…) : pourcentage de chaque limite (session de 5 h, semaine…)
et heure de remise à zéro, pour les agents comme pour Kris.

- `poste usage [agent]` : un agent, `claude` par défaut.
- `poste usage --all` : tous les agents relevés.
- `poste usage --refresh` : refait le relevé avant d'afficher.
- `poste usage --json` : l'enregistrement brut du collecteur.

## D'où viennent les chiffres

Du collecteur d'Omarchy, pas de poste : `omarchy-agent-usage-update` appelle un collecteur par agent
(`omarchy-agent-usage-claude`…), qui interroge le service de l'agent avec la connexion de son outil, et
écrit `~/.local/state/omarchy/agents/usage/<agent>.json`. C'est la même source que le centre de
contrôle de l'île. `poste usage` refait le relevé (`--limits-only`) s'il date de plus de 5 minutes.

Code de sortie 1 si un agent n'a pas de relevé ou si son relevé signale une erreur (connexion
expirée : le message du collecteur dit quoi faire).

Module-script (shell, `jq`), pas de caisse Rust.
