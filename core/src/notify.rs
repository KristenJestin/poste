//! Desktop notifications through `notify-send`.

use std::process::{Command, Stdio};

/// Builds the `notify-send` command (separate from sending so it can be tested).
pub fn command(summary: &str, body: &str) -> Command {
    let mut c = Command::new("notify-send");
    c.arg("--app-name=poste").arg("--").arg(summary).arg(body);
    c
}

/// Sends a notification. Returns `false` if `notify-send` is missing or fails:
/// a missed notification must never make a module fail.
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
    fn command_arguments() {
        let c = command("Title", "Body");
        assert_eq!(c.get_program(), "notify-send");
        let args: Vec<_> = c
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args, ["--app-name=poste", "--", "Title", "Body"]);
    }
}
