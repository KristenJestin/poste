# Règles du module power

- Démon léger : une passe toutes les 60 s, jamais de boucle active, aucune dépendance lourde (serde, toml, libc).
- Aucun test ne lit le vrai `/sys/class/power_supply` ni n'envoie de vraie notification : `POSTE_POWER_SUPPLY_DIR` et `POSTE_POWER_NOTIFY` (réservées aux tests) les remplacent ; l'état va dans un `XDG_RUNTIME_DIR` temporaire. Jamais de test de charge.
- La logique d'alerte est dans `engine::step` (fonction pure, testée) : ne pas y mettre d'entrée-sortie. Une alerte par passage de seuil, réarmée quand la condition disparaît.
- Les piles de périphériques (`scope` = `Device`) ne sont jamais la batterie du système.
- L'unité systemd est dans `systemd/` et déclarée par `units` dans `module.toml` : `poste sys setup` l'active ; ne pas lancer `systemctl enable` à la main depuis le code du module.
- Seuils = config utilisateur (`~/.config/poste/power.toml`) : ne jamais l'écraser.
