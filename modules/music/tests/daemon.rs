//! Test d'intégration du démon, sans son : `POSTE_MUSIC_AO=null`, faux serveur
//! Navidrome local, HOME et XDG_* temporaires, trousseau remplacé par un fichier
//! (`POSTE_MUSIC_TEST_SECRET`), nom MPRIS propre au test.
//!
//! Le service MPRIS vit sur un bus de session privé (`dbus-daemon` lancé par le test, jamais le vrai bus) ; sans `dbus-daemon`,
//! les vérifications MPRIS sont sautées avec un message.

#[allow(dead_code)]
#[path = "../src/fake.rs"]
mod fake;

use fake::Fake;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

const TOKEN: &str = "tok3f9a1c77";
const SALT: &str = "selb81d02";
static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct Sandbox {
    dir: tempfile::TempDir,
    fake: Fake,
    bus_name: String,
    bus_addr: Option<String>,
    bus: Option<std::process::Child>,
}

fn wait_until(secs: u64, what: &str, mut f: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < end {
        if f() {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("délai dépassé : {what}");
}

/// Bus de session privé du test (jamais le vrai bus de l'utilisateur, où le
/// faux lecteur apparaîtrait dans l'île). `None` si `dbus-daemon` est absent.
fn private_bus() -> Option<(std::process::Child, String)> {
    let mut child = Command::new("dbus-daemon")
        .args(["--session", "--nofork", "--print-address=1"])
        .env_remove("DBUS_SESSION_BUS_ADDRESS")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    let mut line = String::new();
    BufReader::new(child.stdout.take()?)
        .read_line(&mut line)
        .ok()?;
    let addr = line.trim().to_string();
    if addr.is_empty() {
        let _ = child.kill();
        return None;
    }
    Some((child, addr))
}

fn song(n: usize, extra: Value) -> Value {
    let mut s = fake::song(n);
    s["id"] = json!(format!("t{n}"));
    s["title"] = json!(format!("Titre {n}"));
    s["duration"] = json!(3);
    s["coverArt"] = json!("al-al1");
    for (k, v) in extra.as_object().unwrap() {
        s[k] = v.clone();
    }
    s
}

impl Sandbox {
    fn new(with_login: bool) -> Sandbox {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = tempfile::tempdir().unwrap();
        let fake = Fake::start(0, TOKEN);
        *fake.state.songs.lock().unwrap() = vec![
            song(
                1,
                json!({"suffix":"flac","bitDepth":24,"samplingRate":96000,"bitRate":2304}),
            ),
            song(2, json!({"suffix":"mp3","bitRate":320})),
            song(3, json!({"suffix":"wav","bitRate":128})),
        ];
        let (bus, bus_addr) = private_bus().unzip();
        let sb = Sandbox {
            bus_name: format!("poste_music_test_{}_{n}", std::process::id()),
            bus_addr,
            bus,
            dir,
            fake,
        };
        for d in ["home", "run"] {
            std::fs::create_dir_all(sb.dir.path().join(d)).unwrap();
        }
        if with_login {
            let conf = sb.dir.path().join("home/.config/poste");
            std::fs::create_dir_all(&conf).unwrap();
            std::fs::write(
                conf.join("music.toml"),
                format!("url = \"{}\"\nuser = \"kris\"\n", sb.fake.url),
            )
            .unwrap();
            std::fs::write(
                sb.secret_file(),
                json!({"u":"kris","t":TOKEN,"s":SALT}).to_string(),
            )
            .unwrap();
        }
        sb
    }

    fn home(&self) -> PathBuf {
        self.dir.path().join("home")
    }

    fn run_dir(&self) -> PathBuf {
        self.dir.path().join("run")
    }

    fn secret_file(&self) -> PathBuf {
        self.dir.path().join("secret.json")
    }

    fn cmd(&self, args: &[&str]) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_poste-music"));
        let h = self.home();
        c.env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .env("HOME", &h)
            .env("XDG_CONFIG_HOME", h.join(".config"))
            .env("XDG_CACHE_HOME", h.join(".cache"))
            .env("XDG_DATA_HOME", h.join(".local/share"))
            .env("XDG_STATE_HOME", h.join(".local/state"))
            .env("XDG_RUNTIME_DIR", self.run_dir())
            .env("POSTE_MUSIC_TEST_SECRET", self.secret_file())
            .env("POSTE_MUSIC_AO", "null")
            .env("POSTE_MUSIC_MPRIS_NAME", &self.bus_name)
            .args(args);
        if let Some(a) = &self.bus_addr {
            c.env("DBUS_SESSION_BUS_ADDRESS", a);
        }
        c
    }

    fn run(&self, args: &[&str]) -> Output {
        self.cmd(args).output().unwrap()
    }

    fn ok(&self, args: &[&str]) -> String {
        let o = self.run(args);
        assert!(
            o.status.success(),
            "{args:?} a échoué : {}",
            String::from_utf8_lossy(&o.stderr)
        );
        String::from_utf8_lossy(&o.stdout).to_string()
    }

    fn now(&self) -> Value {
        serde_json::from_str(&self.ok(&["now", "--json"])).unwrap()
    }

    fn socket(&self) -> PathBuf {
        self.run_dir().join("poste/music.sock")
    }

    fn log(&self) -> String {
        std::fs::read_to_string(self.home().join(".local/state/poste/music.log"))
            .unwrap_or_default()
    }

    fn db(&self) -> PathBuf {
        self.home().join(".cache/poste/music/library.db")
    }

    fn calls(&self) -> Vec<String> {
        self.fake.state.calls.lock().unwrap().clone()
    }

    fn scrobbled(&self, id: &str, submission: bool) -> bool {
        self.calls().iter().any(|c| {
            c.starts_with("scrobble?")
                && c.contains(&format!("id={id}&"))
                && c.contains(&format!("submission={submission}"))
        })
    }
}

impl Drop for Sandbox {
    /// Ne laisse jamais un démon derrière un test qui échoue.
    fn drop(&mut self) {
        if let Ok(mut s) = UnixStream::connect(self.socket()) {
            let _ = s.write_all(b"{\"cmd\":\"quit\"}\n");
            let mut l = String::new();
            let _ = BufReader::new(&s).read_line(&mut l);
        }
        if let Some(b) = &mut self.bus {
            let _ = b.kill();
            let _ = b.wait();
        }
    }
}

fn mpris(conn: &Connection, name: &str) -> Proxy<'static> {
    Proxy::new(
        conn,
        name.to_string(),
        "/org/mpris/MediaPlayer2",
        "org.mpris.MediaPlayer2.Player",
    )
    .unwrap()
}

fn metadata(p: &Proxy<'_>) -> HashMap<String, OwnedValue> {
    p.get_property("Metadata").unwrap()
}

fn strings(v: &OwnedValue) -> Vec<String> {
    v.try_clone().unwrap().try_into().unwrap()
}

fn title(p: &Proxy<'_>) -> String {
    let m = metadata(p);
    m.get("xesam:title")
        .map(|v| v.try_clone().unwrap().try_into().unwrap())
        .unwrap_or_default()
}

fn status(p: &Proxy<'_>) -> String {
    p.get_property("PlaybackStatus").unwrap()
}

#[test]
fn lecture_complete_mpris_scrobble_skip_et_fin() {
    let sb = Sandbox::new(true);
    sb.ok(&["sync", "--full"]);
    assert!(sb.db().is_file());

    // play : le démon démarre, la file est chargée.
    let out = sb.ok(&["play", "album", "al1"]);
    assert!(out.contains("3 morceaux"), "{out}");
    wait_until(5, "premier morceau", || sb.now()["state"] == "playing");

    // Fichiers du démon : droits et contenu.
    let sock = sb.socket();
    let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&sock), 0o600);
    assert_eq!(mode(sock.parent().unwrap()), 0o700);
    assert!(sock.parent().unwrap().join("music.lock").exists());

    // état en JSON, tel que l'île le lira
    let now = sb.now();
    println!("now --json : {now}");
    assert_eq!(now["title"], "Titre 1");
    assert_eq!(now["quality"], "FLAC · 24 bit / 96 kHz · 2304 kbps");
    assert_eq!(now["index"], 1);
    assert_eq!(now["queue_len"], 3);
    assert_eq!(now["duration"], 3.0);
    let text = sb.ok(&["now"]);
    assert!(text.starts_with("En lecture : Titre 1\n"), "{text}");
    assert!(text.contains("FLAC · 24 bit / 96 kHz · 2304 kbps"));

    let mut p_changes: Option<Arc<Mutex<Vec<String>>>> = None;
    let conn = sb.bus_addr.as_ref().map(|a| {
        zbus::blocking::connection::Builder::address(a.as_str())
            .unwrap()
            .build()
            .unwrap()
    });
    let proxy = conn.as_ref().map(|c| {
        let name = format!("org.mpris.MediaPlayer2.{}", sb.bus_name);
        let p = mpris(c, &name);
        let seen = Arc::new(Mutex::new(Vec::<String>::new()));
        let props = zbus::blocking::fdo::PropertiesProxy::builder(c)
            .destination(name)
            .unwrap()
            .path("/org/mpris/MediaPlayer2")
            .unwrap()
            .build()
            .unwrap();
        let sink = seen.clone();
        std::thread::spawn(move || {
            for sig in props.receive_properties_changed().unwrap() {
                let args = sig.args().unwrap();
                let mut s = sink.lock().unwrap();
                for k in args.changed_properties().keys() {
                    s.push(k.to_string());
                }
            }
        });
        p_changes = Some(seen);
        p
    });
    if proxy.is_none() {
        eprintln!("NOTE : dbus-daemon absent, vérifications MPRIS sautées");
    }

    if let Some(p) = &proxy {
        wait_until(5, "MPRIS", || title(p) == "Titre 1");
        let m = metadata(p);
        assert_eq!(status(p), "Playing");
        let id: OwnedObjectPath = m["mpris:trackid"].try_clone().unwrap().try_into().unwrap();
        assert!(id.as_str().starts_with("/org/poste/music/track/"));
        assert_eq!(
            m["mpris:length"].try_clone().unwrap().try_into(),
            Ok(3_000_000i64)
        );
        assert_eq!(strings(&m["xesam:artist"]), ["Beyoncé"]);
        assert_eq!(strings(&m["xesam:albumArtist"]), ["Beyoncé"]);
        let album: String = m["xesam:album"].try_clone().unwrap().try_into().unwrap();
        assert_eq!(album, "Discovery");
        let q = "FLAC · 24 bit / 96 kHz · 2304 kbps";
        assert_eq!(strings(&m["xesam:comment"]), [q]);
        let pq: String = m["poste:quality"].try_clone().unwrap().try_into().unwrap();
        assert_eq!(pq, q);
        assert!(!m.contains_key("xesam:url"));
        for (k, v) in &m {
            let t = format!("{k}={v:?}");
            assert!(!t.contains("stream.view") && !t.contains(TOKEN), "{t}");
        }
        // pochette locale téléchargée (en arrière-plan) puis annoncée
        wait_until(5, "pochette", || metadata(p).contains_key("mpris:artUrl"));
        let art: String = metadata(p)["mpris:artUrl"]
            .try_clone()
            .unwrap()
            .try_into()
            .unwrap();
        let file = art.strip_prefix("file://").expect("file://");
        let png = std::fs::read(file).unwrap();
        assert_eq!(&png[1..4], b"PNG");
        assert!(file.ends_with("/.cache/poste/music/covers/al-al1.img"));
        let ident: String = Proxy::new(
            conn.as_ref().unwrap(),
            format!("org.mpris.MediaPlayer2.{}", sb.bus_name),
            "/org/mpris/MediaPlayer2",
            "org.mpris.MediaPlayer2",
        )
        .unwrap()
        .get_property("Identity")
        .unwrap();
        assert_eq!(ident, "Musique");
        let vol: f64 = p.get_property("Volume").unwrap();
        assert!((vol - 1.0).abs() < 1e-9);
        p.set_property("Volume", 0.5f64).unwrap();
        wait_until(3, "volume", || {
            (p.get_property::<f64>("Volume").unwrap() - 0.5).abs() < 1e-9
        });
    }

    // next tout de suite : morceau passé
    sb.ok(&["next"]);
    wait_until(5, "titre 2", || sb.now()["title"] == "Titre 2");
    let skipped: Vec<String> = {
        let c = rusqlite::Connection::open(sb.db()).unwrap();
        let mut st = c.prepare("SELECT id FROM skips").unwrap();
        let rows = st.query_map([], |r| r.get(0)).unwrap();
        rows.map(|r| r.unwrap()).collect()
    };
    assert_eq!(skipped, ["t1"]);
    if let Some(p) = &proxy {
        wait_until(3, "MPRIS titre 2", || title(p) == "Titre 2");
        let m = metadata(p);
        let q: String = m["poste:quality"].try_clone().unwrap().try_into().unwrap();
        assert_eq!(q, "MP3 · 320 kbps");
        assert!(p_changes
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .contains(&"Metadata".to_string()));
    }

    // pause par MPRIS, reprise par la commande
    if let Some(p) = &proxy {
        p.call_method("PlayPause", &()).unwrap();
        wait_until(3, "pause", || status(p) == "Paused");
        assert_eq!(sb.now()["state"], "paused");
        sb.ok(&["resume"]);
        wait_until(3, "reprise", || status(p) == "Playing");
        assert!(p_changes
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .contains(&"PlaybackStatus".to_string()));
    } else {
        sb.ok(&["pause"]);
        wait_until(3, "pause", || sb.now()["state"] == "paused");
        sb.ok(&["resume"]);
        wait_until(3, "reprise", || sb.now()["state"] == "playing");
    }
    sb.ok(&["toggle"]);
    assert_eq!(sb.now()["state"], "paused");
    sb.ok(&["toggle"]);
    assert_eq!(sb.now()["state"], "playing");

    // t2 joue au-delà de 50 % : écoute signalée ; t3 suit
    wait_until(8, "scrobble t2", || sb.scrobbled("t2", true));
    assert!(sb.scrobbled("t1", false) && sb.scrobbled("t2", false));
    assert!(
        !sb.scrobbled("t1", true),
        "un morceau passé n'est pas signalé"
    );
    wait_until(8, "titre 3", || sb.now()["title"] == "Titre 3");
    wait_until(8, "scrobble t3", || sb.scrobbled("t3", true));
    assert!(
        sb.calls()
            .iter()
            .filter(|c| c.starts_with("stream?"))
            .count()
            >= 3
    );

    // fin de la file : le démon s'arrête, le service MPRIS et le socket disparaissent
    wait_until(15, "fin du démon", || !sb.socket().exists());
    if let Some(c) = &conn {
        let dbus = zbus::blocking::fdo::DBusProxy::new(c).unwrap();
        let name = format!("org.mpris.MediaPlayer2.{}", sb.bus_name);
        wait_until(5, "service MPRIS retiré", || {
            !dbus
                .name_has_owner(name.as_str().try_into().unwrap())
                .unwrap()
        });
    }
    assert_eq!(sb.now()["state"], "stopped");
    let out = sb.run(&["toggle"]);
    assert!(!out.status.success());

    // journal : écrasé à chaque démarrage, sans jeton
    let log = sb.log();
    assert!(
        log.contains("démarrage du démon") && log.contains("arrêt du démon"),
        "{log}"
    );
    assert!(
        !log.contains(TOKEN) && !log.contains(SALT) && !log.contains("stream.view"),
        "{log}"
    );
}

#[test]
fn stop_puis_inactivite_arrete_le_demon() {
    let sb = Sandbox::new(true);
    let mut c = sb.cmd(&["play", "track", "t2"]);
    c.env("POSTE_MUSIC_IDLE_SECS", "2");
    assert!(c.output().unwrap().status.success());
    wait_until(5, "lecture", || sb.now()["state"] == "playing");
    // file ajoutée à la suite
    sb.ok(&["queue", "track", "t3"]);
    assert_eq!(sb.now()["queue_len"], 2);
    sb.ok(&["stop"]);
    assert_eq!(sb.now()["state"], "stopped");
    assert_eq!(sb.now()["queue_len"], 0);
    wait_until(10, "arrêt après inactivité", || !sb.socket().exists());
}

#[test]
fn jeton_refuse_pendant_la_lecture() {
    let sb = Sandbox::new(true);
    sb.ok(&["play", "album", "al1"]);
    wait_until(5, "lecture", || sb.now()["state"] == "playing");
    // le serveur refuse désormais le jeton : le prochain flux échoue
    *sb.fake.state.expected_token.lock().unwrap() = "autre".into();
    wait_until(20, "arrêt sur refus", || !sb.socket().exists());
    assert!(sb.home().join(".cache/poste/music/auth-refused").exists());
    assert!(
        sb.log().contains("authentification refusée"),
        "{}",
        sb.log()
    );
}

#[test]
fn pas_de_demarrage_sans_connexion_ni_serveur() {
    // pas connecté
    let sb = Sandbox::new(false);
    let o = sb.run(&["play", "album", "al1"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stderr).contains("poste music login"));
    assert!(!sb.socket().exists());
    let o = sb.run(&["queue", "album", "al1"]);
    assert_eq!(o.status.code(), Some(1));

    // jeton refusé
    let sb = Sandbox::new(true);
    *sb.fake.state.expected_token.lock().unwrap() = "autre".into();
    let o = sb.run(&["play", "album", "al1"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(sb.home().join(".cache/poste/music/auth-refused").exists());
    assert!(!sb.socket().exists());

    // serveur injoignable
    let sb = Sandbox::new(true);
    *sb.fake.state.raw.lock().unwrap() = Some((503, "x".into()));
    let o = sb.run(&["play", "album", "al1"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("injoignable"));
    assert!(!sb.socket().exists());
    assert!(!sb.home().join(".cache/poste/music/auth-refused").exists());

    // sans démon : état arrêté, contrôles refusés
    assert_eq!(sb.now()["state"], "stopped");
    for c in ["toggle", "pause", "resume", "next", "prev", "stop"] {
        assert_eq!(sb.run(&[c]).status.code(), Some(1), "{c}");
    }
}
