"""Emit the Rust advance-width tables of the bundled print fonts.

Reads html-frozen/fonts/arimo-latin-wght-normal.woff2 (default instance,
wght 400) and tinos-latin-400-normal.woff2 with fontTools and prints, for
each, every character its cmap maps from U+0020 up with that glyph's
advance width (hmtx), sorted by code point, as the `(u32, u16)` slices
crates/bir-print/src/frozen_html.rs holds (ARIMO_ADVANCES, TINOS_ADVANCES).
tools/test_font_advances.mjs checks the Rust tables against the font files
on every `just shell-ui-test` without fontTools.

    python3 tools/font_advances.py
"""

from pathlib import Path

from fontTools.ttLib import TTFont

ROOT = Path(__file__).resolve().parent.parent
FONTS = [
    ("ARIMO", "arimo-latin-wght-normal.woff2"),
    ("TINOS", "tinos-latin-400-normal.woff2"),
]

for name, file in FONTS:
    font = TTFont(ROOT / "html-frozen" / "fonts" / file)
    cmap = font.getBestCmap()
    hmtx = font["hmtx"].metrics
    rows = [(cp, hmtx[glyph][0]) for cp, glyph in sorted(cmap.items()) if cp >= 0x20]
    print(f"/// {file}: units per em {font['head'].unitsPerEm}; every character its")
    print("/// cmap maps from U+0020 up, with its advance width (`hmtx`), by code point.")
    print(f"const {name}_ADVANCES: &[(u32, u16)] = &[")
    for cp, advance in rows:
        print(f"    (0x{cp:04X}, {advance}),")
    print("];")
