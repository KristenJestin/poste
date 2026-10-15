#!/usr/bin/env bash
# Builds the archive of a poste version: what `poste sys setup` reads in a root
# (config/, bootstrap/, the module files it puts in place, the binaries in target/release/),
# plus VERSION (commit and date). Only files tracked by git go in.
# Usage: .github/package.sh [OUTPUT_FOLDER]   (default: target/dist)
# The binaries come from target/release/ (cargo build --release), or from $POSTE_BIN_DIR.
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
out="${1:-$repo/target/dist}"
bin="${POSTE_BIN_DIR:-$repo/target/release}"
name="poste-x86_64-linux.tar.gz"

commit="$(git -C "$repo" rev-parse HEAD)"
date="$(TZ=UTC0 git -C "$repo" show -s --date=format-local:%Y-%m-%dT%H:%M:%SZ --format=%cd HEAD)"

stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT

git -C "$repo" archive --format=tar HEAD config bootstrap modules addons | tar -x -C "$stage"

# Of a module, only what setup puts in place: module.toml, island/, hypr/, bin/, systemd/ and the
# program of a script module (poste-<name>). An add-on also keeps its bootstrap/.
for m in "$stage"/modules/*/ "$stage"/addons/*/; do
  [[ -d $m ]] || continue
  n="$(basename "$m")"
  keep=bootstrap
  [[ $m == "$stage"/addons/* ]] || keep="poste-$n"
  find "$m" -mindepth 1 -maxdepth 1 \
    ! -name module.toml ! -name island ! -name hypr ! -name bin ! -name systemd ! -name "$keep" \
    ! -name "poste-$n" -exec rm -rf {} +
done

mkdir -p "$stage/target/release"
found=0
for b in "$bin"/poste*; do
  if [[ -f $b && -x $b && $(basename "$b") != *.* ]]; then
    cp "$b" "$stage/target/release/"
    found=1
  fi
done
if ((found == 0)); then
  echo "package: no poste* binary in $bin (cargo build --release first)" >&2
  exit 1
fi

printf 'commit=%s\ndate=%s\n' "$commit" "$date" > "$stage/VERSION"

mkdir -p "$out"
tar -czf "$out/$name" --owner=0 --group=0 -C "$stage" .
(cd "$out" && sha256sum "$name" > "$name.sha256")
echo "Archive: $out/$name ($(cut -c1-12 "$out/$name.sha256"))"
