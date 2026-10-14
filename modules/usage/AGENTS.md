# Règles du module usage

- Lecture seule : ne jamais écrire dans `~/.local/state/omarchy/agents/usage/` (c'est le collecteur
  d'Omarchy qui l'écrit) ni toucher aux scripts d'Omarchy, qui seraient écrasés à sa mise à jour.
- Ne jamais lire ni afficher les jetons de connexion des agents (`~/.claude/.credentials.json`…) :
  le collecteur s'en charge.
- Module-script : pas de `Cargo.toml`, pas dans les `members` du `Cargo.toml` racine.
- L'historique (`~/.local/state/poste/usage/history.jsonl`) n'a qu'un écrivain, `poste usage
  record` (le minuteur) : une ligne seulement quand les limites d'un agent changent, et jamais rien
  d'autre que l'heure, l'agent et les limites (libellé, pourcentage, remise à zéro).
- Le minuteur est déclaré dans `units` de `module.toml` : `poste sys setup` l'active ; ne pas lancer
  `systemctl enable` à la main depuis le module. Tester avec un `XDG_STATE_HOME` temporaire et un faux
  `omarchy-agent-usage-update` en tête du `PATH`, jamais sur le vrai historique.
