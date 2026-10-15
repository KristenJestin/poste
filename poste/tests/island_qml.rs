//! QML tests of the island (`tests/island/tst_*.qml`), run by Qt 6's qmltestrunner
//! with offscreen rendering: no window opens. Skipped, with a message, if Qt 6 does
//! not have its qmltestrunner (package qt6-declarative).

use std::path::Path;
use std::process::Command;

const RUNNER: &str = "/usr/lib/qt6/bin/qmltestrunner";

#[test]
fn island_qml() {
    if !Path::new(RUNNER).exists() {
        eprintln!("island QML tests skipped: {RUNNER} missing");
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
