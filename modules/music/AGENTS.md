# Règles du module music

- Le mot de passe n'existe que dans `login`, le temps de calculer le jeton : aucune structure,
  aucun fichier, aucun journal ne le garde. Le jeton et le sel ne vont que dans le trousseau
  (Secret Service) et dans l'URL de la requête, jamais dans un message, un `Display`, un `Debug`,
  un panic ni une sortie.
- Ne jamais afficher une erreur `ureq` telle quelle (elle contient l'URL) : passer par `ApiError`.
- Trois états de connexion, toujours distingués : `no-login`, `auth-refused` (codes 40 à 44, traité
  comme `no-login`), `unreachable` (ne jamais demander de reconnexion). Pas d'erreur muette.
- En `--json`, un état d'échec donne une seule entrée du lanceur et le code 0.
- `search` ne touche ni au réseau ni au trousseau quand la copie existe (objectif : moins de 30 ms).
- La copie complète s'écrit dans un fichier temporaire puis est renommée : un échec garde l'ancienne.
- Pas de tokio, pas de dépendance à libdbus (Secret Service par `zbus` pur Rust).
- Ne jamais chercher d'identifiants hors de `login` (pas de lecture de la config d'autres applications).
- Les tests parlent à un faux serveur local (`src/fake.rs`), jamais au vrai.
