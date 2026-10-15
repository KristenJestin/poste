# Règles du module vocaline

- Ne jamais lancer `press`, `release`, `start`, `toggle` ni `stop` pour tester : ça ouvre le micro.
  `watch` et `status` sont sans risque. Pour voir de vrais évènements, demander une dictée ; sinon,
  rejouer un flux inventé.
- Vocaline reste un programme à part : rien d'île, de thème, de son ni de Hyprland n'entre dans son
  dépôt, et ce module ne réimplémente rien de Vocaline (il passe les commandes telles quelles).
- Le contrat du flux est `docs/events.md` du dépôt Vocaline : un type ou un champ inconnu est ignoré.
- Aucun chemin propre à une machine dans le module (ni le chemin d'une version d'essai, ni la config
  de Vocaline) : `~/.config/vocaline/` vient du dépôt de données de la machine.
- Aucun texte dicté dans un fichier, un journal ni un message de commit.
- Le module ne connaît l'île que par `island/` (slot `dictation`) ; la touche est dans
  `hypr/vocaline.lua`, jamais dans `config/`.
- `poste-vocaline` reste un script shell court, sans dépendance.
