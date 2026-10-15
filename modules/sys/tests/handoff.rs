//! `poste-sys sync` passe la main au `poste-sys` de la version qu'il vient d'installer.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

const ARCHIVE: &str = "poste-x86_64-linux.tar.gz";

fn script(path: &Path, body: &str) {
    fs::write(path, format!("#!/bin/sh\n{body}")).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

/// `poste-sys sync --yes` sur une version publiée dont le `poste-sys` est `bin` (script shell, `$LOG`
/// désigne son journal) ; renvoie le succès et le journal.
fn sync_with(bin: &str) -> (bool, String) {
    let d = tempfile::tempdir().unwrap();
    let home = d.path().join("home");
    let log = d.path().join("relais.log");

    // Version publiée : son poste-sys note comment il est lancé.
    let src = d.path().join("v1");
    fs::create_dir_all(src.join("target/release")).unwrap();
    fs::write(src.join("target/release/poste"), "bin").unwrap();
    fs::write(
        src.join("VERSION"),
        "commit=1111111\ndate=2026-10-01T10:00:00Z\n",
    )
    .unwrap();
    script(
        &src.join("target/release/poste-sys"),
        &format!("LOG='{}'\n{bin}", log.display()),
    );
    let pubdir = d.path().join("pub");
    fs::create_dir_all(&pubdir).unwrap();
    let archive = pubdir.join(ARCHIVE);
    assert!(Command::new("tar")
        .arg("-czf")
        .arg(&archive)
        .arg("-C")
        .arg(&src)
        .arg(".")
        .status()
        .unwrap()
        .success());
    let out = Command::new("sha256sum").arg(&archive).output().unwrap();
    let sum = String::from_utf8(out.stdout).unwrap();
    let sum = sum.split_whitespace().next().unwrap();
    fs::write(
        pubdir.join(format!("{ARCHIVE}.sha256")),
        format!("{sum}  {ARCHIVE}\n"),
    )
    .unwrap();

    // Faux curl : copie `<pubdir>/<nom>` vers `-o` ou la sortie.
    let curl = d.path().join("fake-curl");
    script(
        &curl,
        &format!(
            "out=\nwhile [ $# -gt 1 ]; do\n  if [ \"$1\" = -o ]; then out=\"$2\"; shift; fi\n  shift\ndone\nf='{}'/$(basename \"$1\")\n[ -f \"$f\" ] || exit 22\nif [ -n \"$out\" ]; then cp \"$f\" \"$out\"; else cat \"$f\"; fi\n",
            pubdir.display()
        ),
    );

    fs::create_dir_all(home.join(".config/poste")).unwrap();
    fs::write(
        home.join(".config/poste/config.toml"),
        "source = \"release\"\n",
    )
    .unwrap();

    // Ce qui touche la session de l'utilisateur (île, Hyprland, unités) : remplacé par des
    // programmes qui notent leur appel et échouent. Aucun ne doit être appelé ici.
    let stubs = d.path().join("stubs");
    let session = d.path().join("session.log");
    fs::create_dir_all(&stubs).unwrap();
    for prog in [
        "hyprctl",
        "omarchy",
        "systemctl",
        "notify-send",
        "qs",
        "quickshell",
    ] {
        script(
            &stubs.join(prog),
            &format!("echo \"{prog} $*\" >> '{}'\nexit 1\n", session.display()),
        );
    }
    let path = format!(
        "{}:{}",
        stubs.display(),
        std::env::var("PATH").unwrap_or_default()
    );

    let st = Command::new(env!("CARGO_BIN_EXE_poste-sys"))
        .args(["sync", "--yes"])
        .env("HOME", &home)
        .env("PATH", path)
        .env("POSTE_SYS_TEST_SYSTEMCTL", stubs.join("systemctl"))
        .env_remove("XDG_DATA_HOME")
        .env_remove("XDG_CONFIG_HOME")
        .env_remove("XDG_STATE_HOME")
        .env_remove("XDG_CACHE_HOME")
        .env("POSTE_SYS_TEST_CURL", &curl)
        .env("POSTE_SYS_RELEASE_URL", "https://example.com/r")
        .status()
        .unwrap();
    // La version est installée et current la désigne, relais ou pas.
    let current = home.join(".local/share/poste/current");
    assert_eq!(
        fs::canonicalize(&current).unwrap(),
        fs::canonicalize(home.join(".local/share/poste/versions/1111111")).unwrap()
    );
    assert_eq!(fs::read_to_string(&session).unwrap_or_default(), "");
    (st.success(), fs::read_to_string(&log).unwrap_or_default())
}

#[test]
fn sync_passe_la_main_au_nouveau_poste_sys() {
    let (ok, log) = sync_with("echo \"$*\" >> \"$LOG\"\n");
    assert!(ok);
    assert_eq!(log, "finish --help\nfinish --skip-build --yes --prune\n");
}

#[test]
fn sync_garde_la_main_si_la_version_ne_connait_pas_finish() {
    // Version d'avant le relais : clap refuse `finish` (code 2) ; la suite est faite ici.
    let (ok, log) = sync_with("echo \"$*\" >> \"$LOG\"\nexit 2\n");
    assert!(ok);
    assert_eq!(log, "finish --help\n");
}
