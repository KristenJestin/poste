//! Tests QML de l'île (`tests/island/tst_*.qml`), lancés par le qmltestrunner
//! de Qt 6 en rendu hors écran : aucune fenêtre ne s'ouvre. Sautés, avec un
//! message, si Qt 6 n'a pas son qmltestrunner (paquet qt6-declarative).

use std::path::Path;
use std::process::Command;

const RUNNER: &str = "/usr/lib/qt6/bin/qmltestrunner";

#[test]
fn island_qml() {
    if !Path::new(RUNNER).exists() {
        eprintln!("tests QML de l'île sautés : {RUNNER} absent");
        return;
    }
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/island");
    let output = Command::new(RUNNER)
        .arg("-input")
        .arg(&dir)
        .env("QT_QPA_PLATFORM", "offscreen")
        .output()
        .expect("qmltestrunner");
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
