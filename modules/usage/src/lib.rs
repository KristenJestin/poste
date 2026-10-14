//! Module usage : quotas des agents IA (collecteur d'Omarchy), historique des relevés, appels relus
//! dans les journaux des agents, fenêtres de 5 h et comparaisons.

pub mod calls;
pub mod history;
pub mod quota;
pub mod stats;
pub mod time;

use anyhow::{bail, Result};
use calls::{Agent, Sources};
use poste_core::Env;
use std::io::Write;
use std::path::PathBuf;

const USAGE: &str = "\
Usage : poste usage [agent…] [--all] [--refresh] [--json]
        poste usage history [agent…] [--since <date>] [--json]
        poste usage calls [--since <date>] [--until <date>] [--agent claude|codex] [--json]
        poste usage windows [--since <date>] [--until <date>] [--agent claude|codex] [--json]
        poste usage compare <période A> <période B> [--agent claude|codex] [--json]
        poste usage record

  agent      claude (défaut), codex… : un fichier par agent dans le collecteur d'Omarchy
  --all      tous les agents relevés
  --refresh  refait le relevé avant d'afficher
  --json     sortie JSON (une ligne par élément, pour jq ; compare : un objet)
  history    l'historique des relevés (24 dernières heures par défaut)
  calls      un appel par ligne, doublons retirés (24 dernières heures par défaut)
  windows    les fenêtres de 5 h et leurs indicateurs (7 derniers jours par défaut)
  compare    les indicateurs de deux périodes côte à côte ; période : <début>..<fin>
             (fin absente : maintenant), ex. « 14 days ago..7 days ago » « 7 days ago.. »
  record     relève tous les agents et ajoute à l'historique ce qui a changé (le minuteur)

Dates : ISO 8601 ou tout ce que comprend date -d (« 3 days ago », « yesterday 14:00 »…).
";

struct Ctx {
    env: Env,
}

impl Ctx {
    fn history_file(&self) -> PathBuf {
        self.env.state_dir().join("usage/history.jsonl")
    }
    fn collector(&self) -> PathBuf {
        quota::collector_dir(&self.env.state_home)
    }
    fn sources(&self) -> Sources {
        Sources::from_env(&self.env.home)
    }
}

/// Options communes aux commandes de lecture.
#[derive(Default)]
struct Opts {
    since: Option<String>,
    until: Option<String>,
    agent: Option<String>,
    json: bool,
    all: bool,
    refresh: bool,
    rest: Vec<String>,
}

fn parse_opts(args: &[String]) -> Result<Opts> {
    let mut o = Opts::default();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = |name: &str| -> Result<String> {
            match it.next() {
                Some(v) => Ok(v.clone()),
                None => bail!("{name} demande une valeur"),
            }
        };
        match a.as_str() {
            "--since" => o.since = Some(value("--since")?),
            "--until" => o.until = Some(value("--until")?),
            "--agent" => o.agent = Some(value("--agent")?),
            "--json" => o.json = true,
            "--all" => o.all = true,
            "--refresh" => o.refresh = true,
            s if s.starts_with("--since=") => o.since = Some(s[8..].into()),
            s if s.starts_with("--until=") => o.until = Some(s[8..].into()),
            s if s.starts_with("--agent=") => o.agent = Some(s[8..].into()),
            s if s.starts_with('-') && s.len() > 1 && !s.contains("..") => {
                bail!("option inconnue {s}")
            }
            _ => o.rest.push(a.clone()),
        }
    }
    Ok(o)
}

fn agent_filter(o: &Opts) -> Result<Option<Agent>> {
    match o.agent.as_deref() {
        None => Ok(None),
        Some(a) => match Agent::parse(a) {
            Some(x) => Ok(Some(x)),
            None => bail!("agent inconnu : {a} (claude ou codex)"),
        },
    }
}

fn range(o: &Opts, default_since: &str) -> Result<(i64, i64)> {
    let since = time::parse_user(o.since.as_deref().unwrap_or(default_since))?;
    let until = match &o.until {
        Some(u) => time::parse_user(u)?,
        None => time::now() + 1,
    };
    if until <= since {
        bail!("--until doit venir après --since");
    }
    Ok((since, until))
}

/// `début..fin`, fin absente : maintenant.
fn period(s: &str) -> Result<(i64, i64)> {
    let Some((a, b)) = s.split_once("..") else {
        bail!("période illisible : {s} (attendu <début>..<fin>)");
    };
    let start = time::parse_user(a.trim())?;
    let end = if b.trim().is_empty() {
        time::now() + 1
    } else {
        time::parse_user(b.trim())?
    };
    if end <= start {
        bail!("période vide : {s}");
    }
    Ok((start, end))
}

fn print(s: &str) {
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(s.as_bytes());
}

pub fn run(argv: &[String]) -> i32 {
    let env = match Env::from_process() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("poste usage : {e:#}");
            return 1;
        }
    };
    let ctx = Ctx { env };
    let (cmd, rest) = match argv.first().map(String::as_str) {
        Some(c @ ("record" | "history" | "calls" | "windows" | "compare")) => (c, &argv[1..]),
        Some("-h" | "--help" | "help") => {
            print(USAGE);
            return 0;
        }
        _ => ("quota", argv),
    };
    let res = parse_opts(rest).and_then(|o| match cmd {
        "record" => record(&ctx),
        "history" => show_history(&ctx, &o),
        "calls" => show_calls(&ctx, &o),
        "windows" => show_windows(&ctx, &o),
        "compare" => compare(&ctx, &o),
        _ => show_quota(&ctx, &o),
    });
    match res {
        Ok(code) => code,
        Err(e) => {
            eprintln!("poste usage {cmd} : {e:#}");
            if e.to_string().starts_with("option inconnue") {
                eprint!("{USAGE}");
            }
            2
        }
    }
}

fn show_quota(ctx: &Ctx, o: &Opts) -> Result<i32> {
    let dir = ctx.collector();
    let agents = if o.all {
        quota::known_agents(&dir)
    } else if o.rest.is_empty() {
        vec!["claude".to_string()]
    } else {
        o.rest.clone()
    };
    let stale: Vec<String> = agents
        .iter()
        .filter(|a| {
            o.refresh
                || quota::age(&dir.join(format!("{a}.json"))).is_none_or(|s| s > quota::MAX_AGE)
        })
        .cloned()
        .collect();
    if !stale.is_empty() {
        quota::refresh(&ctx.env.path, &stale);
    }
    let mut code = 0;
    for a in &agents {
        let f = dir.join(format!("{a}.json"));
        let (Some(v), Some(age)) = (quota::read(&f), quota::age(&f)) else {
            eprintln!("{a} : aucun relevé (agent inconnu du collecteur d'Omarchy ?)");
            code = 1;
            continue;
        };
        if o.json {
            print(&format!("{v}\n"));
            continue;
        }
        let (text, ok) = quota::render(&v, age);
        print(&text);
        if !ok {
            code = 1;
        }
    }
    Ok(code)
}

fn record(ctx: &Ctx) -> Result<i32> {
    let dir = ctx.collector();
    quota::refresh(&ctx.env.path, &[]);
    let now = time::now();
    let snaps: Vec<history::Entry> = quota::known_agents(&dir)
        .iter()
        .filter_map(|a| history::snapshot(a, &quota::read(&dir.join(format!("{a}.json")))?, now))
        .collect();
    history::append_changed(&ctx.history_file(), &snaps)?;
    Ok(0)
}

fn show_history(ctx: &Ctx, o: &Opts) -> Result<i32> {
    let (since, until) = range(o, "24 hours ago")?;
    let file = ctx.history_file();
    if !file.exists() {
        eprintln!(
            "poste usage history : pas encore d'historique ({})",
            file.display()
        );
        return Ok(1);
    }
    let mut names = o.rest.clone();
    names.extend(o.agent.clone());
    for e in history::load(&file) {
        let t = time::parse_iso(&e.t).unwrap_or(0);
        if t < since || t >= until || !(names.is_empty() || names.contains(&e.agent)) {
            continue;
        }
        if o.json {
            print(&format!("{}\n", serde_json::to_string(&e)?));
        } else {
            print(&format!("{}\n", history::render(&e)));
        }
    }
    Ok(0)
}

fn show_calls(ctx: &Ctx, o: &Opts) -> Result<i32> {
    let (since, until) = range(o, "24 hours ago")?;
    let calls = calls::load(&ctx.sources(), agent_filter(o)?, since, until);
    let mut buf = String::new();
    for c in &calls {
        if o.json {
            buf.push_str(&c.to_json().to_string());
        } else {
            let t = &c.tokens;
            buf.push_str(&format!(
                "{}  {:<6} {:<8} {:<20} {:<7} {:<4} {:<20} entrée {} cache lu {} écrit {} sortie {} (raisonnement {})",
                time::local(c.t, "%d/%m %H:%M:%S"),
                c.agent,
                c.session.get(..8).unwrap_or(&c.session),
                c.model,
                c.effort.as_deref().unwrap_or("?"),
                if c.subagent { "sous" } else { "" },
                c.project,
                stats::human(t.input),
                stats::human(t.cache_read),
                stats::human(t.cache_write),
                stats::human(t.output),
                stats::human(t.reasoning),
            ));
        }
        buf.push('\n');
    }
    print(&buf);
    Ok(0)
}

/// Fenêtres qui croisent [since, until), avec leurs indicateurs.
fn window_stats(ctx: &Ctx, agent: Option<Agent>, since: i64, until: i64) -> Vec<stats::Stats> {
    let entries = history::load(&ctx.history_file());
    // Une fenêtre commencée avant `since` compte entière, et une fenêtre estimée part du premier
    // appel après un temps calme : appels lus un jour plus tôt, pour que le découpage ne dépende
    // pas de `--since` (sauf activité continue de plus d'un jour).
    let calls = calls::load(
        &ctx.sources(),
        agent,
        since - 24 * 3600,
        until + stats::FIVE_HOURS,
    );
    let ws = stats::windows(&entries, &calls, agent.map(Agent::name), since, until);
    ws.iter().map(|w| stats::stats(w, &calls)).collect()
}

fn show_windows(ctx: &Ctx, o: &Opts) -> Result<i32> {
    let (since, until) = range(o, "7 days ago")?;
    let all = window_stats(ctx, agent_filter(o)?, since, until);
    for st in &all {
        if o.json {
            print(&format!("{}\n", st.to_json()));
        } else {
            print(&format!("{}\n", stats::render(st)));
        }
    }
    Ok(0)
}

fn compare(ctx: &Ctx, o: &Opts) -> Result<i32> {
    let [a, b] = o.rest.as_slice() else {
        bail!("compare demande deux périodes, ex. « 14 days ago..7 days ago » « 7 days ago.. »");
    };
    let (pa, pb) = (period(a)?, period(b)?);
    let all = window_stats(ctx, agent_filter(o)?, pa.0.min(pb.0), pa.1.max(pb.1));
    let pick = |p: (i64, i64)| -> stats::Summary {
        let v: Vec<stats::Stats> = all
            .iter()
            .filter(|s| s.window.start >= p.0 && s.window.start < p.1)
            .cloned()
            .collect();
        stats::summarize(&v)
    };
    let (sa, sb) = (pick(pa), pick(pb));
    if o.json {
        let side = |p: (i64, i64), s: &stats::Summary| {
            let mut v = s.to_json();
            v["since"] = time::iso(p.0).into();
            v["until"] = time::iso(p.1).into();
            v
        };
        print(&format!(
            "{}\n",
            serde_json::json!({"a": side(pa, &sa), "b": side(pb, &sb)})
        ));
    } else {
        let label = |p: (i64, i64)| format!("{} → {}", time::short(p.0), time::short(p.1));
        print(&stats::render_compare((&label(pa), &sa), (&label(pb), &sb)));
    }
    Ok(0)
}
