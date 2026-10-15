#!/usr/bin/env python3
"""Screen-share picker for xdg-desktop-portal-hyprland
(screencopy:custom_picker_binary in ~/.config/hypr/xdph.conf, line put by poste sys setup from the module.toml of the record module).

If `poste record` has just requested a window (file pick.json less than
20 s old in $XDG_RUNTIME_DIR/poste-record), answers directly with that
window, showing nothing. Otherwise (screen share of a browser, of OBS…),
hands over to the usual picker, hyprland-preview-share-picker.

xdph protocol: the list of windows arrives in XDPH_WINDOW_SHARING_LIST,
"handle[HC>]class[HT>]title[HE>]address[HA>]" one after the other; the answer is
"[SELECTION]r/window:<handle>" on the standard output."""
import json, os, sys, time

PICK = os.path.join(os.environ.get("XDG_RUNTIME_DIR", "/tmp"), "poste-record", "pick.json")
FALLBACK = "hyprland-preview-share-picker"


def fallback():
    os.execvp(FALLBACK, [FALLBACK] + sys.argv[1:])


try:
    with open(PICK) as f:
        pick = json.load(f)
    os.remove(PICK)
except (OSError, ValueError):
    fallback()

if time.time() - float(pick.get("at", 0)) > 20:
    fallback()


def as_int(value):
    try:
        return int(str(value), 0)
    except ValueError:
        try:
            return int(str(value), 16)
        except ValueError:
            return None


want = as_int(pick.get("address"))
# Trace of the last choice (window titles only), for troubleshooting.
try:
    with open(os.path.join(os.path.dirname(PICK), "picker.log"), "w") as log:
        log.write("want=%s\n%s\n" % (pick.get("address"), os.environ.get("XDPH_WINDOW_SHARING_LIST", "")))
except OSError:
    pass
for entry in os.environ.get("XDPH_WINDOW_SHARING_LIST", "").split("[HA>]"):
    if "[HE>]" not in entry:
        continue
    handle = entry.split("[HC>]", 1)[0]
    addr = entry.rsplit("[HE>]", 1)[1]
    if want is not None and as_int(addr) == want:
        print("[SELECTION]r/window:" + handle, flush=True)
        sys.exit(0)

fallback()
