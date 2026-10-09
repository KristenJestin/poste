# Règles du module record

- Ne JAMAIS arrêter, mettre en pause ni perturber un enregistrement en cours. Avant toute
  manipulation (test, installation, redémarrage de l'île ou du portail), lancer
  `poste record status` : si `"recording": true`, ne rien appliquer et attendre.
- Ne jamais lancer `poste record start` pour tester (il enregistre vraiment l'écran) ; tester la
  lecture de config, `items` et `status` seulement.
- C'est un script (Python), installé tel quel : pas de Cargo.toml dans ce dossier. Une réécriture
  en Rust n'est pas décidée ; si elle l'est, le dossier devient une caisse `poste-record` comme `music`.
- Le module ne connaît pas l'île autrement que par ses fichiers `island/` (slot `capture`,
  extension du lanceur) et la commande `omarchy-shell poste.island launcherIn record`.
- Rien du module ne vit hors de lui : raccourcis dans `hypr/record.lua`, ligne `xdph.conf` dans
  `module.toml` ; ne rien remettre dans `config/` (bindings.lua, xdph.conf, launcher.json).
- Les préréglages sont de la config utilisateur (`~/.config/poste/record/presets.json`) : ne jamais les écraser.
