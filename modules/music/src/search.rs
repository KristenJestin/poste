//! Recherche dans la copie locale (FTS5) et mise en forme des résultats.

use anyhow::Result;
use rusqlite::{params, Connection};
use serde_json::{json, Value};

pub const LIMIT_PLAYLISTS: usize = 5;
pub const LIMIT_ARTISTS: usize = 5;
pub const LIMIT_ALBUMS: usize = 8;
pub const LIMIT_TRACKS: usize = 12;

/// Icônes Nerd Font des actions secondaires.
const ICON_PLAY: &str = "\u{f040a}";
const ICON_SHUFFLE: &str = "\u{f049f}";
const ICON_QUEUE: &str = "\u{f0415}";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Playlist,
    Artist,
    Album,
    Track,
}

impl Kind {
    pub fn word(self) -> &'static str {
        match self {
            Kind::Playlist => "playlist",
            Kind::Artist => "artist",
            Kind::Album => "album",
            Kind::Track => "track",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Kind::Playlist => "\u{f0cb8}",
            Kind::Artist => "\u{f0803}",
            Kind::Album => "\u{f0025}",
            Kind::Track => "\u{f0388}",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    pub kind: Kind,
    pub id: String,
    pub name: String,
    pub artist: String,
    pub album: String,
    pub year: i64,
    /// Albums d'un artiste, ou morceaux d'une playlist.
    pub count: i64,
    pub duration: i64,
    pub cover: Option<String>,
}

/// Requête FTS5 à préfixes : « daf pun » donne `"daf"* "pun"*`.
/// `None` si le texte ne contient aucun mot.
pub fn fts_query(input: &str) -> Option<String> {
    let words: Vec<String> = input
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| format!("\"{w}\"*"))
        .collect();
    (!words.is_empty()).then(|| words.join(" "))
}

fn hits(
    conn: &Connection,
    sql: &str,
    q: &str,
    limit: usize,
    map: impl Fn(&rusqlite::Row) -> rusqlite::Result<Hit>,
) -> Result<Vec<Hit>> {
    let mut st = conn.prepare_cached(sql)?;
    let rows = st.query_map(params![q, limit as i64], |r| map(r))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Résultats dans l'ordre : playlists, artistes, albums, morceaux.
pub fn search(conn: &Connection, input: &str) -> Result<Vec<Hit>> {
    let Some(q) = fts_query(input) else {
        return Ok(Vec::new());
    };
    let blank = |kind, id: String, name: String| Hit {
        kind,
        id,
        name,
        artist: String::new(),
        album: String::new(),
        year: 0,
        count: 0,
        duration: 0,
        cover: None,
    };
    let mut out = hits(
        conn,
        "SELECT p.id, p.name, p.songCount, p.duration, p.coverArt
         FROM fts_playlists f JOIN playlists p ON p.id = f.id
         WHERE fts_playlists MATCH ?1 ORDER BY f.rank LIMIT ?2",
        &q,
        LIMIT_PLAYLISTS,
        |r| {
            Ok(Hit {
                count: r.get(2)?,
                duration: r.get(3)?,
                cover: r.get(4)?,
                ..blank(Kind::Playlist, r.get(0)?, r.get(1)?)
            })
        },
    )?;
    out.extend(hits(
        conn,
        "SELECT a.id, a.name, a.albumCount, a.coverArt
         FROM fts_artists f JOIN artists a ON a.id = f.id
         WHERE fts_artists MATCH ?1 ORDER BY f.rank LIMIT ?2",
        &q,
        LIMIT_ARTISTS,
        |r| {
            Ok(Hit {
                count: r.get(2)?,
                cover: r.get(3)?,
                ..blank(Kind::Artist, r.get(0)?, r.get(1)?)
            })
        },
    )?);
    out.extend(hits(
        conn,
        "SELECT a.id, a.name, a.artist, a.year, a.coverArt
         FROM fts_albums f JOIN albums a ON a.id = f.id
         WHERE fts_albums MATCH ?1 ORDER BY f.rank LIMIT ?2",
        &q,
        LIMIT_ALBUMS,
        |r| {
            Ok(Hit {
                artist: r.get(2)?,
                year: r.get(3)?,
                cover: r.get(4)?,
                ..blank(Kind::Album, r.get(0)?, r.get(1)?)
            })
        },
    )?);
    out.extend(hits(
        conn,
        "SELECT t.id, t.title, t.artist, t.album, t.coverArt
         FROM fts_tracks f JOIN tracks t ON t.id = f.id
         WHERE fts_tracks MATCH ?1 ORDER BY f.rank, t.playCount DESC LIMIT ?2",
        &q,
        LIMIT_TRACKS,
        |r| {
            Ok(Hit {
                artist: r.get(2)?,
                album: r.get(3)?,
                cover: r.get(4)?,
                ..blank(Kind::Track, r.get(0)?, r.get(1)?)
            })
        },
    )?);
    Ok(out)
}

/// Toutes les playlists, par nom.
pub fn playlists(conn: &Connection) -> Result<Vec<Hit>> {
    let mut st = conn.prepare(
        "SELECT id, name, songCount, duration, coverArt FROM playlists
         ORDER BY name COLLATE NOCASE",
    )?;
    let rows = st.query_map([], |r| {
        Ok(Hit {
            kind: Kind::Playlist,
            id: r.get(0)?,
            name: r.get(1)?,
            artist: String::new(),
            album: String::new(),
            year: 0,
            count: r.get(2)?,
            duration: r.get(3)?,
            cover: r.get(4)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn plural(n: i64, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// `2 h 05` ou `45 min`.
pub fn human_duration(secs: i64) -> String {
    let mins = (secs.max(0) + 30) / 60;
    if mins >= 60 {
        format!("{} h {:02}", mins / 60, mins % 60)
    } else {
        format!("{mins} min")
    }
}

impl Hit {
    pub fn subtitle(&self) -> String {
        match self.kind {
            Kind::Playlist => format!(
                "Playlist · {} · {}",
                plural(self.count, "track", "tracks"),
                human_duration(self.duration)
            ),
            Kind::Artist => format!("Artist · {}", plural(self.count, "album", "albums")),
            Kind::Album => {
                let mut s = format!("Album · {}", self.artist);
                if self.year > 0 {
                    s.push_str(&format!(" · {}", self.year));
                }
                s
            }
            Kind::Track => format!("Track · {} — {}", self.artist, self.album),
        }
    }

    fn command(&self, verb: &str, flag: &str) -> String {
        let flag = if flag.is_empty() {
            String::new()
        } else {
            format!(" {flag}")
        };
        format!(
            "poste music {verb} {} {}{flag}",
            self.kind.word(),
            shell_word(&self.id)
        )
    }

    /// Entrée au format du lanceur de l'île.
    pub fn to_json(&self) -> Value {
        let cmd = |c: String| json!({"type": "command", "command": c});
        let act = |label: &str, icon: &str, c: String| json!({"label": label, "icon": icon, "action": cmd(c)});
        let mut actions = vec![act("Lire", ICON_PLAY, self.command("play", ""))];
        if self.kind != Kind::Track {
            actions.push(act(
                "Shuffle",
                ICON_SHUFFLE,
                self.command("play", "--shuffle"),
            ));
        }
        actions.push(act(
            "Ajouter à la file",
            ICON_QUEUE,
            self.command("queue", ""),
        ));
        let mut v = json!({
            "id": format!("{}:{}", self.kind.word(), self.id),
            "name": self.name,
            "subtitle": self.subtitle(),
            "icon": self.kind.icon(),
            "verb": "Play",
            "action": cmd(self.command("play", "")),
            "actions": actions,
        });
        if let Some(c) = &self.cover {
            v["_cover"] = json!(c);
        }
        v
    }
}

/// Protège un mot pour le shell s'il contient autre chose que des caractères sûrs.
pub fn shell_word(s: &str) -> String {
    if !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "_.:-".contains(c))
    {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

pub fn to_json(hits: &[Hit]) -> Value {
    Value::Array(hits.iter().map(Hit::to_json).collect())
}

/// Sortie lisible.
pub fn render(hits: &[Hit]) -> String {
    if hits.is_empty() {
        return "Aucun résultat\n".to_string();
    }
    let mut out = String::new();
    for h in hits {
        out.push_str(&format!("{}\n    {}\n", h.name, h.subtitle()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use serde_json::json;

    fn sample() -> (tempfile::TempDir, Connection) {
        let d = tempfile::tempdir().unwrap();
        let mut conn = db::create(&d.path().join("l.db")).unwrap();
        let tx = conn.transaction().unwrap();
        db::insert_artist(
            &tx,
            &json!({"id":"ar1","name":"Daft Punk","albumCount":4,"coverArt":"ar-ar1"}),
        )
        .unwrap();
        db::insert_artist(&tx, &json!({"id":"ar2","name":"Beyoncé","albumCount":1})).unwrap();
        db::insert_album(&tx, &json!({"id":"al1","name":"Discovery","artist":"Daft Punk","artistId":"ar1","year":2001,"coverArt":"al-al1"})).unwrap();
        db::insert_track(&tx, &json!({"id":"t1","title":"One More Time","artist":"Daft Punk","album":"Discovery","albumId":"al1","coverArt":"mf-t1","genres":[{"name":"Electro"}],"playCount":3})).unwrap();
        db::insert_track(
            &tx,
            &json!({"id":"t2","title":"Halo","artist":"Beyoncé","album":"I Am... Sasha Fierce"}),
        )
        .unwrap();
        db::replace_playlists(
            &tx,
            &[json!({"id":"p1","name":"Daft Mix","songCount":42,"duration":7500})],
        )
        .unwrap();
        tx.commit().unwrap();
        (d, conn)
    }

    #[test]
    fn requete_fts() {
        assert_eq!(fts_query("daf pun").as_deref(), Some("\"daf\"* \"pun\"*"));
        assert_eq!(fts_query("  \"x\" -- ").as_deref(), Some("\"x\"*"));
        assert_eq!(fts_query(" ... "), None);
    }

    #[test]
    fn prefixes_casse_accents() {
        let (_d, conn) = sample();
        let r = search(&conn, "daf pun").unwrap();
        assert!(r
            .iter()
            .any(|h| h.kind == Kind::Artist && h.name == "Daft Punk"));
        assert!(r
            .iter()
            .any(|h| h.kind == Kind::Track && h.name == "One More Time"));
        let r = search(&conn, "BEYONCE").unwrap();
        assert!(r.iter().any(|h| h.name == "Beyoncé"));
        let r = search(&conn, "beyoncé hal").unwrap();
        assert!(r.iter().any(|h| h.name == "Halo"));
        assert!(search(&conn, "zzz").unwrap().is_empty());
        assert!(search(&conn, "").unwrap().is_empty());
    }

    #[test]
    fn ordre_des_types() {
        let (_d, conn) = sample();
        let kinds: Vec<_> = search(&conn, "daft")
            .unwrap()
            .iter()
            .map(|h| h.kind)
            .collect();
        assert_eq!(
            kinds,
            [Kind::Playlist, Kind::Artist, Kind::Album, Kind::Track]
        );
    }

    #[test]
    fn json_d_une_entree() {
        let (_d, conn) = sample();
        let r = search(&conn, "daft").unwrap();
        let v = to_json(&r);
        let pl = &v[0];
        assert_eq!(pl["id"], "playlist:p1");
        assert_eq!(pl["name"], "Daft Mix");
        assert_eq!(pl["subtitle"], "Playlist · 42 tracks · 2 h 05");
        assert_eq!(pl["verb"], "Play");
        assert_eq!(pl["action"]["type"], "command");
        assert_eq!(pl["action"]["command"], "poste music play playlist p1");
        assert!(pl.get("_cover").is_none());
        let acts = pl["actions"].as_array().unwrap();
        let labels: Vec<_> = acts.iter().map(|a| a["label"].as_str().unwrap()).collect();
        assert_eq!(labels, ["Lire", "Shuffle", "Ajouter à la file"]);
        for a in acts {
            assert!(a.get("name").is_none());
            assert!(!a["icon"].as_str().unwrap().is_empty());
            assert_eq!(a["action"]["type"], "command");
        }
        assert_eq!(acts[1]["icon"], "󰒟");
        assert_eq!(
            acts[1]["action"]["command"],
            "poste music play playlist p1 --shuffle"
        );
        assert_eq!(acts[2]["icon"], "󰐕");
        assert_eq!(
            acts[2]["action"]["command"],
            "poste music queue playlist p1"
        );
        // Un morceau n'a pas de lecture aléatoire.
        let t = v[3]["actions"].as_array().unwrap();
        assert_eq!(t.len(), 2);
        assert_eq!(t[1]["label"], "Ajouter à la file");
        assert_eq!(v[1]["subtitle"], "Artist · 4 albums");
        assert_eq!(v[1]["_cover"], "ar-ar1");
        assert_eq!(v[2]["subtitle"], "Album · Daft Punk · 2001");
        assert_eq!(v[3]["subtitle"], "Track · Daft Punk — Discovery");
        assert_eq!(v[3]["action"]["command"], "poste music play track t1");
        assert_eq!(v[3]["_cover"], "mf-t1");
    }

    #[test]
    fn durees_et_mots() {
        assert_eq!(human_duration(7500), "2 h 05");
        assert_eq!(human_duration(2700), "45 min");
        assert_eq!(plural(1, "album", "albums"), "1 album");
        assert_eq!(shell_word("al-12"), "al-12");
        assert_eq!(shell_word("a b'c"), "'a b'\\''c'");
    }
}
