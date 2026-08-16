#!/usr/bin/env python3
"""One-off Stage 2 TIN sitting: official crop beside the corrected comb.

Serves on 127.0.0.1:4191 (leave :4190 for `just review-serve` on forms/).
Crops and the page live under tmp/tin-stage2-sitting/ (gitignored). Verdicts
are stored in localStorage until the user copies them; nothing is committed
as an approval.

    python3 tools/formgen/review/serve_tin_sitting.py
"""
from __future__ import annotations

import argparse
import hashlib
import http.server
import json
import os
import pathlib
import shutil
import socketserver
import subprocess
import sys

REPO = pathlib.Path(__file__).resolve().parents[3]
OUT = REPO / "tmp" / "tin-stage2-sitting"
CROPS = OUT / "crops"
HOST = "127.0.0.1"
PORT = 4191
DPI = 144
SCALE = DPI / 72.0
PAD_PT = 10.0

SITES = [
    {
        "id": "C01",
        "title": "2550M Feb 2007 — primary TIN branch (pre-printed 000 → writable 5)",
        "form": "2550m-2007",
        "html": "/forms-corrected/2550m-2007/index.html",
        "pdf": pathlib.Path.home() / "Downloads/forms/2550M/bir2550m.pdf",
        "sha256": "9fb4101ace8c781436dac85df138a8fb9790775291affe2dada030c490d0d2b6",
        "box": (180.24, 118.80, 213.12, 134.40),
        "page_pt": (612.0, 1008.0),
        "note": "The vector '000' stays painted under the new inputs. Stage 2 does not un-print official ink. That is required, not a bug.",
    },
    {
        "id": "C02",
        "title": "0605 1999 — item 9 TIN branch (writable 3 → 5)",
        "form": "0605-1999",
        "html": "/forms-corrected/0605-1999/index.html",
        "pdf": pathlib.Path.home() / "Downloads/forms/0605/0605version1999_09.02.2022_copy.pdf",
        "sha256": "de04419766c59bf27fdeb854c0f7c3f98601900caa20630442e671e2313e536f",
        "box": (183.00, 246.12, 223.92, 265.08),
        "page_pt": (612.0, 936.0),
        "note": "Harvested inventory is 0605-v2003 against 1999 artwork — that revision gap is declared in the record.",
    },
    {
        "id": "C03",
        "title": "2551M 2002 — primary TIN branch (writable 3 → 5)",
        "form": "2551m-2002",
        "html": "/forms-corrected/2551m-2002/index.html",
        "pdf": pathlib.Path.home() / "Downloads/forms/2551M/2551m.pdf",
        "sha256": "f678be684558b8fb15a026b70a7c473f904fd07d49df64e0345fe1c0f81de71e",
        "box": (174.00, 190.80, 207.36, 210.24),
        "page_pt": (612.0, 1008.0),
        "note": "No harvested fields.json. Authority is the 2026-08-15 3-3-3-5 rule plus an honest in-repo gap.",
    },
    {
        "id": "C04",
        "title": "2553 1999 — item 6 TIN branch (writable 3 → 5)",
        "form": "extra/2553-1999",
        "html": "/forms-corrected/extra/2553-1999/index.html",
        "pdf": pathlib.Path.home() / "Downloads/forms/2553v1999/42792553.pdf",
        "sha256": "e52f96fe48aba2890078f889930744a4e13a4defe1284aa9c5292e2c702a20e5",
        "box": (172.08, 189.36, 205.44, 208.80),
        "page_pt": (612.0, 1008.0),
        "note": None,
    },
    {
        "id": "C05",
        "title": "1600WP 2010 — item 5 PRIMARY TIN branch (writable 4 → 5)",
        "form": "extra/1600wp-2010",
        "html": "/forms-corrected/extra/1600wp-2010/index.html",
        "pdf": pathlib.Path.home() / "Downloads/forms/1600WPv2010/1600WP p1ENCS.pdf",
        "sha256": "6ea2ef0f6c84a68ef1c50ad63f4ff0e95a68258f52b62b98f305c861c8b75d55",
        "box": (218.52, 139.20, 276.00, 156.72),
        "page_pt": (612.0, 936.0),
        "note": "Must ship with C06 (agent TIN on the same page). Do not approve one without the other.",
    },
    {
        "id": "C06",
        "title": "1600WP 2010 — AGENT TIN tail (writable 4 → 5)",
        "form": "extra/1600wp-2010",
        "html": "/forms-corrected/extra/1600wp-2010/index.html",
        "pdf": pathlib.Path.home() / "Downloads/forms/1600WPv2010/1600WP p1ENCS.pdf",
        "sha256": "6ea2ef0f6c84a68ef1c50ad63f4ff0e95a68258f52b62b98f305c861c8b75d55",
        "box": (366.48, 465.96, 401.28, 479.76),
        "page_pt": (612.0, 936.0),
        "note": "Neighbouring TIN groups are text, not combs. No harvested agent-branch field_key.",
    },
    {
        "id": "C07",
        "title": "1604CF 2008 — primary TIN branch (writable 4 → 5)",
        "form": "extra/1604cf-2008",
        "html": "/forms-corrected/extra/1604cf-2008/index.html",
        "pdf": pathlib.Path.home() / "Downloads/forms/1604CF/1604-CF July 2008 ENCS final.pdf",
        "sha256": "877fbeee071752b2d9af72924647196e6dafa71a2412e74bc9f17897767cc2e7",
        "box": (204.24, 129.84, 250.80, 144.72),
        "page_pt": (612.0, 1008.0),
        "note": "Artwork prints four compartments with '000' in the first three and a blank fourth. No harvested fields.json.",
    },
]


def sha256(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def crop_official(site: dict) -> None:
    pdf = site["pdf"]
    digest = sha256(pdf)
    if digest != site["sha256"]:
        raise SystemExit(f"{site['id']}: {pdf} sha256 {digest} != {site['sha256']}")
    x0, y0, x1, y1 = site["box"]
    x = max(0, int(round((x0 - PAD_PT) * SCALE)))
    y = max(0, int(round((y0 - PAD_PT) * SCALE)))
    w = int(round((x1 - x0 + 2 * PAD_PT) * SCALE))
    h = int(round((y1 - y0 + 2 * PAD_PT) * SCALE))
    prefix = CROPS / site["id"]
    subprocess.run(
        ["pdftocairo", "-png", "-r", str(DPI), "-f", "1", "-l", "1",
         "-x", str(x), "-y", str(y), "-W", str(w), "-H", str(h),
         str(pdf), str(prefix)],
        check=True,
    )
    produced = prefix.parent / f"{prefix.name}-1.png"
    dest = CROPS / f"{site['id']}.png"
    produced.replace(dest)


def clip_style(site: dict) -> str:
    x0, y0, x1, y1 = site["box"]
    page_w, page_h = site["page_pt"]
    width = (x1 - x0 + 2 * PAD_PT) * SCALE
    height = (y1 - y0 + 2 * PAD_PT) * SCALE
    left = (x0 - PAD_PT) * SCALE
    top = (y0 - PAD_PT) * SCALE
    return (
        f"--clip-w:{width:.1f}px;--clip-h:{height:.1f}px;"
        f"--page-w:{page_w * SCALE:.1f}px;--page-h:{page_h * SCALE:.1f}px;"
        f"--shift-x:{-left:.1f}px;--shift-y:{-top:.1f}px"
    )


def write_page() -> None:
    cards = []
    for site in SITES:
        note = f'<p class="note">{site["note"]}</p>' if site["note"] else ""
        cards.append(f"""
<section class="site" data-id="{site['id']}">
  <h2>{site['id']} — {site['title']}</h2>
  {note}
  <div class="pair">
    <figure>
      <img src="crops/{site['id']}.png" alt="official {site['id']} crop">
      <figcaption>Official artwork (old printed shape)</figcaption>
    </figure>
    <figure>
      <div class="clip" style="{clip_style(site)}">
        <iframe title="corrected {site['id']}" src="{site['html']}"></iframe>
      </div>
      <figcaption>Corrected render (5 writable slots, same outer box)</figcaption>
    </figure>
  </div>
  <fieldset>
    <legend>Your verdict (recorded verbatim)</legend>
    <label><input type="radio" name="{site['id']}" value="approve"> Approve</label>
    <label><input type="radio" name="{site['id']}" value="reject"> Reject</label>
    <textarea name="{site['id']}-notes" rows="3" placeholder="verbatim notes, required on reject"></textarea>
  </fieldset>
</section>
""")
    html = f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>TIN Stage 2 sitting — 7 sites</title>
<style>
  body {{ font: 15px/1.4 ui-sans-serif, system-ui, sans-serif; margin: 24px; color: #111; }}
  h1 {{ font-size: 22px; }}
  .law {{ background: #fff7e0; border: 1px solid #e0c060; padding: 12px 16px; max-width: 920px; }}
  .site {{ border-top: 2px solid #222; margin-top: 32px; padding-top: 16px; }}
  .pair {{ display: flex; gap: 24px; flex-wrap: wrap; align-items: flex-start; }}
  figure {{ margin: 0; }}
  figcaption {{ font-size: 13px; color: #444; margin-top: 6px; }}
  img {{ display: block; background: #fff; border: 1px solid #ccc; }}
  .clip {{ width: var(--clip-w); height: var(--clip-h); overflow: hidden; border: 1px solid #ccc; background: #fff; position: relative; }}
  .clip iframe {{ border: 0; position: absolute; left: var(--shift-x); top: var(--shift-y); width: var(--page-w); height: var(--page-h); }}
  .note {{ background: #eef4ff; border-left: 4px solid #245; padding: 8px 12px; }}
  fieldset {{ margin-top: 12px; max-width: 640px; }}
  textarea {{ width: 100%; margin-top: 8px; }}
  #dump {{ width: 100%; min-height: 160px; font-family: ui-monospace, monospace; }}
</style>
</head>
<body>
<h1>TIN Stage 2 sitting — 7 census sites</h1>
<p class="law">Non-regression only. These corrected combs <strong>diverge from the official artwork on purpose</strong>.
Approve means “five writable slots inside the same printed box, nothing else moved”.
Reject names what is wrong. Status stays <code>declared</code> until you approve.
C05 and C06 are one form: do not split them.</p>
{''.join(cards)}
<h2>Recorded verdicts</h2>
<p>Stored in this browser until you copy them into the evidence notes.</p>
<textarea id="dump" readonly></textarea>
<button type="button" id="copy">Copy JSON</button>
<script>
const ids = {json.dumps([s["id"] for s in SITES])};
const key = "tin-stage2-sitting-verdicts";
function read() {{
  const out = {{}};
  for (const id of ids) {{
    const verdict = (document.querySelector('input[name="'+id+'"]:checked') || {{}}).value || null;
    const notes = (document.querySelector('textarea[name="'+id+'-notes"]') || {{}}).value || "";
    out[id] = {{verdict, notes}};
  }}
  return out;
}}
function writeDump() {{
  const payload = {{reviewed_by: "Uriah", sitting: "TIN Stage 2", url: location.href, verdicts: read()}};
  document.getElementById("dump").value = JSON.stringify(payload, null, 2);
  localStorage.setItem(key, document.getElementById("dump").value);
}}
function restore() {{
  const raw = localStorage.getItem(key);
  if (!raw) return;
  try {{
    const parsed = JSON.parse(raw);
    for (const [id, row] of Object.entries(parsed.verdicts || {{}})) {{
      if (row.verdict) {{
        const input = document.querySelector('input[name="'+id+'"][value="'+row.verdict+'"]');
        if (input) input.checked = true;
      }}
      const ta = document.querySelector('textarea[name="'+id+'-notes"]');
      if (ta && row.notes) ta.value = row.notes;
    }}
  }} catch (e) {{}}
}}
document.querySelectorAll("input, textarea").forEach(el => el.addEventListener("input", writeDump));
document.getElementById("copy").addEventListener("click", () => {{
  writeDump();
  navigator.clipboard.writeText(document.getElementById("dump").value);
}});
restore();
writeDump();
</script>
</body>
</html>
"""
    (OUT / "index.html").write_text(html, encoding="utf-8")


class Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=str(REPO), **kwargs)

    def log_message(self, fmt: str, *args: object) -> None:
        sys.stderr.write("%s - %s\n" % (self.address_string(), fmt % args))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--port", type=int, default=PORT)
    parser.add_argument("--skip-crops", action="store_true")
    args = parser.parse_args()
    if not (REPO / "forms-corrected").is_dir():
        sys.exit("forms-corrected/ is missing; apply the ledger first")
    OUT.mkdir(parents=True, exist_ok=True)
    CROPS.mkdir(parents=True, exist_ok=True)
    if not args.skip_crops:
        for site in SITES:
            print(f"crop {site['id']} from {site['pdf'].name}", file=sys.stderr)
            crop_official(site)
    write_page()
    sitting = OUT / "index.html"
    class SittingHandler(Handler):
        def translate_path(self, path: str) -> str:
            if path in ("", "/", "/index.html"):
                return str(sitting)
            if path.startswith("/crops/"):
                return str(CROPS / path[len("/crops/"):])
            return super().translate_path(path)

    print(f"sitting: http://{HOST}:{args.port}/", file=sys.stderr)
    socketserver.TCPServer.allow_reuse_address = True
    with socketserver.TCPServer((HOST, args.port), SittingHandler) as httpd:
        httpd.serve_forever()
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except KeyboardInterrupt:
        raise SystemExit(0)
