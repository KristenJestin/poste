//! Côté client des commandes de lecture : parle au démon par le socket, le
//! démarre au besoin pour `play` et `queue`.

use crate::api::Client;
use crate::notify;
use crate::proto::{self, Now, Request, Response};
use crate::resolve;
use crate::search::Kind;
use crate::state::{self, Failure};
use crate::track::clock;
use anyhow::{anyhow, bail, Context, Result};
use poste_core::Env;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, Instant};

/// Temps laissé au démon pour répondre après son lancement.
const START_WAIT: Duration = Duration::from_secs(3);
/// Temps laissé à une requête (une grande file se charge morceau par morceau).
const REPLY_WAIT: Duration = Duration::from_secs(30);

/// Connexion au démon s'il tourne.
fn connect() -> Option<UnixStream> {
    UnixStream::connect(proto::socket_path().ok()?).ok()
}

/// Lance le démon, détaché (nouvelle session), journal écrasé à chaque démarrage.
fn spawn_daemon(env: &Env) -> Result<std::process::Child> {
    let log = proto::log_path(env);
    if let Some(dir) = log.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let out =
        std::fs::File::create(&log).with_context(|| format!("ouverture de {}", log.display()))?;
    let err = out.try_clone()?;
    let exe = std::env::current_exe().context("chemin du programme")?;
    let mut c = Command::new(exe);
    c.arg("daemon").stdin(Stdio::null()).stdout(out).stderr(err);
    // SAFETY : `setsid` est sûr entre `fork` et `exec`.
    unsafe {
        c.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    c.spawn().context("lancement du démon")
}

/// Connexion au démon, qu'on démarre s'il ne tourne pas.
fn ensure_daemon(env: &Env) -> Result<UnixStream> {
    if let Some(s) = connect() {
        return Ok(s);
    }
    let mut child = spawn_daemon(env)?;
    let start = Instant::now();
    loop {
        if let Some(s) = connect() {
            return Ok(s);
        }
        if let Ok(Some(status)) = child.try_wait() {
            // Un autre démon a pu prendre la place pendant ce temps.
            if let Some(s) = connect() {
                return Ok(s);
            }
            bail!(
                "le démon s'est arrêté ({status}) : voir {}",
                proto::log_path(env).display()
            );
        }
        if start.elapsed() > START_WAIT {
            bail!(
                "le démon ne répond pas : voir {}",
                proto::log_path(env).display()
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn exchange(stream: &UnixStream, req: &Request) -> Result<Response> {
    proto::exchange(stream, req, REPLY_WAIT)
}

/// Échec présenté à l'utilisateur : message, notification (depuis le lanceur),
/// code de sortie.
fn report(f: &Failure) -> ExitCode {
    eprintln!("{f}");
    let summary = match f {
        Failure::NoLogin | Failure::AuthRefused(_) => "Navidrome : connexion requise",
        Failure::Unreachable { .. } => "Navidrome injoignable",
        Failure::Internal(_) => "Musique : erreur",
    };
    notify::send(summary, &f.to_string());
    ExitCode::from(f.exit_code())
}

/// `play` et `queue` : prépare la liste, démarre le démon si besoin, l'envoie.
pub fn play_or_queue(env: &Env, kind: Kind, id: &str, shuffle: bool, queue: bool) -> ExitCode {
    match try_play_or_queue(env, kind, id, shuffle, queue) {
        Ok(()) => ExitCode::SUCCESS,
        Err(f) => report(&f),
    }
}

fn try_play_or_queue(
    env: &Env,
    kind: Kind,
    id: &str,
    shuffle: bool,
    queue: bool,
) -> Result<(), Failure> {
    let running = connect();
    state::quick_check(env)?;
    let client: Client = state::open_client(env)?;
    if running.is_none() {
        // Le démon ne démarre que si le serveur est joignable et le jeton accepté.
        let r = client.ping().map_err(|e| state::from_api(e, client.url()));
        state::record(env, &r);
        r?;
    }
    let result = resolve::resolve(&client, kind, id);
    state::record(env, &result);
    let tracks = result?;
    let n = tracks.len();
    let req = if queue {
        Request::Queue { tracks }
    } else {
        Request::Play { tracks, shuffle }
    };
    let stream = match running {
        Some(s) => s,
        None => ensure_daemon(env)?,
    };
    let reply = exchange(&stream, &req)?;
    if !reply.ok {
        return Err(Failure::Internal(anyhow!(
            "{}",
            reply.error.unwrap_or_else(|| "refusé par le démon".into())
        )));
    }
    println!(
        "{} {n} morceau{}",
        if queue {
            "Ajouté à la file :"
        } else {
            "Lecture :"
        },
        if n > 1 { "x" } else { "" }
    );
    Ok(())
}

/// Contrôles simples (`toggle`, `pause`…) : ne démarrent jamais le démon.
pub fn control(req: Request) -> ExitCode {
    let Some(stream) = connect() else {
        eprintln!("Rien en lecture (le démon ne tourne pas)");
        return ExitCode::from(1);
    };
    match exchange(&stream, &req) {
        Ok(r) if r.ok => ExitCode::SUCCESS,
        Ok(r) => {
            eprintln!(
                "{}",
                r.error.unwrap_or_else(|| "refusé par le démon".into())
            );
            ExitCode::from(1)
        }
        Err(e) => {
            eprintln!("poste music : {e:#}");
            ExitCode::from(1)
        }
    }
}

/// Texte de `now`.
pub fn render_now(n: &Now) -> String {
    let state = match n.state.as_str() {
        "playing" => "En lecture",
        "paused" => "En pause",
        _ => return "Rien en lecture\n".to_string(),
    };
    let mut out = format!("{state} : {}\n", n.title);
    let who = [n.artist.as_str(), n.album.as_str()]
        .iter()
        .filter(|s| !s.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join(" · ");
    if !who.is_empty() {
        out.push_str(&format!("  {who}\n"));
    }
    let mut line = format!("  {} / {}", clock(n.position), clock(n.duration));
    if !n.quality.is_empty() {
        line.push_str(&format!(" · {}", n.quality));
    }
    out.push_str(&line);
    out.push('\n');
    out.push_str(&format!("  Morceau {} sur {}\n", n.index, n.queue_len));
    out
}

/// `now` : ne démarre pas le démon ; sans démon, état arrêté.
pub fn now(json: bool) -> ExitCode {
    let n = match connect() {
        None => Now {
            state: "stopped".into(),
            ..Now::default()
        },
        Some(stream) => match exchange(&stream, &Request::Now) {
            Ok(Response { now: Some(n), .. }) => n,
            Ok(r) => {
                eprintln!("{}", r.error.unwrap_or_else(|| "réponse inattendue".into()));
                return ExitCode::from(1);
            }
            Err(e) => {
                eprintln!("poste music : {e:#}");
                return ExitCode::from(1);
            }
        },
    };
    if json {
        match serde_json::to_string(&n) {
            Ok(s) => println!("{s}"),
            Err(e) => {
                eprintln!("poste music : {e}");
                return ExitCode::from(3);
            }
        }
    } else {
        print!("{}", render_now(&n));
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rendu_de_now() {
        assert_eq!(
            render_now(&Now {
                state: "stopped".into(),
                ..Now::default()
            }),
            "Rien en lecture\n"
        );
        let n = Now {
            state: "paused".into(),
            title: "Titre".into(),
            artist: "Artiste".into(),
            album: "Album".into(),
            duration: 225.0,
            position: 83.4,
            quality: "MP3 · 320 kbps".into(),
            index: 2,
            queue_len: 3,
            ..Now::default()
        };
        assert_eq!(
            render_now(&n),
            "En pause : Titre\n  Artiste · Album\n  1:23 / 3:45 · MP3 · 320 kbps\n  Morceau 2 sur 3\n"
        );
    }
}
