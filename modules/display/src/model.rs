//! Écrans branchés (sortie de `hyprctl monitors all -j`) et choix du profil.

use crate::config::{Config, Profile};
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Monitor {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub disabled: bool,
}

pub fn parse_monitors(json: &str) -> Result<Vec<Monitor>, serde_json::Error> {
    serde_json::from_str(json)
}

/// Un critère correspond-il à cet écran ? `connector:NOM` ou morceau de description (sans casse).
pub fn matches(pattern: &str, m: &Monitor) -> bool {
    let p = pattern.trim();
    if let Some(name) = p.strip_prefix("connector:") {
        return m.name.eq_ignore_ascii_case(name.trim());
    }
    m.description.to_lowercase().contains(&p.to_lowercase())
}

/// Écrans externes (tous sauf l'interne).
pub fn externals<'a>(cfg: &Config, mons: &'a [Monitor]) -> Vec<&'a Monitor> {
    mons.iter().filter(|m| m.name != cfg.internal).collect()
}

/// Empreinte des écrans branchés : change quand on branche ou débranche.
pub fn signature(mons: &[Monitor]) -> String {
    let mut v: Vec<String> = mons
        .iter()
        .map(|m| format!("{}|{}", m.name, m.description))
        .collect();
    v.sort();
    v.join(";")
}

/// Combien de critères du profil correspondent ; `None` s'il ne correspond pas.
fn score(p: &Profile, ext: &[&Monitor]) -> Option<usize> {
    if p.detect.is_empty() && p.detect_any.is_empty() {
        return ext.is_empty().then_some(0);
    }
    if ext.is_empty() {
        return None;
    }
    if !p.detect.iter().all(|d| ext.iter().any(|m| matches(d, m))) {
        return None;
    }
    let any_hit = p
        .detect_any
        .iter()
        .any(|d| ext.iter().any(|m| matches(d, m)));
    if !p.detect_any.is_empty() && !any_hit {
        return None;
    }
    Some(p.detect.len() + usize::from(any_hit))
}

/// Le profil qui correspond le mieux, `None` si aucun.
pub fn choose<'a>(cfg: &'a Config, mons: &[Monitor]) -> Option<&'a Profile> {
    let ext = externals(cfg, mons);
    let mut best: Option<(&Profile, usize)> = None;
    for p in &cfg.profile {
        if let Some(s) = score(p, &ext) {
            // Strictement meilleur : à égalité, le premier du fichier garde la place.
            if best.is_none_or(|(_, b)| s > b) {
                best = Some((p, s));
            }
        }
    }
    best.map(|(p, _)| p)
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// Un faux `hyprctl monitors all -j` : (nom, description).
    pub fn fake(mons: &[(&str, &str)]) -> String {
        let items: Vec<String> = mons
            .iter()
            .map(|(n, d)| {
                format!(
                    "{{\"name\":\"{n}\",\"description\":\"{d}\",\"disabled\":false,\"width\":1}}"
                )
            })
            .collect();
        format!("[{}]", items.join(","))
    }

    pub const EDP: (&str, &str) = ("eDP-1", "BOE 0x0B8E");
    pub const IIYAMA: [(&str, &str); 3] = [
        ("DP-3", "Iiyama North America PL2470H 0x33303433"),
        ("DP-1", "Iiyama North America PL2470H 0x33363338"),
        ("DP-2", "Iiyama North America PL2470H 0x31303231"),
    ];

    fn cfg() -> Config {
        Config::parse(include_str!("../display.toml.example")).unwrap()
    }

    fn pick(mons: &[(&str, &str)]) -> Option<String> {
        let ms = parse_monitors(&fake(mons)).unwrap();
        choose(&cfg(), &ms).map(|p| p.name.clone())
    }

    #[test]
    fn portable_seul_est_nomade() {
        assert_eq!(pick(&[EDP]).as_deref(), Some("nomade"));
    }

    #[test]
    fn trois_iiyama_est_maison() {
        let mut v = vec![EDP];
        v.extend(IIYAMA);
        assert_eq!(pick(&v).as_deref(), Some("maison"));
        // Deux seulement (sur DP-3 et DP-2) : pas la maison, et pas le bureau non plus.
        let v = vec![EDP, IIYAMA[0], IIYAMA[2]];
        assert_eq!(pick(&v), None);
    }

    #[test]
    fn prises_du_bureau_est_boulot_un_seul_suffit() {
        assert_eq!(
            pick(&[EDP, ("HDMI-A-1", "Dell P2422H")]).as_deref(),
            Some("boulot")
        );
        assert_eq!(pick(&[EDP, ("DP-1", "LG 27")]).as_deref(), Some("boulot"));
    }

    #[test]
    fn ecran_inconnu_aucun_profil() {
        assert_eq!(pick(&[EDP, ("HDMI-A-2", "Epson projector")]), None);
    }

    #[test]
    fn le_plus_precis_gagne_puis_le_premier() {
        let c = Config::parse(
            "[[profile]]\nname=\"a\"\ndetect_any=[\"x\"]\n[[profile]]\nname=\"b\"\ndetect=[\"x\",\"y\"]\n[[profile]]\nname=\"c\"\ndetect_any=[\"x\"]\n",
        )
        .unwrap();
        let ms = parse_monitors(&fake(&[EDP, ("DP-1", "X Y")])).unwrap();
        assert_eq!(choose(&c, &ms).unwrap().name, "b");
        let ms = parse_monitors(&fake(&[EDP, ("DP-1", "X")])).unwrap();
        assert_eq!(choose(&c, &ms).unwrap().name, "a");
    }

    #[test]
    fn description_sans_casse_et_signature() {
        let m = Monitor {
            name: "DP-1".into(),
            description: "Iiyama PL2470H".into(),
            disabled: false,
        };
        assert!(matches("pl2470h", &m) && matches("connector:dp-1", &m) && !matches("zzz", &m));
        let a = parse_monitors(&fake(&[EDP, ("DP-1", "a")])).unwrap();
        let b = parse_monitors(&fake(&[("DP-1", "a"), EDP])).unwrap();
        assert_eq!(signature(&a), signature(&b));
        assert_ne!(signature(&a), signature(&a[..1]));
    }
}
