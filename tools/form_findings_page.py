#!/usr/bin/env python3
"""Render docs/form-findings/<CODE>/page.html from its findings.json.

Copies the form's screenshots (tmp/form-findings/shots/<CODE>/*.png, the print
render tmp/form-findings/print/<slug>.png) into docs/form-findings/<CODE>/shots/
as 900 px JPEGs and records them in findings.json `screenshots`. Manual review
fields are applied: `dropped` {id: reason} hides an automatic finding (listed
under "Checked and set aside"), `manual` [{section, what, where, fix,
evidence, shot}] adds findings seen by eye.

    python3 tools/form_findings_page.py CODE [CODE ...]
"""
import html
import json
import sys
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "docs" / "form-findings"
SHOTS = ROOT / "tmp" / "form-findings" / "shots"
PRINT = ROOT / "tmp" / "form-findings" / "print"
RULES_URL = "https://claude.ai/artifact/Wvpp9iaKx6xmfXHivio359"

SECTIONS = [
    ("R0", "Profile → form, one way", "The profile fills the return; the return never writes back."),
    ("R1", "Library opens blank", "From the Form library no year, month, quarter or date is filled."),
    ("R2", "Profile boxes: tucked away, editable", "Profile boxes collapse per section, Edit on hover, blank ones stay open."),
    ("R3", "Same order as the paper", "Every printed box, in its printed part and item, top to bottom."),
    ("R4", "Dates read like dates", "Type 12102020 or 12/10/2020, see Dec 10, 2020; pick month and year."),
    ("R5", "Char boxes only for fixed formats", "Names, addresses and email leave the boxes when they overflow."),
    ("R6", "Locked boxes look locked", "A box that needs another answer first looks faded and says why."),
    ("R7", "Only money looks like money", "Two decimals for pesos only; codes and numbers stay as typed."),
    ("R8", "Penalties: computed, overridable", "Computed by default; one Edit unlocks surcharge, interest, compromise."),
    ("preview", "Preview = page", "What the page holds is what the preview prints, in the right box."),
    ("validation", "Messages land on boxes", "Every official message sits under the box it is about."),
]
APP_WIDE = {"R0", "R1", "R2", "R4", "R6"}

CSS = """
:root{--bg:#f3f5f8;--panel:#fff;--muted:#eef1f5;--paper:#fffdf6;--paper-line:#d9d2bd;--ink:#18202c;--ink-2:#5a6474;--line:#dde2ea;
--accent:#1f4fa3;--accent-soft:#e4ecfa;--good:#1d7a46;--good-soft:#e1f3e8;--bad:#b3261e;--bad-soft:#fbe5e3;--warn:#8a5a00;--warn-soft:#fbefd6;
--body:"Public Sans","Helvetica Neue",Arial,sans-serif;--mono:"IBM Plex Mono",ui-monospace,Menlo,monospace}
@media (prefers-color-scheme:dark){:root:not([data-theme="light"]){--bg:#11151c;--panel:#181d26;--muted:#1f2530;--paper:#1d1b16;--paper-line:#4a4436;--ink:#e8ecf2;--ink-2:#9aa4b4;--line:#2c3442;
--accent:#8fb1f0;--accent-soft:#1d2b46;--good:#6fd19a;--good-soft:#15301f;--bad:#f19a92;--bad-soft:#3a1a18;--warn:#f0c46a;--warn-soft:#3a2c10;color-scheme:dark}}
:root[data-theme="dark"]{--bg:#11151c;--panel:#181d26;--muted:#1f2530;--paper:#1d1b16;--paper-line:#4a4436;--ink:#e8ecf2;--ink-2:#9aa4b4;--line:#2c3442;
--accent:#8fb1f0;--accent-soft:#1d2b46;--good:#6fd19a;--good-soft:#15301f;--bad:#f19a92;--bad-soft:#3a1a18;--warn:#f0c46a;--warn-soft:#3a2c10;color-scheme:dark}
*{box-sizing:border-box}body{background:var(--bg);color:var(--ink);font-family:var(--body);font-size:15px;line-height:1.45}
.wrap{max-width:1040px;margin:0 auto;padding-block:28px 64px;padding-left:max(16px,env(safe-area-inset-left,0px));padding-right:max(16px,env(safe-area-inset-right,0px));display:flex;flex-direction:column;gap:22px}
h1{font-weight:800;font-size:clamp(26px,4.4vw,38px);line-height:1.1;margin:0;text-wrap:balance}h2{margin:0;font-size:19px;text-wrap:balance}h3{margin:0;font-size:15px}
p{margin:0}.lede{color:var(--ink-2);max-width:66ch}.mono,code{font-family:var(--mono);font-size:12.5px}
a{color:var(--accent)}
.tiles{display:grid;grid-template-columns:repeat(auto-fill,minmax(150px,1fr));gap:8px}
.tile{display:flex;flex-direction:column;gap:4px;padding:10px 12px;border-radius:12px;border:1px solid var(--line);background:var(--panel);text-decoration:none;color:var(--ink);min-width:0}
.tile .k{font:600 11px var(--mono);color:var(--ink-2);letter-spacing:.05em}.tile b{font-size:14px}
.tile.ok{background:var(--good-soft)}.tile.findings{background:var(--bad-soft)}.tile .n{font:600 13px var(--mono)}
.sec{background:var(--panel);border:1px solid var(--line);border-radius:16px;padding:18px;display:flex;flex-direction:column;gap:12px}
.sechead{display:flex;flex-wrap:wrap;gap:8px;align-items:center}.sechead .k{font:600 12px var(--mono);color:var(--accent)}
.badge{font:600 11px var(--mono);padding:2px 8px;border-radius:999px;background:var(--accent-soft);color:var(--accent);white-space:nowrap}
.badge.good{background:var(--good-soft);color:var(--good)}.badge.bad{background:var(--bad-soft);color:var(--bad)}.badge.warn{background:var(--warn-soft);color:var(--warn)}
.ask{color:var(--ink-2)}
.card{display:grid;grid-template-columns:minmax(0,1fr) minmax(0,300px);gap:14px;border-top:1px solid var(--line);padding-top:12px}
.card.noshot{grid-template-columns:minmax(0,1fr)}
.card .body{display:flex;flex-direction:column;gap:6px;min-width:0}.card .what{font-size:15.5px;font-weight:600}
.card .row{font-size:13px;color:var(--ink-2)}.card .row b{color:var(--ink);font-weight:600}
.card img{width:100%;max-width:100%;border:1px solid var(--line);border-radius:8px;display:block}
details summary{cursor:pointer;color:var(--ink-2);font-size:13px}details .keys{font:500 12px var(--mono);color:var(--ink-2);word-break:break-all;margin-top:4px}
.gallery{display:grid;grid-template-columns:repeat(auto-fill,minmax(220px,1fr));gap:10px}.gallery figure{margin:0;display:flex;flex-direction:column;gap:4px}
.gallery img{width:100%;border:1px solid var(--line);border-radius:8px}.gallery figcaption{font:500 12px var(--mono);color:var(--ink-2)}
.print img{width:100%;max-width:560px;border:1px solid var(--paper-line);border-radius:4px;display:block}
.notes{background:var(--paper);border:1px solid var(--paper-line);border-radius:12px;padding:14px;white-space:pre-wrap}
@media (max-width:760px){.card{grid-template-columns:1fr}}
"""


def esc(s):
    return html.escape(str(s or ""))


def jpeg(src, dst, width=900, crop=None):
    dst.parent.mkdir(parents=True, exist_ok=True)
    im = Image.open(src).convert("RGB")
    if crop:
        im = im.crop(crop(im))
    if im.width > width:
        im = im.resize((width, round(im.height * width / im.width)), Image.LANCZOS)
    im.save(dst, "JPEG", quality=72, optimize=True)


def copy_shots(code, slug):
    d = OUT / code / "shots"
    names = []
    def order(png):
        pi, a = png.stem[1:].split("-")
        return int(pi), int(a)

    for png in sorted((SHOTS / code).glob("p*.png"), key=order):
        out = d / (png.stem + ".jpg")
        if not out.exists() or out.stat().st_mtime < png.stat().st_mtime:
            # Drop the sidebar and the form's title band: the solo part's
            # card never starts above 23% of the window (shortest header).
            jpeg(png, out, crop=lambda im: (round(im.width * 0.244), round(im.height * 0.23), im.width, im.height))
        names.append("shots/" + out.name)
    pp = PRINT / f"{slug}.png"
    prints = []
    if pp.exists():
        for old in d.glob("print-*.jpg"):
            old.unlink()
        out = d / "print.jpg"
        jpeg(pp, out)
        prints.append("shots/" + out.name)
    return names, prints


def render(code):
    path = OUT / code / "findings.json"
    data = json.loads(path.read_text())
    shots, prints = copy_shots(code, data["slug"])
    data["screenshots"] = shots + prints
    path.write_text(json.dumps(data, indent=1, ensure_ascii=False) + "\n")
    dropped = data.get("dropped", {})
    manual = data.get("manual", [])
    parts = data.get("parts", [])
    shotset = set(shots)

    def shot_img(name):
        rel = f"shots/{name}.jpg"
        return (f'<a href="{rel}" target="_blank" rel="noopener"><img src="{rel}" alt="page screenshot {esc(name)}" loading="lazy"></a>'
                if rel in shotset else "")

    tiles, secs = [], []
    for key, name, ask in SECTIONS:
        sec = data["rules"][key]
        items = [f for f in sec["findings"] if f["id"] not in dropped]
        items += [dict(m, id=f"{key}.m{i}", manual=True) for i, m in enumerate(manual) if m.get("section") == key]
        status = "findings" if items else ("n/a" if sec["status"] == "n/a" else "ok")
        n = len(items)
        tiles.append(f'<a class="tile {status}" href="#{key}"><span class="k">{key}</span><b>{esc(name)}</b>'
                     f'<span class="n">{"✓ ok" if status == "ok" else ("n/a" if status == "n/a" else f"{n} to fix")}</span></a>')
        cards = []
        for f in items:
            img = shot_img(f["shot"]) if f.get("shot") else ""
            keys = f.get("keys") or []
            tag = '<span class="badge warn">seen by eye</span> ' if f.get("manual") else ""
            cards.append(
                f'<div class="card{"" if img else " noshot"}"><div class="body">'
                f'<p class="what">{tag}{esc(f["what"])}</p>'
                + (f'<p class="row"><b>Where</b> {esc(f["where"])}</p>' if f.get("where") else "")
                + (f'<p class="row"><b>Change</b> {esc(f["fix"])}</p>' if f.get("fix") else "")
                + f'<p class="row"><b>Evidence</b> <span class="mono">{esc(f["evidence"])}</span></p>'
                + (f'<details><summary>{len(keys)} box key(s)</summary><div class="keys">{esc(", ".join(keys))}</div></details>' if keys else "")
                + f'</div>{img}</div>')
        if not items:
            cards.append(f'<p class="ask">✓ {esc(sec.get("summary") or "Nothing to fix.")}</p>')
        aside = [f'<li><span class="mono">{esc(i)}</span> {esc(r)}</li>' for i, r in dropped.items() if i.startswith(key + ".")]
        scope = '<span class="badge">fix once, all forms</span>' if key in APP_WIDE and items else ""
        secs.append(
            f'<section class="sec" id="{key}"><div class="sechead"><span class="k">{key}</span><h2>{esc(name)}</h2>'
            f'<span class="badge {"bad" if items else "good"}">{f"{n} to fix" if items else "ok"}</span>{scope}</div>'
            f'<p class="ask">{esc(ask)}</p>{"".join(cards)}'
            + (f'<details><summary>Checked and set aside ({len(aside)})</summary><ul>{"".join(aside)}</ul></details>' if aside else "")
            + "</section>")
    gallery = "".join(f'<figure><a href="{s}" target="_blank" rel="noopener"><img src="{s}" alt="{esc(s)}" loading="lazy"></a><figcaption>{esc(part_caption(s, parts))}</figcaption></figure>' for s in shots)
    printed = "".join(f'<img src="{p}" alt="print preview page {i + 1}" loading="lazy">' for i, p in enumerate(prints))
    total = sum(1 for key, _, _ in SECTIONS for f in data["rules"][key]["findings"] if f["id"] not in dropped) + len(manual)
    notes = data.get("review_notes", "").strip()
    page = f"""<title>{esc(code)} Form Findings</title>
<meta name="claude:viewport-fit-sides" content="cover">
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Public+Sans:wght@400;600;800&family=IBM+Plex+Mono:wght@500;600&display=swap">
<style>{CSS}</style>
<div class="wrap">
<header style="display:flex;flex-direction:column;gap:8px">
<span class="badge" style="align-self:flex-start">BIR Form {esc(code)} · revision {esc(data.get("revision"))}</span>
<h1>{esc(code)} findings</h1>
<p class="lede">{esc(data.get("title"))}. Checked against the <a href="{RULES_URL}">approved rules</a>: the form page, the print preview and validation. {total} thing(s) to fix. Nothing has been changed yet.</p>
</header>
<nav class="tiles" aria-label="Rules">{"".join(tiles)}</nav>
{f'<section class="sec"><h2>Notes from the hand review</h2><div class="notes">{esc(notes)}</div></section>' if notes else ""}
{"".join(secs)}
<section class="sec"><h2>Print preview (worst-case filling)</h2><p class="ask">Longest names, full combs, largest amounts, from the same print pipeline the app uses.</p><div class="print" style="display:flex;flex-direction:column;gap:10px">{printed or '<p class="ask">No render.</p>'}</div></section>
<section class="sec"><h2>Every part of the form page</h2><p class="ask">Debug app, synthetic taxpayer 123-456-788, five items per picture.</p><div class="gallery">{gallery}</div></section>
</div>
"""
    (OUT / code / "page.html").write_text(page)
    return OUT / code / "page.html", data["screenshots"]


def part_caption(s, parts):
    stem = s.split("/")[-1].split(".")[0]
    try:
        pi, a = stem[1:].split("-")
        p = parts[int(pi)]
        return f"{p['n']} · items {int(a) + 1}–{min(int(a) + 5, p['items'])}"
    except (ValueError, IndexError, KeyError):
        return stem


if __name__ == "__main__":
    for code in sys.argv[1:]:
        page, shots = render(code)
        print(page, len(shots))
