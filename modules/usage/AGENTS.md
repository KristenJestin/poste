# Règles du module usage

- Lecture seule : ne jamais écrire dans `~/.local/state/omarchy/agents/usage/` (c'est le collecteur
  d'Omarchy qui l'écrit) ni toucher aux scripts d'Omarchy, qui seraient écrasés à sa mise à jour.
- Ne jamais lire ni afficher les jetons de connexion des agents (`~/.claude/.credentials.json`…) :
  le collecteur s'en charge.
- Module-script : pas de `Cargo.toml`, pas dans les `members` du `Cargo.toml` racine.
