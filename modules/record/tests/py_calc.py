"""Calculations of the original Python script, to compare them with those of the Rust version.
Reads a JSON on the standard input, writes the results as JSON. Usage: py_calc.py <poste-record.py>"""
import json, sys
from importlib.machinery import SourceFileLoader
from importlib.util import module_from_spec, spec_from_loader

loader = SourceFileLoader("rec", sys.argv[1])
mod = module_from_spec(spec_from_loader("rec", loader))
loader.exec_module(mod)

data = json.load(sys.stdin)
out = {}
out["audio"] = [mod.audio_sources(a, app) for a, app in data["audio"]]
out["slug"] = [mod.slug(t) for t in data["slug"]]
out["monitor_rect"] = [mod.monitor_rect(m) for m in data["monitors"]]
out["window_rect"] = [mod.window_rect(c, data["monitors"]) for c in data["clients"]]
out["find_window"] = []
mod.clients = lambda: data["clients"]
for m in data["matches"]:
    w = mod.find_window(m)
    out["find_window"].append(w["address"] if w else None)
out["app_audio"] = []
for client, names in data["app_audio"]:
    mod.app_audio_names = lambda names=names: names
    out["app_audio"].append(mod.app_audio_for(client))
out["cmd_of"] = [mod.cmd_of(*a) for a in data["cmd_of"]]
print(json.dumps(out))
