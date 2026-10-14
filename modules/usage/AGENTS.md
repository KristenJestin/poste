# Règles du module usage

- Lecture seule : ne jamais écrire dans `~/.local/state/omarchy/agents/usage/` (c'est le collecteur
  d'Omarchy qui l'écrit) ni toucher aux scripts d'Omarchy, qui seraient écrasés à sa mise à jour.
- Ne jamais lire ni afficher les jetons de connexion des agents (`~/.claude/.credentials.json`…) :
  le collecteur s'en charge.
- Caisse Rust (`Cargo.toml`, listée dans les `members` du `Cargo.toml` racine) : aucune dépendance
  lourde (serde, serde_json, libc).
- L'historique (`~/.local/state/poste/usage/history.jsonl`) n'a qu'un écrivain, `poste usage
  record` (le minuteur) : une ligne seulement quand les limites d'un agent changent, et jamais rien
  d'autre que l'heure, l'agent et les limites (libellé, pourcentage, remise à zéro).
- Le minuteur est déclaré dans `units` de `module.toml` : `poste sys setup` l'active ; ne pas lancer
  `systemctl enable` à la main depuis le module.
- Journaux des agents : lecture seule, et jamais rien de leur contenu dans une sortie (ni texte des
  messages, ni jeton, ni identifiant de compte) : heures, sessions, modèles, efforts, noms de
  dossiers, nombres de tokens. Un effort absent est `null`, jamais deviné.
- Tests : journaux, collecteur et historique inventés dans un dossier temporaire (`HOME` de test,
  faux `omarchy-agent-usage-update` en tête du `PATH`), jamais les vrais. Une vérification sur les
  vrais journaux n'affiche que des agrégats, une fois de temps en temps, jamais en boucle.
