#!/usr/bin/env bash
# Construit l'archive d'une version de poste : ce que `poste sys setup` lit dans une racine
# (config/, bootstrap/, les fichiers de modules qu'il pose, les binaires dans target/release/),
# plus VERSION (commit et date). Seuls les fichiers suivis par git y entrent.
# Usage : .github/package.sh [DOSSIER_DE_SORTIE]   (défaut : target/dist)
# Les binaires viennent de target/release/ (cargo build --release), ou de $POSTE_BIN_DIR.
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
out="${1:-$repo/target/dist}"
bin="${POSTE_BIN_DIR:-$repo/target/release}"
name="poste-x86_64-linux.tar.gz"

commit="$(git -C "$repo" rev-parse HEAD)"
date="$(TZ=UTC0 git -C "$repo" show -s --date=format-local:%Y-%m-%dT%H:%M:%SZ --format=%cd HEAD)"

stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT

git -C "$repo" archive --format=tar HEAD config bootstrap modules | tar -x -C "$stage"

# D'un module, seulement ce que setup pose : module.toml, island/, hypr/, bin/, systemd/ et le
# programme d'un module-script (poste-<nom>).
for m in "$stage"/modules/*/; do
  n="$(basename "$m")"
  find "$m" -mindepth 1 -maxdepth 1 \
    ! -name module.toml ! -name island ! -name hypr ! -name bin ! -name systemd ! -name "poste-$n" \
    -exec rm -rf {} +
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
  echo "package : aucun binaire poste* dans $bin (cargo build --release d'abord)" >&2
  exit 1
fi

printf 'commit=%s\ndate=%s\n' "$commit" "$date" > "$stage/VERSION"

mkdir -p "$out"
tar -czf "$out/$name" --owner=0 --group=0 -C "$stage" .
(cd "$out" && sha256sum "$name" > "$name.sha256")
echo "Archive : $out/$name ($(cut -c1-12 "$out/$name.sha256"))"
