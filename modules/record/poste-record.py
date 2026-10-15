#!/usr/bin/env python3
"""Record module of poste: screen recording with gpu-screen-recorder, driven by the island.
Run with `poste record <command>`.

  poste record start --preset ID            a preset of record.json
  poste record start --window ADDR [--audio A]   a window (Hyprland address)
  poste record start --monitor NAME [--audio A]  a whole screen
  poste record start --region [--audio A]        a region picked with the mouse
  poste record stop | pause | toggle            stop, pause / resume, one or the other
  poste record status                           the state as JSON
  poste record watch                            one JSON line per state change
  poste record items                            the launcher list ("Record" extension)
  poste record open-dir | open-last [--preset ID]   the folder, or the last recording
  poste record hotkey                           SHIFT+PRINT: stops the recording in progress,
                                                otherwise opens the island's launcher on "Record"

Sound (--audio): none, app (the sound of the window's app), desktop, mic,
desktop+mic, app+mic; or raw gpu-screen-recorder sources separated
by "|" (app:Unity|default_input).

Presets: ~/.config/poste/record/presets.json (see DEFAULT_CONFIG). A
preset chooses its sound with "audioSource" (same values as --audio) and,
with "app", can give "audioAppName": the name of the app in PipeWire
when it differs from its window class (Unity announces itself as "FMOD Ex App").
"cursor" (true / false) shows or hides the mouse in the video.
A window goes through the capture portal: the choice of the window is
passed to the picker by the module's bin/poste-share-picker ("pick" file in
$XDG_RUNTIME_DIR/poste-record), so without a click. A screen or a region go
directly through KMS, without the portal.

A watching process (start --supervise) runs gpu-screen-recorder,
waits for it to end, clears the state and notifies: "Recording saved" (a click opens
the folder) or the error. stop sends SIGINT (gpu-screen-recorder finishes the
file), pause SIGUSR2."""
import json, os, re, signal, subprocess, sys, time

HOME = os.path.expanduser("~")
RUN = os.path.join(os.environ.get("XDG_RUNTIME_DIR", "/tmp"), "poste-record")
STATE = os.path.join(RUN, "state.json")
PICK = os.path.join(RUN, "pick.json")
LOG = os.path.join(RUN, "last.log")
CONFIG = os.path.join(os.environ.get("XDG_CONFIG_HOME") or os.path.join(HOME, ".config"), "poste/record/presets.json")
VIDEOS = os.environ.get("XDG_VIDEOS_DIR") or os.path.join(HOME, "Videos")

DEFAULT_CONFIG = {
    "//": "Recording presets (poste-record). window.class / window.title: Hyprland regular expressions. audio: none, app, desktop, mic, desktop+mic, app+mic or raw sources (app:Unity|default_input). dir: output folder (~ accepted).",
    "defaults": {"dir": "~/Videos", "fps": 60, "quality": "very_high", "codec": "auto", "container": "mp4", "cursor": True},
    "presets": [],
}


def load_config():
    try:
        with open(CONFIG) as f:
            cfg = json.load(f)
    except (OSError, ValueError):
        cfg = DEFAULT_CONFIG
    d = dict(DEFAULT_CONFIG["defaults"])
    d.update(cfg.get("defaults") or {})
    return d, [p for p in (cfg.get("presets") or []) if p.get("enabled", True)]


def notify(summary, body="", exec_args=None, urgency="normal"):
    cmd = ["omarchy-notification-send", "--app-name", "Recorder", "-g", "󰑋", "-u", urgency, summary, body]
    if exec_args:
        cmd += ["--exec"] + exec_args
    try:
        subprocess.run(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=5)
    except (OSError, subprocess.TimeoutExpired):
        pass


def read_state():
    try:
        with open(STATE) as f:
            st = json.load(f)
    except (OSError, ValueError):
        return None
    try:
        os.kill(int(st.get("pid", 0)), 0)
    except (OSError, ValueError, TypeError):
        return None
    return st


def write_state(st):
    os.makedirs(RUN, exist_ok=True)
    tmp = STATE + ".tmp"
    with open(tmp, "w") as f:
        json.dump(st, f)
    os.replace(tmp, STATE)


def public(st):
    if not st:
        return {"t": "status", "recording": False}
    now = time.time() * 1000
    paused_at = st.get("pausedAt")
    elapsed = (paused_at or now) - st["started"] - st.get("pausedTotal", 0)
    return {"t": "status", "recording": True, "paused": bool(paused_at), "recordMs": int(max(0, elapsed)),
            "label": st.get("label", ""), "file": st.get("file", "")}


# ---------------------------------------------------------------- cibles
def clients():
    try:
        return json.loads(subprocess.run(["hyprctl", "clients", "-j"], capture_output=True, text=True, timeout=3).stdout)
    except (OSError, ValueError, subprocess.TimeoutExpired):
        return []


def monitors():
    try:
        return json.loads(subprocess.run(["hyprctl", "monitors", "-j"], capture_output=True, text=True, timeout=3).stdout)
    except (OSError, ValueError, subprocess.TimeoutExpired):
        return []


def app_audio_names():
    try:
        out = subprocess.run(["gpu-screen-recorder", "--list-application-audio"], capture_output=True, text=True, timeout=5).stdout
    except (OSError, subprocess.TimeoutExpired):
        return []
    return [l.strip() for l in out.splitlines() if l.strip()]


def app_audio_for(client):
    """The PipeWire name of a window's app ("Zen", "Unity"…), if it plays sound."""
    names = app_audio_names()
    keys = [str(client.get("class", "")), str(client.get("initialClass", "")), str(client.get("title", "")).split(" ")[0]]
    for k in keys:
        for n in names:
            if k and (n.lower() == k.lower() or n.lower() in k.lower() or k.lower() in n.lower()):
                return n
    return None


def find_window(match):
    cls, title = match.get("class"), match.get("title")
    found = [c for c in clients() if c.get("mapped", True)
             and (not cls or re.search(cls, str(c.get("class", "")), re.I))
             and (not title or re.search(title, str(c.get("title", "")), re.I))]
    # The largest: the main window rather than a dialog box.
    found.sort(key=lambda c: -(c.get("size", [0, 0])[0] * c.get("size", [0, 0])[1]))
    return found[0] if found else None


def audio_sources(audio, app=None):
    audio = str(audio or "none")
    if "|" in audio or ":" in audio or audio.startswith("default_") or audio.startswith("alsa_"):
        return audio
    parts = []
    for word in audio.split("+"):
        if word == "desktop":
            parts.append("default_output")
        elif word == "mic":
            parts.append("default_input")
        elif word == "app":
            parts.append("app:" + app if app else "default_output")
    return "|".join(parts)


def slug(text):
    text = re.sub(r"[/\\:*?\"<>|]+", " ", str(text)).strip()
    return re.sub(r"\s+", " ", text)[:60] or "Recording"


# ---------------------------------------------------------------- start / stop
def start(args):
    if read_state():
        notify("Already recording", "Stop the current recording first.")
        return 1
    d, presets = load_config()
    opts = dict(d)
    target, label, audio_app = None, "Recording", None
    if "--preset" in args:
        pid = args[args.index("--preset") + 1]
        preset = next((p for p in presets if p.get("id") == pid), None)
        if not preset:
            notify("Recording", "Unknown preset: " + pid, urgency="critical")
            return 1
        opts.update({k: v for k, v in preset.items() if k in ("dir", "fps", "quality", "codec", "container", "resolution", "audioAppName", "cursor")})
        if preset.get("audioSource"):
            opts["audio"] = preset["audioSource"]
        label = preset.get("name", pid)
        if preset.get("window"):
            win = find_window(preset["window"])
            if not win:
                notify("Nothing to record", label + " has no open window.", urgency="critical")
                return 1
            target = ("window", win)
        elif preset.get("monitor"):
            target = ("monitor", preset["monitor"])
        else:
            target = ("region", None)
    elif "--window" in args:
        addr = args[args.index("--window") + 1]
        win = next((c for c in clients() if c.get("address") == addr), None)
        if not win:
            notify("Nothing to record", "That window is gone.", urgency="critical")
            return 1
        target, label = ("window", win), str(win.get("class") or "Window")
    elif "--monitor" in args:
        name = args[args.index("--monitor") + 1]
        target, label = ("monitor", name), "Screen " + name
    elif "--region" in args:
        target, label = ("region", None), "Region"
    else:
        print(__doc__)
        return 2
    if "--audio" in args:
        opts["audio"] = args[args.index("--audio") + 1]
    opts.setdefault("audio", "app" if target[0] == "window" else "desktop")
    if target[0] == "window":
        audio_app = app_audio_for(target[1]) if "app" in str(opts["audio"]).split("+") else None
        # An app that is not playing anything yet is not listed: we aim at the name
        # given by the preset ("audioAppName": Unity announces itself as "FMOD Ex App"),
        # otherwise its class; gpu-screen-recorder takes it as soon as it emits sound.
        if opts.get("audioAppName"):
            audio_app = str(opts["audioAppName"])
        elif not audio_app and "app" in str(opts["audio"]).split("+"):
            audio_app = str(target[1].get("class") or "")

    cmd = ["gpu-screen-recorder"]
    if target[0] == "window":
        os.makedirs(RUN, exist_ok=True)
        with open(PICK, "w") as f:
            json.dump({"address": target[1]["address"], "at": time.time()}, f)
        cmd += ["-w", "portal", "-restore-portal-session", "no"]
    elif target[0] == "monitor":
        cmd += ["-w", target[1]]
    else:
        try:
            geo = subprocess.run(["slurp", "-d"], capture_output=True, text=True, timeout=120).stdout.strip()
        except (OSError, subprocess.TimeoutExpired):
            geo = ""
        m = re.match(r"(-?\d+),(-?\d+) (\d+)x(\d+)", geo)
        if not m:
            return 0   # choice cancelled
        cmd += ["-w", "region", "-region", "%sx%s+%s+%s" % (m.group(3), m.group(4), m.group(1), m.group(2))]

    out_dir = os.path.expanduser(str(opts.get("dir") or VIDEOS))
    if not os.path.isdir(os.path.dirname(out_dir.rstrip("/")) or "/"):
        out_dir = VIDEOS   # disk absent: Videos folder
    os.makedirs(out_dir, exist_ok=True)
    stamp = time.strftime("%Y-%m-%d %H-%M-%S")
    out = os.path.join(out_dir, "%s %s.%s" % (slug(label), stamp, opts.get("container", "mp4")))
    cmd += ["-f", str(opts.get("fps", 60)), "-q", str(opts.get("quality", "very_high")),
            "-k", str(opts.get("codec", "auto")), "-fm", "cfr", "-cursor", "yes" if opts.get("cursor", True) else "no",
            "-fallback-cpu-encoding", "yes", "-o", out]
    if opts.get("container", "mp4") == "mp4":
        cmd += ["-ac", "aac"]   # readable by editing software (Opus in MP4 is poorly supported)
    if opts.get("resolution"):
        cmd += ["-s", str(opts["resolution"])]
    sources = audio_sources(opts["audio"], audio_app)
    if sources:
        cmd += ["-a", sources]

    # The watcher runs detached: it keeps gpu-screen-recorder and cleans up at the end.
    subprocess.Popen([sys.executable, os.path.realpath(__file__), "--supervise", json.dumps(
        {"cmd": cmd, "file": out, "label": label})], start_new_session=True,
        stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    return 0


def supervise(spec):
    spec = json.loads(spec)
    os.makedirs(RUN, exist_ok=True)
    with open(LOG, "w") as log:
        proc = subprocess.Popen(spec["cmd"], stdin=subprocess.DEVNULL, stdout=log, stderr=log)
    write_state({"pid": proc.pid, "started": time.time() * 1000, "pausedAt": None, "pausedTotal": 0,
                 "file": spec["file"], "label": spec["label"]})
    code = proc.wait()
    try:
        os.remove(STATE)
    except OSError:
        pass
    try:
        os.remove(PICK)
    except OSError:
        pass
    path = spec["file"]
    if os.path.isfile(path) and os.path.getsize(path) > 0:
        notify("Recording saved", os.path.basename(path), ["xdg-open", os.path.dirname(path)])
    else:
        try:
            with open(LOG) as f:
                tail = [l.strip() for l in f if l.strip()][-1:]
        except OSError:
            tail = []
        notify("Recording failed", (tail[0] if tail else "gpu-screen-recorder exited with code %d" % code)[:200],
               urgency="critical")


def out_dir_for(preset_id=None):
    d, presets = load_config()
    p = next((x for x in presets if x.get("id") == preset_id), None) if preset_id else None
    out = os.path.expanduser(str((p or {}).get("dir") or d.get("dir") or VIDEOS))
    return out if os.path.isdir(out) else VIDEOS


def open_dir(preset_id=None):
    subprocess.Popen(["xdg-open", out_dir_for(preset_id)], start_new_session=True,
                     stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    return 0


def finished_videos(d):
    """The finished videos of a folder (not the one being recorded)."""
    st = read_state()
    busy = st.get("file") if st else None
    try:
        names = os.listdir(d)
    except OSError:
        return []
    return [os.path.join(d, f) for f in names if f.lower().endswith((".mp4", ".mkv", ".webm", ".mov"))
            and os.path.join(d, f) != busy]


def open_last(preset_id=None):
    d = out_dir_for(preset_id)
    files = finished_videos(d)
    if not files:
        notify("No recording yet", d.replace(HOME, "~"))
        return 1
    last = max(files, key=os.path.getmtime)
    subprocess.Popen(["xdg-open", last], start_new_session=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    return 0


def signal_recorder(sig):
    st = read_state()
    if not st:
        return None
    os.kill(int(st["pid"]), sig)
    return st


def stop():
    st = signal_recorder(signal.SIGINT)
    return 0 if st else 1


def pause(want=None):
    st = read_state()
    if not st:
        return 1
    paused = bool(st.get("pausedAt"))
    if want is not None and want == paused:
        return 0
    os.kill(int(st["pid"]), signal.SIGUSR2)
    now = time.time() * 1000
    if paused:
        st["pausedTotal"] = st.get("pausedTotal", 0) + now - st["pausedAt"]
        st["pausedAt"] = None
    else:
        st["pausedAt"] = now
    write_state(st)
    return 0


def watch():
    last = None
    while True:
        cur = public(read_state())
        key = (cur.get("recording"), cur.get("paused"), cur.get("label"))
        if key != last or cur.get("recording"):
            # In progress: one line per second, to resynchronize the island's timer.
            print(json.dumps(cur), flush=True)
            last = key
        time.sleep(1 if cur.get("recording") else 0.5)


# ---------------------------------------------------------------- lanceur
def cmd_of(*args):
    return " ".join(["poste", "record"] + ["'%s'" % a.replace("'", "'\\''") for a in args])


def item(id_, name, subtitle, icon, command, verb="Record", keywords=(), actions=(), highlight=None):
    out = {"id": id_, "name": name, "subtitle": subtitle, "icon": icon, "verb": verb,
           "keywords": list(keywords), "action": {"type": "command", "command": command},
           "actions": [{"label": l, "icon": i, "action": {"type": "command", "command": c}} for l, i, c in actions]}
    if highlight:
        out["highlight"] = highlight
    return out


def monitor_rect(m):
    """Logical rectangle of a screen (rotation and scale included)."""
    scale = float(m.get("scale") or 1)
    w, h = m.get("width", 0) / scale, m.get("height", 0) / scale
    if int(m.get("transform", 0)) % 2 == 1:
        w, h = h, w
    return {"x": m.get("x", 0), "y": m.get("y", 0), "w": round(w), "h": round(h)}


def window_rect(c, mons):
    """Rectangle of a window, only if its workspace is shown somewhere
    (the island frames the target chosen in the launcher)."""
    ws = (c.get("workspace") or {}).get("id")
    shown = any(ws in ((m.get("activeWorkspace") or {}).get("id"), (m.get("specialWorkspace") or {}).get("id")) for m in mons)
    if not shown:
        return None
    (x, y), (w, h) = c.get("at", [0, 0]), c.get("size", [0, 0])
    return {"x": x, "y": y, "w": w, "h": h, "window": True}


def desktop_icons():
    """Window class -> icon, from the .desktop files (StartupWMClass or name)."""
    icons = {}
    dirs = [os.path.join(HOME, ".local/share/applications"), "/usr/share/applications"]
    for d in dirs:
        try:
            names = os.listdir(d)
        except OSError:
            continue
        for n in names:
            if not n.endswith(".desktop"):
                continue
            icon, wm = None, None
            try:
                with open(os.path.join(d, n), errors="replace") as f:
                    for line in f:
                        if line.startswith("[") and not line.startswith("[Desktop Entry]"):
                            break
                        if line.startswith("Icon="):
                            icon = line[5:].strip()
                        elif line.startswith("StartupWMClass="):
                            wm = line[15:].strip()
            except OSError:
                continue
            if icon:
                for key in filter(None, (wm, n[:-8])):
                    icons.setdefault(key.lower(), icon)
    return icons


def audio_actions(*target, preset=None):
    where = ("--preset", preset) if preset else ()
    return [("No audio", "󰝟", cmd_of("start", *target, "--audio", "none")),
            ("With desktop audio", "󰕾", cmd_of("start", *target, "--audio", "desktop")),
            ("With desktop + microphone", "󰍬", cmd_of("start", *target, "--audio", "desktop+mic")),
            ("Play last recording", "󰐊", cmd_of("open-last", *where)),
            ("Open recordings folder", "󰉋", cmd_of("open-dir", *where))]


def items():
    out = []
    st = read_state()
    if st:
        p = public(st)
        out.append(item("stop", "Stop recording", p["label"] + " · " + os.path.basename(p["file"]), "󰓛",
                        cmd_of("stop"), verb="Stop", keywords=["stop", "arrêter"],
                        actions=[("Open recordings folder", "󰉋", "xdg-open '%s'" % os.path.dirname(p["file"]).replace("'", "'\\''"))]))
        out.append(item("pause", "Resume recording" if p["paused"] else "Pause recording", p["label"],
                        "󰐊" if p["paused"] else "󰏤", cmd_of("toggle"), verb="Resume" if p["paused"] else "Pause",
                        keywords=["pause", "resume", "reprendre"]))
        print(json.dumps(out))
        return
    _, presets = load_config()
    mons = monitors()
    for p in presets:
        target = ("--preset", p["id"])
        hl = None
        if p.get("window"):
            w = find_window(p["window"])
            hl = window_rect(w, mons) if w else None
        elif p.get("monitor"):
            hl = next((monitor_rect(m) for m in mons if m.get("name") == p["monitor"]), None)
        out.append(item("preset:" + p["id"], p.get("name", p["id"]), "Preset · " + p["subtitle"] if p.get("subtitle") else "Preset",
                        p.get("icon", "󰑋"), cmd_of("start", *target), keywords=["preset"] + list(p.get("keywords", [])),
                        actions=audio_actions(*target, preset=p["id"]), highlight=hl))
        out[-1]["badge"] = "󰓎"   # "preset" badge on the icon in the launcher
    icons = desktop_icons()
    for c in clients():
        if not c.get("mapped", True) or not c.get("title"):
            continue
        target = ("--window", c["address"])
        cls = str(c.get("class", ""))
        icon = icons.get(cls.lower()) or icons.get(cls.lower().split(".")[-1]) or "󰖯"
        out.append(item("window:" + c["address"], str(c["title"])[:80],
                        "Window · %s · with its own audio" % cls, icon, cmd_of("start", *target),
                        keywords=["window", "fenêtre", str(c.get("class", ""))], actions=audio_actions(*target),
                        highlight=window_rect(c, mons)))
    for m in mons:
        target = ("--monitor", m["name"])
        out.append(item("monitor:" + m["name"], "Screen " + m["name"],
                        "%sx%s · %s · with desktop audio" % (m.get("width"), m.get("height"), m.get("description", "")[:40]),
                        "󰍹", cmd_of("start", *target), keywords=["screen", "écran", "monitor"], actions=audio_actions(*target),
                        highlight=monitor_rect(m)))
    out.append(item("region", "Region…", "Select an area · with desktop audio", "󰩭", cmd_of("start", "--region"),
                    keywords=["region", "zone", "area"], actions=audio_actions("--region")))
    print(json.dumps(out))


def hotkey():
    """SHIFT+PRINT: stops the recording in progress, otherwise opens the launcher on "Record"."""
    if read_state():
        return stop()
    return subprocess.call(["omarchy-shell", "poste.island", "launcherIn", "record"])


def main(argv):
    if not argv:
        print(__doc__)
        return 2
    cmd, rest = argv[0], argv[1:]
    if cmd == "--supervise":
        supervise(rest[0])
        return 0
    if cmd == "start":
        return start(rest)
    if cmd == "stop":
        return stop()
    if cmd == "toggle":
        return pause()
    if cmd == "pause":
        return pause(True)
    if cmd == "resume":
        return pause(False)
    if cmd == "status":
        print(json.dumps(public(read_state())))
        return 0
    if cmd == "watch":
        watch()
        return 0
    if cmd in ("open-dir", "open-last"):
        pid = rest[rest.index("--preset") + 1] if "--preset" in rest else None
        return open_dir(pid) if cmd == "open-dir" else open_last(pid)
    if cmd == "items":
        items()
        return 0
    if cmd == "hotkey":
        return hotkey()
    print(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
