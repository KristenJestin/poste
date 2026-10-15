#!/bin/sh
# Fake outside programs for the poste-record tests (hyprctl, gpu-screen-recorder, slurp,
# omarchy-notification-send, xdg-open, omarchy-shell). Copied under each of these names; the
# behaviour depends on the call name. Nothing is ever really recorded.
#
# Variables: FAKE_DIR (work folder of the test). Files read in FAKE_DIR:
#   clients.json, monitors.json  answers of `hyprctl clients|monitors -j`
#   app_audio                    answer of `gpu-screen-recorder --list-application-audio`
#   slurp                        answer of `slurp -d` (absent: choice cancelled)
#   gsr_mode                     quick (default): writes the file and exits; hold: waits for SIGINT
#                                (SIGUSR2 logged, "gsr-ready" once the signals are taken over);
#                                fail: error without a file; nofile: exits without a file
# Journal: FAKE_DIR/journal, one line "name<TAB>arg<TAB>arg…" per call.
name=$(basename "$0")
J="$FAKE_DIR/journal"

log() {
  line=$name
  for a in "$@"; do line="$line	$a"; done
  printf '%s\n' "$line" >> "$J"
}

case "$name" in
  hyprctl)
    log "$@"
    [ -f "$FAKE_DIR/$1.json" ] && cat "$FAKE_DIR/$1.json"
    exit 0
    ;;
  slurp)
    log "$@"
    [ -f "$FAKE_DIR/slurp" ] && cat "$FAKE_DIR/slurp"
    exit 0
    ;;
  gpu-screen-recorder)
    log "$@"
    if [ "$1" = "--list-application-audio" ]; then
      [ -f "$FAKE_DIR/app_audio" ] && cat "$FAKE_DIR/app_audio"
      exit 0
    fi
    out=
    prev=
    for a in "$@"; do
      [ "$prev" = "-o" ] && out=$a
      prev=$a
    done
    mode=quick
    [ -f "$FAKE_DIR/gsr_mode" ] && mode=$(cat "$FAKE_DIR/gsr_mode")
    case "$mode" in
      quick)
        printf 'video' > "$out"
        exit 0
        ;;
      nofile)
        exit 0
        ;;
      fail)
        echo "first line" >&2
        echo "boom: cannot capture" >&2
        exit 3
        ;;
      hold)
        printf 'video' > "$out"
        trap 'printf "gsr-signal\tUSR2\n" >> "$J"' USR2
        trap 'printf "gsr-signal\tINT\n" >> "$J"; kill "$sp" 2>/dev/null; exit 0' INT
        printf 'gsr-ready\n' >> "$J"
        sleep 20 &
        sp=$!
        while kill -0 "$sp" 2>/dev/null; do wait "$sp"; done
        exit 0
        ;;
    esac
    ;;
  *)
    log "$@"
    exit 0
    ;;
esac
