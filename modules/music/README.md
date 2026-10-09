# poste music

Client de serveur Navidrome (API Subsonic/OpenSubsonic, serveur 0.63). Étapes 2.1 à 2.5 :
connexion, gestion du jeton, copie locale de la bibliothèque, recherche en ligne de
commande, lecteur de fond (démon libmpv, MPRIS, écoutes signalées au serveur), favoris,
playlists et mixes calculés localement.

## Commandes

- `poste music login` : demande l'adresse (la valeur de `music.toml` est proposée),
  l'utilisateur, puis le mot de passe sans écho. Calcule le jeton Subsonic
  `t = md5(motdepasse + sel)` (sel aléatoire de 16 caractères alphanumériques), oublie le
  mot de passe, vérifie avec `ping.view`, puis range `{"u","t","s"}` dans le trousseau
  (Secret Service, attributs `service=poste-music` et `url=<serveur>`). Puis lance la copie
  de la bibliothèque en arrière-plan (« Copie de la bibliothèque en cours en arrière-plan »).
- `poste music logout` : supprime le secret du trousseau et l'utilisateur de `music.toml`.
- `poste music status` : serveur, utilisateur, état de la connexion, taille, date et âge de la copie, synchronisation en cours ou non.
- `poste music sync [--full]` : met à jour la copie locale à la main. Inutile en usage normal (voir « Synchronisation automatique »).
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
- `poste music volume [<0-100> | +5 | -5]` : sans argument, affiche le volume ; sinon le règle (0 à 130 %,
  `+5` et `-5` relatifs). Le volume est mémorisé dans `~/.local/state/poste/music.toml` (écriture regroupée
  environ 1 s après le dernier changement, qu'il vienne de la commande, de MPRIS ou de l'île) et restauré au
  démarrage du démon ; 50 % par défaut. Sans démon, la commande écrit seulement la valeur mémorisée.
- `poste music now [--json]` : morceau, position, qualité, état. Sans démon : état `stopped`.
  JSON : `state` (`playing`, `paused`, `stopped`), `id`, `title`, `artist`, `album`,
  `album_artist`, `duration` et `position` (secondes), `quality`, `cover` (chemin local ou
  null), `index`, `queue_len`, `volume` (0 à 1, comme MPRIS ; 1 = 100 %, jusqu'à 1,3), `starred` (le morceau est un favori).
- `poste music like [<kind> <id>]`, `unlike [<kind> <id>]`, `like --toggle` : favoris
  (voir « Favoris et playlists »).
- `poste music playlist add|remove|create|rename|delete|pick` : écriture des playlists.
- `poste music home [--json]` : contenu du lanceur sans saisie, d'abord les entrées de `mixes`, puis les
  playlists (mêmes états de connexion que `search`).
- `poste music mix me|rediscover|track|album|artist|genre …` et `poste music mixes [--json]` :
  mixes locaux (voir « Mixes »).

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
  `xesam:comment`, `poste:starred` (booléen, le morceau est un favori) et `poste:quality` (qualité, ex. « FLAC · 24 bit / 96 kHz · 2304 kbps »,
  « MP3 · 320 kbps »). Pas de `xesam:url`. `PropertiesChanged` à chaque changement de
  morceau, d'état ou de volume ; `Seeked` sur déplacement.
- Volume : démarre au volume mémorisé (50 % la première fois). mpv applique ReplayGain par morceau
  (`replaygain=track`, `replaygain-clip=no`, `replaygain-fallback=0`) : les fichiers sans tags ne sont pas corrigés.
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

## Favoris et playlists

Toutes ces commandes écrivent d'abord sur le serveur (API Subsonic), puis mettent à jour la
copie locale sans synchronisation complète (sans copie, seul le serveur est écrit). Elles
passent par les trois états de connexion habituels (voir plus bas), avec une notification.

- `poste music like [<kind> <id>]` / `unlike [<kind> <id>]` : `kind` = `track`, `album` ou
  `artist` (`star.view` / `unstar.view` avec `id`, `albumId`, `artistId`). Sans argument, le
  morceau en cours (demandé au démon ; sans lecture : « Rien en lecture », code 1, rien n'est
  envoyé au serveur). `like --toggle` inverse l'état. Notification « ♥ Ajouté aux favoris :
  <titre> » ou « Retiré des favoris ». Le démon met à jour `starred` (`now --json`, MPRIS
  `poste:starred`) du morceau en cours ; au changement de morceau il relit l'état dans la copie.
- `poste music playlist add <playlistId> [track <id> | album <id> | current]` :
  `updatePlaylist` avec `songIdToAdd` répété (`current`, le morceau en cours, est le défaut).
- `poste music playlist remove <playlistId> <index>` : `songIndexToRemove`. Le rang part de 0
  (comme l'API) et est vérifié sur la playlist du serveur avant l'écriture.
- `poste music playlist create <nom> [current | track <id> | album <id>]`,
  `playlist rename <id> <nom…>`, `playlist delete <id>` (sans confirmation).
  `create --prompt [cible]` demande le nom au clavier (pour `xdg-terminal-exec`).
- `poste music playlist pick [--json] [current | track <id> | album <id>]` : les playlists de
  l'utilisateur (celles des autres ne sont pas modifiables), chacune avec l'action
  `poste music playlist add <pid> <cible>`, après une entrée « Nouvelle playlist… » (󰐕) qui lance
  `xdg-terminal-exec poste music playlist create --prompt <cible>`.

## Mixes

Calculés sur `library.db`, sans réseau ni radio, en une vingtaine de millisecondes pour
40 000 morceaux (chargement compris). `mix` résout les morceaux dans la copie (identifiants
de la copie, aucun appel API par morceau) et les envoie au démon comme `play` ; l'adresse de
flux reste construite par le démon.

Options de `mix` : `--queue` (ajoute à la file), `--print` (affiche la liste sans jouer),
`--json` (entrées de morceaux au format du lanceur, sans jouer), `--seed N` (résultat
reproductible), `--size N` (50 par défaut).

**Générateur commun**, à partir d'un ensemble candidat :

1. Exclus : morceaux notés 1 ou 2 étoiles, passés (table `skips`) dans les 7 derniers jours,
   et joués dans les dernières 24 h tant qu'il reste assez de candidats (au moins la taille demandée).
2. Trois paniers : A « aimés » (favori, noté 4 ou plus, ou parmi les 20 % les plus écoutés des
   morceaux écoutés de la bibliothèque), B « familiers » (déjà écoutés ou notés 3, hors A),
   C « découverte » (le reste).
3. Parts A/B/C : 40/35/25 ; 30/30/40 si C compte plus de 3 fois A + B. Un panier vide ou
   trop petit est redistribué aux autres, au prorata de leurs parts.
4. Tirage pondéré sans remise dans chaque panier (plus d'écoutes, favori, bonne note : plus
   de poids), plafond de 10 % par artiste (il cède s'il n'y a pas assez d'alternatives), puis
   un ordre qui n'approche jamais deux morceaux du même artiste à moins de 4 positions ni du
   même album à moins de 6 tant que c'est possible.
5. 50 morceaux par défaut.

- `mix me` : toute la bibliothèque.
- `mix track|album|artist <id ou nom>` : candidats = morceaux qui partagent au moins un genre
  avec la source (union des genres de ses morceaux). La source ouvre le mix (le morceau, ou
  le titre aimé, mieux noté ou le plus écouté de l'album ou de l'artiste). Depuis un artiste,
  ses morceaux pèsent 3 fois plus et son plafond est de 30 %. Moins de 20 candidats par les
  genres : complément par les morceaux du même artiste, puis par le mix personnel, jusqu'à la taille demandée.
- `mix genre <nom>` : le même mécanisme à partir d'un genre.
- `mix rediscover` : albums avec au moins une écoute mais pas joués depuis 90 jours ; 1 à 3
  morceaux par album, étalés.
- `poste music mixes --json` : entrées « Mix perso » (󰒝), « Redécouvertes » (󰄉) et « Mix <genre> »
  pour les 6 genres les plus présents parmi les morceaux aimés ou fréquents. Chaque entrée
  a l'action « Lire » et « Ajouter à la file ».

## Fichiers

- `~/.config/poste/music.toml` : `url` et `user` (rien de secret).
- `~/.cache/poste/music/library.db` : copie locale (SQLite, FTS5 `unicode61 remove_diacritics 2`),
  `XDG_CACHE_HOME` respecté. Recréée à chaque synchronisation complète (fichier `.tmp` puis renommage).
- `~/.cache/poste/music/covers/` : pochettes locales (512 px, démon) ; `covers/thumbs/` : vignettes
  de 128 px pour le lanceur ; `<id>.img.none` : l'image par défaut de Navidrome (écartée).
- `$XDG_RUNTIME_DIR/poste/music.sock`, `music.lock` : socket et verrou du démon.
- `~/.local/state/poste/music.log` : journal du démon ; `music-sync.log` : journal des synchronisations de fond.
- `~/.cache/poste/music/auth-refused` : marqueur du dernier refus du serveur (voir plus bas).

## États de connexion

| État | Cause | Sortie lisible | Avec `--json` (`search`, `playlists`, `playlist pick`, `mixes`, `home`, `mix`) |
| --- | --- | --- | --- |
| `no-login` | pas de secret ou de configuration | « Pas connecté : lance `poste music login` », code 1 | entrée « Se connecter à Navidrome » (`xdg-terminal-exec poste music login`), code 0 |
| `auth-refused` | `status="failed"`, code Subsonic 40 à 44 | idem (traité comme `no-login`), code 1 | idem |
| `unreachable` | réseau, délai de 8 s, HTTP non 200, JSON invalide, autre erreur Subsonic | « Serveur injoignable », code 2, sans demander de reconnexion | entrée « Navidrome injoignable » (sous-titre : l'adresse), code 0 |

Un problème local (trousseau absent, disque, base) donne le code 3 et un message ; en `--json`,
une entrée « Erreur du module music ». Jamais d'erreur muette.

`like`, `unlike` et `playlist add|remove|create|rename|delete` appellent le serveur : mêmes
états, mêmes codes (1, 1, 2), mêmes notifications que `play`. `mix`, `mixes` et `playlist pick`
ne lisent que la copie locale ; `mix` n'a besoin du serveur que pour jouer (hors ligne,
`--print` et `--json` fonctionnent).

`search` et `playlists` interrogent la copie locale sans réseau ni trousseau : l'état n'est
donc connu que par `music.toml` (absent ou sans utilisateur : `no-login`) et par le marqueur
`auth-refused`, posé par `status`, `sync` ou `login` quand le serveur refuse le jeton, effacé
au premier succès. Sans copie locale, ils lancent une synchronisation complète en arrière-plan et répondent
tout de suite « Copie de la bibliothèque en cours… » (voir « Synchronisation automatique »).

## Copie locale

- Sync complète : `search3` avec une requête vide, par pages de 500 (artistes, albums, morceaux),
  `getPlaylists`, `getStarred2`, le tout dans une transaction sur un fichier temporaire.
- Sync incrémentale : `getScanStatus` (`lastScan`) comparé à la valeur stockée ; inchangé :
  playlists, favoris, compteurs d'écoute (`getAlbumList2` `recent` et `frequent`, 500 ; les
  morceaux d'un album dont les compteurs ont bougé sont relus avec `getAlbum`) ; changé : sync complète.
- Recherche : préfixes FTS5 (« daf pun » trouve « Daft Punk »), sans casse ni accents. Ordre :
  playlists (5), artistes (5), albums (8), morceaux (12).

## Synchronisation automatique

L'utilisateur n'a jamais à lancer `poste music sync`. Toutes les synchronisations de fond sont
un processus détaché (`poste-music sync --background`, nouvelle session, entrées et sorties
fermées) qui relit lui-même le trousseau : aucun identifiant dans les arguments ni l'environnement.

- Après un `login` réussi : synchronisation complète de fond.
- `search`, `home`, `playlists`, `mixes`, `mix` : avec une copie de plus d'1 heure (`synced_at` de
  `meta`), la réponse vient tout de suite de la copie actuelle et une synchronisation incrémentale
  part en fond. Sans copie : une synchronisation complète part en fond (le lanceur n'attend pas)
  et la réponse est l'entrée « Copie de la bibliothèque en cours… » (󰑓, sans action) en `--json`,
  ce message sur stderr (code 4) en sortie lisible. Une synchronisation déjà en cours n'est pas relancée.
- Démon : incrémentale à son démarrage si la copie manque ou a plus de 15 minutes, puis toutes les
  30 minutes tant qu'il tourne (compteurs d'écoute, « joués récemment » des mixes).
- Une seule synchronisation à la fois (`$XDG_RUNTIME_DIR/poste/music-sync.lock`, `flock`) ; une
  demande faite pendant qu'une autre tourne est ignorée (la commande `sync` à la main le dit).
- Erreurs de fond : `~/.local/state/poste/music-sync.log` (une ligne par synchronisation, sans
  jeton, repris à zéro au-delà de 64 Ko). Un refus d'authentification pose le marqueur
  `auth-refused` : le lanceur affiche alors « Se connecter ». Pas de nouvelle synchronisation
  de fond tant que le marqueur est là.
- Réservées aux tests : `POSTE_MUSIC_STALE_SECS` (1 h), `POSTE_MUSIC_DAEMON_FRESH_SECS` (15 min),
  `POSTE_MUSIC_DAEMON_EVERY_SECS` (30 min).

## Format JSON des résultats

Une entrée par résultat, au format du lanceur de l'île : `id` (`<kind>:<id>`), `name`,
`subtitle`, `icon`, `verb: "Play"`, `action` (`poste music play <kind> <id>`), `actions`
et, quand la pochette existe, `icon` = chemin local de l'image (voir « Pochettes du lanceur »). Chaque action secondaire est
`{"label", "icon" (glyphe Nerd Font), "action": {"type":"command","command":…}}` :
« Lire », « Shuffle » (sauf pour un morceau), « Ajouter à la file », puis, hors playlists,
« Mix instantané » (󰒝, `mix <kind> <id>`) et « J'aime » (󰋑) ou « Je n'aime plus » (󰋕) selon
l'état local ; pour les morceaux et les albums, « Ajouter à une playlist… » (󰲸) dont l'action est
`{"type":"submenu","command":"poste music playlist pick --json <kind> <id>"}`. Le sous-titre d'un
morceau est « Track · Artiste · Album ».

## Pochettes du lanceur

Pour toutes les sorties `--json` (`search`, `playlists`, `home`, `mixes`, `mix`, `playlist pick`),
l'identifiant de pochette interne (`_cover`) est remplacé par `"icon": "<chemin local>"` :
d'abord la vignette `covers/thumbs/<id>.img` (128 px), à défaut la pochette du démon
(`covers/<id>.img`). Ce qui manque est téléchargé en parallèle (8 requêtes, `getCoverArt` 128)
avec un budget total de 250 ms ; au-delà, l'entrée garde son glyphe et le reste est confié à un
processus détaché (`poste-music cover-fetch`, nouvelle session, sous-commande cachée) pour la
frappe suivante. Les identifiants lui parviennent par son entrée standard et il relit le
trousseau lui-même : aucun jeton dans les arguments, l'environnement ou la sortie. Un seul
`cover-fetch` à la fois (verrou `covers/.fetch.lock`). Le trousseau n'est lu que s'il manque
une vignette. L'image par défaut de Navidrome (md5 `fcec14e686ad5159a8ed7c51936116d7`) n'est jamais
affichée : un marqueur `.none` évite de la retélécharger.

## Sécurité

Le jeton ne figure dans aucun message, journal ou sortie : les erreurs `ureq` (qui contiennent
l'URL) ne sont jamais affichées, seules la méthode et la nature de l'erreur le sont.

## Partie île du module

`modules/music/island/` est installé dans `~/.local/share/poste/modules/music/island/` ; l'île le
découvre sans connaître le module (voir `README.md` à la racine, « Ce que les modules apportent ») :

- `launcher.json` : l'extension `poste-music` du lanceur (mots-clés `play`, `music`).
- `island.json` + `PlayerExtra.qml` : slot `player-extra` de la carte du lecteur. Pour notre lecteur
  seulement (bus MPRIS `poste_music`) : qualité du flux (`poste:quality`), cœur des favoris
  (`poste:starred`, `poste music like --toggle`) et pastille de la source du son qui ouvre
  l'extension du lanceur. La mise en page (où se placent la ligne de détail et le bouton) reste dans
  `PlayerView.qml` / `ActivityCard.qml` de l'île, qui ne savent rien de la musique : ils lisent le
  contrat du slot.
