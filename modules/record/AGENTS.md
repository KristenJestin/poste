# Règles du module record

- Ne JAMAIS arrêter, mettre en pause ni perturber un enregistrement en cours. Avant toute
  manipulation (test, installation, redémarrage de l'île ou du portail), lancer
  `poste record status` : si `"recording": true`, ne rien appliquer et attendre.
- Ne jamais lancer `poste record start` pour tester (il enregistre vraiment l'écran) ; tester la
  lecture de config, `items` et `status` seulement. Les tests automatiques n'utilisent jamais le vrai
  gpu-screen-recorder : `POSTE_RECORD_GSR`, `POSTE_RECORD_HYPRCTL` et `POSTE_RECORD_NOTIFY` (réservées
  aux tests) le remplacent par `tests/fakes/fake.sh`, avec HOME, `XDG_RUNTIME_DIR` et PATH temporaires.
  Ne jamais écrire dans le vrai `$XDG_RUNTIME_DIR/poste-record` depuis un test.
- C'est une caisse Rust (`poste-record`, binaire et bibliothèque, dans les `members` racine), comme `music`.
  `poste-record.py` et `poste-share-picker.py` sont l'ancienne version Python, gardés seulement comme
  référence des tests de parité (`tests/parity.rs`, `tests/calc.rs`) et pour revenir en arrière ; ils ne
  sont pas installés. Les supprimer une fois la version Rust validée en vrai (et les tests de parité avec).
- Toute modification de format (`status`, `watch`, `items`, `state.json`, `pick.json`) doit rester lisible
  par l'île (`island/RecordState.qml`, `island/launcher.json`) : les tests de parité comparent les sorties
  à l'octet près avec le Python, ne pas les « corriger » sans décision.
- `bin/poste-share-picker` reste un script shell minimal : c'est le chemin stable de la ligne `xdph.conf`,
  le travail est dans `poste-record share-picker`. Pas de tokio ; dépendances minimales (serde, serde_json,
  libc, regex).
- Le module ne connaît pas l'île autrement que par ses fichiers `island/` (slot `capture`,
  extension du lanceur) et la commande `omarchy-shell poste.island launcherIn record`.
- Rien du module ne vit hors de lui : raccourcis dans `hypr/record.lua`, ligne `xdph.conf` dans
  `module.toml` ; ne rien remettre dans `config/` (bindings.lua, xdph.conf, launcher.json).
- Les préréglages sont de la config utilisateur (`~/.config/poste/record/presets.json`) : ne jamais les écraser.
