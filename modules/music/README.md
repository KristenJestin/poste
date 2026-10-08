# poste music

Client de serveur Navidrome (API Subsonic/OpenSubsonic, serveur 0.63). Étape 2.1 :
connexion, gestion du jeton, copie locale de la bibliothèque, recherche en ligne de
commande. Pas encore de lecture audio, de MPRIS ni d'intégration à l'île ; la commande
`poste music play <kind> <id>` citée dans les résultats JSON n'existe pas encore.

## Commandes

- `poste music login` : demande l'adresse (la valeur de `music.toml` est proposée),
  l'utilisateur, puis le mot de passe sans écho. Calcule le jeton Subsonic
  `t = md5(motdepasse + sel)` (sel aléatoire de 16 caractères alphanumériques), oublie le
  mot de passe, vérifie avec `ping.view`, puis range `{"u","t","s"}` dans le trousseau
  (Secret Service, attributs `service=poste-music` et `url=<serveur>`).
- `poste music logout` : supprime le secret du trousseau et l'utilisateur de `music.toml`.
- `poste music status` : serveur, utilisateur, état de la connexion, taille et date de la copie.
- `poste music sync [--full]` : met à jour la copie locale.
- `poste music search <texte…> [--json]` : recherche dans la copie locale.
- `poste music playlists [--json]` : liste des playlists.

## Fichiers

- `~/.config/poste/music.toml` : `url` et `user` (rien de secret).
- `~/.cache/poste/music/library.db` : copie locale (SQLite, FTS5 `unicode61 remove_diacritics 2`),
  `XDG_CACHE_HOME` respecté. Recréée à chaque synchronisation complète (fichier `.tmp` puis renommage).
- `~/.cache/poste/music/auth-refused` : marqueur du dernier refus du serveur (voir plus bas).

## États de connexion

| État | Cause | Sortie lisible | Avec `--json` (`search`, `playlists`) |
| --- | --- | --- | --- |
| `no-login` | pas de secret ou de configuration | « Pas connecté : lance `poste music login` », code 1 | entrée « Se connecter à Navidrome » (`xdg-terminal-exec poste music login`), code 0 |
| `auth-refused` | `status="failed"`, code Subsonic 40 à 44 | idem (traité comme `no-login`), code 1 | idem |
| `unreachable` | réseau, délai de 8 s, HTTP non 200, JSON invalide, autre erreur Subsonic | « Serveur injoignable », code 2, sans demander de reconnexion | entrée « Navidrome injoignable » (sous-titre : l'adresse), code 0 |

Un problème local (trousseau absent, disque, base) donne le code 3 et un message ; en `--json`,
une entrée « Erreur du module music ». Jamais d'erreur muette.

`search` et `playlists` interrogent la copie locale sans réseau ni trousseau : l'état n'est
donc connu que par `music.toml` (absent ou sans utilisateur : `no-login`) et par le marqueur
`auth-refused`, posé par `status`, `sync` ou `login` quand le serveur refuse le jeton, effacé
au premier succès. Sans copie locale, ils lancent d'abord une synchronisation complète.

## Copie locale

- Sync complète : `search3` avec une requête vide, par pages de 500 (artistes, albums, morceaux),
  `getPlaylists`, `getStarred2`, le tout dans une transaction sur un fichier temporaire.
- Sync incrémentale : `getScanStatus` (`lastScan`) comparé à la valeur stockée ; inchangé :
  playlists, favoris, compteurs d'écoute (`getAlbumList2` `recent` et `frequent`, 500 ; les
  morceaux d'un album dont les compteurs ont bougé sont relus avec `getAlbum`) ; changé : sync complète.
- Recherche : préfixes FTS5 (« daf pun » trouve « Daft Punk »), sans casse ni accents. Ordre :
  playlists (5), artistes (5), albums (8), morceaux (12).

## Format JSON des résultats

Une entrée par résultat, au format du lanceur de l'île : `id` (`<kind>:<id>`), `name`,
`subtitle`, `icon`, `verb: "Play"`, `action` (`poste music play <kind> <id>`), `actions`
(lire, lecture aléatoire sauf pour un morceau, ajouter à la file) et `_cover` (identifiant
de pochette) quand il existe.

## Sécurité

Le jeton ne figure dans aucun message, journal ou sortie : les erreurs `ureq` (qui contiennent
l'URL) ne sont jamais affichées, seules la méthode et la nature de l'erreur le sont.
