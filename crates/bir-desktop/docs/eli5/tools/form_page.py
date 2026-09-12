#!/usr/bin/env python3
"""Draw one ELI5 page per BIR form from rules/forms/<id>/{fields,calculations,validations,workflow}.json
and html-frozen/<bundle>/index.html.

Sections are UX-oriented (Phase 0 decision 6); paper Parts/Schedules are reference only.
Every field appears exactly once. Labels prefer fields.json `label` (decision 8).

usage: form_page.py <rules-id> <frozen-bundle> <out.html>
"""
from __future__ import annotations

import collections
import html
import json
import pathlib
import re
import sys

ROOT = next(p for p in pathlib.Path(__file__).resolve().parents if (p / "rules" / "forms").is_dir())
rules_id, bundle, out = sys.argv[1], sys.argv[2], sys.argv[3]
R = ROOT / "rules/forms" / rules_id
F = json.load(open(R / "fields.json"))
C = json.load(open(R / "calculations.json"))
V = json.load(open(R / "validations.json"))
W = json.load(open(R / "workflow.json")) if (R / "workflow.json").is_file() else {}
H = (ROOT / "html-frozen" / bundle / "index.html").read_text(errors="replace")
title_m = re.search(r"<title>([^<]+)</title>", H)
title = title_m.group(1).strip() if title_m else rules_id

# Optional hand overrides when auto is wrong (2551Q inventory labels are key-like).
SECTIONS_OVERRIDE: dict[str, list[tuple[str, list[tuple[int | None, str]]]]] = {}
LABELS_OVERRIDE: dict[str, dict[str, str]] = {
    "2551q-v2018": {
        "1": "For the: Calendar / Fiscal",
        "2": "Year ended (MM/YYYY)",
        "3": "Quarter",
        "4": "Amended return?",
        "5": "Number of sheets attached",
        "6": "TIN",
        "7": "RDO code",
        "8": "Taxpayer's name",
        "9": "Registered address + ZIP",
        "10": "Contact number",
        "11": "Email address",
        "12": "Tax relief under special law / treaty? (+ specify)",
        "13": "Income-tax-rate election (graduated / 8%)",
        "14": "Total tax due (from Schedule 1 item 7)",
        "15": "Creditable percentage tax withheld (2307)",
        "16": "Tax paid in previously filed return (amended)",
        "17": "Other tax credit / payment (specify)",
        "18": "Total tax credits / payments (15+16+17)",
        "19": "Tax still payable / (overpayment) (14−18)",
        "20": "Surcharge",
        "21": "Interest",
        "22": "Compromise",
        "23": "Total penalties (20+21+22)",
        "24": "Total amount payable / (overpayment) (19+23)",
        "None": "Overpayment choice · tax agent accreditation · payment rows 25–28",
        "None:hdr": "TIN + taxpayer name repeated on page 2",
        "None:sched": "Six ATC lines (ATC, taxable amount, rate, tax due) + item 7 total",
    },
}

SRC = {
    "profile": re.compile(
        r"TIN|BranchCode|registeredName|registeredAddress|zipCode|telNo|txtEmail|txtRDOCode|"
        r"TaxpayerName|LineofBus|txtTaxpayerName|withholdingAgent|TradeName|Address",
        re.I,
    ),
    "period": re.compile(
        r"forThe_|rtnMonth|txtYear|qtr_|txtMonth|ReturnPeriod|DueMonth|DueDay|DueYear|YearEnded",
        re.I,
    ),
}
KIND = {"text": "text / money", "radio": "radio", "select-one": "select", "checkbox": "checkbox"}

questions: list[str] = []


def key_of(x: dict) -> str:
    sk = x.get("serialized_key")
    if sk:
        return str(sk).split(":")[-1]
    fk = x.get("field_key")
    if fk:
        return str(fk).split(":")[-1]
    return "unknown_field"


def page_of(x: dict) -> int:
    p = x.get("page")
    try:
        return int(p) if p is not None else 1
    except (TypeError, ValueError):
        return 1


def occurrence_of(x: dict) -> int:
    o = x.get("serialized_occurrence")
    try:
        return int(o) if o is not None else 1
    except (TypeError, ValueError):
        return 1


def source_of(x: dict) -> str:
    k = key_of(x)
    if x.get("required") == "computed":
        return "computed"
    if SRC["profile"].search(k):
        return "profile"
    if SRC["period"].search(k):
        return "period"
    return "typed"


def clean_label(raw: str | None, fallback_key: str) -> str:
    if not raw:
        return fallback_key
    s = str(raw).strip()
    if not s:
        return fallback_key
    # Strip form-prefixed inventory keys: frm2551Qv2018:txt14 → txt14
    if re.match(r"^frm\w+:", s, re.I):
        s = s.split(":", 1)[-1]
    if s == fallback_key or re.fullmatch(r"[A-Za-z]*\d+[A-Za-z]*\d*", s):
        # still key-like; title-case camel/underscore lightly
        nice = re.sub(r"([a-z])([A-Z])", r"\1 \2", s)
        nice = nice.replace("_", " ")
        return nice
    return s


def group_label(item: str, fields: list[dict], overrides: dict[str, str]) -> str:
    if item in overrides:
        return overrides[item]
    labels = []
    for x in fields:
        lab = clean_label(x.get("label"), key_of(x))
        if lab and lab not in labels:
            labels.append(lab)
    if not labels:
        return f"Item {item}" if item not in ("None", "None:hdr", "None:sched") else "Ungrouped"
    # Prefer shortest human-looking label; else first
    human = [l for l in labels if " " in l or len(l) > 12]
    pick = min(human or labels, key=len)
    if item not in ("None", "None:hdr", "None:sched") and not item.startswith("Schedule"):
        if not pick.lower().startswith("item"):
            return f"Item {item} — {pick}"
    return pick


def item_sort_key(item: str):
    if item.startswith("Schedule"):
        return (2, item, "")
    if item.startswith("None"):
        return (3, item, "")
    m = re.match(r"^(\d+)([A-Za-z]*)$", item)
    if m:
        return (0, int(m.group(1)), m.group(2))
    return (1, item, "")


def sub_group(x: dict) -> str:
    """Stable group id within a page for item rows."""
    k = key_of(x)
    item = x.get("item_number")
    item_s = "None" if item is None else str(item)
    if item_s.startswith("Schedule"):
        return item_s
    if k.startswith("txtPg2") or (page_of(x) == 2 and item_s == "None" and "Pg2" in k):
        return "None:hdr"
    # 2551Q-style schedule rows on page 2 with item None and ATC keys
    if page_of(x) >= 2 and item_s == "None" and re.search(
        r"ATC|Sched|txtTotalSched|drpATC|txtATC", k, re.I
    ):
        return "None:sched"
    if page_of(x) >= 2 and item_s == "None" and not k.startswith("txtPg2"):
        # page-2 non-header none: keep as schedule-ish bucket for forms like 2551Q extras
        if re.search(r"CurrentPage|MaxPage|FinalFlag|Enroll|ebirOnline|driveSelect", k, re.I):
            return "None:sched"
    return item_s


def paper_headings(html_text: str) -> list[str]:
    heads = re.findall(
        r">\s*((?:Part|Schedule|PART|SCHEDULE)\s*[IVX0-9]*[^<]{0,60})",
        html_text,
        re.I,
    )
    out_h = []
    for h in heads:
        h = " ".join(h.split())
        if h not in out_h:
            out_h.append(h)
    return out_h


def derive_frequency() -> str:
    deadlines = W.get("filing_deadlines") or []
    blob = json.dumps(deadlines).lower() + " " + json.dumps(W.get("phases") or {}).lower()
    # Field hints
    keys = " ".join(key_of(x) for x in F["fields"]).lower()
    labels = " ".join(str(x.get("label") or "") for x in F["fields"]).lower()
    text = blob + " " + keys + " " + labels
    if re.search(r"\bqtr_|quarter|quarterly\b", text) and not re.search(
        r"monthly filing|each covered month|for each covered month", blob
    ):
        # quarterly deadlines listing Q1..Q4 alone is weak; prefer explicit
        if "qtr_" in keys or "quarter" in labels or re.search(r"within twenty-five days after the end of the taxable quarter", blob):
            return "quarterly"
    if re.search(r"monthly|each covered month|for each covered month|10th day of the following month", text):
        return "monthly"
    if re.search(r"\bannual|calendar year|once a year\b", text):
        return "annual"
    if "qtr_" in keys:
        return "quarterly"
    if "txtmonth" in keys and "qtr_" not in keys:
        return "monthly"
    questions.append(
        f"Filing frequency for {rules_id}: workflow/fields did not clearly say monthly/quarterly/annual — marked ?"
    )
    return "?"


def ux_bucket(page: int, item: str, fields: list[dict]) -> str:
    """Assign a UX section id for a whole item group (fields stay together)."""
    keys = [key_of(x) for x in fields]
    labels_l = " ".join(clean_label(x.get("label"), key_of(x)) for x in fields).lower()
    sources = {source_of(x) for x in fields}

    if item.startswith("Schedule") or item == "None:sched":
        return f"schedule:{item}"
    if item == "None:hdr" or all(k.startswith("txtPg2") for k in keys):
        return "print_xml"
    if item == "None" or item.startswith("None"):
        return "payment"

    # Numbered items
    if sources <= {"period"} or (
        "period" in sources and sources <= {"period", "typed"} and re.search(
            r"amended|quarter|year|month|sheet|period|due date|for the", labels_l
        )
    ):
        # period-dominated early items
        if "profile" not in sources:
            return "period"

    if "profile" in sources and sources <= {"profile", "typed", "computed"}:
        # identity cluster — but avoid classifying tax lines that merely mention address
        if not re.search(r"tax due|withheld|remittance|surcharge|interest|payable|atc", labels_l):
            if any(source_of(x) == "profile" for x in fields) and not re.search(
                r"txt\d{2,}|drpATC|taxDue|Amount", " ".join(keys)
            ):
                return "identity"

    if re.search(
        r"surcharge|interest|compromise|penalt|tax credit|still payable|overpayment|"
        r"total amount payable|total tax credits|total penalties|amount still",
        labels_l,
    ) or re.search(r"txt(1[89]|2[0-9]|3[0-9]|4[0-9])$", " ".join(keys)):
        # weak fallback for totals — refined below by item order
        pass

    return "computation"  # default; rebucketed by order next


def rebucket_by_order(
    groups: dict[tuple[int, str], list[dict]],
) -> list[tuple[str, str, list[tuple[int, str]]]]:
    """Return sections as (section_id, title, [(page, item), ...])."""
    # Partition specials first
    specials = {"print_xml": [], "payment": [], "schedules": collections.OrderedDict()}
    numbered: list[tuple[int, str, list[dict]]] = []

    for (page, item), xs in groups.items():
        if item.startswith("Schedule") or item == "None:sched":
            sid = item if item.startswith("Schedule") else "Schedule (unnumbered rows)"
            specials["schedules"].setdefault(sid, []).append((page, item))
        elif item == "None:hdr" or (
            all(key_of(x).startswith("txtPg2") for x in xs) and item.startswith("None")
        ):
            specials["print_xml"].append((page, item))
        elif item == "None" or item.startswith("None"):
            specials["payment"].append((page, item))
        else:
            numbered.append((page, item, xs))

    numbered.sort(key=lambda t: (t[0], item_sort_key(t[1])))

    # Classify numbered into period / identity / computation / totals using
    # sources + position among unique item numbers on the primary page.
    by_item: list[tuple[str, list[tuple[int, str, list[dict]]]]] = []
    # collapse same item across pages into one logical item list
    item_map: collections.OrderedDict[str, list[tuple[int, str, list[dict]]]] = collections.OrderedDict()
    for page, item, xs in numbered:
        item_map.setdefault(item, []).append((page, item, xs))

    items_ordered = list(item_map.keys())
    n = len(items_ordered)
    # Pre-label each item
    prelim = {}
    for idx, item in enumerate(items_ordered):
        xs_all = [x for _p, _i, xs in item_map[item] for x in xs]
        keys = [key_of(x) for x in xs_all]
        labels_l = " ".join(clean_label(x.get("label"), key_of(x)) for x in xs_all).lower()
        sources = {source_of(x) for x in xs_all}
        bucket = "computation"
        if sources <= {"period"} or (
            "period" in sources
            and "profile" not in sources
            and idx <= max(4, n // 5)
        ):
            bucket = "period"
        if "profile" in sources and idx <= max(8, n // 3) and not re.search(
            r"tax due|withheld amount|remittance|surcharge|atc code", labels_l
        ):
            # identity if mostly profile or classic identity keys
            if sources <= {"profile", "typed", "computed", "period"} and (
                sum(1 for x in xs_all if source_of(x) == "profile") >= 1
                and not re.search(r"^txt(1[4-9]|[2-9]\d)$", " ".join(keys))
            ):
                # Don't steal computation money fields early
                if not re.search(
                    r"taxable|tax due|withholding tax|amount of|gross|income|sales|vat",
                    labels_l,
                ):
                    bucket = "identity"
        if re.search(
            r"surcharge|interest|compromise|penalt|tax credit|still payable|overpayment|"
            r"total amount payable|total tax credits|total penalties|amount payable|"
            r"refund|creditable",
            labels_l,
        ):
            bucket = "totals"
        # Late computed totals
        if bucket == "computation" and idx >= max(0, n - max(6, n // 4)):
            if all(source_of(x) in {"computed", "typed"} for x in xs_all) and re.search(
                r"total|payable|penalty|credit", labels_l
            ):
                bucket = "totals"
        prelim[item] = bucket

    # Smooth: period should be a prefix, identity next, totals a suffix where possible
    # Convert identity that appears after computation back to computation
    seen_comp = False
    for item in items_ordered:
        if prelim[item] == "computation":
            seen_comp = True
        elif prelim[item] in {"period", "identity"} and seen_comp:
            prelim[item] = "computation"

    seen_totals = False
    for item in reversed(items_ordered):
        if prelim[item] == "totals":
            seen_totals = True
        elif seen_totals and prelim[item] == "computation":
            # keep computation before totals; don't drag earlier back
            pass

    titles = {
        "period": "Filing period & return flags",
        "identity": "Taxpayer identity (profile-sourced)",
        "computation": "Main tax computation / withholdings",
        "totals": "Credits, penalties, totals",
        "payment": "Payment / signature / agency / workflow",
        "print_xml": "Print / XML only (hidden in editor)",
    }

    sections: list[tuple[str, str, list[tuple[int, str]]]] = []
    for bid, title in [
        ("period", titles["period"]),
        ("identity", titles["identity"]),
        ("computation", titles["computation"]),
        ("totals", titles["totals"]),
    ]:
        entries = []
        for item in items_ordered:
            if prelim[item] == bid:
                for page, it, _xs in item_map[item]:
                    entries.append((page, it))
        if entries:
            sections.append((bid, title, entries))

    for sid, entries in specials["schedules"].items():
        sections.append((f"schedule:{sid}", sid if sid.startswith("Schedule") else f"Schedule — {sid}", entries))

    if specials["payment"]:
        sections.append(("payment", titles["payment"], specials["payment"]))
    if specials["print_xml"]:
        sections.append(("print_xml", titles["print_xml"], specials["print_xml"]))

    return sections


def build_groups() -> dict[tuple[int, str], list[dict]]:
    groups: dict[tuple[int, str], list[dict]] = collections.defaultdict(list)
    for x in F["fields"]:
        page = page_of(x)
        groups[(page, sub_group(x))].append(x)
    # Move page-2 copies of identity items (RDO/email) into their item row on page 1 when item is numeric
    for (p, i), xs in list(groups.items()):
        if p >= 2 and re.fullmatch(r"\d+[A-Za-z]?", i or ""):
            # keep with same item number — UX can show page copy chip; attach to first page group if exists
            if (1, i) in groups:
                groups[(1, i)].extend(xs)
                del groups[(p, i)]
    return groups


groups = build_groups()
overrides = LABELS_OVERRIDE.get(rules_id, {})

if rules_id in SECTIONS_OVERRIDE:
    section_defs = []
    for name, entries in SECTIONS_OVERRIDE[rules_id]:
        # entries already (page,item)
        section_defs.append((name, name, entries))
else:
    section_defs = rebucket_by_order(groups)

# Verify partition: every group key appears exactly once
assigned = []
for _sid, _title, entries in section_defs:
    assigned.extend(entries)
assigned_set = set(assigned)
missing = [k for k in groups.keys() if k not in assigned_set]
extra = [k for k in assigned if k not in groups]
if missing or extra or len(assigned) != len(assigned_set):
    # Fail soft into a single dump section so we never drop fields
    questions.append(
        f"Auto section partition needed repair for {rules_id}: missing={missing!r} extra={extra!r} dup={len(assigned)!=len(assigned_set)}"
    )
    section_defs = [
        (
            "all",
            "All fields (auto-section repair)",
            sorted(groups.keys(), key=lambda t: (t[0], item_sort_key(t[1]))),
        )
    ]

drawn = 0
sections_html = []
for _sid, name, entries in section_defs:
    # stable unique entries
    seen = set()
    rows = []
    count_fields = 0
    for page, it in entries:
        if (page, it) in seen:
            continue
        seen.add((page, it))
        xs = groups.get((page, it), [])
        if not xs:
            continue
        label = group_label(it, xs, overrides)
        chips = []
        for x in xs:
            k = key_of(x)
            drawn += 1
            count_fields += 1
            req = {"required": "req", "optional": "opt", "conditional": "cond", "computed": "calc"}.get(
                x.get("required") or "optional", "opt"
            )
            cond = x.get("required_when") or x.get("visible_when") or x.get("enabled_when") or ""
            occ = f" · p{page_of(x)} copy" if occurrence_of(x) > 1 else ""
            chips.append(
                f'<span class="f {req} src-{source_of(x)}" title="{html.escape(str(cond))}">'
                f'{html.escape(k)}<small>{html.escape(KIND.get(x["control_kind"], x["control_kind"]))}{html.escape(occ)}</small></span>'
            )
        num = it.split(":")[0]
        num_disp = "·" if num.startswith("None") else num
        rows.append(
            f'<div class="item"><div class="n">{html.escape(num_disp)}</div>'
            f'<div class="l">{html.escape(label)}</div><div class="chips">{"".join(chips)}</div></div>'
        )
    if not rows:
        continue
    sections_html.append(
        f'<details open class="sec"><summary>{html.escape(name)} '
        f"<em>{count_fields} fields</em></summary>{''.join(rows)}</details>"
    )

calcs = []
for cid in C.get("evaluation_order") or []:
    if isinstance(C["calculations"], list):
        c = next((c for c in C["calculations"] if (c.get("id") or c.get("calculation_id")) == cid), {})
    else:
        c = C["calculations"].get(cid, {})
    ins = ", ".join(str(i).split(":")[-1] for i in (c.get("inputs") or c.get("sources") or []))
    tgt = str(c.get("target") or c.get("target_field") or c.get("output") or "").split(":")[-1]
    calcs.append(
        f'<div class="calc"><b>{html.escape(str(cid))}</b>'
        f'<span class="arrow">{html.escape(ins)} ⟶ {html.escape(tgt or "(see rule)")}</span>'
        f'<i>{html.escape(str(c.get("description") or c.get("summary") or c.get("notes") or "")[:140])}</i></div>'
    )

rules = V["rules"] if isinstance(V.get("rules"), list) else list((V.get("rules") or {}).values())
vals = "".join(
    f'<li><b>{html.escape(str(r.get("id") or r.get("rule_id")))}</b> — '
    f'{html.escape(", ".join(str(f).split(":")[-1] for f in (r.get("fields") or r.get("field_keys") or [])))}'
    f'<i>{html.escape(str(r.get("description") or r.get("summary") or r.get("message") or "")[:160])}</i></li>'
    for r in rules
)

freq = derive_frequency()
heads = paper_headings(H)
heads_note = ", ".join(heads[:8]) if heads else "(no Part/Schedule headings found in frozen HTML)"

count_ok = drawn == F["field_count"]
q_html = ""
if questions:
    q_html = (
        '<details open class="sec"><summary>Open questions <em>do not invent answers</em></summary><ul class="v">'
        + "".join(f"<li>{html.escape(q)}</li>" for q in questions)
        + "</ul></details>"
    )

page = f"""<!doctype html><html lang="en"><head><meta charset="utf-8"><title>ELI5 · {html.escape(title)}</title>
<style>
:root{{--bg:#0b0b0c;--card:#161618;--line:#2a2a2e;--fg:#f2f2f2;--mut:#9a9aa3;--req:#ff6b6b;--opt:#7c8cff;--calc:#4ade80;--cond:#fbbf24;--prof:#38bdf8}}
body{{margin:0;background:var(--bg);color:var(--fg);font:16px/1.4 -apple-system,Segoe UI,Helvetica,Arial,sans-serif}}
header{{padding:32px 40px 8px}} h1{{margin:0;font-size:34px}} .sub{{color:var(--mut)}}
.glance{{display:flex;gap:14px;padding:12px 40px 0;flex-wrap:wrap}} .g{{background:var(--card);border:1px solid var(--line);border-radius:12px;padding:12px 16px;min-width:140px}} .g b{{display:block;font-size:26px}} .g span{{color:var(--mut);font-size:13px}}
main{{padding:16px 40px 60px;display:grid;gap:14px}}
.sec{{background:var(--card);border:1px solid var(--line);border-radius:14px;padding:6px 18px 14px}} summary{{cursor:pointer;font-weight:700;font-size:20px;padding:10px 0}} summary em{{color:var(--mut);font-weight:400;font-size:14px;margin-left:10px;font-style:normal}}
.item{{display:grid;grid-template-columns:64px 320px 1fr;gap:10px;align-items:start;padding:8px 0;border-top:1px dashed var(--line)}} .n{{font-weight:800;font-size:18px;color:var(--mut)}} .l{{font-weight:600}}
.chips{{display:flex;flex-wrap:wrap;gap:6px}} .f{{border:1px solid var(--line);border-radius:8px;padding:3px 8px;font-size:13px;background:#1d1d21}} .f small{{display:block;color:var(--mut);font-size:11px}}
.f.req{{border-color:var(--req)}} .f.calc{{border-color:var(--calc);background:#0f2a1a}} .f.cond{{border-color:var(--cond)}} .src-profile{{box-shadow:inset 3px 0 0 var(--prof)}} .src-period{{box-shadow:inset 3px 0 0 #a78bfa}}
.legend{{display:flex;gap:16px;flex-wrap:wrap;padding:4px 40px;color:var(--mut);font-size:13px}} .legend i{{display:inline-block;width:12px;height:12px;border-radius:3px;margin-right:6px;vertical-align:-1px;border:1px solid var(--line)}}
.calc{{display:grid;grid-template-columns:230px 1fr;gap:10px;padding:8px 0;border-top:1px dashed var(--line)}} .calc .arrow{{font-family:ui-monospace,Menlo,monospace;font-size:14px;color:var(--calc)}} .calc i{{grid-column:2;color:var(--mut);font-size:13px;font-style:normal}}
ul.v{{margin:0;padding-left:18px}} ul.v li{{padding:5px 0;border-top:1px dashed var(--line)}} ul.v i{{display:block;color:var(--mut);font-size:13px;font-style:normal}}
.count{{font-size:22px;font-weight:800;color:{'var(--calc)' if count_ok else 'var(--req)'}}}
footer{{padding:16px 40px 40px;color:var(--mut);font-size:13px;border-top:1px solid var(--line)}} code{{color:#e5e5e5}}
</style></head><body>
<header><h1>{html.escape(title)}</h1><div class="sub">Every field the app page must show. Sections are <b>UX-oriented</b> for the screen editor (paper Parts/Schedules are reference only). Drawn from the XML field inventory + frozen HTML, not from the current Rust view.</div></header>
<div class="glance"><div class="g"><b>{F["field_count"]}</b><span>fields in fields.json</span></div><div class="g"><b>{drawn}</b><span>fields drawn here</span></div><div class="g"><b>{len(C.get("evaluation_order") or [])}</b><span>calculations</span></div><div class="g"><b>{len(rules)}</b><span>validation rules</span></div><div class="g"><b>{html.escape(str(F.get("revision")))}</b><span>revision</span></div><div class="g"><b>{html.escape(freq)}</b><span>frequency</span></div></div>
<div class="legend"><span><i style="border-color:var(--req)"></i>required</span><span><i></i>optional</span><span><i style="border-color:var(--cond)"></i>conditional (hover for the rule)</span><span><i style="border-color:var(--calc);background:#0f2a1a"></i>computed — never typed</span><span><i style="box-shadow:inset 3px 0 0 var(--prof)"></i>comes from the taxpayer profile</span><span><i style="box-shadow:inset 3px 0 0 #a78bfa"></i>comes from the filing period</span><span>everything else: typed, or pre-filled from the form template</span></div>
<main>
{"".join(sections_html)}
<details open class="sec"><summary>Computed fields <em>arrows from inputs · calculations.json, evaluation order</em></summary>{"".join(calcs) if calcs else "<p class=sub>No calculations listed.</p>"}</details>
<details class="sec"><summary>Validations that block Save / Submit <em>{len(rules)} rules · validations.json</em></summary><ul class="v">{vals if vals else "<li>None listed.</li>"}</ul></details>
{q_html}
<div class="count">{"✓" if count_ok else "✗"} {drawn} of {F["field_count"]} fields drawn{"" if count_ok else " — MISMATCH"}</div>
</main>
<footer><b>Sources</b> · <code>rules/forms/{html.escape(rules_id)}/fields.json</code> (field_count {F["field_count"]}, sha {html.escape(str(F.get("inventory_sha256") or "")[:12])}…) · <code>rules/forms/{html.escape(rules_id)}/calculations.json</code> · <code>rules/forms/{html.escape(rules_id)}/validations.json</code> · <code>html-frozen/{html.escape(bundle)}/index.html</code> · paper headings (reference): {html.escape(heads_note)}. Sections regrouped for screen UX (Phase 0 decision 6). Page-2 TIN/name repeats are drawn under <i>Print / XML only (hidden in editor)</i> (decision 7). Generated by <code>crates/bir-desktop/docs/eli5/tools/form_page.py</code>.</footer>
</body></html>"""
pathlib.Path(out).parent.mkdir(parents=True, exist_ok=True)
pathlib.Path(out).write_text(page)
print(f"{out}: drawn {drawn} / {F['field_count']} {'OK' if count_ok else 'MISMATCH'}")
if questions:
    for q in questions:
        print(f"  ? {q}")
