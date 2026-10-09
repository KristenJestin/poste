//! Lancement des programmes extérieurs (hyprctl, slurp, gpu-screen-recorder, notifications,
//! xdg-open). Trois d'entre eux sont remplaçables par une variable d'environnement
//! RÉSERVÉE AUX TESTS : `POSTE_RECORD_GSR`, `POSTE_RECORD_HYPRCTL`, `POSTE_RECORD_NOTIFY`.

use std::ffi::OsString;
use std::io::Read;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub const GSR: (&str, &str) = ("POSTE_RECORD_GSR", "gpu-screen-recorder");
pub const HYPRCTL: (&str, &str) = ("POSTE_RECORD_HYPRCTL", "hyprctl");
pub const NOTIFY: (&str, &str) = ("POSTE_RECORD_NOTIFY", "omarchy-notification-send");

/// Le programme à lancer : la variable d'environnement de test si elle est définie.
pub fn prog(p: (&str, &str)) -> OsString {
    std::env::var_os(p.0)
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| p.1.into())
}

/// `subprocess.run(..., capture_output=True, text=True, timeout=t).stdout` :
/// `None` si le programme est introuvable ou dépasse le délai (il est alors tué).
pub fn capture(program: &OsString, args: &[&str], timeout: Duration) -> Option<String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut out = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = out.read_to_end(&mut buf);
        buf
    });
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                return None;
            }
        }
    }
    let buf = reader.join().ok()?;
    Some(String::from_utf8_lossy(&buf).into_owned())
}

/// Lance un programme dans sa propre session, sans attendre (`start_new_session=True`).
pub fn spawn_detached(
    program: &OsString,
    args: &[OsString],
) -> std::io::Result<std::process::Child> {
    let mut c = Command::new(program);
    c.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // SAFETY : `setsid` est sûr entre `fork` et `exec`.
    unsafe {
        c.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    c.spawn()
}

/// `notify(summary, body, exec_args, urgency)` : `omarchy-notification-send`, 5 s au plus.
/// Un échec est sans conséquence.
pub fn notify(summary: &str, body: &str, exec_args: Option<&[String]>, urgency: &str) {
    let mut c = Command::new(prog(NOTIFY));
    c.args([
        "--app-name",
        "Recorder",
        "-g",
        "\u{f044b}",
        "-u",
        urgency,
        summary,
        body,
    ]);
    if let Some(a) = exec_args {
        c.arg("--exec").args(a);
    }
    let Ok(mut child) = c
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return;
    };
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return;
            }
        }
    }
}

/// Date locale `%Y-%m-%d %H-%M-%S` (`time.strftime`).
pub fn stamp() -> String {
    // SAFETY : `localtime_r` et `strftime` écrivent dans des tampons locaux valides.
    unsafe {
        let t = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        let mut buf = [0u8; 64];
        let n = libc::strftime(
            buf.as_mut_ptr() as *mut libc::c_char,
            buf.len(),
            c"%Y-%m-%d %H-%M-%S".as_ptr(),
            &tm,
        );
        String::from_utf8_lossy(&buf[..n]).into_owned()
    }
}

/// `time.time() * 1000`.
pub fn now_ms() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}
