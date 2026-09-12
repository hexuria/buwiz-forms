#!/usr/bin/env python3
"""Draw one ELI5 page per BIR form from rules/forms/<id>/{fields,calculations,validations}.json
and html-frozen/<bundle>/index.html. The page is the field-for-field spec the app page must show.

usage: form_page.py <rules-id> <frozen-bundle> <out.html>   e.g. 2551q-v2018 2551q-2018 crates/bir-desktop/docs/eli5/forms/2551Q.html
"""
import json, sys, html, re, collections, pathlib

ROOT = next(p for p in pathlib.Path(__file__).resolve().parents if (p / "rules" / "forms").is_dir())
rules_id, bundle, out = sys.argv[1], sys.argv[2], sys.argv[3]
R = ROOT / "rules/forms" / rules_id
F = json.load(open(R / "fields.json"))
C = json.load(open(R / "calculations.json"))
V = json.load(open(R / "validations.json"))
H = (ROOT / "html-frozen" / bundle / "index.html").read_text()
title = re.search(r"<title>([^<]+)</title>", H).group(1)

# ---- paper-form structure: sections and item labels (per form; 2551Q v2018 here) ----
SECTIONS = {
    "2551q-v2018": [
        ("Part I — Background Information", 1, ["1","2","3","4","5","6","7","8","9","10","11","12","13"]),
        ("Part II — Total Tax Payable", 1, ["14","15","16","17","18","19","20","21","22","23","24"]),
        ("Part III — Details of Payment + signature", 1, ["None"]),
        ("Page 2 header (auto-copied)", 2, ["None:hdr"]),
        ("Schedule 1 — Computation of Tax", 2, ["None:sched"]),
    ],
}
LABELS = {
    "2551q-v2018": {
        "1":"For the: Calendar / Fiscal","2":"Year ended (MM/YYYY)","3":"Quarter","4":"Amended return?","5":"Number of sheets attached",
        "6":"TIN","7":"RDO code","8":"Taxpayer's name","9":"Registered address + ZIP","10":"Contact number","11":"Email address",
        "12":"Tax relief under special law / treaty? (+ specify)","13":"Income-tax-rate election (graduated / 8%)",
        "14":"Total tax due (from Schedule 1 item 7)","15":"Creditable percentage tax withheld (2307)","16":"Tax paid in previously filed return (amended)",
        "17":"Other tax credit / payment (specify)","18":"Total tax credits / payments (15+16+17)","19":"Tax still payable / (overpayment) (14−18)",
        "20":"Surcharge","21":"Interest","22":"Compromise","23":"Total penalties (20+21+22)","24":"Total amount payable / (overpayment) (19+23)",
        "None":"Overpayment choice · tax agent accreditation · payment rows 25–28",
        "None:hdr":"TIN + taxpayer name repeated on page 2","None:sched":"Six ATC lines (ATC, taxable amount, rate, tax due) + item 7 total",
    },
}
SRC = {  # who fills a field: profile / template / typed / computed / period
    "profile": re.compile(r"TIN|BranchCode|registeredName|registeredAddress|zipCode|telNo|txtEmail|txtRDOCode|TaxpayerName|LineofBus"),
    "period":  re.compile(r"forThe_|rtnMonth|txtYear|qtr_"),
}
def source_of(x):
    k = x["serialized_key"].split(":")[-1]
    if x["required"] == "computed": return "computed"
    if SRC["profile"].search(k): return "profile"
    if SRC["period"].search(k): return "period"
    return "typed"
KIND = {"text":"text / money","radio":"radio","select-one":"select"}

def sub_group(x):
    k = x["serialized_key"].split(":")[-1]
    if x["page"] == 2 and str(x["item_number"]) == "None":
        return "None:hdr" if k.startswith("txtPg2") else "None:sched"
    return str(x["item_number"])

groups = collections.defaultdict(list)
for x in F["fields"]:
    groups[(x["page"], sub_group(x))].append(x)
# page-2 items 7 (RDO) and 11 (email) belong to Part I on paper
for (p, i), xs in list(groups.items()):
    if p == 2 and i in ("7", "11"):
        groups[(1, i)] += xs; del groups[(p, i)]

drawn = 0
sections_html = []
for name, page, items in SECTIONS[rules_id]:
    rows = []
    for it in items:
        xs = groups.get((page, it), [])
        if not xs: continue
        label = LABELS[rules_id].get(it, it)
        chips = []
        for x in xs:
            k = x["serialized_key"].split(":")[-1]; drawn += 1
            req = {"required":"req","optional":"opt","conditional":"cond","computed":"calc"}[x["required"]]
            cond = x["required_when"] or x["visible_when"] or x["enabled_when"] or ""
            occ = f" · p{x['page']} copy" if x.get("serialized_occurrence", 1) > 1 else ""
            chips.append(f'<span class="f {req} src-{source_of(x)}" title="{html.escape(cond)}">{html.escape(k)}<small>{KIND.get(x["control_kind"], x["control_kind"])}{occ}</small></span>')
        num = it.split(":")[0]
        rows.append(f'<div class="item"><div class="n">{html.escape(num if num!="None" else "·")}</div><div class="l">{html.escape(label)}</div><div class="chips">{"".join(chips)}</div></div>')
    sections_html.append(f'<details open class="sec"><summary>{html.escape(name)} <em>page {page} · {sum(len(groups.get((page,i),[])) for i in items)} fields</em></summary>{"".join(rows)}</details>')

calcs = []
for cid in C["evaluation_order"]:
    c = next((c for c in C["calculations"] if (c.get("id") or c.get("calculation_id")) == cid), None) if isinstance(C["calculations"], list) else C["calculations"].get(cid, {})
    ins = ", ".join(str(i).split(":")[-1] for i in (c.get("inputs") or c.get("sources") or []))
    tgt = str(c.get("target") or c.get("target_field") or c.get("output") or "").split(":")[-1]
    calcs.append(f'<div class="calc"><b>{html.escape(cid)}</b><span class="arrow">{html.escape(ins)} ⟶ {html.escape(tgt or "(see rule)")}</span><i>{html.escape(str(c.get("description") or c.get("summary") or c.get("notes") or "")[:140])}</i></div>')
rules = V["rules"] if isinstance(V["rules"], list) else list(V["rules"].values())
vals = "".join(f'<li><b>{html.escape(str(r.get("id") or r.get("rule_id")))}</b> — {html.escape(", ".join(str(f).split(":")[-1] for f in (r.get("fields") or r.get("field_keys") or [])))}<i>{html.escape(str(r.get("description") or r.get("summary") or r.get("message") or "")[:160])}</i></li>' for r in rules)

count_ok = drawn == F["field_count"]
page = f"""<!doctype html><html lang="en"><head><meta charset="utf-8"><title>ELI5 · {html.escape(title)}</title>
<style>
:root{{--bg:#0b0b0c;--card:#161618;--line:#2a2a2e;--fg:#f2f2f2;--mut:#9a9aa3;--req:#ff6b6b;--opt:#7c8cff;--calc:#4ade80;--cond:#fbbf24;--prof:#38bdf8}}
body{{margin:0;background:var(--bg);color:var(--fg);font:16px/1.4 -apple-system,Segoe UI,Helvetica,Arial,sans-serif}}
header{{padding:32px 40px 8px}} h1{{margin:0;font-size:34px}} .sub{{color:var(--mut)}}
.glance{{display:flex;gap:14px;padding:12px 40px 0;flex-wrap:wrap}} .g{{background:var(--card);border:1px solid var(--line);border-radius:12px;padding:12px 16px;min-width:140px}} .g b{{display:block;font-size:26px}} .g span{{color:var(--mut);font-size:13px}}
main{{padding:16px 40px 60px;display:grid;gap:14px}}
.sec{{background:var(--card);border:1px solid var(--line);border-radius:14px;padding:6px 18px 14px}} summary{{cursor:pointer;font-weight:700;font-size:20px;padding:10px 0}} summary em{{color:var(--mut);font-weight:400;font-size:14px;margin-left:10px;font-style:normal}}
.item{{display:grid;grid-template-columns:44px 300px 1fr;gap:10px;align-items:start;padding:8px 0;border-top:1px dashed var(--line)}} .n{{font-weight:800;font-size:20px;color:var(--mut)}} .l{{font-weight:600}}
.chips{{display:flex;flex-wrap:wrap;gap:6px}} .f{{border:1px solid var(--line);border-radius:8px;padding:3px 8px;font-size:13px;background:#1d1d21}} .f small{{display:block;color:var(--mut);font-size:11px}}
.f.req{{border-color:var(--req)}} .f.calc{{border-color:var(--calc);background:#0f2a1a}} .f.cond{{border-color:var(--cond)}} .src-profile{{box-shadow:inset 3px 0 0 var(--prof)}} .src-period{{box-shadow:inset 3px 0 0 #a78bfa}}
.legend{{display:flex;gap:16px;flex-wrap:wrap;padding:4px 40px;color:var(--mut);font-size:13px}} .legend i{{display:inline-block;width:12px;height:12px;border-radius:3px;margin-right:6px;vertical-align:-1px;border:1px solid var(--line)}}
.calc{{display:grid;grid-template-columns:230px 1fr;gap:10px;padding:8px 0;border-top:1px dashed var(--line)}} .calc .arrow{{font-family:ui-monospace,Menlo,monospace;font-size:14px;color:var(--calc)}} .calc i{{grid-column:2;color:var(--mut);font-size:13px;font-style:normal}}
ul.v{{margin:0;padding-left:18px}} ul.v li{{padding:5px 0;border-top:1px dashed var(--line)}} ul.v i{{display:block;color:var(--mut);font-size:13px;font-style:normal}}
.count{{font-size:22px;font-weight:800;color:{'var(--calc)' if count_ok else 'var(--req)'}}}
footer{{padding:16px 40px 40px;color:var(--mut);font-size:13px;border-top:1px solid var(--line)}} code{{color:#e5e5e5}}
</style></head><body>
<header><h1>{html.escape(title)}</h1><div class="sub">Every field the app page must show, in the order of the paper form. Drawn from the XML field inventory, not from the current Rust view.</div></header>
<div class="glance"><div class="g"><b>{F["field_count"]}</b><span>fields in fields.json</span></div><div class="g"><b>{drawn}</b><span>fields drawn here</span></div><div class="g"><b>{len(C["evaluation_order"])}</b><span>calculations</span></div><div class="g"><b>{len(rules)}</b><span>validation rules</span></div><div class="g"><b>{F["revision"]}</b><span>revision</span></div><div class="g"><b>{"quarterly"}</b><span>frequency</span></div></div>
<div class="legend"><span><i style="border-color:var(--req)"></i>required</span><span><i></i>optional</span><span><i style="border-color:var(--cond)"></i>conditional (hover for the rule)</span><span><i style="border-color:var(--calc);background:#0f2a1a"></i>computed — never typed</span><span><i style="box-shadow:inset 3px 0 0 var(--prof)"></i>comes from the taxpayer profile</span><span><i style="box-shadow:inset 3px 0 0 #a78bfa"></i>comes from the filing period</span><span>everything else: typed, or pre-filled from the form template</span></div>
<main>
{"".join(sections_html)}
<details open class="sec"><summary>Computed fields <em>arrows from inputs · calculations.json, evaluation order</em></summary>{"".join(calcs)}</details>
<details class="sec"><summary>Validations that block Save / Submit <em>{len(rules)} rules · validations.json</em></summary><ul class="v">{vals}</ul></details>
<div class="count">{"✓" if count_ok else "✗"} {drawn} of {F["field_count"]} fields drawn{"" if count_ok else " — MISMATCH"}</div>
</main>
<footer><b>Sources</b> · <code>rules/forms/{rules_id}/fields.json</code> (field_count {F["field_count"]}, sha {F["inventory_sha256"][:12]}…) · <code>rules/forms/{rules_id}/calculations.json</code> · <code>rules/forms/{rules_id}/validations.json</code> · <code>html-frozen/{bundle}/index.html</code> · section order and item labels from the frozen HTML (Part I / Part II / Part III / Schedule 1). Generated by <code>crates/bir-desktop/docs/eli5/tools/form_page.py</code>.</footer>
</body></html>"""
pathlib.Path(out).write_text(page)
print(f"{out}: drawn {drawn} / {F['field_count']} {'OK' if count_ok else 'MISMATCH'}")
