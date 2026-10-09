#!/usr/bin/env python3
"""Icônes Tabler à la place des glyphes Nerd Font d'Omarchy.

Une seule source : glyphs.tsv (point de code Nerd -> icône Tabler). Elle sert à :

  build      générer « Poste Nerd Font » : copie de chaque graisse de FiraCode Nerd Font,
             familles renommées, points de code de glyphs.tsv redirigés vers les contours
             Tabler (webfont @tabler/icons-webfont, MIT, épinglée par sha256). Écrit dans
             ~/.local/share/fonts/poste/ puis lance fc-cache. Demande fonttools :
             uv run --no-project --with fonttools python3 poste-icons.py build
  check      code 0 si les polices posées sont à jour (bibliothèque standard seule)
  glyphs-js  réécrire Glyphs.js de l'île depuis glyphs.tsv (--check : comparer seulement)
  scan       recenser les glyphes PUA utilisés par Omarchy et ses plugins absents de glyphs.tsv
  sheet      planche de contrôle PNG, original et remplacement (fonttools et pillow)

Rien ici ne touche à fontconfig : la règle qui donne la police au shell est
60-poste-shell-icons.conf, posée à la main (voir README.md).
"""

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent
TSV = HERE / "glyphs.tsv"
ISLAND = REPO / "config/.config/omarchy/plugins/poste.island"
GLYPHS_JS = ISLAND / "components/Glyphs.js"
ASSETS = ISLAND / "assets"

FAMILY = "Poste Nerd Font"
SOURCE_FAMILY = "FiraCode Nerd Font"

TABLER_VERSION = "3.49.0"
TABLER_URL = "https://cdn.jsdelivr.net/npm/@tabler/icons-webfont@{v}/{path}"
TABLER_FILES = {
    # nom local : (chemin dans le paquet npm, sha256)
    "tabler-icons.ttf": ("dist/fonts/tabler-icons.ttf",
                         "d291676dc4c28b386552753bf16cf2bef0e93e5543f8e0b5c5d72c0da41ae1bc"),
    "tabler-icons-300.ttf": ("dist/fonts/tabler-icons-300.ttf",
                             "69eb77ef26c6aae9247fdb40ad429b98c7ce20a3c8bb0d63d585fc6fdb05748a"),
    "tabler-icons-filled.ttf": ("dist/fonts/tabler-icons-filled.ttf",
                                "08304212f3f43271ee700156044a298a081ad2a24b30e28b4f3346dd2fe7ecf9"),
    "LICENSE": ("LICENSE", "b740a1d46122672da62833e97f7e7c8a13fa85cbc7445b584b297cc00dde93db"),
}

# Où chercher les glyphes utilisés (scan). Le sélecteur d'emojis et de glyphes
# (emojis.json) et la galerie de développement ne sont pas des icônes de l'interface.
SCAN_ROOTS = ["/usr/share/omarchy/default/omarchy/omarchy-menu.jsonc", "/usr/share/omarchy/shell",
              "~/.config/omarchy"]
SCAN_SKIP = ["plugins/emojis/emojis.json", "dev-gallery", "/.git/", "node_modules", ".bak"]


def xdg(var, default):
    return Path(os.environ.get(var) or Path.home() / default)


def out_dir():
    return xdg("XDG_DATA_HOME", ".local/share") / "fonts/poste"


def cache_dir():
    return xdg("XDG_CACHE_HOME", ".cache") / "poste/tabler-webfont" / TABLER_VERSION


# ------------------------------------------------------------------ glyphs.tsv

class Row:
    def __init__(self, code, tabler, filled, island, nerd, note):
        self.code = int(code, 16)
        self.tabler = None if tabler == "-" else tabler
        self.filled = filled == "1"
        # Nom pour l'île (Glyphs.js) : vide = le même que la police, « - » = aucun.
        self.island = None if island == "-" else (island or self.tabler)
        self.nerd = nerd
        self.note = note

    @property
    def char(self):
        return chr(self.code)


def load_rows():
    rows = []
    seen = set()
    for n, line in enumerate(TSV.read_text(encoding="utf-8").splitlines(), 1):
        if not line.strip() or line.startswith("#"):
            continue
        parts = line.split("\t")
        if len(parts) != 6:
            sys.exit(f"glyphs.tsv:{n} : 6 colonnes attendues (séparées par des tabulations), {len(parts)} trouvées")
        row = Row(*parts)
        if row.code in seen:
            sys.exit(f"glyphs.tsv:{n} : {parts[0]} en double")
        seen.add(row.code)
        rows.append(row)
    return rows


# ------------------------------------------------------------------ sources

def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def tabler_files(download=True):
    """Chemins de la webfont Tabler en cache, téléchargée et vérifiée si besoin."""
    d = cache_dir()
    paths = {}
    for name, (rel, digest) in TABLER_FILES.items():
        p = d / name
        if not (p.exists() and sha256(p) == digest):
            if not download:
                return None
            d.mkdir(parents=True, exist_ok=True)
            url = TABLER_URL.format(v=TABLER_VERSION, path=rel)
            print(f"téléchargement de {url}")
            tmp = p.with_suffix(p.suffix + ".part")
            with urllib.request.urlopen(url, timeout=60) as r, open(tmp, "wb") as f:
                shutil.copyfileobj(r, f)
            got = sha256(tmp)
            if got != digest:
                tmp.unlink()
                sys.exit(f"{name} : sha256 inattendu ({got}), rien n'est écrit")
            tmp.replace(p)
        paths[name] = p
    return paths


def source_fonts(family):
    """Fichiers de la famille source (une par graisse), via fontconfig."""
    out = subprocess.run(["fc-list", "-f", "%{file}\n", family], capture_output=True, text=True).stdout
    files = sorted({Path(l) for l in out.splitlines() if l.strip()})
    files = [f for f in files if f.suffix.lower() == ".ttf" and out_dir() not in f.parents]
    if not files:
        sys.exit(f"aucune police « {family} » trouvée par fc-list (paquet ttf-firacode-nerd ?)")
    return files


def stamp_of(family):
    """Empreinte de tout ce dont dépendent les polices générées."""
    h = hashlib.sha256()
    h.update(Path(__file__).read_bytes())
    h.update(TSV.read_bytes())
    h.update(family.encode())
    for name, (_, digest) in sorted(TABLER_FILES.items()):
        h.update(f"{name}:{digest}".encode())
    for f in source_fonts(family):
        st = f.stat()
        h.update(f"{f}:{st.st_size}:{int(st.st_mtime)}".encode())
    return h.hexdigest()


def out_name(src):
    suffix = src.stem.rsplit("-", 1)[-1]
    return f"{FAMILY.replace(' ', '')}-{suffix}.ttf"


# ------------------------------------------------------------------ build

def build_font(src, dst, rows, tabler, family):
    from fontTools.pens.boundsPen import BoundsPen
    from fontTools.pens.transformPen import TransformPen
    from fontTools.pens.ttGlyphPen import TTGlyphPen
    from fontTools.ttLib import TTFont

    # Sans recalcul : les glyphes d'origine, la boîte englobante et les drapeaux de head
    # restent octet pour octet ceux de la source ; maxp est mis à jour à la main plus bas.
    font = TTFont(src, recalcBBoxes=False, recalcTimestamp=False)
    upm = font["head"].unitsPerEm
    hhea = font["hhea"]
    # Centre vertical des icônes Nerd : milieu de la ligne (ascent + descent) / 2.
    cy = (hhea.ascent + hhea.descent) / 2
    light = font["OS/2"].usWeightClass <= 300
    outline = tabler["tabler-icons-300.ttf" if light else "tabler-icons.ttf"]
    sets = {False: outline, True: tabler["tabler-icons-filled.ttf"]}
    cmap = font.getBestCmap()
    glyf = font["glyf"]
    hmtx = font["hmtx"]
    order = font.getGlyphOrder()
    made = {}
    redirect = {}
    for row in rows:
        if row.tabler is None or row.code not in cmap:
            continue
        key = (row.tabler, row.filled)
        if key not in made:
            tf = sets[row.filled]
            tgs = tf.getGlyphSet()
            if row.tabler not in tgs:
                sys.exit(f"{row.tabler}{' (plein)' if row.filled else ''} absent de la webfont Tabler {TABLER_VERSION}")
            # Tabler : grille de 24 dans 1000 unités, centre (500, 400). Une grille de 24 = un em,
            # comme les icônes Material des Nerd Fonts.
            s = upm / tf["head"].unitsPerEm
            bp = BoundsPen(tgs)
            tgs[row.tabler].draw(bp)
            x0, _, x1, _ = bp.bounds
            adv = hmtx[cmap[row.code]][0]
            w = (x1 - x0) * s
            # Comme le patcher Nerd Font (variante non Mono) : centré dans la cellule s'il y tient,
            # sinon calé à gauche et débordant à droite.
            dx = (adv - w) / 2 - x0 * s if w <= adv else -x0 * s
            pen = TTGlyphPen(None)
            tgs[row.tabler].draw(TransformPen(pen, (s, 0, 0, s, dx, cy - 400 * s)))
            g = pen.glyph()
            name = f"poste.{row.tabler}{'.filled' if row.filled else ''}"
            order.append(name)
            glyf[name] = g
            g.recalcBounds(glyf)
            hmtx[name] = (adv, g.xMin if g.numberOfContours else 0)
            maxp = font["maxp"]
            maxp.maxPoints = max(maxp.maxPoints, len(g.coordinates))
            maxp.maxContours = max(maxp.maxContours, g.numberOfContours)
            head = font["head"]
            if g.xMin < head.xMin or g.yMin < head.yMin or g.xMax > head.xMax or g.yMax > head.yMax:
                sys.exit(f"{row.tabler} : sort de la boîte englobante de la police")
            made[key] = name
        redirect[row.code] = made[key]
    font.setGlyphOrder(order)
    for table in font["cmap"].tables:
        if table.isUnicode():
            for code, name in redirect.items():
                if code in table.cmap:
                    table.cmap[code] = name
    rename(font, family)
    keep = metrics(font)
    font.save(dst)
    # Les métriques ne doivent pas bouger : relu et comparé.
    got = metrics(TTFont(dst))
    if got != keep:
        dst.unlink()
        diff = [k for k in keep if keep[k] != got[k]]
        sys.exit(f"{dst.name} : métriques changées ({', '.join(diff)})")
    return len(redirect)


METRICS = {
    "head": ["unitsPerEm", "xMin", "yMin", "xMax", "yMax", "macStyle", "lowestRecPPEM", "indexToLocFormat"],
    "hhea": ["ascent", "descent", "lineGap", "advanceWidthMax", "minLeftSideBearing",
             "minRightSideBearing", "xMaxExtent", "caretSlopeRise", "caretSlopeRun"],
    "OS/2": ["xAvgCharWidth", "usWeightClass", "usWidthClass", "fsSelection", "sTypoAscender",
             "sTypoDescender", "sTypoLineGap", "usWinAscent", "usWinDescent", "sxHeight", "sCapHeight",
             "panose", "ulUnicodeRange1", "ulUnicodeRange2", "ulUnicodeRange3", "ulUnicodeRange4"],
}


def metrics(font):
    def plain(v):
        return vars(v) if hasattr(v, "__dict__") else v
    return {f"{t}.{k}": plain(getattr(font[t], k)) for t, fields in METRICS.items() for k in fields}


def rename(font, family):
    name = font["name"]
    ps = name.getDebugName(6) or ""
    ps_prefix = ps.split("-")[0]
    pairs = [(family, FAMILY), (family.replace(" ", ""), FAMILY.replace(" ", ""))]
    if ps_prefix:
        pairs.append((ps_prefix, "PosteNF"))
    for rec in name.names:
        try:
            text = rec.toUnicode()
        except UnicodeDecodeError:
            continue
        new = text
        if rec.nameID in (1, 3, 4, 6, 16, 18, 21):
            for a, b in pairs:
                new = new.replace(a, b)
        if rec.nameID == 5:
            new = f"{text};poste icons Tabler {TABLER_VERSION}"
        if new != text:
            rec.string = new


def cmd_build(args):
    import logging
    logging.getLogger("fontTools").setLevel(logging.ERROR)
    rows = load_rows()
    family = args.family
    tabler = tabler_files()
    from fontTools.ttLib import TTFont
    tfonts = {k: TTFont(v) for k, v in tabler.items() if k.endswith(".ttf")}
    d = out_dir()
    d.mkdir(parents=True, exist_ok=True)
    written = set()
    for src in source_fonts(family):
        dst = d / out_name(src)
        tmp = dst.with_suffix(".tmp")
        n = build_font(src, tmp, rows, tfonts, family)
        tmp.replace(dst)
        written.add(dst.name)
        print(f"{dst} : {n} glyphes Tabler")
    for old in d.glob("*.ttf"):
        if old.name not in written:
            old.unlink()
    shutil.copyfile(tabler["LICENSE"], d / "LICENSE-tabler-icons")
    (d / "README").write_text(
        f"{FAMILY} : {family} (SIL OFL 1.1) avec des icônes Tabler {TABLER_VERSION} (MIT).\n"
        f"Généré par {Path(__file__).relative_to(REPO)} (dépôt poste) : ne pas modifier à la main.\n",
        encoding="utf-8")
    subprocess.run(["fc-cache", "-f", str(d)], check=False)
    (d / ".stamp").write_text(stamp_of(family) + "\n")


def cmd_check(args):
    d = out_dir()
    stamp = d / ".stamp"
    if not stamp.exists():
        print(f"{FAMILY} : absente de {d}")
        return 1
    expected = {out_name(f) for f in source_fonts(args.family)}
    present = {p.name for p in d.glob("*.ttf")}
    if expected != present or stamp.read_text().strip() != stamp_of(args.family):
        print(f"{FAMILY} : à régénérer")
        return 1
    print(f"{FAMILY} : à jour ({len(present)} graisses)")
    return 0


# ------------------------------------------------------------------ Glyphs.js de l'île

def glyphs_js(rows):
    def has(name):
        return (ASSETS / "tabler" / f"{name}.svg").exists()

    icons, filled = [], []
    for r in rows:
        if r.island and has(r.island) and (not r.filled or has(r.island + "-filled")):
            icons.append(f'  "{r.char}": "{r.island}",{" " * max(1, 26 - len(r.island))}// {r.code:04X} {r.nerd}')
            if r.filled:
                filled.append(f'"{r.char}": 1')
    lines = [
        ".pragma library",
        "// Glyphes Nerd Font de l'île -> icônes Tabler (assets/tabler, MIT).",
        "// GÉNÉRÉ par bootstrap/icons/poste-icons.py glyphs-js depuis bootstrap/icons/glyphs.tsv,",
        "// la même table que la police « Poste Nerd Font » du shell : ne pas modifier à la main.",
        "// Glyph.qml affiche l'icône quand elle est ici, sinon le glyphe d'origine. Une ligne",
        "// n'apparaît ici que si son SVG (et sa variante -filled si besoin) est dans assets/tabler.",
        "var icons = {",
        *[l if i < len(icons) - 1 else l.replace('",', '" ', 1) for i, l in enumerate(icons)],
        "}",
        "",
        "// Pleines par défaut (commandes média, cœur plein) ; le contour seul y paraît trop maigre.",
        "var solid = { " + ", ".join(filled) + " }",
        "",
        "// Chemin de l'icône sous assets/ (« tabler/microphone »), ou \"\".",
        "function iconFor(glyph) {",
        "  var name = icons[String(glyph || \"\")] || \"\"",
        "  return name !== \"\" ? \"tabler/\" + name : \"\"",
        "}",
        "",
        "function prefersFilled(glyph) {",
        "  return !!solid[String(glyph || \"\")]",
        "}",
        "",
    ]
    return "\n".join(lines)


def cmd_glyphs_js(args):
    text = glyphs_js(load_rows())
    current = GLYPHS_JS.read_text(encoding="utf-8") if GLYPHS_JS.exists() else ""
    if args.check:
        if current != text:
            print(f"{GLYPHS_JS} n'est pas à jour : poste-icons.py glyphs-js")
            return 1
        print("Glyphs.js à jour")
        return 0
    if current != text:
        GLYPHS_JS.write_text(text, encoding="utf-8")
        print(f"écrit {GLYPHS_JS}")
    return 0


# ------------------------------------------------------------------ scan

def pua(c):
    return 0xE000 <= c <= 0xF8FF or 0xF0000 <= c <= 0x10FFFD


ESCAPE = re.compile(r"\\u\{([0-9a-fA-F]{4,6})\}|\\u([0-9a-fA-F]{4})|\\U([0-9a-fA-F]{8})")


def scan_uses():
    uses = {}

    def scan(p):
        if any(s in p for s in SCAN_SKIP):
            return
        try:
            text = Path(p).read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError):
            return
        found = {ord(ch) for ch in text if pua(ord(ch))}
        found |= {int(next(g for g in m.groups() if g), 16) for m in ESCAPE.finditer(text)}
        for c in found:
            if pua(c):
                uses.setdefault(c, set()).add(p)

    for root in SCAN_ROOTS:
        root = os.path.expanduser(root)
        if os.path.isfile(root):
            scan(root)
            continue
        for d, _, files in os.walk(root, followlinks=True):
            for f in files:
                scan(os.path.join(d, f))
    return uses


def cmd_scan(args):
    known = {r.code for r in load_rows()}
    uses = scan_uses()
    missing = sorted(c for c in uses if c not in known)
    for c in missing:
        print(f"{c:04X}\t{chr(c)}\t" + ", ".join(sorted(uses[c]))[:200])
    print(f"{len(uses)} glyphes utilisés, {len(missing)} absents de glyphs.tsv")
    return 1 if missing else 0


# ------------------------------------------------------------------ planche de contrôle

def cmd_sheet(args):
    from PIL import Image, ImageDraw, ImageFont

    rows = load_rows()
    style = args.style
    poste = out_dir() / f"{FAMILY.replace(' ', '')}-{style}.ttf"
    srcs = [f for f in source_fonts(args.family) if f.stem.endswith("-" + style)]
    if not poste.exists() or not srcs:
        sys.exit(f"police {style} absente : lancer build d'abord")
    size = args.size
    f_src = ImageFont.truetype(str(srcs[0]), size)
    f_new = ImageFont.truetype(str(poste), size)
    f_txt = ImageFont.truetype(str(srcs[0]), 13)
    mapped = [r for r in rows if r.tabler]
    kept = [r for r in rows if not r.tabler]
    cols, cw, ch = 5, 300, size * 2 + 28
    out = Path(args.out)
    per = args.per_page
    pages = [mapped[i:i + per] for i in range(0, len(mapped), per)] + [kept]
    names = []
    for p, chunk in enumerate(pages):
        lines = (len(chunk) + cols - 1) // cols
        img = Image.new("RGB", (cols * cw, lines * ch + 30), "white")
        d = ImageDraw.Draw(img)
        title = (f"{style} : original FiraCode (gauche) / Poste (droite), cellule = avance, ligne = base"
                 if p < len(pages) - 1 else f"{style} : glyphes laissés tels quels (sans équivalent Tabler)")
        d.text((8, 8), title, fill="black", font=f_txt)
        asc = f_src.getmetrics()[0]
        for i, r in enumerate(chunk):
            x, y = (i % cols) * cw, 30 + (i // cols) * ch
            d.rectangle([x, y, x + cw - 1, y + ch - 1], outline="#dddddd")
            cell = int(f_src.getlength("M"))
            for k, f in enumerate((f_src, f_new) if r.tabler else (f_src,)):
                gx, gy = x + 12 + k * (cell * 2 + 24), y + 6
                d.rectangle([gx, gy, gx + cell, gy + int(size * 1.25)], outline="#9fd3ff")
                d.line([gx - 4, gy + asc, gx + cell + 4, gy + asc], fill="#ffb0b0")
                d.text((gx, gy), r.char, fill="black", font=f)
            label = f"{r.code:04X} {r.nerd}"
            d.text((x + 6, y + ch - 30), label[:38], fill="#555555", font=f_txt)
            d.text((x + 6, y + ch - 16), (r.tabler or "-") + (" (plein)" if r.filled else ""),
                   fill="#0b5" if r.tabler else "#c33", font=f_txt)
        name = out.with_name(f"{out.stem}-{p + 1:02d}{out.suffix}")
        img.save(name)
        names.append(str(name))
    print("\n".join(names))


# ------------------------------------------------------------------

def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--family", default=SOURCE_FAMILY, help=f"famille source (défaut : {SOURCE_FAMILY})")
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("build")
    sub.add_parser("check")
    g = sub.add_parser("glyphs-js")
    g.add_argument("--check", action="store_true")
    sub.add_parser("scan")
    s = sub.add_parser("sheet")
    s.add_argument("out")
    s.add_argument("--style", default="Regular")
    s.add_argument("--size", type=int, default=40)
    s.add_argument("--per-page", type=int, default=60)
    args = ap.parse_args()
    fn = {"build": cmd_build, "check": cmd_check, "glyphs-js": cmd_glyphs_js,
          "scan": cmd_scan, "sheet": cmd_sheet}[args.cmd]
    sys.exit(fn(args) or 0)


if __name__ == "__main__":
    main()
