# poste music

Client de serveur Navidrome (API Subsonic/OpenSubsonic, serveur 0.63). Étapes 2.1 et 2.3 :
connexion, gestion du jeton, copie locale de la bibliothèque, recherche en ligne de
commande, et lecteur de fond (démon libmpv, MPRIS, écoutes signalées au serveur).

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
- `poste music play <kind> <id> [--shuffle]` : `kind` = `playlist`, `album`, `artist`, `track`.
  Remplace la file et lance la lecture. Un artiste donne tous ses albums (année puis nom),
  morceaux dans l'ordre du serveur ; un morceau se joue seul. La liste vient de l'API
  (`getPlaylist`, `getAlbum`, `getArtist`, `getSong`), pas de la copie locale.
- `poste music queue <kind> <id>` : ajoute à la fin de la file (lance la lecture si la file est vide).
- `poste music toggle | pause | resume | next | prev | stop` : contrôles. Ils ne démarrent
  jamais le démon (sans démon : « Rien en lecture », code 1). `prev` recommence le morceau
  après 3 s de lecture, sinon revient au précédent. `next` sur le dernier morceau termine la file.
- `poste music now [--json]` : morceau, position, qualité, état. Sans démon : état `stopped`.
  JSON : `state` (`playing`, `paused`, `stopped`), `id`, `title`, `artist`, `album`,
  `album_artist`, `duration` et `position` (secondes), `quality`, `cover` (chemin local ou
  null), `index`, `queue_len`, `volume` (0 à 1).

`play` et `queue` démarrent le démon s'il ne tourne pas, mais seulement si la connexion est
bonne : `no-login`, `auth-refused` et `unreachable` donnent un message clair (code 1, 1 ou 2)
et une notification `notify-send -a "Musique"`, sans lancer le démon.

## Le démon

`poste-music daemon` (sous-commande cachée) est lancé détaché (nouvelle session) par `play`
ou `queue`. Un seul à la fois (verrou `$XDG_RUNTIME_DIR/poste/music.lock`). Il contient
libmpv (crate `libmpv2`, `ao` par défaut donc PipeWire), lit les flux
`<serveur>/rest/stream.view?id=…&format=raw&…` passés à mpv en mémoire (jamais écrits,
journalisés ni exposés), et garde la file en phase avec la playlist de mpv.

- Contrôle : socket Unix `$XDG_RUNTIME_DIR/poste/music.sock` (0600, dossier 0700), une
  requête JSON par ligne (`{"cmd":"next"}`), une réponse JSON par ligne
  (`{"ok":true}` ou `{"ok":false,"error":"…"}`, `now` ajoute l'objet `now`).
- MPRIS 2 : `org.mpris.MediaPlayer2.poste_music`, identité « Musique », sans DesktopEntry.
  Métadonnées : `mpris:trackid` (`/org/poste/music/track/<n>`), `mpris:length`, `xesam:title`,
  `xesam:artist`, `xesam:album`, `xesam:albumArtist`, `mpris:artUrl` (pochette locale),
  `xesam:comment` et `poste:quality` (qualité, ex. « FLAC · 24 bit / 96 kHz · 2304 kbps »,
  « MP3 · 320 kbps »). Pas de `xesam:url`. `PropertiesChanged` à chaque changement de
  morceau, d'état ou de volume ; `Seeked` sur déplacement.
- Pochettes : `getCoverArt` (512) dans `~/.cache/poste/music/covers/<id>.img`, une fois, en arrière-plan.
- Écoutes : `scrobble` `submission=false` au début de chaque morceau, `submission=true` à
  50 % de la durée ou 4 minutes réellement jouées (pauses exclues). Échec ignoré.
- Morceaux passés : un `next` avant 50 % de la durée (30 s au plus) note `(id, date)` dans la
  table `skips` de `library.db` (conservée par les synchronisations complètes), pour exclure
  ces titres des mixes pendant 7 jours. Rien n'est noté sans copie locale.
- Erreurs : un flux illisible est noté dans le journal et mpv passe au suivant ; un `ping`
  teste alors le serveur. Refus d'authentification : arrêt, marqueur `auth-refused`,
  notification « Navidrome : reconnexion requise » (un clic lance `xdg-terminal-exec poste music login`).
- Fin : file terminée, `Quit` MPRIS, ou pause/arrêt pendant 10 minutes : le démon quitte
  (service MPRIS retiré, socket supprimé). SIGTERM aussi.
- Journal : `~/.local/state/poste/music.log`, écrasé à chaque démarrage, sans jeton.
- Variables : `POSTE_MUSIC_AO` fixe la sortie audio de mpv (`null` pour les tests).
  Réservées aux tests : `POSTE_MUSIC_TEST_SECRET`, `POSTE_MUSIC_MPRIS_NAME`, `POSTE_MUSIC_IDLE_SECS`.

## Fichiers

- `~/.config/poste/music.toml` : `url` et `user` (rien de secret).
- `~/.cache/poste/music/library.db` : copie locale (SQLite, FTS5 `unicode61 remove_diacritics 2`),
  `XDG_CACHE_HOME` respecté. Recréée à chaque synchronisation complète (fichier `.tmp` puis renommage).
- `~/.cache/poste/music/covers/` : pochettes locales.
- `$XDG_RUNTIME_DIR/poste/music.sock`, `music.lock` : socket et verrou du démon.
- `~/.local/state/poste/music.log` : journal du démon.
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
et `_cover` (identifiant de pochette) quand il existe. Chaque action secondaire est
`{"label", "icon" (glyphe Nerd Font), "action": {"type":"command","command":…}}` :
« Lire », « Shuffle » (sauf pour un morceau), « Ajouter à la file ».

## Sécurité

Le jeton ne figure dans aucun message, journal ou sortie : les erreurs `ureq` (qui contiennent
l'URL) ne sont jamais affichées, seules la méthode et la nature de l'erreur le sont.
