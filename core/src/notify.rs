//! Notifications de bureau via `notify-send`.

use std::process::{Command, Stdio};

/// Construit la commande `notify-send` (séparé de l'envoi pour être testable).
pub fn command(summary: &str, body: &str) -> Command {
    let mut c = Command::new("notify-send");
    c.arg("--app-name=poste").arg("--").arg(summary).arg(body);
    c
}

/// Envoie une notification. Renvoie `false` si `notify-send` est absent ou échoue :
/// une notification manquée ne doit jamais faire échouer un module.
pub fn send(summary: &str, body: &str) -> bool {
    command(summary, body)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_de_la_commande() {
        let c = command("Titre", "Corps");
        assert_eq!(c.get_program(), "notify-send");
        let args: Vec<_> = c
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args, ["--app-name=poste", "--", "Titre", "Corps"]);
    }
}
