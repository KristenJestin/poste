# Règles pour les agents (dépôt poste)

- Documentation et commentaires en français ; code et identifiants en anglais. Pas de tiret cadratin.
- Chaque module est une application à part, avec son `README.md` et son `AGENTS.md` (ses propres règles).
- `poste` (l'aiguillage) ne code aucun module en dur : il cherche `poste-<nom>`, c'est tout.
- `poste sys setup` ne doit jamais écraser ni supprimer ce qu'il n'a pas posé ; avant de remplacer un fichier posé par lui, il garde une copie dans `~/.local/state/poste/backup/`.
- Aucun secret dans le dépôt ni dans les logs : les identifiants vont dans le trousseau système (Secret Service).
- Pas de `sudo`.
- Tester avant de commiter : `cargo test` et `cargo clippy -- -D warnings`.
- Ne pas modifier ce qui n'appartient pas à poste (`~/.config/*` existants, chezmoi, Hyprland, l'île Quickshell) tant que le projet ne l'a pas décidé.
