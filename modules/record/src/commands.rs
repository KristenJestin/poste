//! The subcommands: start, supervisor, stop, pause, watch, open-dir, open-last, hotkey.

use crate::audio::{app_audio_for, audio_sources, slug};
use crate::env::{find_preset, load_config, out_dir_for, Env};
use crate::hypr::{clients, find_window, Obj};
use crate::proc::{capture, notify, now_ms, prog, spawn_detached, stamp, GSR};
use crate::pyval::{basename, dirname, dumps, join, py_str, truthy};
use crate::state::{public, read_state, write_state, State};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::ffi::OsString;
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Duration;

pub const USAGE: &str = include_str!("usage.txt");

/// Target of a recording.
pub enum Target {
    Window(Obj),
    Monitor(String),
    Region,
}

/// What the supervisor receives: the gpu-screen-recorder command, the file, the label.
#[derive(Serialize, Deserialize)]
pub struct Spec {
    pub cmd: Vec<String>,
    pub file: String,
    pub label: String,
}

/// The value that follows `flag` in `args`. `Err` if the flag is the last argument.
fn flag_value(args: &[String], flag: &str) -> Result<Option<String>, String> {
    match args.iter().position(|a| a == flag) {
        None => Ok(None),
        Some(i) => args
            .get(i + 1)
            .cloned()
            .map(Some)
            .ok_or_else(|| format!("{flag} needs a value")),
    }
}

/// Common gpu-screen-recorder options (after the target), in the order of the original script.
pub fn gsr_args(
    head: Vec<String>,
    opts: &Map<String, Value>,
    out: &str,
    sources: &str,
) -> Vec<String> {
    let get = |k: &str, d: Value| opts.get(k).cloned().unwrap_or(d);
    let mut cmd = head;
    let cursor = match opts.get("cursor") {
        None => true,
        Some(v) => truthy(v),
    };
    cmd.extend([
        "-f".to_string(),
        py_str(&get("fps", Value::from(60))),
        "-q".into(),
        py_str(&get("quality", Value::from("very_high"))),
        "-k".into(),
        py_str(&get("codec", Value::from("auto"))),
        "-fm".into(),
        "cfr".into(),
        "-cursor".into(),
        if cursor { "yes" } else { "no" }.into(),
        "-fallback-cpu-encoding".into(),
        "yes".into(),
        "-o".into(),
        out.to_string(),
    ]);
    if matches!(get("container", Value::from("mp4")), Value::String(ref s) if s == "mp4") {
        // Readable by editing software (Opus in MP4 is poorly supported).
        cmd.extend(["-ac".to_string(), "aac".into()]);
    }
    if let Some(r) = opts.get("resolution").filter(|r| truthy(r)) {
        cmd.extend(["-s".to_string(), py_str(r)]);
    }
    if !sources.is_empty() {
        cmd.extend(["-a".to_string(), sources.to_string()]);
    }
    cmd
}

/// `"app" in str(audio).split("+")`.
fn wants_app(audio: &Value) -> bool {
    py_str(audio).split('+').any(|w| w == "app")
}

/// Picks the region with `slurp -d`: `WxH+X+Y`. `None` if cancelled.
fn pick_region() -> Option<String> {
    let geo = capture(
        &prog(("POSTE_RECORD_SLURP", "slurp")),
        &["-d"],
        Duration::from_secs(120),
    )
    .unwrap_or_default();
    let re = regex::Regex::new(r"^(-?\d+),(-?\d+) (\d+)x(\d+)").expect("fixed expression");
    let c = re.captures(geo.trim())?;
    Some(format!("{}x{}+{}+{}", &c[3], &c[4], &c[1], &c[2]))
}

pub fn start(env: &Env, args: &[String]) -> i32 {
    if read_state(env).is_some() {
        notify(
            "Already recording",
            "Stop the current recording first.",
            None,
            "normal",
        );
        return 1;
    }
    match start_inner(env, args) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("poste-record: {e}");
            1
        }
    }
}

fn start_inner(env: &Env, args: &[String]) -> Result<i32, String> {
    let (d, presets) = load_config(env);
    let mut opts = d;
    let (target, label): (Target, String);
    if let Some(pid) = flag_value(args, "--preset")? {
        let Some(preset) = find_preset(&presets, &pid) else {
            notify(
                "Recording",
                &format!("Unknown preset: {pid}"),
                None,
                "critical",
            );
            return Ok(1);
        };
        for k in [
            "dir",
            "fps",
            "quality",
            "codec",
            "container",
            "resolution",
            "audioAppName",
            "cursor",
        ] {
            if let Some(v) = preset.get(k) {
                opts.insert(k.into(), v.clone());
            }
        }
        if let Some(a) = preset.get("audioSource").filter(|a| truthy(a)) {
            opts.insert("audio".into(), a.clone());
        }
        label = preset
            .get("name")
            .map(py_str)
            .unwrap_or_else(|| pid.clone());
        if let Some(w) = preset.get("window").filter(|w| truthy(w)) {
            match find_window(w, &clients()) {
                Some(win) => target = Target::Window(win),
                None => {
                    notify(
                        "Nothing to record",
                        &format!("{label} has no open window."),
                        None,
                        "critical",
                    );
                    return Ok(1);
                }
            }
        } else if let Some(m) = preset.get("monitor").filter(|m| truthy(m)) {
            target = Target::Monitor(py_str(m));
        } else {
            target = Target::Region;
        }
    } else if let Some(addr) = flag_value(args, "--window")? {
        let win = clients()
            .into_iter()
            .find(|c| matches!(c.get("address"), Some(Value::String(a)) if *a == addr));
        let Some(win) = win else {
            notify(
                "Nothing to record",
                "That window is gone.",
                None,
                "critical",
            );
            return Ok(1);
        };
        label = match win.get("class") {
            Some(c) if truthy(c) => py_str(c),
            _ => "Window".to_string(),
        };
        target = Target::Window(win);
    } else if let Some(name) = flag_value(args, "--monitor")? {
        label = format!("Screen {name}");
        target = Target::Monitor(name);
    } else if args.iter().any(|a| a == "--region") {
        target = Target::Region;
        label = "Region".into();
    } else {
        println!("{USAGE}");
        return Ok(2);
    }
    if let Some(a) = flag_value(args, "--audio")? {
        opts.insert("audio".into(), Value::from(a));
    }
    if !opts.contains_key("audio") {
        let def = if matches!(target, Target::Window(_)) {
            "app"
        } else {
            "desktop"
        };
        opts.insert("audio".into(), Value::from(def));
    }

    let mut audio_app: Option<String> = None;
    if let Target::Window(win) = &target {
        let want_app = wants_app(&opts["audio"]);
        if want_app {
            audio_app = app_audio_for(win);
        }
        // An app that is not playing anything yet is not listed: we aim at the name given by the
        // preset ("audioAppName": Unity announces itself as "FMOD Ex App"), otherwise its class;
        // gpu-screen-recorder takes it as soon as it emits sound.
        if let Some(n) = opts.get("audioAppName").filter(|n| truthy(n)) {
            audio_app = Some(py_str(n));
        } else if audio_app.is_none() && want_app {
            audio_app = Some(match win.get("class") {
                Some(c) if truthy(c) => py_str(c),
                _ => String::new(),
            });
        }
    }

    let mut head = vec![prog(GSR).to_string_lossy().into_owned()];
    match &target {
        Target::Window(win) => {
            std::fs::create_dir_all(&env.run).map_err(|e| e.to_string())?;
            let pick = Pick {
                address: win.get("address").cloned().unwrap_or(Value::Null),
                at: now_ms() / 1000.0,
            };
            std::fs::write(env.pick_file(), dumps(&pick)).map_err(|e| e.to_string())?;
            head.extend(["-w", "portal", "-restore-portal-session", "no"].map(String::from));
        }
        Target::Monitor(name) => head.extend(["-w".to_string(), name.clone()]),
        Target::Region => {
            let Some(region) = pick_region() else {
                return Ok(0); // choice cancelled
            };
            head.extend(["-w".to_string(), "region".into(), "-region".into(), region]);
        }
    }

    let raw = match opts.get("dir").filter(|v| truthy(v)) {
        Some(v) => py_str(v),
        None => env.videos.clone(),
    };
    let mut out_dir = env.expanduser(&raw);
    let parent = dirname(out_dir.trim_end_matches('/'));
    let parent = if parent.is_empty() {
        "/".to_string()
    } else {
        parent
    };
    if !std::path::Path::new(&parent).is_dir() {
        out_dir = env.videos.clone(); // disk absent: Videos folder
    }
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("{out_dir}: {e}"))?;
    let container = opts
        .get("container")
        .map(py_str)
        .unwrap_or_else(|| "mp4".into());
    let out = join(
        &out_dir,
        &format!("{} {}.{}", slug(&label), stamp(), container),
    );
    let sources = audio_sources(&py_str(&opts["audio"]), audio_app.as_deref());
    let cmd = gsr_args(head, &opts, &out, &sources);

    // The supervisor runs detached: it keeps gpu-screen-recorder and cleans up at the end.
    let spec = dumps(&Spec {
        cmd,
        file: out,
        label,
    });
    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    spawn_detached(
        &me.into_os_string(),
        &[OsString::from("--supervise"), OsString::from(spec)],
    )
    .map_err(|e| e.to_string())?;
    Ok(0)
}

#[derive(Serialize)]
struct Pick {
    address: Value,
    at: f64,
}

/// The supervisor: runs gpu-screen-recorder, writes the state, waits for the end, cleans up and notifies.
pub fn supervise(env: &Env, spec: &str) -> i32 {
    let Ok(spec) = serde_json::from_str::<Spec>(spec) else {
        eprintln!("poste-record: invalid supervisor specification");
        return 1;
    };
    let _ = std::fs::create_dir_all(&env.run);
    let log = match std::fs::File::create(env.log_file()) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("poste-record: {e}");
            return 1;
        }
    };
    let spawned = Command::new(&spec.cmd[0])
        .args(&spec.cmd[1..])
        .stdin(Stdio::null())
        .stdout(
            log.try_clone()
                .map(Stdio::from)
                .unwrap_or_else(|_| Stdio::null()),
        )
        .stderr(Stdio::from(log))
        .spawn();
    let mut child = match spawned {
        Ok(c) => c,
        Err(e) => {
            // The original script stopped here without saying anything; we warn.
            let _ = std::fs::write(env.log_file(), format!("{}: {e}\n", spec.cmd[0]));
            notify(
                "Recording failed",
                &format!("{}: {e}", spec.cmd[0]),
                None,
                "critical",
            );
            let _ = std::fs::remove_file(env.pick_file());
            return 1;
        }
    };
    let st = State {
        pid: child.id() as i64,
        started: now_ms(),
        paused_at: None,
        paused_total: 0.0,
        file: spec.file.clone(),
        label: spec.label.clone(),
    };
    let _ = write_state(env, &st);
    let status = child.wait();
    let code = match status {
        Ok(s) => {
            use std::os::unix::process::ExitStatusExt;
            s.code().or_else(|| s.signal().map(|n| -n)).unwrap_or(-1)
        }
        Err(_) => -1,
    };
    let _ = std::fs::remove_file(env.state_file());
    let _ = std::fs::remove_file(env.pick_file());
    let path = &spec.file;
    let saved = std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.len() > 0);
    if saved {
        notify(
            "Recording saved",
            &basename(path),
            Some(&["xdg-open".to_string(), dirname(path)]),
            "normal",
        );
    } else {
        let tail = std::fs::read(env.log_file())
            .ok()
            .and_then(|b| {
                String::from_utf8_lossy(&b)
                    .lines()
                    .map(|l| l.trim().to_string())
                    .rfind(|l| !l.is_empty())
            })
            .unwrap_or_else(|| format!("gpu-screen-recorder exited with code {code}"));
        notify(
            "Recording failed",
            &crate::pyval::head_chars(&tail, 200),
            None,
            "critical",
        );
    }
    0
}

/// `stop`: SIGINT to gpu-screen-recorder, which finishes the file cleanly.
pub fn stop(env: &Env) -> i32 {
    let Some(st) = read_state(env) else {
        return 1;
    };
    // SAFETY: signal sent to a pid checked alive by `read_state`.
    let sent = unsafe { libc::kill(st.pid as libc::pid_t, libc::SIGINT) };
    if sent == 0 {
        0
    } else {
        1
    }
}

/// `pause` (`want = Some(true)`), `resume` (`Some(false)`) or `toggle` (`None`).
pub fn pause(env: &Env, want: Option<bool>) -> i32 {
    let Some(mut st) = read_state(env) else {
        return 1;
    };
    let paused = st.paused_at.is_some();
    if want == Some(paused) {
        return 0;
    }
    // SAFETY: signal sent to a pid checked alive by `read_state`.
    if unsafe { libc::kill(st.pid as libc::pid_t, libc::SIGUSR2) } != 0 {
        return 1;
    }
    let now = now_ms();
    if let Some(at) = st.paused_at {
        st.paused_total += now - at;
        st.paused_at = None;
    } else {
        st.paused_at = Some(now);
    }
    match write_state(env, &st) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("poste-record: {e}");
            1
        }
    }
}

/// `watch`: one JSON line per state change, and one per second during the recording.
pub fn watch(env: &Env) -> i32 {
    type Key = (bool, Option<bool>, Option<String>);
    let mut last: Option<Key> = None;
    loop {
        let cur = public(read_state(env).as_ref());
        let key: Key = (cur.recording, cur.paused, cur.label.clone());
        if last.as_ref() != Some(&key) || cur.recording {
            // In progress: one line per second, to resynchronize the island's timer.
            let mut out = std::io::stdout().lock();
            if writeln!(out, "{}", dumps(&cur))
                .and_then(|_| out.flush())
                .is_err()
            {
                return 0;
            }
            last = Some(key);
        }
        std::thread::sleep(Duration::from_millis(if cur.recording {
            1000
        } else {
            500
        }));
    }
}

fn open_detached(target: &str) -> i32 {
    match spawn_detached(&OsString::from("xdg-open"), &[OsString::from(target)]) {
        Ok(_) => 0,
        Err(e) => {
            eprintln!("poste-record: xdg-open: {e}");
            1
        }
    }
}

/// `open-dir [--preset ID]`.
pub fn open_dir(env: &Env, preset: Option<&str>) -> i32 {
    open_detached(&out_dir_for(env, preset))
}

/// Finished videos of a folder (not the one being recorded), in disk order.
pub fn finished_videos(env: &Env, d: &str) -> Vec<String> {
    let busy = read_state(env).map(|s| s.file);
    let Ok(rd) = std::fs::read_dir(d) else {
        return Vec::new();
    };
    rd.flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|f| {
            let l = f.to_lowercase();
            [".mp4", ".mkv", ".webm", ".mov"]
                .iter()
                .any(|x| l.ends_with(x))
        })
        .map(|f| join(d, &f))
        .filter(|p| busy.as_deref() != Some(p.as_str()))
        .collect()
}

/// `open-last [--preset ID]`.
pub fn open_last(env: &Env, preset: Option<&str>) -> i32 {
    let d = out_dir_for(env, preset);
    let files = finished_videos(env, &d);
    let mut best: Option<(std::time::SystemTime, String)> = None;
    for f in files {
        let Ok(m) = std::fs::metadata(&f).and_then(|m| m.modified()) else {
            continue;
        };
        // Tie: the first one found wins, like `max()`.
        if best.as_ref().is_none_or(|(t, _)| m > *t) {
            best = Some((m, f));
        }
    }
    let Some((_, last)) = best else {
        notify(
            "No recording yet",
            &d.replace(&env.home, "~"),
            None,
            "normal",
        );
        return 1;
    };
    open_detached(&last)
}

/// SHIFT+PRINT: stops the recording in progress, otherwise opens the launcher on "Record".
pub fn hotkey(env: &Env) -> i32 {
    if read_state(env).is_some() {
        return stop(env);
    }
    match Command::new("omarchy-shell")
        .args(["poste.island", "launcherIn", "record"])
        .status()
    {
        Ok(s) => s.code().unwrap_or(1),
        Err(e) => {
            eprintln!("poste-record: omarchy-shell: {e}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn opts(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    fn defaults() -> Map<String, Value> {
        opts(
            json!({"fps": 60, "quality": "very_high", "codec": "auto", "container": "mp4", "cursor": true}),
        )
    }

    #[test]
    fn gsr_arguments_by_default() {
        let a = gsr_args(
            vec!["gsr".into(), "-w".into(), "DP-1".into()],
            &defaults(),
            "/v/a.mp4",
            "default_output",
        );
        assert_eq!(
            a.join(" "),
            "gsr -w DP-1 -f 60 -q very_high -k auto -fm cfr -cursor yes -fallback-cpu-encoding yes -o /v/a.mp4 -ac aac -a default_output"
        );
    }

    #[test]
    fn gsr_arguments_mkv_without_cursor_or_sound() {
        let mut o = defaults();
        o.insert("container".into(), json!("mkv"));
        o.insert("cursor".into(), json!(false));
        o.insert("resolution".into(), json!("1920x1080"));
        o.insert("fps".into(), json!(30.0));
        let a = gsr_args(vec!["gsr".into()], &o, "/v/a.mkv", "");
        assert_eq!(
            a.join(" "),
            "gsr -f 30.0 -q very_high -k auto -fm cfr -cursor no -fallback-cpu-encoding yes -o /v/a.mkv -s 1920x1080"
        );
    }

    #[test]
    fn flag_without_value() {
        let a = vec!["--preset".to_string()];
        assert!(flag_value(&a, "--preset").is_err());
        assert_eq!(flag_value(&a, "--window"), Ok(None));
    }

    #[test]
    fn app_sound() {
        assert!(wants_app(&json!("app")));
        assert!(wants_app(&json!("app+mic")));
        assert!(!wants_app(&json!("desktop")));
        assert!(!wants_app(&json!("app:Unity")));
    }
}
