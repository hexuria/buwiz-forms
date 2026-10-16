#!/usr/bin/env python3
"""Render docs/form-findings/index.html: every form's findings per rule, with
links to the per-form pages recorded in docs/form-findings/manifest.json.

    python3 tools/form_findings_index.py

Repeated issues are listed from tools/form_findings_themes.json
({"themes": [{"rule": "R3", "what": "...", "forms": ["1701", ...]}]}), written by
hand after reviewing every form.
"""
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from form_findings_page import CSS, RULES_URL, SECTIONS, APP_WIDE, esc

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "docs" / "form-findings"
THEMES = Path(__file__).resolve().parent / "form_findings_themes.json"

EXTRA_CSS = """
.grid{overflow-x:auto;border:1px solid var(--line);border-radius:12px;background:var(--panel)}
.grid table{border-collapse:collapse;width:100%;font-size:13px}
.grid th,.grid td{padding:6px 8px;border-bottom:1px solid var(--line);text-align:center;white-space:nowrap}
.grid th{font:600 11px var(--mono);color:var(--ink-2);letter-spacing:.04em;position:sticky;top:0;background:var(--panel)}
.grid td.form{text-align:left}.grid td.form a{font:600 13px var(--mono)}
.grid td.form span{display:block;color:var(--ink-2);font-size:12px;white-space:normal;min-width:180px}
.grid td.z{color:var(--good)}.grid td.n{background:var(--bad-soft);color:var(--bad);font:600 12px var(--mono)}
.grid td.tot{font:600 13px var(--mono)}
.themes{display:flex;flex-direction:column;gap:10px;margin:0;padding:0;list-style:none}
.themes li{display:flex;flex-direction:column;gap:4px;border-top:1px solid var(--line);padding-top:10px}
.themes .codes{display:flex;flex-wrap:wrap;gap:4px}
"""


def load():
    manifest = json.loads((OUT / "manifest.json").read_text())
    forms = []
    for code, entry in sorted(manifest["forms"].items()):
        data = json.loads((OUT / code / "findings.json").read_text())
        dropped = data.get("dropped", {})
        counts = {}
        for key, _, _ in SECTIONS:
            counts[key] = sum(1 for f in data["rules"][key]["findings"] if f["id"] not in dropped)
        for m in data.get("manual", []):
            counts[m["section"]] = counts.get(m["section"], 0) + 1
        forms.append({"code": code, "title": data.get("title", ""), "url": entry["url"], "counts": counts,
                      "reviewed": data.get("reviewed", False)})
    return manifest, forms


def main():
    manifest, forms = load()
    themes = json.loads(THEMES.read_text())["themes"] if THEMES.exists() else []
    urls = {f["code"]: f["url"] for f in forms}
    total = sum(sum(f["counts"].values()) for f in forms)

    head = "".join(f'<th title="{esc(name)}">{esc(key)}</th>' for key, name, _ in SECTIONS)
    rows = []
    for f in forms:
        cells = "".join(f'<td class="n">{n}</td>' if n else '<td class="z">✓</td>'
                        for n in (f["counts"][k] for k, _, _ in SECTIONS))
        rows.append(f'<tr><td class="form"><a href="{esc(f["url"])}">{esc(f["code"])}</a><span>{esc(f["title"])}</span></td>'
                    f'{cells}<td class="tot">{sum(f["counts"].values())}</td></tr>')

    tiles = []
    for key, name, _ in SECTIONS:
        with_findings = sum(1 for f in forms if f["counts"][key])
        n = sum(f["counts"][key] for f in forms)
        tiles.append(f'<div class="tile {"findings" if n else "ok"}"><span class="k">{key}{" · all forms" if key in APP_WIDE else ""}</span>'
                     f'<b>{esc(name)}</b><span class="n">{with_findings} form(s) · {n}</span></div>')

    theme_items = "".join(
        f'<li><div class="sechead"><span class="k">{esc(t["rule"])}</span><b>{esc(t["what"])}</b></div>'
        f'<div class="codes">{"".join(f"<a class=badge href={chr(34)}{esc(urls.get(c, chr(35)))}{chr(34)}>{esc(c)}</a>" for c in t["forms"])}</div></li>'
        for t in themes)

    page = f"""<title>Form Page Findings</title>
<meta name="claude:viewport-fit-sides" content="cover">
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Public+Sans:wght@400;600;800&family=IBM+Plex+Mono:wght@500;600&display=swap">
<style>{CSS}{EXTRA_CSS}</style>
<div class="wrap">
<header style="display:flex;flex-direction:column;gap:8px">
<span class="badge" style="align-self:flex-start">{len(forms)} BIR forms · checked {esc(max(e["published"] for e in manifest["forms"].values()))}</span>
<h1>Form page findings</h1>
<p class="lede">Every form page, print preview and validation, checked against the <a href="{RULES_URL}">approved rules</a>.
{total} thing(s) to fix across {len(forms)} forms, each with its picture on the form's own page. Nothing has been changed yet.</p>
</header>
<nav class="tiles" aria-label="Rules">{"".join(tiles)}</nav>
{f'<section class="sec"><h2>What repeats across forms</h2><p class="ask">Fix these as one change each; the badges open the form pages that show them.</p><ul class="themes">{theme_items}</ul></section>' if themes else ""}
<section class="sec"><h2>Every form</h2><p class="ask">Count of things to fix per rule. Click a form code for its pictures and details.</p>
<div class="grid"><table><thead><tr><th style="text-align:left">Form</th>{head}<th>All</th></tr></thead><tbody>{"".join(rows)}</tbody></table></div></section>
</div>
"""
    (OUT / "index.html").write_text(page)
    print(OUT / "index.html")


if __name__ == "__main__":
    main()
