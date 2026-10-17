#!/usr/bin/env python3
"""Render docs/form-findings/priority.html: the fix order for the per-form
findings, grouped into pull requests, from tools/form_findings_priority.json.

    python3 tools/form_findings_priority.py

Order is by filing deadline: Q3 quarterly due in October, monthly, the rest
of Q3 quarterly, yearly, as needed. The published page lets the maintainer
star the forms they file most; stars live in the artifact's `nominations`
collection (one document per form code) and move that form into the
"Nominated" group right after the shared fixes.
"""
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from form_findings_page import CSS, RULES_URL, SECTIONS, esc

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "docs" / "form-findings"
PRIORITY = Path(__file__).resolve().parent / "form_findings_priority.json"

WAVES = [
    ("shared", "Shared fixes", "once, for all"),
    ("q3oct", "Q3 · October", "Oct 25 – 31"),
    ("monthly", "Monthly", "Nov 5 – 20"),
    ("q3nov", "Rest of Q3", "Nov 15 – 29"),
    ("yearly", "Yearly", "Jan 31 – Apr 15"),
    ("asneeded", "As needed", "any time"),
]

EXTRA_CSS = """
[hidden]{display:none!important}
:root{--w-shared:#1f4fa3;--w-q3oct:#b3261e;--w-monthly:#b45f06;--w-q3nov:#8a5a00;--w-yearly:#1d7a46;--w-asneeded:#5a6474;--star:#c98a00}
@media (prefers-color-scheme:dark){:root:not([data-theme="light"]){--w-shared:#8fb1f0;--w-q3oct:#f19a92;--w-monthly:#f0a868;--w-q3nov:#f0c46a;--w-yearly:#6fd19a;--w-asneeded:#9aa4b4;--star:#f0c46a}}
:root[data-theme="dark"]{--w-shared:#8fb1f0;--w-q3oct:#f19a92;--w-monthly:#f0a868;--w-q3nov:#f0c46a;--w-yearly:#6fd19a;--w-asneeded:#9aa4b4;--star:#f0c46a}
.road{display:grid;grid-template-columns:repeat(7,minmax(0,1fr));gap:6px}
.stop{display:flex;flex-direction:column;gap:2px;padding:12px 10px;border-radius:12px;background:var(--panel);border:1px solid var(--line);border-top:6px solid var(--w);min-width:0}
.stop .n{font:800 28px var(--body);color:var(--w);line-height:1}.stop b{font-size:14px}.stop span{font:500 11.5px var(--mono);color:var(--ink-2)}
.stop.star{--w:var(--star)}
.arrow{font:600 12px var(--mono);color:var(--ink-2)}
.pr{background:var(--panel);border:1px solid var(--line);border-left:6px solid var(--w);border-radius:14px;padding:16px;display:flex;flex-direction:column;gap:10px}
.pr .top{display:flex;flex-wrap:wrap;align-items:baseline;gap:10px}
.pr .id{font:800 22px var(--body);color:var(--w)}.pr h2{font-size:18px}
.pr .why{color:var(--ink-2)}
.pr .branch{display:flex;flex-wrap:wrap;gap:6px;align-items:center;font-size:12.5px;color:var(--ink-2)}
.pr .branch code{background:var(--muted);padding:2px 6px;border-radius:6px;word-break:break-all}
.copy{font:600 11px var(--mono);border:1px solid var(--line);background:var(--muted);color:var(--ink);border-radius:999px;padding:2px 10px;cursor:pointer}
.forms{list-style:none;margin:0;padding:0;display:flex;flex-direction:column}
.form{display:grid;grid-template-columns:34px 92px 78px minmax(0,1fr) auto;gap:10px;align-items:center;padding:8px 0;border-top:1px solid var(--line)}
.form .code{font:600 14px var(--mono)}.form .due{font:600 12px var(--mono);color:var(--w)}
.form .worst{font-size:14px;min-width:0}.form .worst small{display:block;color:var(--ink-2);font-size:12.5px}
.form .cnt{font:600 12px var(--mono);color:var(--bad);background:var(--bad-soft);border-radius:999px;padding:2px 8px;white-space:nowrap}
.form.moved{opacity:.45}.form.moved .worst small.mv{color:var(--star);font-weight:600}
.starbtn{width:30px;height:30px;border-radius:50%;border:1px solid var(--line);background:var(--panel);color:var(--ink-2);font-size:16px;cursor:pointer;line-height:1;padding:0}
.starbtn[aria-pressed="true"]{color:var(--star);border-color:var(--star)}
.starbtn:disabled{cursor:default;opacity:.5}
.shared{display:flex;flex-direction:column;gap:6px}.shared div{display:flex;gap:10px;align-items:baseline;font-size:15px}
.shared .rule{font:600 12px var(--mono);color:var(--w)}
.empty{color:var(--ink-2);font-size:14px;padding:6px 0}
.status{font-size:12.5px;color:var(--ink-2)}
@media (max-width:760px){.road{grid-template-columns:repeat(2,minmax(0,1fr))}
.form{grid-template-columns:34px minmax(0,1fr) auto;grid-template-areas:"s c n" "s d d" "s w w"}
.form .starcell{grid-area:s;align-self:start}.form .code{grid-area:c}.form .cnt{grid-area:n}.form .due{grid-area:d}.form .worst{grid-area:w}}
"""

SCRIPT = """
<script>
(function(){
  const wanted = new Set();
  const rows = [...document.querySelectorAll('.pr:not(#nominated) .form')];
  const lane = document.getElementById('nominated');
  const list = lane.querySelector('.forms');
  const empty = lane.querySelector('.empty');
  const status = document.getElementById('star-status');
  let col = null;
  function render(){
    list.querySelectorAll('.form').forEach(n => n.remove());
    rows.forEach(row => {
      const code = row.dataset.code, on = wanted.has(code);
      row.classList.toggle('moved', on);
      const mv = row.querySelector('small.mv'); if (mv) mv.hidden = !on;
      if (on) {
        const copy = row.cloneNode(true);
        copy.classList.remove('moved');
        copy.querySelector('small.mv').textContent = 'From ' + row.closest('.pr').dataset.id;
        copy.querySelector('small.mv').hidden = false;
        list.appendChild(copy);
      }
    });
    empty.hidden = wanted.size > 0;
    document.getElementById('star-count').textContent = wanted.size;
    document.querySelectorAll('.starbtn').forEach(b => {
      const on = wanted.has(b.closest('.form').dataset.code);
      b.setAttribute('aria-pressed', on ? 'true' : 'false');
      b.textContent = on ? '\\u2605' : '\\u2606';
      b.title = on ? 'Nominated: click to put it back' : 'I file this a lot: move it to the top';
      b.disabled = !col;
    });
  }
  document.addEventListener('click', async e => {
    const b = e.target.closest('.starbtn'); if (!b || !col) return;
    const code = b.closest('.form').dataset.code;
    b.disabled = true;
    try {
      if (wanted.has(code)) await col.doc(code).delete();
      else await col.doc(code).set({nominated: true, at: new Date().toISOString()});
    } catch (err) { status.textContent = 'Could not save that star (' + (err && err.code || 'error') + ').'; }
    b.disabled = false;
  });
  document.addEventListener('click', async e => {
    const c = e.target.closest('.copy'); if (!c) return;
    const text = c.dataset.copy;
    try { await navigator.clipboard.writeText(text); c.textContent = 'copied'; }
    catch (_) { const r = document.createRange(); r.selectNodeContents(c.previousElementSibling); const s = getSelection(); s.removeAllRanges(); s.addRange(r); }
  });
  render();
  if (!window.claude || !window.claude.use) { status.textContent = 'Stars need the claude.ai viewer. Tell Claude which forms you file most.'; return; }
  window.claude.use('db').then(db => {
    if (!db) { status.textContent = 'Sign in to star forms, or tell Claude which forms you file most.'; return; }
    col = db.collection('nominations');
    status.textContent = 'Stars are saved for Claude to read when it plans the next PR.';
    col.onSnapshot(snap => {
      wanted.clear();
      snap.docs.forEach(d => { if (d.exists) wanted.add(d.id); });
      render();
    }, err => { status.textContent = 'Stars unavailable (' + (err && err.code || 'error') + ').'; });
    render();
  });
})();
</script>
"""


def count(code):
    data = json.loads((OUT / code / "findings.json").read_text())
    dropped = data.get("dropped", {})
    n = sum(1 for key, _, _ in SECTIONS for f in data["rules"][key]["findings"] if f["id"] not in dropped)
    return n + len(data.get("manual", [])), data.get("title", "")


def form_row(f, urls):
    n, title = count(f["code"])
    note = f'<small>{esc(f["note"])}</small>' if f.get("note") else ""
    return (f'<li class="form" data-code="{esc(f["code"])}">'
            f'<span class="starcell"><button class="starbtn" type="button" aria-pressed="false" disabled>&#9734;</button></span>'
            f'<a class="code" href="{esc(urls[f["code"]])}" title="{esc(title)}">{esc(f["code"])}</a>'
            f'<span class="due">{esc(f["due"])}</span>'
            f'<span class="worst">{esc(f["worst"])}{note}<small class="mv" hidden>Moved up to Nominated</small></span>'
            f'<span class="cnt">{n} to fix</span></li>')


def main():
    manifest = json.loads((OUT / "manifest.json").read_text())
    urls = {code: e["url"] for code, e in manifest["forms"].items()}
    plan = json.loads(PRIORITY.read_text())
    prs = plan["prs"]

    per_wave = {key: [p for p in prs if p["wave"] == key] for key, _, _ in WAVES}
    road = []
    for i, (key, name, when) in enumerate(WAVES):
        forms = sum(len(p["forms"]) for p in per_wave[key])
        num = "all" if key == "shared" else str(forms)
        unit = "43 forms" if key == "shared" else ("form" if forms == 1 else "forms")
        road.append(f'<div class="stop" style="--w:var(--w-{key})"><span>step {i + 1}</span>'
                    f'<span class="n">{num}</span><span>{unit}</span><b>{esc(name)}</b><span>{esc(when)}</span></div>')
        if key == "shared":
            road.append('<div class="stop star"><span>★ starred</span><span class="n" id="star-count">0</span>'
                        '<span>forms</span><b>Your most-used</b><span>join the top</span></div>')
    cards = []
    for p in prs:
        w = f"var(--w-{p['wave']})"
        if p["wave"] == "shared":
            body = '<div class="shared">' + "".join(
                f'<div><span class="rule">{esc(s["rule"])}</span><span>{esc(s["what"])} <small class="status">({s["forms"]} forms)</small></span></div>'
                for s in p["shared"]) + "</div>"
        else:
            body = '<ul class="forms">' + "".join(form_row(f, urls) for f in p["forms"]) + "</ul>"
        codes = " ".join(f["code"] for f in p["forms"])
        copy_text = p["branch"] + (f"  # {codes}" if codes else "")
        cards.append(f'''<section class="pr" data-id="{esc(p["id"])}" style="--w:{w}">
<div class="top"><span class="id">{esc(p["id"])}</span><h2>{esc(p["name"])}</h2></div>
<p class="why">{esc(p["why"])}</p>
{body}
<div class="branch">Branch <code>{esc(p["branch"])}</code><button class="copy" type="button" data-copy="{esc(copy_text)}">copy</button></div>
</section>''')
        if p["wave"] == "shared":
            cards.append('''<section class="pr" id="nominated" data-id="Nominated" style="--w:var(--star)">
<div class="top"><span class="id">★</span><h2>Nominated: the forms you file most</h2></div>
<p class="why">Tap ☆ on any form below and it moves up here, right after the shared fixes.</p>
<ul class="forms"></ul><p class="empty">Nothing starred yet.</p>
<p class="status" id="star-status">Loading stars…</p>
</section>''')

    total = sum(count(f["code"])[0] for p in prs for f in p["forms"])
    page = f"""<title>Form Fix Order</title>
<meta name="claude:viewport-fit-sides" content="cover">
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Public+Sans:wght@400;600;800&family=IBM+Plex+Mono:wght@500;600&display=swap">
<style>{CSS}{EXTRA_CSS}</style>
<div class="wrap">
<header style="display:flex;flex-direction:column;gap:10px">
<p class="mono"><a href="{esc(manifest.get("index_url", ""))}">← All findings</a> · <a href="{RULES_URL}">The rules</a> · as of {esc(plan["as_of"])}</p>
<h1>What to fix first</h1>
<p class="lede">Fix the forms you have to file soonest. Top card first. Each card is one pull request. Tap a form code to see its findings.</p>
</header>
<div class="road">{"".join(road)}</div>
<p class="status">{esc(plan["note"])} {total} findings across 43 forms.</p>
{"".join(cards)}
</div>
{SCRIPT}"""
    (OUT / "priority.html").write_text(page)
    print(OUT / "priority.html")


if __name__ == "__main__":
    main()
