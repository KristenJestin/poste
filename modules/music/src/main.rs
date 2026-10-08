//! `poste-music` : client Navidrome (API Subsonic/OpenSubsonic), copie locale
//! de la bibliothèque, recherche, et lecteur de fond (démon libmpv + MPRIS).

mod api;
mod cover;
mod ctl;
mod daemon;
mod db;
#[cfg(test)]
mod fake;
mod mpris;
mod notify;
mod proto;
mod resolve;
mod rules;
mod search;
mod state;
mod store;
mod sync;
mod track;

use anyhow::{bail, Result};
use clap::{Parser, Subcommand, ValueEnum};
use poste_core::time::rfc3339;
use poste_core::Env;
use rusqlite::Connection;
use state::Failure;
use std::io::{BufRead, Write};
use std::process::ExitCode;

#[derive(Parser)]
#[command(
    name = "poste music",
    bin_name = "poste music",
    version,
    about = "Client Navidrome : connexion, copie locale, recherche"
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

/// Ce qu'on peut lire.
#[derive(Clone, Copy, ValueEnum)]
enum KindArg {
    Playlist,
    Album,
    Artist,
    Track,
}

impl From<KindArg> for search::Kind {
    fn from(k: KindArg) -> Self {
        match k {
            KindArg::Playlist => search::Kind::Playlist,
            KindArg::Album => search::Kind::Album,
            KindArg::Artist => search::Kind::Artist,
            KindArg::Track => search::Kind::Track,
        }
    }
}

#[derive(Subcommand)]
enum Cmd {
    /// Se connecte à un serveur Navidrome (le mot de passe n'est pas conservé)
    Login,
    /// Supprime le secret du trousseau
    Logout,
    /// Serveur, utilisateur, état de la connexion, copie locale
    Status,
    /// Met à jour la copie locale de la bibliothèque
    Sync {
        /// Copie complète, sans comparer le dernier scan du serveur
        #[arg(long)]
        full: bool,
    },
    /// Cherche dans la copie locale
    Search {
        #[arg(required = true, num_args = 1..)]
        text: Vec<String>,
        /// Sortie JSON au format du lanceur de l'île
        #[arg(long)]
        json: bool,
    },
    /// Liste les playlists
    Playlists {
        #[arg(long)]
        json: bool,
    },
    /// Lance la lecture d'une playlist, d'un album, d'un artiste ou d'un morceau
    Play {
        kind: KindArg,
        id: String,
        /// Mélange la file
        #[arg(long)]
        shuffle: bool,
    },
    /// Ajoute à la fin de la file
    Queue { kind: KindArg, id: String },
    /// Lecture ou pause
    Toggle,
    /// Met en pause
    Pause,
    /// Reprend la lecture
    Resume,
    /// Morceau suivant
    Next,
    /// Morceau précédent (ou début du morceau en cours)
    Prev,
    /// Arrête la lecture et vide la file
    Stop,
    /// Morceau en cours, position, qualité, état
    Now {
        /// Sortie JSON pour l'île
        #[arg(long)]
        json: bool,
    },
    /// Démon de lecture (lancé par `play` et `queue`)
    #[command(hide = true)]
    Daemon,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("poste music : {e:#}");
            ExitCode::from(1)
        }
    }
}

fn run(cli: Cli) -> Result<ExitCode> {
    let env = Env::from_process()?;
    match cli.command {
        Cmd::Login => login(&env),
        Cmd::Logout => logout(&env),
        Cmd::Status => status(&env),
        Cmd::Sync { full } => Ok(done(sync_cmd(&env, full))),
        Cmd::Search { text, json } => {
            let text = text.join(" ");
            Ok(emit(search_cmd(&env, &text), json))
        }
        Cmd::Playlists { json } => Ok(emit(playlists_cmd(&env), json)),
        Cmd::Play { kind, id, shuffle } => {
            Ok(ctl::play_or_queue(&env, kind.into(), &id, shuffle, false))
        }
        Cmd::Queue { kind, id } => Ok(ctl::play_or_queue(&env, kind.into(), &id, false, true)),
        Cmd::Toggle => Ok(ctl::control(proto::Request::Toggle)),
        Cmd::Pause => Ok(ctl::control(proto::Request::Pause)),
        Cmd::Resume => Ok(ctl::control(proto::Request::Resume)),
        Cmd::Next => Ok(ctl::control(proto::Request::Next)),
        Cmd::Prev => Ok(ctl::control(proto::Request::Prev)),
        Cmd::Stop => Ok(ctl::control(proto::Request::Stop)),
        Cmd::Now { json } => Ok(ctl::now(json)),
        Cmd::Daemon => {
            daemon::run(&env)?;
            Ok(ExitCode::SUCCESS)
        }
    }
}

/// Affiche un échec en sortie lisible : message sur stderr, code non nul.
fn fail(f: &Failure) -> ExitCode {
    eprintln!("{f}");
    ExitCode::from(f.exit_code())
}

fn done(r: Result<(), Failure>) -> ExitCode {
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(f) => fail(&f),
    }
}

/// Résultat de `search` ou `playlists` : JSON (échec = une entrée, code 0) ou texte.
fn emit(r: Result<Vec<search::Hit>, Failure>, json: bool) -> ExitCode {
    match (r, json) {
        (Ok(hits), true) => {
            println!("{}", search::to_json(&hits));
            ExitCode::SUCCESS
        }
        (Ok(hits), false) => {
            print!("{}", search::render(&hits));
            ExitCode::SUCCESS
        }
        (Err(f), true) => {
            println!("{}", serde_json::Value::Array(vec![f.launcher_entry()]));
            ExitCode::SUCCESS
        }
        (Err(f), false) => fail(&f),
    }
}

/// Copie locale prête à interroger : garde-fou rapide, sinon première synchronisation.
fn library(env: &Env) -> Result<Connection, Failure> {
    state::quick_check(env)?;
    if let Some(conn) = db::open_existing(env)? {
        return Ok(conn);
    }
    eprintln!("Première synchronisation de la bibliothèque…");
    let r = state::open_client(env).and_then(|c| sync::run(env, &c, true));
    state::record(env, &r);
    r?;
    db::open_existing(env)?.ok_or_else(|| {
        Failure::Internal(anyhow::anyhow!(
            "copie locale introuvable après synchronisation"
        ))
    })
}

fn search_cmd(env: &Env, text: &str) -> Result<Vec<search::Hit>, Failure> {
    let conn = library(env)?;
    Ok(search::search(&conn, text)?)
}

fn playlists_cmd(env: &Env) -> Result<Vec<search::Hit>, Failure> {
    let conn = library(env)?;
    Ok(search::playlists(&conn)?)
}

fn sync_cmd(env: &Env, full: bool) -> Result<(), Failure> {
    let r = state::open_client(env).and_then(|c| sync::run(env, &c, full));
    state::record(env, &r);
    let report = r?;
    let what = match report.kind {
        sync::Kind::Full => "Copie complète",
        sync::Kind::Light => "Copie à jour (playlists, favoris, écoutes)",
    };
    let c = report.counts;
    println!(
        "{what} : {} morceaux, {} albums, {} artistes, {} playlists",
        c.tracks, c.albums, c.artists, c.playlists
    );
    Ok(())
}

fn ask(label: &str, default: Option<&str>) -> Result<String> {
    let mut err = std::io::stderr();
    match default {
        Some(d) => write!(err, "{label} [{d}] : ")?,
        None => write!(err, "{label} : ")?,
    }
    err.flush()?;
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line)?;
    let line = line.trim();
    Ok(if line.is_empty() {
        default.unwrap_or("").to_string()
    } else {
        line.to_string()
    })
}

fn login(env: &Env) -> Result<ExitCode> {
    let settings = store::Settings::load(env)?;
    let url = api::normalize_url(&ask("Adresse du serveur", settings.url.as_deref())?);
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        bail!("l'adresse doit commencer par http:// ou https://");
    }
    let user = ask("Utilisateur", settings.user.as_deref())?;
    if user.is_empty() {
        bail!("utilisateur vide");
    }
    // Le mot de passe ne vit que le temps du calcul du jeton.
    let creds = {
        let password = rpassword::prompt_password("Mot de passe : ")?;
        api::Creds::from_password(&user, &password)?
    };
    let client = api::Client::new(&url, creds.clone());
    match client.ping() {
        Ok(()) => {
            store::save_secret(&url, &creds)?;
            store::Settings {
                url: Some(url.clone()),
                user: Some(user.clone()),
            }
            .save(env)?;
            store::clear_auth_refused(env);
            println!("Connecté à {url} en tant que {user}");
            Ok(ExitCode::SUCCESS)
        }
        Err(e) => {
            let f = state::from_api(e, &url);
            match &f {
                Failure::AuthRefused(_) => {
                    eprintln!("Utilisateur ou mot de passe refusé par {url}")
                }
                other => eprintln!("{other}"),
            }
            Ok(ExitCode::from(f.exit_code()))
        }
    }
}

fn logout(env: &Env) -> Result<ExitCode> {
    let n = store::delete_secrets()?;
    // Sans utilisateur, la configuration vaut « pas connecté » ; l'adresse reste proposée au prochain login.
    let mut settings = store::Settings::load(env)?;
    settings.user = None;
    settings.save(env)?;
    store::clear_auth_refused(env);
    if n == 0 {
        println!("Déjà déconnecté (aucun secret dans le trousseau)");
    } else {
        println!("Déconnecté : secret supprimé du trousseau");
    }
    Ok(ExitCode::SUCCESS)
}

fn megabytes(bytes: u64) -> String {
    format!("{:.1} Mo", bytes as f64 / 1_048_576.0)
}

fn status(env: &Env) -> Result<ExitCode> {
    let settings = store::Settings::load(env)?;
    println!(
        "Serveur     : {}",
        settings.url.as_deref().unwrap_or("(non défini)")
    );
    println!(
        "Utilisateur : {}",
        settings.user.as_deref().unwrap_or("(non défini)")
    );

    let state = match state::open_client(env) {
        Ok(client) => {
            let r = client.ping().map_err(|e| state::from_api(e, client.url()));
            state::record(env, &r);
            r
        }
        Err(f) => Err(f),
    };
    match &state {
        Ok(()) => println!("Connexion   : connecté"),
        Err(f) => println!("Connexion   : {} ({f})", f.state()),
    }

    match db::open_existing(env)? {
        Some(conn) => {
            let c = db::counts(&conn)?;
            let size = std::fs::metadata(db::db_path(env))
                .map(|m| m.len())
                .unwrap_or(0);
            let when = db::get_meta(&conn, "synced_at")
                .and_then(|s| s.parse::<u64>().ok())
                .map(rfc3339)
                .unwrap_or_else(|| "date inconnue".into());
            println!(
                "Copie       : {} morceaux, {} albums, {} artistes, {} playlists ({}), synchronisée le {when}",
                c.tracks, c.albums, c.artists, c.playlists, megabytes(size)
            );
        }
        None => println!("Copie       : aucune (lance `poste music sync`)"),
    }
    Ok(match state {
        Ok(()) => ExitCode::SUCCESS,
        Err(f) => ExitCode::from(f.exit_code()),
    })
}
