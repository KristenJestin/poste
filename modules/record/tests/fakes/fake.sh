#!/bin/sh
# Faux programmes extérieurs pour les tests de poste-record (hyprctl, gpu-screen-recorder, slurp,
# omarchy-notification-send, xdg-open, omarchy-shell). Copié sous chacun de ces noms ; le
# comportement dépend du nom d'appel. Rien n'est jamais réellement enregistré.
#
# Variables : FAKE_DIR (dossier de travail du test). Fichiers lus dans FAKE_DIR :
#   clients.json, monitors.json  réponses de `hyprctl clients|monitors -j`
#   app_audio                    réponse de `gpu-screen-recorder --list-application-audio`
#   slurp                        réponse de `slurp -d` (absent : choix annulé)
#   gsr_mode                     quick (défaut) : écrit le fichier et sort ; hold : attend SIGINT
#                                (SIGUSR2 journalisé) ; fail : erreur sans fichier ; nofile : sort sans fichier
# Journal : FAKE_DIR/journal, une ligne « nom<TAB>arg<TAB>arg… » par appel.
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
