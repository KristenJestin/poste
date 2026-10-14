//! Fenêtres de 5 h et leurs indicateurs.
//!
//! Une fenêtre vient de l'historique des relevés : la limite de session (« Session (5-hour) »,
//! « 5h window ») donne sa remise à zéro R, la fenêtre est [R - 5 h, R). Pic de pourcentage, et
//! heure du premier relevé à 100 % (plafond atteint). Sans historique (avant qu'il existe, ou un
//! trou), la fenêtre est estimée d'après les appels : elle commence au premier appel qui ne tombe
//! dans aucune fenêtre (à l'heure pile en dessous pour Claude, dont les remises tombent à l'heure),
//! et dure 5 h ; marquée `estimated`, sans pourcentage.
//!
//! Les appels ne sont que ceux des journaux de cette machine : un quota entamé ailleurs compte dans
//! le pourcentage mais pas dans les appels.

use crate::calls::{Call, Tokens};
use crate::history::Entry;
use crate::time;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashSet};

pub const FIVE_HOURS: i64 = 5 * 3600;
/// Deux remises à zéro à moins de 2 min l'une de l'autre : la même fenêtre.
const SAME_RESET: i64 = 120;
/// Tranche de temps pour compter les sessions en parallèle.
const SLOT: i64 = 300;

#[derive(Debug, Clone, PartialEq)]
pub struct Window {
    pub agent: String,
    pub start: i64,
    pub end: i64,
    pub estimated: bool,
    pub peak_percent: Option<f64>,
    /// Heure du premier relevé à 100 %.
    pub capped_at: Option<i64>,
}

/// La limite de 5 h d'un agent, d'après son libellé.
pub fn is_session_limit(label: &str) -> bool {
    let l = label.to_lowercase();
    !l.contains("week")
        && !l.contains("month")
        && (l.contains("5-hour")
            || l.contains("5h")
            || l.contains("5 h")
            || l.starts_with("session"))
}

/// Les fenêtres connues par l'historique, triées par agent puis début.
pub fn history_windows(entries: &[Entry]) -> Vec<Window> {
    let mut out: Vec<Window> = Vec::new();
    for e in entries {
        let Some(t) = time::parse_iso(&e.t) else {
            continue;
        };
        for l in e.limits.iter().filter(|l| is_session_limit(&l.label)) {
            let Some(reset) = time::parse_iso(&l.resets_at) else {
                continue;
            };
            let i = match out
                .iter()
                .position(|w| w.agent == e.agent && (w.end - reset).abs() <= SAME_RESET)
            {
                Some(i) => i,
                None => {
                    out.push(Window {
                        agent: e.agent.clone(),
                        start: reset - FIVE_HOURS,
                        end: reset,
                        estimated: false,
                        peak_percent: None,
                        capped_at: None,
                    });
                    out.len() - 1
                }
            };
            let w = &mut out[i];
            w.peak_percent = Some(w.peak_percent.map_or(l.percent, |p| p.max(l.percent)));
            if l.percent >= 100.0 && w.capped_at.is_none_or(|c| t < c) {
                w.capped_at = Some(t);
            }
        }
    }
    out.sort_by(|a, b| a.agent.cmp(&b.agent).then(a.start.cmp(&b.start)));
    // Une remise qui glisse d'un relevé à l'autre de plus de SAME_RESET donne deux fenêtres
    // qui se recouvrent : la suivante commence à la fin de la précédente, pour qu'un appel ne
    // compte que dans une.
    for i in 1..out.len() {
        if out[i].agent == out[i - 1].agent && out[i].start < out[i - 1].end {
            out[i].start = out[i - 1].end;
        }
    }
    out
}

/// Fenêtres estimées pour les appels qu'aucune fenêtre de l'historique ne couvre.
pub fn estimate(calls: &[Call], known: &[Window]) -> Vec<Window> {
    let mut out: Vec<Window> = Vec::new();
    for c in calls {
        let mine = |w: &&Window| w.agent == c.agent;
        if known
            .iter()
            .filter(mine)
            .any(|w| c.t >= w.start && c.t < w.end)
        {
            continue;
        }
        if out
            .iter()
            .filter(mine)
            .any(|w| c.t >= w.start && c.t < w.end)
        {
            continue;
        }
        let mut start = if c.agent == "claude" {
            c.t - c.t.rem_euclid(3600)
        } else {
            c.t
        };
        // Pas de chevauchement avec une fenêtre connue ou déjà estimée.
        for w in known.iter().chain(out.iter()).filter(mine) {
            if w.start <= c.t && w.end > start {
                start = start.max(w.end);
            }
        }
        let mut end = start + FIVE_HOURS;
        for w in known.iter().filter(mine) {
            if w.start > start {
                end = end.min(w.start);
            }
        }
        out.push(Window {
            agent: c.agent.to_string(),
            start,
            end,
            estimated: true,
            peak_percent: None,
            capped_at: None,
        });
    }
    out
}

/// Toutes les fenêtres (historique et estimées) qui croisent [since, until), triées par début.
pub fn windows(
    entries: &[Entry],
    calls: &[Call],
    agent: Option<&str>,
    since: i64,
    until: i64,
) -> Vec<Window> {
    let mut known = history_windows(entries);
    known.retain(|w| agent.is_none_or(|a| a == w.agent));
    let mut all = estimate(calls, &known);
    all.extend(known);
    all.retain(|w| w.end > since && w.start < until);
    all.sort_by(|a, b| a.start.cmp(&b.start).then(a.agent.cmp(&b.agent)));
    all
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Part {
    pub calls: usize,
    pub tokens: Tokens,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stats {
    pub window: Window,
    pub calls: usize,
    pub subagent_calls: usize,
    pub sessions: usize,
    /// Le plus de sessions qui ont fait un appel dans la même tranche de 5 min.
    pub parallel_max: usize,
    pub tokens: Tokens,
    pub by_effort: BTreeMap<String, Part>,
    pub by_model: BTreeMap<String, Part>,
}

pub fn effort_name(e: &Option<String>) -> String {
    e.clone().unwrap_or_else(|| "unknown".into())
}

pub fn stats(w: &Window, calls: &[Call]) -> Stats {
    let mine: Vec<&Call> = calls
        .iter()
        .filter(|c| c.agent == w.agent && c.t >= w.start && c.t < w.end)
        .collect();
    let mut tokens = Tokens::default();
    let mut by_effort: BTreeMap<String, Part> = BTreeMap::new();
    let mut by_model: BTreeMap<String, Part> = BTreeMap::new();
    let mut sessions = HashSet::new();
    let mut slots: BTreeMap<i64, HashSet<&str>> = BTreeMap::new();
    for c in &mine {
        tokens.add(&c.tokens);
        for (map, k) in [
            (&mut by_effort, effort_name(&c.effort)),
            (&mut by_model, c.model.clone()),
        ] {
            let p = map.entry(k).or_default();
            p.calls += 1;
            p.tokens.add(&c.tokens);
        }
        sessions.insert(c.session.as_str());
        slots
            .entry(c.t.div_euclid(SLOT))
            .or_default()
            .insert(c.session.as_str());
    }
    Stats {
        window: w.clone(),
        calls: mine.len(),
        subagent_calls: mine.iter().filter(|c| c.subagent).count(),
        sessions: sessions.len(),
        parallel_max: slots.values().map(HashSet::len).max().unwrap_or(0),
        tokens,
        by_effort,
        by_model,
    }
}

fn pct(part: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        (part as f64 * 1000.0 / total as f64).round() / 10.0
    }
}

fn parts_json(map: &BTreeMap<String, Part>, calls: usize, output: u64) -> Value {
    let mut m = Map::new();
    for (k, p) in map {
        m.insert(
            k.clone(),
            json!({
                "calls": p.calls,
                "calls_share": pct(p.calls as u64, calls as u64),
                "output": p.tokens.output,
                "output_share": pct(p.tokens.output, output),
                "total": p.tokens.total(),
            }),
        );
    }
    Value::Object(m)
}

fn per_point(t: &Tokens, points: f64) -> Value {
    if points <= 0.0 {
        return Value::Null;
    }
    let d = |v: u64| (v as f64 / points).round() as u64;
    json!({
        "input": d(t.input),
        "cache_read": d(t.cache_read),
        "cache_write": d(t.cache_write),
        "output": d(t.output),
        "reasoning": d(t.reasoning),
        "total": d(t.total()),
    })
}

impl Stats {
    /// Points de quota dont les tokens peuvent être rapportés : le pic d'une fenêtre relevée.
    fn points(&self) -> f64 {
        if self.window.estimated {
            0.0
        } else {
            self.window.peak_percent.unwrap_or(0.0)
        }
    }

    pub fn to_json(&self) -> Value {
        let w = &self.window;
        json!({
            "agent": w.agent,
            "start": time::iso(w.start),
            "end": time::iso(w.end),
            "estimated": w.estimated,
            "peak_percent": w.peak_percent,
            "capped_after_minutes": w.capped_at.map(|c| (c - w.start) / 60),
            "calls": self.calls,
            "subagent_calls": self.subagent_calls,
            "sessions": self.sessions,
            "parallel_sessions_max": self.parallel_max,
            "tokens": self.tokens.to_json(),
            "tokens_per_point": per_point(&self.tokens, self.points()),
            "effort": parts_json(&self.by_effort, self.calls, self.tokens.output),
            "models": parts_json(&self.by_model, self.calls, self.tokens.output),
        })
    }
}

/// Les indicateurs de plusieurs fenêtres réunis (pour `compare`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Summary {
    pub windows: usize,
    pub estimated: usize,
    pub capped: usize,
    pub capped_after_sum: i64,
    pub calls: usize,
    pub subagent_calls: usize,
    pub parallel_max_sum: usize,
    pub tokens: Tokens,
    /// Tokens et points des seules fenêtres relevées (pour les tokens par point).
    pub measured_tokens: Tokens,
    pub points: f64,
    pub by_effort: BTreeMap<String, Part>,
    pub by_model: BTreeMap<String, Part>,
}

pub fn summarize(all: &[Stats]) -> Summary {
    let mut s = Summary::default();
    for st in all {
        s.windows += 1;
        s.estimated += usize::from(st.window.estimated);
        if let Some(c) = st.window.capped_at {
            s.capped += 1;
            s.capped_after_sum += c - st.window.start;
        }
        s.calls += st.calls;
        s.subagent_calls += st.subagent_calls;
        s.parallel_max_sum += st.parallel_max;
        s.tokens.add(&st.tokens);
        if st.points() > 0.0 {
            s.measured_tokens.add(&st.tokens);
            s.points += st.points();
        }
        for (dst, src) in [
            (&mut s.by_effort, &st.by_effort),
            (&mut s.by_model, &st.by_model),
        ] {
            for (k, p) in src {
                let d = dst.entry(k.clone()).or_default();
                d.calls += p.calls;
                d.tokens.add(&p.tokens);
            }
        }
    }
    s
}

impl Summary {
    pub fn capped_after_mean(&self) -> Option<i64> {
        (self.capped > 0).then(|| self.capped_after_sum / self.capped as i64)
    }

    fn mean(&self, v: usize) -> f64 {
        if self.windows == 0 {
            0.0
        } else {
            (v as f64 * 10.0 / self.windows as f64).round() / 10.0
        }
    }

    pub fn to_json(&self) -> Value {
        json!({
            "windows": self.windows,
            "estimated_windows": self.estimated,
            "capped_windows": self.capped,
            "capped_after_minutes_mean": self.capped_after_mean().map(|s| s / 60),
            "calls": self.calls,
            "subagent_calls": self.subagent_calls,
            "calls_per_window": self.mean(self.calls),
            "parallel_sessions_max_mean": self.mean(self.parallel_max_sum),
            "tokens": self.tokens.to_json(),
            "tokens_per_point": per_point(&self.measured_tokens, self.points),
            "effort": parts_json(&self.by_effort, self.calls, self.tokens.output),
            "models": parts_json(&self.by_model, self.calls, self.tokens.output),
        })
    }
}

// --- Texte ---

/// `950`, `12,3 k`, `4,5 M`, `1,2 G`.
pub fn human(n: u64) -> String {
    let f = n as f64;
    let (v, unit) = if f >= 1e9 {
        (f / 1e9, " G")
    } else if f >= 1e6 {
        (f / 1e6, " M")
    } else if f >= 1e3 {
        (f / 1e3, " k")
    } else {
        return n.to_string();
    };
    format!("{v:.1}{unit}").replace('.', ",")
}

/// Un nombre à virgule, à la française.
pub fn fr(v: f64) -> String {
    v.to_string().replace('.', ",")
}

pub fn tokens_line(t: &Tokens) -> String {
    format!(
        "entrée {}, cache lu {}, cache écrit {}, sortie {} (raisonnement {})",
        human(t.input),
        human(t.cache_read),
        human(t.cache_write),
        human(t.output),
        human(t.reasoning)
    )
}

/// `medium 80 %, high 20 %` (part des appels), du plus gros au plus petit.
pub fn shares_line(map: &BTreeMap<String, Part>, calls: usize) -> String {
    let mut v: Vec<(&String, &Part)> = map.iter().collect();
    v.sort_by(|a, b| b.1.calls.cmp(&a.1.calls).then(a.0.cmp(b.0)));
    v.iter()
        .map(|(k, p)| {
            let k = if k.as_str() == "unknown" {
                "inconnu"
            } else {
                k.as_str()
            };
            format!("{k} {} %", fr(pct(p.calls as u64, calls as u64)))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn render(st: &Stats) -> String {
    let w = &st.window;
    let mut head = format!(
        "{}  {} → {}",
        w.agent,
        time::short(w.start),
        time::local(w.end, "%H:%M")
    );
    if w.estimated {
        head.push_str("  (estimée, sans relevé)");
    }
    if let Some(p) = w.peak_percent {
        head.push_str(&format!("  pic {} %", fr(p)));
    }
    if let Some(c) = w.capped_at {
        head.push_str(&format!(
            "  plafond atteint après {}",
            time::duration(c - w.start)
        ));
    }
    let mut out = format!(
        "{head}\n  {} appels ({} de sous-agents), {} sessions, {} en parallèle au plus\n",
        st.calls, st.subagent_calls, st.sessions, st.parallel_max
    );
    if st.calls > 0 {
        out.push_str(&format!("  tokens : {}\n", tokens_line(&st.tokens)));
        let pts = st.points();
        if pts > 0.0 {
            out.push_str(&format!(
                "  par point de quota : sortie {}, total {}\n",
                human((st.tokens.output as f64 / pts) as u64),
                human((st.tokens.total() as f64 / pts) as u64)
            ));
        }
        out.push_str(&format!(
            "  effort : {}\n",
            shares_line(&st.by_effort, st.calls)
        ));
        out.push_str(&format!(
            "  modèles : {}\n",
            shares_line(&st.by_model, st.calls)
        ));
    }
    out
}

/// Les mêmes indicateurs côte à côte, une ligne par indicateur.
pub fn render_compare(a: (&str, &Summary), b: (&str, &Summary)) -> String {
    let rows = |s: &Summary| -> Vec<String> {
        let pp = |v: u64| {
            if s.points > 0.0 {
                human((v as f64 / s.points) as u64)
            } else {
                "-".into()
            }
        };
        vec![
            format!("{} ({} estimées)", s.windows, s.estimated),
            match s.capped_after_mean() {
                Some(d) => format!("{} fois, après {} en moyenne", s.capped, time::duration(d)),
                None if s.windows == s.estimated => "inconnu (aucun relevé)".into(),
                None => "jamais".into(),
            },
            format!("{} ({} de sous-agents)", s.calls, s.subagent_calls),
            fr(s.mean(s.calls)),
            fr(s.mean(s.parallel_max_sum)),
            human(s.tokens.input),
            human(s.tokens.cache_read),
            human(s.tokens.cache_write),
            human(s.tokens.output),
            human(s.tokens.reasoning),
            format!(
                "sortie {}, total {}",
                pp(s.measured_tokens.output),
                pp(s.measured_tokens.total())
            ),
            shares_line(&s.by_effort, s.calls),
            shares_line(&s.by_model, s.calls),
        ]
    };
    let labels = [
        "fenêtres de 5 h",
        "plafond atteint",
        "appels",
        "appels par fenêtre",
        "sessions en parallèle",
        "entrée",
        "cache lu",
        "cache écrit",
        "sortie",
        "raisonnement",
        "tokens par point",
        "effort (appels)",
        "modèles (appels)",
    ];
    let (ra, rb) = (rows(a.1), rows(b.1));
    let wa = ra
        .iter()
        .map(|s| s.chars().count())
        .max()
        .unwrap_or(0)
        .max(a.0.chars().count());
    let pad = |s: &str, w: usize| format!("{s}{}", " ".repeat(w.saturating_sub(s.chars().count())));
    let mut out = format!("{}  {}  {}\n", pad("", 22), pad(a.0, wa), b.0);
    for (i, l) in labels.iter().enumerate() {
        out.push_str(&format!("{}  {}  {}\n", pad(l, 22), pad(&ra[i], wa), rb[i]));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::Limit;

    fn entry(t: i64, agent: &str, pct: f64, reset: i64) -> Entry {
        Entry {
            t: time::iso(t),
            agent: agent.into(),
            limits: vec![
                Limit {
                    label: "Session (5-hour)".into(),
                    percent: pct,
                    resets_at: time::iso(reset),
                },
                Limit {
                    label: "Weekly (7-day)".into(),
                    percent: 50.0,
                    resets_at: time::iso(reset + 86_400),
                },
            ],
        }
    }

    fn call(t: i64, session: &str, effort: Option<&str>, out: u64) -> Call {
        Call {
            t,
            agent: "claude",
            session: session.into(),
            project: "demo".into(),
            model: "model-a".into(),
            effort: effort.map(str::to_string),
            subagent: session == "sub",
            tokens: Tokens {
                input: 10,
                cache_read: 100,
                cache_write: 20,
                output: out,
                reasoning: out / 2,
            },
            key: format!("{session}-{t}"),
        }
    }

    const H: i64 = 3600;
    const T0: i64 = 1_700_000_000 - 1_700_000_000 % 3600; // à l'heure pile

    #[test]
    fn libelles_de_session() {
        assert!(is_session_limit("Session (5-hour)"));
        assert!(is_session_limit("5h window"));
        assert!(!is_session_limit("Weekly (7-day)"));
        assert!(!is_session_limit("Fable Weekly"));
    }

    #[test]
    fn fenetre_relevee_pic_et_plafond() {
        let reset = T0 + 5 * H;
        let e = vec![
            entry(T0 + 600, "claude", 10.0, reset),
            entry(T0 + 2 * H, "claude", 100.0, reset + 40),
            entry(T0 + 3 * H, "claude", 100.0, reset),
        ];
        let w = history_windows(&e);
        assert_eq!(w.len(), 1);
        assert_eq!((w[0].start, w[0].end), (T0, reset));
        assert_eq!(w[0].peak_percent, Some(100.0));
        assert_eq!(w[0].capped_at, Some(T0 + 2 * H));
        let calls = vec![
            call(T0 + 60, "s1", Some("high"), 100),
            call(T0 + 120, "s2", Some("medium"), 300),
            call(T0 + 200, "sub", None, 100),
            call(T0 + 6 * H, "s1", Some("high"), 1),
        ];
        let st = stats(&w[0], &calls);
        assert_eq!(st.calls, 3);
        assert_eq!(st.subagent_calls, 1);
        assert_eq!(st.sessions, 3);
        assert_eq!(st.parallel_max, 3);
        assert_eq!(st.tokens.output, 500);
        let j = st.to_json();
        assert_eq!(j["capped_after_minutes"], 120);
        assert_eq!(j["effort"]["unknown"]["calls"], 1);
        assert_eq!(j["effort"]["medium"]["output_share"], 60.0);
        assert_eq!(j["tokens_per_point"]["output"], 5);
        assert_eq!(j["estimated"], false);
    }

    #[test]
    fn fenetres_estimees_avant_l_historique() {
        let reset = T0 + 20 * H;
        let e = vec![entry(T0 + 16 * H, "claude", 30.0, reset)];
        let calls = vec![
            call(T0 + 1800, "s1", None, 1),       // fenêtre estimée T0 → T0 + 5 h
            call(T0 + 4 * H, "s1", None, 1),      // même fenêtre
            call(T0 + 5 * H + 60, "s1", None, 1), // nouvelle : T0 + 5 h
            call(T0 + 13 * H, "s1", None, 1), // T0 + 13 h, coupée au début de la fenêtre relevée (15 h)
            call(T0 + 16 * H, "s1", None, 1), // couverte par l'historique
        ];
        let w = windows(&e, &calls, None, 0, i64::MAX);
        let got: Vec<(i64, i64, bool)> = w
            .iter()
            .map(|w| ((w.start - T0) / H, (w.end - T0) / H, w.estimated))
            .collect();
        assert_eq!(
            got,
            vec![(0, 5, true), (5, 10, true), (13, 15, true), (15, 20, false)]
        );
        let st = stats(&w[0], &calls);
        assert_eq!(st.calls, 2);
        assert_eq!(st.to_json()["tokens_per_point"], Value::Null);
    }

    #[test]
    fn resume_et_comparaison() {
        let reset = T0 + 5 * H;
        let e = vec![entry(T0 + 3 * H, "claude", 100.0, reset)];
        let calls = vec![
            call(T0 + 60, "s1", Some("high"), 100),
            call(T0 + 6 * H, "s1", Some("low"), 50),
        ];
        let w = windows(&e, &calls, None, 0, i64::MAX);
        let all: Vec<Stats> = w.iter().map(|w| stats(w, &calls)).collect();
        let s = summarize(&all);
        assert_eq!((s.windows, s.estimated, s.capped, s.calls), (2, 1, 1, 2));
        assert_eq!(s.capped_after_mean(), Some(3 * H));
        let j = s.to_json();
        assert_eq!(j["calls_per_window"], 1.0);
        // Seule la fenêtre relevée compte pour les tokens par point : 100 de sortie / 100 points.
        assert_eq!(j["tokens_per_point"]["output"], 1);
        let text = render_compare(("A", &s), ("B", &Summary::default()));
        assert!(text.contains("plafond atteint"));
        assert!(text.contains("inconnu (aucun relevé)"));
    }

    #[test]
    fn nombres_lisibles() {
        assert_eq!(human(950), "950");
        assert_eq!(human(12_345), "12,3 k");
        assert_eq!(human(4_500_000), "4,5 M");
    }
}
