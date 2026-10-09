# Règles du module display

- Ne JAMAIS appliquer un profil sur la machine de développement (le PC fixe a trois écrans en service) : pas de `poste display apply` ni `auto` sans `--dry-run`, pas de `hyprctl eval`/`keyword` de modification. Lecture seule (`status`, `list`, `items`, `--dry-run`, `hyprctl monitors`) autorisée.
- Les tests n'utilisent jamais le vrai `hyprctl` ni le vrai socket : `POSTE_DISPLAY_HYPRCTL`, `POSTE_DISPLAY_NOTIFY` (réservées aux tests), `XDG_RUNTIME_DIR` et HOME temporaires. Jamais de test de charge.
- Choix du profil (`model::choose`) et génération des commandes (`plan::commands`) sont des fonctions pures : pas d'entrée-sortie dedans.
- Chaque commande a sa forme Lua (`hyprctl eval`) et son repli historique ; garder les deux alignées.
- `display.toml` est de la config utilisateur (`~/.config/poste/display.toml`) : ne jamais l'écraser ; `display.toml.example` doit rester valide (un test le charge).
- Le démon ne doit pas boucler : `auto` compare l'empreinte des écrans, les événements sont regroupés (`debounce_ms`), le rechargement de config déclenché par notre propre application est ignoré 3 s.
- L'unité systemd est dans `systemd/`, déclarée par `units` dans `module.toml` ; `poste sys setup` l'active.
