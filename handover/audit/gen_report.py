"""Build the per-form UI audit report (index + one page per form) from audit/forms/<CODE>.json.

Usage: python3 -I gen_report.py <audit_dir> <out_dir>
"""
import html, json, os, re, sys

AUDIT, OUT = sys.argv[1], sys.argv[2]
FORMS = os.path.join(AUDIT, "forms")
os.makedirs(os.path.join(OUT, "f"), exist_ok=True)
E = html.escape

ISSUES = {
    "period-buttons": ("Period picker is a wall of buttons", "Pick the month or quarter from a compact list."),
    "period-editable": ("Period can be changed after it was chosen", "The library already fixed the period; show it locked."),
    "atc": ("ATC picking is inconsistent", "One ATC: a combobox. Several: a multi-select combobox."),
    "validation": ("Errors are far from the field", "Show each error under the field that caused it."),
    "lists": ("Rows can't grow like the paper form", "Let schedules and payee lists add and remove rows."),
    "choice-buttons": ("Long choices shown as button rows", "More than four options fit better in a dropdown."),
    "other": ("Other", ""),
}
ORDER = ["period-buttons", "period-editable", "atc", "validation", "lists", "choice-buttons", "other"]
SEV = {"high": 3, "medium": 2, "low": 1}

CSS = """
/* Layout: a ledger of forms; each finding is a 'now' slip beside one or more 'fix' slips. */
:root{--paper:#f5f6f2;--sheet:#ffffff;--ink:#1d2846;--muted:#5c6478;--rule:#d5d9e0;--accent:#2754c5;
--bad:#b8322a;--bad-bg:#fbeceb;--good:#1f7a4d;--good-bg:#e7f4ec;--warn:#9a6a00;--warn-bg:#fbf2dc;--chip:#eef1f7;
--display:"Archivo",system-ui,sans-serif;--body:"Public Sans",system-ui,sans-serif;--mono:"IBM Plex Mono",ui-monospace,monospace}
@media (prefers-color-scheme:dark){:root:not([data-theme="light"]){--paper:#12151d;--sheet:#1a1f2b;--ink:#e6e9f2;--muted:#a3abbd;
--rule:#2e3546;--accent:#7ea2ff;--bad:#ff8a80;--bad-bg:#3a1f22;--good:#6fd3a0;--good-bg:#173126;--warn:#f0c463;--warn-bg:#352b15;--chip:#252c3c;color-scheme:dark}}
:root[data-theme="dark"]{--paper:#12151d;--sheet:#1a1f2b;--ink:#e6e9f2;--muted:#a3abbd;--rule:#2e3546;--accent:#7ea2ff;
--bad:#ff8a80;--bad-bg:#3a1f22;--good:#6fd3a0;--good-bg:#173126;--warn:#f0c463;--warn-bg:#352b15;--chip:#252c3c;color-scheme:dark}
body{background:var(--paper);color:var(--ink);font-family:var(--body);line-height:1.5;padding-inline:16px;padding-block:24px 48px}
.wrap{max-width:1080px;margin:0 auto;display:grid;gap:28px}
h1,h2,h3{font-family:var(--display);text-wrap:balance;margin:0;line-height:1.15}
h1{font-size:clamp(1.7rem,4vw,2.6rem);font-weight:800;letter-spacing:-.01em}
h2{font-size:1.35rem;font-weight:700}h3{font-size:1.05rem;font-weight:700}
p{margin:0;max-width:65ch}.muted{color:var(--muted)}
.code{font-family:var(--mono);font-weight:600;letter-spacing:.02em}
a{color:var(--accent)}a:focus-visible,button:focus-visible{outline:2px solid var(--accent);outline-offset:2px}
.eyebrow{font-family:var(--mono);font-size:.72rem;letter-spacing:.12em;text-transform:uppercase;color:var(--muted)}
.grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(150px,1fr));gap:10px}
.tile{display:grid;gap:6px;padding:12px;background:var(--sheet);border:1px solid var(--rule);border-radius:6px;text-decoration:none;color:inherit}
.tile:hover{border-color:var(--accent)}.tile .code{font-size:1.15rem}
.dots{display:flex;gap:4px;flex-wrap:wrap}.dot{width:10px;height:10px;border-radius:50%;background:var(--rule)}
.dot.high{background:var(--bad)}.dot.medium{background:var(--warn)}.dot.low{background:var(--muted)}
.legend{display:flex;gap:14px;flex-wrap:wrap;font-size:.85rem}.legend span{display:inline-flex;gap:6px;align-items:center}
.issues{display:grid;grid-template-columns:repeat(auto-fit,minmax(240px,1fr));gap:12px}
.issue{background:var(--sheet);border:1px solid var(--rule);border-radius:6px;padding:14px;display:grid;gap:8px;align-content:start}
.count{font-family:var(--display);font-size:2rem;font-weight:800;line-height:1}
.finding{background:var(--sheet);border:1px solid var(--rule);border-radius:8px;padding:16px;display:grid;gap:14px}
.fhead{display:flex;gap:10px;align-items:baseline;flex-wrap:wrap}
.sev{font-family:var(--mono);font-size:.7rem;text-transform:uppercase;letter-spacing:.1em;padding:2px 8px;border-radius:99px}
.sev.high{background:var(--bad-bg);color:var(--bad)}.sev.medium{background:var(--warn-bg);color:var(--warn)}.sev.low{background:var(--chip);color:var(--muted)}
.compare{display:grid;grid-template-columns:repeat(auto-fit,minmax(260px,1fr));gap:12px}
.slip{border:1px dashed var(--rule);border-radius:6px;padding:12px;display:grid;gap:10px;align-content:start;min-width:0}
.slip.now{border-color:var(--bad);background:var(--bad-bg)}.slip.fix{border-color:var(--good);background:var(--good-bg)}
.tag{font-family:var(--mono);font-size:.7rem;letter-spacing:.1em;text-transform:uppercase;font-weight:600}
.slip.now .tag{color:var(--bad)}.slip.fix .tag{color:var(--good)}
.where{font-family:var(--mono);font-size:.72rem;color:var(--muted);overflow-wrap:anywhere}
/* mock controls */
.mock{background:var(--sheet);border:1px solid var(--rule);border-radius:6px;padding:10px;display:grid;gap:8px;min-width:0}
.mlabel{font-size:.8rem;color:var(--muted)}
.btns{display:flex;flex-wrap:wrap;gap:4px}.btn{font-size:.75rem;padding:3px 8px;border:1px solid var(--rule);border-radius:4px;background:var(--sheet);white-space:nowrap}
.btn.on{background:var(--accent);color:var(--sheet);border-color:var(--accent)}
.combo{display:flex;justify-content:space-between;align-items:center;border:1px solid var(--rule);border-radius:4px;padding:6px 8px;font-size:.85rem;gap:6px;min-width:0}
.combo .caret{width:0;height:0;border-left:5px solid transparent;border-right:5px solid transparent;border-top:6px solid var(--muted);flex-shrink:0}
.menu{border:1px solid var(--rule);border-radius:4px;font-size:.8rem;display:grid}.menu div{padding:4px 8px;border-bottom:1px solid var(--rule)}.menu div:last-child{border-bottom:0}
.menu .hl{background:var(--chip)}
.chips{display:flex;flex-wrap:wrap;gap:4px;min-width:0}.chip{font-family:var(--mono);font-size:.72rem;background:var(--chip);border-radius:99px;padding:2px 8px;white-space:nowrap}
.lock{display:inline-flex;align-items:center;gap:8px;background:var(--chip);border-radius:4px;padding:6px 10px;font-size:.85rem;width:fit-content}
.lock svg{width:12px;height:12px;fill:var(--muted)}
.field{display:grid;gap:4px}.input{border:1px solid var(--rule);border-radius:4px;padding:6px 8px;font-size:.85rem}
.input.err{border-color:var(--bad)}.errmsg{font-size:.75rem;color:var(--bad)}
.panel{border:1px solid var(--warn);background:var(--warn-bg);border-radius:4px;padding:8px;font-size:.78rem;display:grid;gap:4px}
.table{display:grid;font-size:.78rem;border:1px solid var(--rule);border-radius:4px;overflow:hidden}
.trow{display:grid;grid-template-columns:28px 1fr 1fr 28px;border-bottom:1px solid var(--rule)}.trow>div{padding:4px 6px;border-right:1px solid var(--rule)}
.trow>div:last-child{border-right:0;text-align:center;color:var(--muted)}.trow.h{background:var(--chip);font-weight:600}
.add{font-size:.78rem;color:var(--accent);font-weight:600}
.dialog{border:1px solid var(--bad);border-radius:6px;padding:10px;font-size:.82rem;display:grid;gap:4px;background:var(--sheet)}
.crumbs{display:flex;gap:12px;flex-wrap:wrap;font-size:.85rem}
.facts{display:grid;grid-template-columns:repeat(auto-fit,minmax(180px,1fr));gap:8px}
.fact{background:var(--sheet);border:1px solid var(--rule);border-radius:6px;padding:10px;display:grid;gap:2px}
.fact b{font-family:var(--display);font-size:1.4rem}
.ok{color:var(--good);font-weight:600}
@media (prefers-reduced-motion:no-preference){.tile{transition:border-color .15s}}
"""
FONTS = '<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Archivo:wght@600;700;800&family=IBM+Plex+Mono:wght@400;600&family=Public+Sans:wght@400;600&display=swap">'
LOCK = '<svg viewBox="0 0 12 12" aria-hidden="true"><path d="M3 5V3.5a3 3 0 0 1 6 0V5h1v7H2V5zm1.5 0h3V3.5a1.5 1.5 0 0 0-3 0z"/></svg>'


SCOPE = re.compile(r"commit [0-9a-f]{7}|outside (the|its|this) form|one-line registration|scope", re.I)


def keep(f):
    text = " ".join(str(f.get(k, "")) for k in ("field", "now", "why", "where"))
    return not (f.get("issue") in ("other", None) and SCOPE.search(text))


def load():
    forms = {}
    for f in sorted(os.listdir(FORMS)):
        if f.endswith(".json"):
            try:
                d = json.load(open(os.path.join(FORMS, f), encoding="utf-8"))
                d["findings"] = [x for x in d.get("findings") or [] if keep(x)]
                forms[d.get("code") or f[:-5]] = d
            except Exception as e:  # keep going; show the broken file on the index
                forms[f[:-5]] = {"code": f[:-5], "title": f"(unreadable audit: {e})", "findings": []}
    return forms


def lookup(d, field):
    for key in ("period", "choice_rows", "atc", "lists"):
        for item in d.get(key) or []:
            if item.get("label") and field and (item["label"] in field or field in item["label"]):
                return key, item
    return None, None


def m_buttons(label, opts, on=0):
    b = "".join(f'<span class="btn{" on" if i == on else ""}">{E(str(o))}</span>' for i, o in enumerate(opts[:16]))
    more = f'<span class="btn">+{len(opts)-16}</span>' if len(opts) > 16 else ""
    return f'<div class="mock"><span class="mlabel">{E(label)}</span><div class="btns">{b}{more}</div></div>'


def m_combo(label, opts, multi=False):
    first = [str(o) for o in opts[:5]] or ["…"]
    if multi:
        head = '<div class="chips">' + "".join(f'<span class="chip">{E(o)} ×</span>' for o in first[:2]) + '</div>'
    else:
        head = f'<span>{E(first[0])}</span>'
    menu = "".join(f'<div class="{"hl" if i == 0 else ""}">{E(o)}</div>' for i, o in enumerate(first))
    return (f'<div class="mock"><span class="mlabel">{E(label)}</span><div class="combo">{head}<span class="caret"></span></div>'
            f'<div class="menu">{menu}</div></div>')


def m_lock(label, value):
    return f'<div class="mock"><span class="mlabel">{E(label)}</span><span class="lock">{LOCK}{E(value)}</span><span class="mlabel">Set by the period you opened. Change it from the library.</span></div>'


def m_inline(field, msg):
    return (f'<div class="mock"><div class="field"><span class="mlabel">{E(field)}</span><div class="input err">&nbsp;</div>'
            f'<span class="errmsg">{E(msg)}</span></div></div>')


def m_panel(field, msg):
    return (f'<div class="mock"><div class="field"><span class="mlabel">{E(field)}</span><div class="input">&nbsp;</div></div>'
            f'<div class="mlabel">… many fields later …</div><div class="panel"><b>Needs review</b><span>{E(msg)}</span></div></div>')


def m_table(label, rows, growable):
    hdr = '<div class="trow h"><div>#</div><div>Name / TIN</div><div>Amount</div><div></div></div>'
    body = "".join(f'<div class="trow"><div>{i+1}</div><div>&nbsp;</div><div>&nbsp;</div><div>{"×" if growable else ""}</div></div>' for i in range(rows))
    add = '<span class="add">+ Add row</span>' if growable else '<span class="mlabel">No way to add more rows</span>'
    return f'<div class="mock"><span class="mlabel">{E(label)}</span><div class="table">{hdr}{body}</div>{add}</div>'


def m_text(text):
    return f'<div class="mock"><span>{E(text)}</span></div>'


def render_now(f, d):
    issue, field, now = f.get("issue", "other"), f.get("field", ""), f.get("now", "")
    _, item = lookup(d, field)
    opts = (item or {}).get("options") or []
    if issue in ("period-buttons", "choice-buttons") or (issue == "atc" and (item or {}).get("control") in ("chips", "button-row")):
        if not opts:
            opts = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"] if issue == "period-buttons" else ["Option 1", "Option 2", "Option 3", "Option 4", "Option 5"]
        return m_buttons(field, opts)
    if issue == "period-editable":
        return m_buttons(field, opts or ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"], on=3)
    if issue == "validation":
        return m_panel(field or "Field", now)
    if issue == "lists":
        n = (item or {}).get("rows_in_ui") or ""
        rows = int(re.search(r"\d+", str(n)).group()) if re.search(r"\d+", str(n)) else 2
        return m_table(field, min(max(rows, 1), 4), False)
    return m_text(now)


def render_option(o, f, d):
    name, detail = o.get("name", ""), o.get("detail", "")
    low = (name + " " + detail).lower()
    field = f.get("field", "")
    _, item = lookup(d, field)
    opts = (item or {}).get("options") or (["Jan", "Feb", "Mar", "Apr", "May"] if f.get("issue", "").startswith("period") else ["Option 1", "Option 2", "Option 3"])
    if "multi" in low:
        m = m_combo(field, opts, multi=True)
    elif any(k in low for k in ("read-only", "readonly", "lock", "chip", "display")) and f.get("issue", "").startswith("period"):
        m = m_lock(field, str(opts[min(3, len(opts) - 1)]) if opts else "April 2026")
    elif any(k in low for k in ("combo", "select", "dropdown")):
        m = m_combo(field, opts)
    elif "segment" in low:
        m = m_buttons(field, opts[:3])
    elif "inline" in low or f.get("issue") == "validation":
        m = m_inline(field or "Field", f.get("now", "") or "This field is required.")
    elif f.get("issue") == "lists" or any(k in low for k in ("add", "row", "grow")):
        m = m_table(field, 2, True)
    else:
        return f'<div class="slip fix"><span class="tag">Option · {E(name)}</span>{m_text(detail or name)}</div>'
    return f'<div class="slip fix"><span class="tag">Option · {E(name)}</span>{m}<p class="muted">{E(detail)}</p></div>'


def page(title, body, back=True):
    nav = '<div class="crumbs"><a href="../index.html">All forms</a></div>' if back else ""
    return f"<title>{E(title)}</title>{FONTS}<style>{CSS}</style><div class=\"wrap\">{nav}{body}</div>"


def worst(d):
    return max([SEV.get(f.get("severity"), 0) for f in d.get("findings") or []] or [0])


def form_page(code, d, global_findings):
    fs = sorted(d.get("findings") or [], key=lambda f: (-SEV.get(f.get("severity"), 0), ORDER.index(f.get("issue")) if f.get("issue") in ORDER else 99))
    pv = d.get("print_preview") or {}
    facts = [
        ("Findings", len(fs)),
        ("Period controls", len(d.get("period") or [])),
        ("ATC fields", len(d.get("atc") or [])),
        ("Growable lists", len(d.get("lists") or [])),
    ]
    facts_html = '<div class="facts">' + "".join(f'<div class="fact"><span class="eyebrow">{E(k)}</span><b>{v}</b></div>' for k, v in facts) + "</div>"
    v = d.get("validation") or {}
    val = "Inline, next to fields" if v.get("inline") else ("Only in the Needs Review panel" if v.get("needs_review_panel") else "Unclear")
    meta = (f'<p class="muted">Page <span class="code">{E(d.get("page", ""))}</span> · {E(d.get("page_kind", ""))} · '
            f'Errors: {E(val)} · Print preview: <span class="code">{E(pv.get("slug", "none"))}</span></p>')
    items = []
    for f in global_findings + fs:
        title = ISSUES.get(f.get("issue"), ISSUES["other"])[0]
        opts = "".join(render_option(o, f, d) for o in (f.get("options") or []))
        items.append(
            f'<section class="finding" id="{E(f.get("id", ""))}"><div class="fhead"><span class="sev {E(f.get("severity", "low"))}">{E(f.get("severity", ""))}</span>'
            f'<h3>{E(title)}</h3><span class="muted">{E(f.get("field", ""))}</span></div>'
            f'<div class="compare"><div class="slip now"><span class="tag">Now</span>{render_now(f, d)}<p class="muted">{E(f.get("why", ""))}</p></div>{opts}</div>'
            f'<span class="where">{E(f.get("where", ""))}</span></section>')
    if not items:
        items.append('<p class="ok">No problems found on this page.</p>')
    body = (f'<header style="display:grid;gap:8px"><span class="eyebrow">BIR Form</span><h1><span class="code">{E(code)}</span></h1>'
            f'<p>{E(d.get("title", ""))}</p>{meta}</header>{facts_html}' + "".join(items))
    return f'<article class="report wrap" id="r-{E(code)}" hidden><div class="crumbs"><a href="#all">All forms</a></div>{body}</article>'


def main():
    forms = load()
    g = json.load(open(os.path.join(AUDIT, "global.json"))) if os.path.exists(os.path.join(AUDIT, "global.json")) else []
    reports = "".join(form_page(code, d, []) for code, d in sorted(forms.items()))
    counts = {k: [] for k in ORDER}
    for code, d in forms.items():
        for f in d.get("findings") or []:
            counts.setdefault(f.get("issue", "other"), []).append(code)
    issue_cards = "".join(
        f'<div class="issue"><span class="count">{len(set(counts[k]))}</span><h3>{E(ISSUES[k][0])}</h3><p class="muted">{E(ISSUES[k][1])}</p>'
        f'<div class="chips">' + "".join(f'<a class="chip" href="#{E(c)}">{E(c)}</a>' for c in sorted(set(counts[k]))) + '</div></div>'
        for k in ORDER if counts.get(k))
    tiles = "".join(
        f'<a class="tile" href="#{E(c)}"><span class="code">{E(c)}</span><span class="dots">'
        + "".join(f'<span class="dot {E(f.get("severity", "low"))}" title="{E(f.get("issue", ""))}"></span>' for f in (d.get("findings") or []))
        + f'</span><span class="muted" style="font-size:.75rem">{len(d.get("findings") or [])} findings</span></a>'
        for c, d in sorted(forms.items()))
    glob = "".join(
        f'<section class="finding"><div class="fhead"><span class="sev {E(x.get("severity", "high"))}">{E(x.get("severity", ""))}</span><h3>{E(x.get("title", ""))}</h3></div>'
        f'<div class="compare"><div class="slip now"><span class="tag">Now</span><div class="dialog"><b>Print preview failed</b><span>{E(x.get("now", ""))}</span></div>'
        f'<p class="muted">{E(x.get("why", ""))}</p></div>' + "".join(
            f'<div class="slip fix"><span class="tag">Option · {E(o.get("name", ""))}</span>{m_text(o.get("detail", ""))}</div>' for o in x.get("options") or [])
        + f'</div><span class="where">{E(x.get("where", ""))}</span></section>' for x in g)
    legend = '<div class="legend"><span><i class="dot high"></i>Blocks filing or loses data</span><span><i class="dot medium"></i>Confusing or wasteful</span><span><i class="dot low"></i>Polish</span></div>'
    body = (f'<header style="display:grid;gap:10px"><span class="eyebrow">e-BIRForms desktop · {len(forms)} of 43 forms audited</span>'
            f'<h1>Form Fields Audit</h1><p>Every form page, checked for the six things that make filling a return slow or confusing. '
            f'Open a form to see each problem next to the ways to fix it.</p>{legend}</header>'
            f'{glob}<h2>What we found, across all forms</h2><div class="issues">{issue_cards}</div>'
            f'<h2>Pick a form</h2><div class="grid">{tiles}</div>')
    router = ('<script>(function(){function show(){var h=(location.hash||"").slice(1);var r=h&&document.getElementById("r-"+h);'
              'document.getElementById("home").hidden=!!r;document.querySelectorAll(".report").forEach(function(e){e.hidden=e!==r});'
              'window.scrollTo(0,0)}window.addEventListener("hashchange",show);show()})();</script>')
    html_out = f"<title>Form Fields Audit</title>{FONTS}<style>{CSS}</style><main><div class=\"wrap\" id=\"home\">{body}</div>{reports}</main>{router}"
    open(os.path.join(OUT, "index.html"), "w", encoding="utf-8").write(html_out)
    print(len(forms), "forms;", sum(len(d.get("findings") or []) for d in forms.values()), "findings")


main()
