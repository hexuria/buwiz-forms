#!/usr/bin/env python3
"""Per-form findings against the approved form-page rules (.claude/GOAL.md).

Reads the host export (tmp/form-findings/screen.json, from the ignored test
`write_form_findings_export`), the runtime audit (tmp/screen-form-audit/
runtime.json), rules/forms/<form>/fields.json, html-frozen/<slug>/
writer-cells.json + index.html, and writes docs/form-findings/<CODE>/
findings.json with one section per rule. Automatic only: each form is then
reviewed by hand (screenshots), which may correct, drop or add findings and
sets `reviewed`. A re-run keeps a reviewed file's manual fields
(`reviewed`, `review_notes`, `manual` findings, `dropped`).

    python3 tools/form_findings.py [CODE ...]
"""
import json
import re
import sys
from collections import defaultdict
from html.parser import HTMLParser
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "docs" / "form-findings"
EXPORT = ROOT / "tmp" / "form-findings" / "screen.json"
RUNTIME = ROOT / "tmp" / "screen-form-audit" / "runtime.json"

MONEY_TYPES = {"decimal-amount", "whole-peso-amount", "money", "decimal-money"}
DATE_TYPES = {"date-string-mm-dd-yyyy", "date-string", "date", "date-component-string",
              "date-component-or-string", "month-year", "month", "year", "integer-year",
              "period-component-string", "month-code", "year-string", "two-digit-tax-year"}
PERIOD_KEY = re.compile(r"(?i)(year|month|quarter|qtr|date|day|period|rtnyear|yearend)")
PENALTY = re.compile(r"(?i)\b(surcharge|interest|compromise|total penalties|penalt)")
ITEM_TOKEN = re.compile(r"^\s*(?:Item\s+)?(\d+)([A-Za-z]?)\b")


class Cells(HTMLParser):
    """html_id -> comb slots / field kind from a frozen sheet."""

    def __init__(self):
        super().__init__()
        self.cells = {}

    def handle_starttag(self, tag, attrs):
        a = dict(attrs)
        if a.get("data-cell-kind") == "field" and a.get("id"):
            self.cells[a["id"]] = {"kind": a.get("data-field-kind"), "slots": int(a.get("data-comb-slots") or 0)}


def rules_dir(code):
    hits = sorted((ROOT / "rules" / "forms").glob(code.lower() + "-v*"))
    return hits[0] if hits else None


def load_fields(code):
    d = rules_dir(code)
    if not d or not (d / "fields.json").exists():
        return {}
    j = json.loads((d / "fields.json").read_text())
    return {x["field_key"]: x for x in (j["fields"] if isinstance(j, dict) else j)}


def load_writer(slug):
    p = ROOT / "html-frozen" / slug / "writer-cells.json"
    if not p.exists():
        return {}, {}
    w = json.loads(p.read_text())
    joins = {}
    for lst in ("joins", "money_joins", "peso_joins", "date_joins"):
        for e in w.get(lst) or []:
            if e.get("writer_key") and e.get("printed"):
                joins.setdefault(e["writer_key"], (e["printed"], e.get("html_id"), lst))
    nj = {}
    for e in w.get("not_joined") or []:
        for k in e.get("writer_keys") or [e.get("writer_key")]:
            if k:
                nj[k] = e.get("reason", "")
    return joins, nj


def load_cells(slug):
    p = ROOT / "html-frozen" / slug / "index.html"
    c = Cells()
    if p.exists():
        c.feed(p.read_text())
    return c.cells


def short(k):
    return k.split(":")[-1]


def num(token):
    m = ITEM_TOKEN.match(token or "")
    return int(m.group(1)) if m else None


def visible_layout(schema):
    """The page's parts and items (hidden fields dropped), with positions."""
    parts = []
    for p in schema["parts"]:
        items = []
        for it in p["items"]:
            if it.get("rows"):
                items.append({"no": it.get("no") or "", "l": it.get("l") or "", "table": True,
                              "keys": [k for row in it.get("keys", []) for k in row], "cols": it.get("cols", [])})
                continue
            fs = [f for f in it.get("f", []) if f.get("t") != "hidden"]
            if fs:
                items.append({"no": it.get("no") or "", "l": it.get("l") or "", "f": fs})
        if items:
            name = p.get("n", "")
            if len(parts) == 0 and re.fullmatch(r"Page \d+", name):
                name = "Return details"
            parts.append({"n": name, "items": items})
    return parts


def finding(what, evidence, keys=(), where="", fix=""):
    return {"what": what, "where": where, "keys": list(keys)[:12], "evidence": evidence, "fix": fix}


def section(findings, ok_note, scope="form"):
    return {"status": "findings" if findings else "ok", "summary": ok_note if not findings else "",
            "scope": scope, "findings": findings}


def analyse(form, runtime):
    code = form["code"]
    schema = form["schema"]
    slug = schema["s"]
    values = form.get("values") or {}
    locks = form.get("locks") or {}
    profile = set(form.get("profile") or [])
    fj = load_fields(code)
    joins, not_joined = load_writer(slug)
    cells = load_cells(slug)
    parts = visible_layout(schema)
    where = {}
    fields = {}
    for pi, p in enumerate(parts):
        for ii, it in enumerate(p["items"]):
            for f in it.get("f", []):
                where[f["k"]] = (pi, ii, p["n"], it["no"], it["l"])
                fields[f["k"]] = f
            for k in it.get("keys", []):
                where.setdefault(k, (pi, ii, p["n"], it["no"], it["l"]))
    loc = lambda k: (f"{where[k][2]} › {where[k][3]} {where[k][4]}".strip() if k in where else "")
    lt = lambda k: (fj.get(k) or {}).get("logical_type") or ""
    rt = runtime.get(code, {}).get("findings", {})
    rules = {}

    # R0 one-way propagation
    rules["R0"] = section([], f"{len(profile)} boxes come from the tax profile; nothing on this page writes back to it "
                          "(screen_form_fill only changes the return's draft).", scope="app")

    # R1 library opens with the period filled
    r1 = []
    for k, f in fields.items():
        v = str(values.get(k, "")).strip()
        is_period = lt(k) in DATE_TYPES or (PERIOD_KEY.search(k.split(":")[-1]) and f.get("t") in ("sel", "int", "radio", "date", "text"))
        if is_period and v and v not in ("false", "0", "00") and k not in profile and not re.search(r"(?i)birth", k):
            r1.append((k, v))
    r1f = []
    if r1:
        r1f.append(finding(
            f"Opened from the Form library, {len(r1)} period/date box(es) arrive filled ({', '.join(f'{short(k)}={v}' for k, v in r1[:6])}).",
            "tmp/form-findings/screen.json values on a fresh open (library path: current year, first period)",
            [k for k, _ in r1], loc(r1[0][0]),
            "Library opening shows these blank; a return opened for a taxpayer keeps its period."))
    rules["R1"] = section(r1f, "No period or date box is filled on a fresh open.")

    # R2 profile boxes
    r2f = []
    shown_profile = [k for k in profile if k in fields]
    if shown_profile:
        by_part = defaultdict(list)
        for k in shown_profile:
            by_part[where[k][2]].append(k)
        r2f.append(finding(
            f"{len(shown_profile)} profile boxes are locked with no way to edit them on this return: "
            + "; ".join(f"{p}: {len(ks)}" for p, ks in by_part.items()) + ".",
            "screen_form_open `profile` + `locks` (Lock::Profile)", shown_profile, "",
            "Collapse them per section into 'From tax profile'; hover Edit, edit stays on this return (blur saves)."))
        r2f.append(finding(
            "A profile value that is blank still arrives locked (the lock is per box, not per value), so the filer cannot type it here.",
            "screen_form_module.rs locks(): Lock::Profile regardless of value", shown_profile[:4], "",
            "Lock only when the profile has a value; a blank one is a normal box in paper order."))
    for x in rt.get("i_profile_fact_printed_but_not_on_form_page", []):
        under = x.get("printed_under") or []
        r2f.append(finding(f"The profile's {x.get('category', '?').upper()} prints on the sheet (under {', '.join(short(k) for k in under)}) but no box on the page shows it.",
                           "tmp/screen-form-audit/runtime.json i_profile_fact_printed_but_not_on_form_page", under, "",
                           "Show it in its printed item, filled from the profile (R2: Edit on hover)."))
    for x in rt.get("i_identity_empty_in_both", []):
        r2f.append(finding(f"{x.get('category', '?').upper()} box '{short(x.get('k', ''))}' is empty on the page and on the print although the profile has the fact.",
                           "tmp/screen-form-audit/runtime.json i_identity_empty_in_both", [x.get("k", "")], x.get("item", ""),
                           "Fill it from the profile, or confirm it is a different fact (e.g. the property's RDO)."))
    rules["R2"] = section(r2f, "No profile-filled boxes on this form.")

    # R3 paper order
    r3f = []
    mism = []
    for k, (pi, ii, pn, no, il) in where.items():
        if k not in joins or not no:
            continue
        pr = joins[k][0]
        a, b = num(pr), num(no)
        if a is not None and b is not None and a != b and not pr.lower().startswith(("schedule", "part")):
            mism.append((k, no, pr))
    if mism:
        r3f.append(finding(
            f"{len(mism)} box(es) sit under a different item number than the paper prints for them, e.g. "
            + "; ".join(f"{short(k)} under {no} but printed '{pr[:40]}'" for k, no, pr in mism[:4]) + ".",
            f"html-frozen/{slug}/writer-cells.json joins vs screen item numbers", [m[0] for m in mism], loc(mism[0][0]),
            "Move each box under its printed item."))
    for pi, p in enumerate(parts):
        seq = [(num(it["no"]), it["no"]) for it in p["items"] if num(it["no"]) is not None]
        back = [(seq[i - 1][1], seq[i][1]) for i in range(1, len(seq)) if seq[i][0] < seq[i - 1][0]]
        if back and not p["n"].startswith("Part IV"):
            r3f.append(finding(f"In '{p['n']}' item numbers go backwards ({', '.join(f'{a}→{b}' for a, b in back[:4])}).",
                               "screen item order (tmp/form-findings/screen.json)", [], p["n"],
                               "Order items top to bottom as printed."))
        unnum = [it for it in p["items"] if not it["no"] and not it.get("table") and p["n"] != "Not on the printed sheet"]
        groups = [it for it in unnum if it["l"] in ("Additional details", "")]
        if groups:
            keys = [f["k"] for it in groups for f in it.get("f", [])]
            r3f.append(finding(f"'{p['n']}' has {len(groups)} unnumbered 'Additional details' group(s) with {len(keys)} box(es).",
                               "screen items without a printed number", keys, p["n"],
                               "Place each box under its printed item, or in 'Not on the printed sheet' if the paper has none."))
    nf = rt.get("v_never_fillable_but_printed", [])
    if nf:
        r3f.append(finding(f"{len(nf)} printed box(es) the page cannot fill: "
                           + "; ".join((x.get("label") or x.get("k") or "")[:32] for x in nf[:5]) + ".",
                           "tmp/screen-form-audit/runtime.json v_never_fillable_but_printed", [x.get("k", "") for x in nf],
                           nf[0].get("item", ""), "Give the page a box for each (or confirm it is a sheet constant)."))
    rest = [p for p in parts if p["n"] == "Not on the printed sheet"]
    if rest:
        n = sum(len(it.get("f", [])) for it in rest[0]["items"])
        r3f.append(finding(f"{n} box(es) are kept in 'Not on the printed sheet'. Confirm none of them is printed.",
                           "screen part 'Not on the printed sheet'",
                           [f["k"] for it in rest[0]["items"] for f in it.get("f", [])], "Not on the printed sheet",
                           "Leave them there (collapsed) or place them if the paper prints them."))
    rules["R3"] = section(r3f, "Screen order and grouping match the paper.")

    # R4 dates
    dk = [k for k, f in fields.items() if f.get("t") == "date" or lt(k) in DATE_TYPES]
    full = [k for k in dk if fields[k].get("t") == "date"]
    split = [k for k in dk if fields[k].get("t") != "date"]
    r4f = []
    if full:
        r4f.append(finding(f"{len(full)} date box(es) take MM/DD/YYYY: show them as 'Dec 10, 2020' and let the calendar change month and year.",
                           "screen kind 'date'", full, loc(full[0]),
                           "Accept 12102020 / 12-10-2020 / 12/10/2020; display 'Mon D, YYYY'; month/year switchers."))
    if split:
        r4f.append(finding(f"{len(split)} split date/period part(s) (month, day, year boxes or selects).",
                           "fields.json logical_type date components", split, loc(split[0]),
                           "Keep the split the official form stores; a library opening shows them blank (R1)."))
    rules["R4"] = section(r4f, "No date boxes on this form.")

    # R5 char boxes
    r5f = []
    growing, overflow, pairs = [], [], []
    for k, f in fields.items():
        if f.get("t") not in ("text", "email", "phone", "money", "int"):
            continue
        j = joins.get(k)
        cell = cells.get(j[1]) if j else None
        slots = cell["slots"] if cell and cell["kind"] == "comb" else 0
        label = f"{f.get('l', '')} {k}".lower()
        is_growing = f.get("t") == "email" or any(w in label for w in ("name", "address", "email", "specify", "description", "particular", "location"))
        if slots and is_growing:
            growing.append((k, slots))
        if slots and f.get("m") and f["m"] > slots and is_growing:
            overflow.append((k, slots, f["m"]))
    if growing:
        r5f.append(finding(
            f"{len(growing)} growing text box(es) print in fixed char boxes: "
            + "; ".join(f"{short(k)} [{s}]" for k, s in growing[:5]) + ".",
            f"html-frozen/{slug}/index.html data-comb-slots via writer-cells joins", [g[0] for g in growing],
            loc(growing[0][0]), "Boxes until the text no longer fits, then a plain field (page) and plain text (print)."))
    if overflow:
        r5f.append(finding(
            f"{len(overflow)} box(es) accept more characters than their printed boxes: "
            + "; ".join(f"{short(k)} {m} > {s}" for k, s, m in overflow[:5]) + ".",
            "screen max length (m) vs comb slots", [o[0] for o in overflow], loc(overflow[0][0]),
            "Switch to plain text when it overflows; never cut the value."))
    for k in fields:
        m = re.match(r"(.*?Address)(\d?)$", k)
        if m and m.group(2) in ("", "1") and (m.group(1) + "2") in fields:
            pairs.append((k, m.group(1) + "2"))
    if pairs:
        r5f.append(finding(f"Two-line address{'es' if len(pairs) > 1 else ''}: " + ", ".join(f"{short(a)} + {short(b)}" for a, b in pairs) + ".",
                           "screen keys", [k for p in pairs for k in p], loc(pairs[0][0]),
                           "Pool both lines' slots; past the total both become one plain field (saved split unchanged)."))
    tin_items = []
    for p in parts:
        for it in p["items"]:
            fs = it.get("f", [])
            tins = [f for f in fs if re.search(r"(?i)TIN\d|TIN[A-Z]?\d|txtTIN", f["k"])]
            if len(tins) >= 3 and len(fs) > 5:
                tin_items.append((it["no"] + " " + it["l"])[:60])
    if tin_items:
        r5f.append(finding(f"TIN parts shown as separate labelled boxes (not one 3·3·3·5 comb) in: {'; '.join(tin_items[:4])}.",
                           "form_page.js isTinItem needs an item of 3-5 text boxes only", [], tin_items[0],
                           "Group the TIN parts as one TIN comb inside the item."))
    rules["R5"] = section(r5f, "No char-box problems found.")

    # R6 conditional enable
    cond = defaultdict(list)
    # Only conditions the filer controls: skip the official runtime's own
    # states (static-disabled boxes, ATC modal slots) and boxes the page
    # locks anyway (computed or profile).
    internal = re.compile(r"(?i)disabled in the static form|rendered in the|runtime|modal|ui state")
    for k in fields:
        ew = (fj.get(k) or {}).get("enabled_when")
        if ew and not internal.search(ew) and not fields[k].get("c") and k not in profile and not re.search(r"(?i)birth", k):
            cond[ew].append(k)
    r6f = []
    for ew, ks in sorted(cond.items(), key=lambda x: -len(x[1]))[:25]:
        r6f.append(finding(f"Enabled only when: {ew} ({len(ks)} box(es)). The page draws them open.",
                           f"rules/forms/{rules_dir(code).name if rules_dir(code) else '?'}/fields.json enabled_when", ks, loc(ks[0]),
                           "Fade them, show what enables them, enable when the condition holds (validation unchanged)."))
    if len(cond) > 25:
        r6f.append(finding(f"... and {len(cond) - 25} more conditions.", "fields.json enabled_when", [], ""))
    rules["R6"] = section(r6f, "No conditionally enabled boxes.")

    # R7 number formatting
    r7f = []
    wrong = [k for k, f in fields.items() if f.get("t") == "money" and lt(k) and lt(k) not in MONEY_TYPES and lt(k) not in ("decimal", "decimal-or-string")]
    if wrong:
        r7f.append(finding(f"{len(wrong)} non-amount box(es) are formatted as money: "
                           + "; ".join(f"{fields[k].get('l', '')[:30]} ({lt(k)})" for k in wrong[:5]) + ".",
                           "screen kind 'money' vs fields.json logical_type", wrong, loc(wrong[0]),
                           "Treat them as text or whole numbers; keep exactly what was typed."))
    # fields.json calls some names amounts (an official metadata quirk); a
    # name, address or ID box is right to stay plain text.
    textual = re.compile(r"(?i)name|address|tin\b|email|specify|description|code|number|no\.")
    plain = [k for k, f in fields.items() if f.get("t") == "text" and lt(k) in MONEY_TYPES
             and not textual.search(f"{f.get('l', '')} {short(k)}")]
    if plain:
        r7f.append(finding(f"{len(plain)} peso amount(s) are plain text boxes (no amount formatting): "
                           + "; ".join(fields[k].get("l", "")[:30] for k in plain[:5]) + ".",
                           "screen kind 'text' vs fields.json logical_type", plain, loc(plain[0]),
                           "Format as pesos."))
    rules["R7"] = section(r7f, "Only peso amounts are formatted as money.")

    # R8 penalties
    def is_penalty(k):
        label, item = fields[k].get("l", ""), where[k][4]
        if re.search(r"(?i)surcharge|compromise|total penalt", label):
            return True
        # "Interest" is a penalty only as its own line or under "Add: Penalties".
        return bool(re.fullmatch(r"(?i)\W*(\d+[A-Z]?\s+)?interest\W*", label) or
                    (re.search(r"(?i)penalt", item) and re.search(r"(?i)interest", label)))
    pen = [k for k in fields if is_penalty(k)]
    r8f = []
    if pen:
        comp = [k for k in pen if fields[k].get("c")]
        edit = [k for k in pen if not fields[k].get("c") and k not in locks]
        state = "; ".join(f"{fields[k].get('l', '')[:28]}: {'computed' if fields[k].get('c') else ('editable' if k not in locks else 'locked')}" for k in pen[:6])
        r8f.append(finding(f"Penalty boxes: {state}.",
                           "screen kinds (c = computed) and locks", pen, loc(pen[0]),
                           "Show the computed surcharge/interest/compromise; one Edit unlocks the three together; total penalties and amount payable recompute."))
    rules["R8"] = section(r8f, "No penalty boxes on this form.")

    # preview
    pv = []
    for c, what in (("vi_field_map_values_not_on_screen", "printed from the draft but not shown on the page"),
                    ("ii_open_mismatch", "page and print disagree on open"),
                    ("ii_roundtrip_mismatch", "page and print disagree after save"),
                    ("v_app_constant_printed", "printed by the app as a constant (check it is right)"),
                    ("legacy_print_fields_failed", "legacy print path fails")):
        xs = [dict(x, k=x.get("k") or x.get("key", "")) for x in rt.get(c, []) if x.get("sheet_prints", True)]
        if xs:
            pv.append(finding(f"{len(xs)} box(es) {what}: " + "; ".join((x.get("label") or x.get("k") or "")[:30] for x in xs[:4]) + ".",
                              f"tmp/screen-form-audit/runtime.json {c}", [x.get("k", "") for x in xs], xs[0].get("item", ""),
                              "Make page and preview agree, from the draft."))
    rules["preview"] = section(pv, "The preview prints what the page holds.")

    # validation
    vf = []
    nk = [e for e in form.get("errors") or [] if not any(k in where for k in e.get("keys") or [])]
    if nk:
        vf.append(finding(f"{len(nk)} message(s) on open land on no visible box: " + "; ".join(e["message"][:50] for e in nk[:3]) + ".",
                          "screen_form_open errors[].keys", [e.get("field", "") for e in nk], "",
                          "Place each message under the box it is about."))
    cites = []
    for e in form.get("errors") or []:
        m = re.search(r"Item(?:s)?\s+(?:no\.\s*)?(\d+)", e["message"])
        ks = [k for k in e.get("keys") or [] if k in where]
        if m and ks:
            shown = where[ks[0]][3]
            if num(shown) is not None and num(shown) != int(m.group(1)):
                cites.append((e["message"], shown, ks[0]))
    if cites:
        vf.append(finding(f"{len(cites)} official message(s) cite another item number than the box they sit under, e.g. "
                          + "; ".join(f"'{m[:45]}' under item {s}" for m, s, _ in cites[:3]) + ".",
                          "official message text vs screen item number (printed numbering, decision a)", [c[2] for c in cites], loc(cites[0][2]),
                          "Message text stays official; decide whether the page adds the printed item number next to it."))
    rules["validation"] = section(vf, "Every message lands on a visible box.")
    # Each finding points at the page screenshot that shows its first box
    # (tmp/form-findings/run_shots.py names them p<part>-<first item of 5>).
    for sec in rules.values():
        for f in sec["findings"]:
            k = next((k for k in f["keys"] if k in where), None)
            if k:
                pi, ii = where[k][0], where[k][1]
                f["shot"] = f"p{pi}-{ii // 5 * 5:02d}"
    for name, sec in rules.items():
        for i, f in enumerate(sec["findings"]):
            f["id"] = f"{name}.{i}"
    return {"form": code, "title": schema.get("t", ""), "revision": schema.get("rev", ""), "slug": slug,
            "parts": [{"n": p["n"], "items": len(p["items"])} for p in parts], "rules": rules}


def main():
    export = json.loads(EXPORT.read_text())
    runtime = {f["code"]: f for f in json.loads(RUNTIME.read_text())["forms"]}
    only = set(sys.argv[1:])
    for form in export["forms"]:
        code = form["code"]
        if only and code not in only:
            continue
        data = analyse(form, runtime)
        d = OUT / code
        d.mkdir(parents=True, exist_ok=True)
        path = d / "findings.json"
        if path.exists():
            old = json.loads(path.read_text())
            for keep in ("reviewed", "review_notes", "manual", "dropped", "screenshots", "verdicts"):
                if keep in old:
                    data[keep] = old[keep]
        data.setdefault("reviewed", False)
        data.setdefault("review_notes", "")
        data.setdefault("screenshots", [])
        path.write_text(json.dumps(data, indent=1, ensure_ascii=False) + "\n")
        counts = {k: len(v["findings"]) for k, v in data["rules"].items()}
        print(code, counts)


if __name__ == "__main__":
    main()
