# Map glyphs

The signed-distance-field glyphs MapLibre draws the review's map labels with, so the map speaks Inter like the rest of
the app. Bundled, never fetched: the map style asks for `/glyphs/{fontstack}/{range}.pbf` with the fontstacks
`inter-400` and `inter-500`.

| Directory | What it is |
|---|---|
| `inter-400/` | Inter at `wght` 400, instanced from `design/fonts/Inter-Variable.woff2` (its only axis is `wght`; no `opsz`) |
| `inter-500/` | The same at `wght` 500 |

Each `<start>-<end>.pbf` is one 256-codepoint range in MapLibre's glyph protobuf format (24px SDF, 3px buffer), the
fontstack inside named `inter-400 Regular` / `inter-500 Regular` (family and style; MapLibre keys by the URL). The
bundled Inter is a Latin subset (230 codepoints), so only the 8 ranges it covers are kept: 0–1023 (Latin, Latin-1,
Latin Extended, spacing and combining marks), 8192–8959 (punctuation, letterlike symbols, arrows, math) and
65024–65279 (the BOM). A label character outside them renders blank. Generated 2026-10-19 with fontTools 4.66.1 and
`build_pbf_glyphs` 1.5.1.

To regenerate, from the repository root (scratch stays in `/tmp`):

```sh
python3 -m venv /tmp/glyphs/venv && /tmp/glyphs/venv/bin/pip install fonttools==4.66.1 brotli
cargo install build_pbf_glyphs --version 1.5.1 --root /tmp/glyphs/cargo
mkdir -p /tmp/glyphs/ttf && /tmp/glyphs/venv/bin/python - <<'EOF'
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont
for w in (400, 500):
    f = TTFont("design/fonts/Inter-Variable.woff2")
    instantiateVariableFont(f, {"wght": w}, inplace=True)
    f.flavor = None
    for r in f["name"].names:
        if r.nameID in (1, 4, 6): r.string = f"inter-{w}"
        elif r.nameID == 2: r.string = "Regular"
    f["name"].names = [r for r in f["name"].names if r.nameID not in (16, 17)]
    f.save(f"/tmp/glyphs/ttf/inter-{w}.ttf")
EOF
/tmp/glyphs/cargo/bin/build_pbf_glyphs /tmp/glyphs/ttf /tmp/glyphs/out
rm -rf app/static/glyphs/inter-400 app/static/glyphs/inter-500 && /tmp/glyphs/venv/bin/python - <<'EOF'
import os, shutil
from fontTools.ttLib import TTFont
for name in ("inter-400", "inter-500"):
    os.makedirs(f"app/static/glyphs/{name}")
    for b in sorted({c // 256 for c in TTFont(f"/tmp/glyphs/ttf/{name}.ttf").getBestCmap()}):
        fn = f"{b * 256}-{b * 256 + 255}.pbf"
        shutil.copy(f"/tmp/glyphs/out/{name}/{fn}", f"app/static/glyphs/{name}/{fn}")
EOF
rm -rf /tmp/glyphs
```

License: SIL Open Font License 1.1, as Inter itself ([`design/fonts/Inter-OFL.txt`](../../../design/fonts/Inter-OFL.txt)). These are glyph bitmaps derived
from the font, distributed with the app under the same license.
