# Règles pour les agents (dépôt poste)

- Documentation et commentaires en français ; code et identifiants en anglais. Pas de tiret cadratin.
- Chaque module est une application à part, avec son `README.md` et son `AGENTS.md` (ses propres règles). Ce qui est propre à un module (entrées de lanceur, QML de l'île, raccourcis Hyprland, lignes de fichiers sans inclusion) vit dans le module (`island/`, `hypr/`, `[[line]]`), jamais dans `config/` : l'île et Hyprland ne connaissent aucun module.
- `poste` (l'aiguillage) ne code aucun module en dur : il cherche `poste-<nom>`, c'est tout.
- `poste sys setup` ne doit jamais écraser ni supprimer ce qu'il n'a pas posé ; avant de remplacer un fichier posé par lui, il garde une copie dans `~/.local/state/poste/backup/`.
- Aucun secret dans le dépôt ni dans les logs : les identifiants vont dans le trousseau système (Secret Service).
- Pas de `sudo` (les étapes root sont affichées, jamais lancées).
- Tester avant de commiter : `cargo test` et `cargo clippy -- -D warnings`.
- Ne jamais arrêter un enregistrement d'écran en cours ni la lecture musicale en cours : vérifier `poste record status` avant de redémarrer l'île ou le portail.
- Ne pas modifier ce qui n'appartient pas à poste (`~/.config/*` existants, chezmoi, Hyprland, l'île Quickshell) tant que le projet ne l'a pas décidé.
- Jamais de test de charge sur la machine (boucles qui occupent le processeur, dizaines de processus en parallèle) : c'est le poste de travail réel de l'utilisateur. Pour un test instable, chercher la cause par lecture et par répétitions simples (`--test-threads=1`, une dizaine de passages au plus).
