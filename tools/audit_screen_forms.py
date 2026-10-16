#!/usr/bin/env python3
"""Static audit of the on-screen form schema (app/forms/forms-schema.json).

Run from anywhere (stdlib only):

    python3 -I tools/audit_screen_forms.py [--repo PATH] [--out DIR]

For every form, item and field of the schema it gathers every label source the
repository carries and flags wrong labels and wrong controls:

  schema        app/forms/forms-schema.json                 (what the app shows)
  fields        rules/forms/<id>/fields.json                (official inventory)
  agent         crates/bir-core/data/agent-forms/<id>.json  (boxes + bindings)
  painted       crates/bir-desktop/src/views/form_<c>_view.rs (human wording)
  paper         html-frozen/<slug>/index.html text layer    (printed captions)
  writer        html-frozen/<slug>/writer-cells.json        (printed captions per key)

Painted labels are tied to official keys through the agent definition's binding
paths: a view assigns `draft.<field>` from an input whose local key carries a
label, and the binding maps an official key to `/<field>`.

Outputs (never applied anywhere):

  <out>/static.json                    machine-readable findings
  <out>/static.md                      per-form summary, findings, fix classes
  <out>/schema-overrides.proposed.json high-confidence fixes only

Every source is read-only. The tool writes only under <out>.
"""
import argparse
import html
import json
import re
from collections import Counter, OrderedDict, defaultdict
from pathlib import Path

# --------------------------------------------------------------------------
# generic text helpers

WORD = re.compile(r"[A-Za-z][A-Za-z']*")
STOP = {"of", "and", "or", "the", "for", "to", "in", "on", "a", "an", "per", "by", "from", "with", "if", "is", "at", "as", "be", "no", "not"}
KEY_PREFIX = re.compile(r"^(txt|rtn|opt|chk|sel|cbo|rdo|drp|ddl|lst|hdn|frm|cb|op|rb|ch|txa|num|amt|dt|lbl|item)(?=[A-Z_0-9])")
ABBREV = {
    "rtn": "Return", "amt": "Amount", "num": "Number", "tot": "Total", "prev": "Previous", "ded": "Deduction",
    "qtr": "Quarter", "addr": "Address", "desc": "Description", "cde": "Code", "sched": "Schedule", "pct": "Percent",
    "yr": "Year", "mo": "Month", "mon": "Monthly", "semi": "Semi-Annual", "fam": "Family", "prop": "Property",
    "conj": "Conjugal", "exc": "Exclusive", "fmv": "FMV", "atc": "ATC", "tin": "TIN", "rdo": "RDO", "zip": "ZIP",
    "inst": "Installment", "ext": "Extension", "tel": "Telephone", "nrc": "NRC", "pg": "Page",
}
BASE_VOCAB = set("""
yes no item items month day year date code name address number amount total tax due rate return amended
email telephone contact zip branch registered taxpayer quarter calendar fiscal sheets attached schedule
others other specify please kind payment cash check bank debit memo interest surcharge compromise penalties
category resident non alien executor administrator estate treaty special law relief installment extension
granted settled judicially frequency monthly quarterly semi annual graduated income percent percentage
online username password secret enrollment serialized imports exports
""".split())


def norm_alnum(s):
    return re.sub(r"[^a-z0-9]", "", s.lower())


def tokens(s):
    return [t.lower() for t in WORD.findall(s or "")]


def content_tokens(s):
    return {t for t in tokens(s) if t not in STOP and len(t) > 1}


def similar(a, b):
    if norm_alnum(a) == norm_alnum(b) and norm_alnum(a):
        return 1.0
    ta, tb = content_tokens(a), content_tokens(b)
    if not ta or not tb:
        return 1.0 if set(tokens(a)) == set(tokens(b)) and tokens(a) else 0.0
    if ta <= tb or tb <= ta:
        return max(0.6, len(ta & tb) / len(ta | tb))
    return len(ta & tb) / len(ta | tb)


ORDINAL = {"first": "1st", "second": "2nd", "third": "3rd", "fourth": "4th"}


def meaning_tokens(s):
    out = set()
    for t in re.findall(r"[A-Za-z0-9%]+", s or ""):
        t = t.lower()
        t = ORDINAL.get(t, t)
        if t in STOP:
            continue
        out.add(t[:5] if t.isalpha() else t)
    return out


def agree(a, b):
    """Same choice in different words ('refunded'/'Refund', '1st'/'First', '8%'/'8% income tax rate')."""
    ta, tb = meaning_tokens(a), meaning_tokens(b)
    if not ta or not tb:
        return norm_alnum(a) == norm_alnum(b)
    return len(ta & tb) >= max(1, 0.5 * min(len(ta), len(tb)))


def key_tail(k):
    k = k.split("#", 1)[0]
    if ":" in k and re.match(r"(?i)frm", k):
        k = k.split(":", 1)[1]
    return k


def strip_item_prefix(label, no=None):
    """'Item 12A — If yes, specify' / '12 Tax relief ...' / '5. Number of ...' -> label text."""
    s = label.strip()
    s = re.sub(r"^(?:Page\s+\d+\s+)?(?:Part\s+[IVX\d]+\s+)?(?:Items?|Line)\s+\d{1,3}[A-Z]?\s*(?:—|–|-|\.|:)?\s*", "", s)
    s = re.sub(r"^\d{1,3}[A-Z]?\s*(?:—|–|-|\.|:)\s+", "", s)
    s = re.sub(r"^\d{1,3}\s?[A-Z]\.\s+", "", s)
    if no:
        s = re.sub(r"^%s\s+(?=[A-Z(])" % re.escape(no), "", s)
    s = re.sub(r"^\d{1,3}[A-Z]?\s+(?=[A-Z(][a-z(A-Z])", "", s)
    return s.strip()


def column_label(text):
    """'4 Others (specify), row 3 Tax Rate' -> 'Tax Rate'; 'XP040 row, Imported column' -> 'Imported'."""
    m = re.search(r"(?i)\brow\s*\d*\s*[,:]?\s+(.+?)(?:\s+column)?$", text)
    if m:
        return m.group(1).strip()
    return text


def humanize_key(k):
    """Best-effort readable label from an official key (low confidence)."""
    segs = [KEY_PREFIX.sub("", x) for x in re.sub(r"\{index\}", "", key_tail(k)).split(":") if x.strip("_")]
    t = " ".join(segs) if segs else key_tail(k)
    t = re.sub(r"^frm\w*?v\d{4}", "", t)
    m = re.match(r"^Pg(\d+)(?:Pt(\d+))?I(\d+[A-Z]?)(.*)$", t)
    pre = ""
    if m:
        pre = "Item %s " % m.group(3)
        t = m.group(4)
    t = re.sub(r"([a-z])([A-Z])", r"\1 \2", t)
    t = re.sub(r"([A-Z]+)([A-Z][a-z])", r"\1 \2", t)
    t = re.sub(r"([A-Za-z])(\d)", r"\1 \2", t)
    t = re.sub(r"[_\-]+", " ", t)
    words = []
    for w in t.split():
        lw = w.lower()
        words.append(ABBREV.get(lw, w if w.isupper() else w.capitalize()))
    out = (pre + " ".join(words)).strip()
    return out or "Value"


def humanize_value(v):
    if v is True:
        return "Yes"
    if v is False:
        return "No"
    s = re.sub(r"([a-z])([A-Z])", r"\1 \2", str(v)).replace("_", " ").strip()
    s = re.sub(r"\b(Or|And|Of|To)\b", lambda m: m.group(1).lower(), s)
    s = re.sub(r"\beight percent\b", "8%", s)
    return s[:1].upper() + s[1:]


# --------------------------------------------------------------------------
# paper (frozen sheet) text layer — same reading as docs/ui-redesign/forms_extract.py

RUN = re.compile(r'<div class="t" id="p(\d+)t\d+" style="left:([\d.]+)pt;top:([\d.]+)pt;[^"]*?font-size:([\d.]+)pt;font-weight:(\d+)[^"]*">(.*?)</div>', re.S)
TAG = re.compile(r"<[^>]+>")


def clean(t):
    t = re.sub(r"\s+", " ", t).strip(" :.-–—")
    t = re.sub(r"\b([B-HJ-Z]) ([a-z]{2,})", r"\1\2", t)
    t = re.sub(r"(?:\s+(?:Yes|No))+$", "", t)
    t = re.sub(r"(?:\s+[/0-9])+$", "", t)
    t = re.sub(r"\s+\)", ")", t).replace("( ", "(")
    return t.strip()


def paper_runs(repo, slug):
    p = repo / "html-frozen" / slug / "index.html"
    if not p.exists():
        return []
    s = p.read_text(errors="replace")
    runs = []
    for m in RUN.finditer(s):
        txt = html.unescape(TAG.sub("", m.group(6))).replace("\xa0", " ").strip()
        if txt:
            runs.append((int(m.group(1)), float(m.group(3)), float(m.group(2)), txt))
    runs.sort()
    return runs


def paper_items(runs, item_numbers):
    """(page, item) -> printed caption text following the item number."""
    lines = []
    for r in runs:
        if lines and lines[-1][0] == r[0] and abs(lines[-1][1] - r[1]) <= 2.0:
            lines[-1][2].append(r)
        else:
            lines.append([r[0], r[1], [r]])
    out = {}
    for page, top, rs in lines:
        rs.sort(key=lambda r: r[2])
        i = 0
        while i < len(rs):
            m = re.match(r"^(\d{1,3}[A-Z]?)(?:\s+(.*))?$", rs[i][3])
            if m and m.group(1) in item_numbers and (page, m.group(1)) not in out:
                parts = [m.group(2)] if m.group(2) else []
                j = i + 1
                while j < len(rs):
                    nm = re.match(r"^(\d{1,3}[A-Z]?)(?:\s|$)", rs[j][3])
                    if nm and nm.group(1) in item_numbers and nm.group(1) != m.group(1):
                        break
                    parts.append(rs[j][3])
                    j += 1
                out[(page, m.group(1))] = clean(" ".join(clean(x) for x in parts if clean(x)))
                i = j
            else:
                i += 1
    return out


# --------------------------------------------------------------------------
# painted views (bir-desktop) — human wording tied to draft paths

RUST_STR = re.compile(r'"((?:[^"\\\n]|\\.)*)"')
NUMBERED = re.compile(r"^(?:Items?\s+|Line\s+)?(\d{1,3}[A-Z]?)\s*(?:—|–|-|\.|:)\s+(\S.*)$|^Items?\s+(\d{1,3}[A-Z]?)\s+(\S.*)$")
DRAFT = r"(?:d|draft|self\.draft|this\.draft)"
CHAIN = r"([a-z_][a-z0-9_]*(?:\.[a-z_][a-z0-9_]*)*)"
SKIP_DRAFT = {"lifecycle", "status", "id", "tin", "profile_id", "filing_status", "is_validated"}


def draft_chain(text_after):
    """'due_date.map(|d| ...)' -> 'due_date'; 'ordinary_deductions.others_description' kept."""
    m = re.match(r"([a-z_][a-z0-9_]*)((?:\.[a-z_][a-z0-9_]*)*)", text_after)
    if not m:
        return None
    segs = [m.group(1)] + [x for x in m.group(2).split(".") if x]
    out, pos = [], 0
    s = text_after
    for seg in segs:
        idx = s.find(seg, pos)
        end = idx + len(seg)
        if re.match(r"\s*\(", s[end:]) or seg in ("clone", "to_string", "as_str", "map", "unwrap_or_default", "trim", "len"):
            break
        out.append(seg)
        pos = end
    return ".".join(out) if out else None


def painted_index(view_path):
    """Return dict with numbered labels, local->label, draft->local, draft->label."""
    res = {"numbered": defaultdict(list), "local_label": {}, "draft_local": defaultdict(set),
           "draft_label": defaultdict(list), "strings": [], "path": str(view_path)}
    if not view_path.exists():
        return res
    src = view_path.read_text()
    lines = src.split("\n")
    line_of = []
    n = 0
    for i, ln in enumerate(lines):
        line_of.extend([i] * (len(ln) + 1))
    # every human-looking literal
    for m in RUST_STR.finditer(src):
        s = m.group(1)
        if re.search(r"[A-Za-z]{2}", s) and " " in s or re.fullmatch(r"[A-Z][a-z]+", s):
            res["strings"].append(s)
        nm = NUMBERED.match(s)
        if nm and not re.search(r"\{\}|\{[A-Za-z_0-9]+\}", s) and len(s) < 140:
            no = nm.group(1) or nm.group(3)
            text = (nm.group(2) or nm.group(4)).strip()
            if re.match(r"^[a-z]", text) and len(text.split()) <= 2:
                continue  # error-message tags like "Item 1 month"
            res["numbered"][no].append((text, line_of[m.start()] + 1))
    # (local, label, ...) tuples
    for m in re.finditer(r'\(\s*"([a-z0-9_]+)"\s*,\s*"((?:[^"\\]|\\.)+)"', src):
        res["local_label"].setdefault(m.group(1), m.group(2))
    # local -> draft field
    for m in re.finditer(r'"([a-z0-9_]+)"\s*=>\s*&?' + DRAFT + r"\.([a-z_][^\n,;{}]*)", src):
        ch = draft_chain(m.group(2))
        if ch:
            res["draft_local"][ch].add(m.group(1))
    for m in re.finditer(r'"([a-z0-9_]+)"\s*=>\s*&\[\s*"([a-z0-9_]+)"', src):
        res["draft_local"][m.group(2)].add(m.group(1))
    for stmt in re.split(r"[;}]", src):
        m = re.search(r"\b" + DRAFT + r"\.([a-z_][a-z0-9_.]*)\s*=[^=][^;]*?\(\s*\"([a-z0-9_]+)\"", stmt)
        if m:
            ch = draft_chain(m.group(1))
            if ch:
                res["draft_local"][ch].add(m.group(2))
    for m in re.finditer(r'\(\s*"([a-z0-9_]+)"[^;{]*\)\s*\{\s*' + DRAFT + r"\.([a-z_][a-z0-9_.]*)\s*=", src):
        ch = draft_chain(m.group(2))
        if ch:
            res["draft_local"][ch].add(m.group(1))
    # direct: a numbered label literal followed closely by get_error("x") / draft.x
    for no, lst in res["numbered"].items():
        for text, ln in lst:
            window = "\n".join(lines[ln - 1: ln + 25])
            m = re.search(r'get_error\("([a-z0-9_]+)"\)|\bdraft\.([a-z_][a-z0-9_]*)', window)
            if m:
                f = m.group(1) or m.group(2)
                if f not in SKIP_DRAFT:
                    res["draft_label"][f].append(("%s — %s" % (no, text), "window", ln))
    for ch, locs in res["draft_local"].items():
        for loc in sorted(locs):
            if loc in res["local_label"]:
                res["draft_label"][ch].append((res["local_label"][loc], "local:" + loc, 0))
    return res


def path_candidates(path):
    segs = [s for s in path.strip("/").split("/") if s and not s.isdigit()]
    if not segs:
        return []
    out = [".".join(segs)]
    if len(segs) > 1:
        out.append(".".join(segs[-2:]))
    out.append(segs[-1])
    return out


# --------------------------------------------------------------------------
# label diagnostics

GENERIC = re.compile(r"^(?:(?:page \d+ )?(?:part [ivx\d]+ )?item \d+[a-z]? (?:amount|value|field|text)|yes|no|item\s*#?\s*\d+[a-z]?|option\s*\d+|row\s*\d*|column\s*\d*|value|field|additional details|\d{1,3}[a-z]?|if yes,? specify|page \d+)$", re.I)
KEYISH = re.compile(r"\bPg\d|Pt\d+I\d|I\d+[A-Z]?[a-z]|\bfrm\w|\btxt[A-Z]|\bdrp[A-Z]|\bddl\b|\bDdl\b|\bopt[A-Z]|\brdo[A-Z]|\bsel[A-Z]|\{index\}|v\d{4}")


def _dict_words():
    p = Path("/usr/share/dict/words")
    if not p.exists():
        return frozenset()
    return frozenset(w.strip().lower() for w in p.read_text(errors="replace").split())


DICT = _dict_words()


class Vocab:
    def __init__(self, words):
        self.words = {w for w in words if len(w) > 1}
        self.long = sorted(w for w in self.words if len(w) >= 4)
        self._trunc = {}
        self._spell = {}

    def known(self, t):
        return t in self.words or t in STOP or t in ABBREV

    def truncation_of(self, t, key):
        """'tegory' -> 'category' when the key carries 'category'."""
        if (t, key) in self._trunc:
            return self._trunc[(t, key)]
        kl = key.lower()
        best = None
        for m in re.finditer(r"[a-z]+", kl):
            w = m.group(0)
        cands = [w for w in self.long if w.endswith(t) and w != t and w in kl]
        if cands:
            best = max(cands, key=len)
        else:
            for m in re.finditer(r"[a-z]{3,}", re.sub(r"([a-z])([A-Z])", r"\1 \2", key_tail(key)).lower()):
                w = m.group(0)
                if w.endswith(t) and w != t and len(w) - len(t) <= 4:
                    best = w
        self._trunc[(t, key)] = best
        return best

    def spelling(self, t):
        if t in self._spell:
            return self._spell[t]
        best = None
        if len(t) >= 5 and t.isalpha() and t not in DICT and not (t[:-1] in self.words and t[-1] in "ab"):
            for w in self.long:
                if abs(len(w) - len(t)) <= 1 and len(w) >= 5 and edit1(t, w):
                    best = w
                    break
        self._spell[t] = best
        return best


def edit1(a, b):
    if a == b:
        return False
    if len(a) == len(b):
        d = [i for i in range(len(a)) if a[i] != b[i]]
        return len(d) == 1 or (len(d) == 2 and d[1] == d[0] + 1 and a[d[0]] == b[d[1]] and a[d[1]] == b[d[0]])
    if len(a) > len(b):
        a, b = b, a
    for i in range(len(b)):
        if b[:i] + b[i + 1:] == a:
            return True
    return False


def label_issues(label, key, vocab, role):
    """Return list of (category, subtype, detail). role: item|field|radio|column."""
    out = []
    L = (label or "").strip()
    if not L:
        return [("LABEL", "empty", "label is empty")]
    tail = key_tail(key) if key else ""
    nt, nl = norm_alnum(tail), norm_alnum(L)
    cryptic = []
    if re.match(r"^[a-z]+[A-Z0-9_]", L) or re.fullmatch(r"[a-z]+", L) and len(L) > 3 and not vocab.known(L):
        cryptic.append("lowercase-start key fragment")
    if "_" in L:
        cryptic.append("underscore key fragment")
    if re.search(r"[a-z][A-Z]", L) and not re.search(r"\b(?:McDonald|iPhone)\b", L):
        cryptic.append("camelCase")
    if re.search(r"[A-Z]{3,}[a-z]", L) and not re.search(r"\b[A-Z]{2,}s\b", L):
        cryptic.append("run-together acronym (e.g. ATCCode)")
    if re.search(r"[a-z]\d", L):
        cryptic.append("word glued to a digit")
    if KEYISH.search(L):
        cryptic.append("contains key syntax")
    toks = L.split()
    singles = [t for t in toks if re.fullmatch(r"[A-Za-z]", t)]
    if len(singles) >= 2 and re.search(r"\b[A-Z] [A-Z]\b", L):
        out.append(("LABEL", "letter-spaced", "single letters separated by spaces (%s)" % " ".join(singles)))
    elif toks and re.fullmatch(r"[B-HJ-Zb-hj-z]", toks[0]) and len(toks) > 1:
        cryptic.append("stray leading letter from a key prefix")
    truncs = []
    if key:
        for t in tokens(L):
            if len(t) >= 2 and not vocab.known(t):
                w = vocab.truncation_of(t, key)
                if w and w not in nl:
                    truncs.append((t, w))
    if truncs:
        cryptic.append("truncated words " + ", ".join("%s→%s" % x for x in truncs))
    t2 = re.sub(r"[^A-Za-z0-9]", "", re.split(r"[:]", tail.split("#")[0])[-1] or tail)
    if nl and len(nl) >= 3 and t2.lower().endswith(nl) and len(t2) > len(nl):
        cut = len(t2) - len(nl)
        if t2[cut].islower() and not t2[cut - 1].isdigit():
            cryptic.append("key with leading letters chopped")
    if cryptic:
        out.append(("LABEL", "cryptic", "; ".join(OrderedDict.fromkeys(cryptic))))
    if GENERIC.match(L) and role in ("item", "column"):
        out.append(("LABEL", "generic", "generic %s label" % role))
    if re.fullmatch(r"(?i)(option\s*\d+|item\s*\d+[a-z]?|\d{1,3}[a-z]?|.*\D\d)", L) and role == "radio":
        out.append(("LABEL", "generic", "radio option label does not say what the choice means"))
    if not cryptic:
        bad = []
        for t in tokens(L):
            if len(t) >= 5 and not vocab.known(t):
                w = vocab.spelling(t)
                if w:
                    bad.append((t, w))
        if bad:
            out.append(("LABEL", "misspelled", ", ".join("%s→%s?" % x for x in bad)))
    if key and not cryptic and nl and nt and (nl == norm_alnum(humanize_key(key)) or nl == norm_alnum(KEY_PREFIX.sub("", tail))):
        out.append(("LABEL", "key-derived", "label is the official key re-spaced, not printed wording"))
    return out


def is_bad_label(label, key, vocab, role):
    return any(s in ("cryptic", "generic", "letter-spaced", "empty") for _, s, _ in label_issues(label, key, vocab, role))


def good_source_label(text, key, vocab):
    if not text or len(text) > 160:
        return False
    if re.search(r"(?i)\bpage \d+ ?[a-z]\b|^item (schedule|part)\b|\bitem \d+[a-z]? [A-Z]?[a-z]*[A-Z]|dynamic row", text):
        return False
    if re.search(r"(?i)\b(be|to|the|of|and|or|for|a|an|per|by|from|with|is)\s*$", text):
        return False  # truncated caption ("If overpayment: To be")
    toks = [t for t in tokens(text) if len(t) >= 2]
    if toks and sum(1 for t in toks if vocab.known(t)) < 0.8 * len(toks):
        return False
    if key:
        nt, nx = norm_alnum(KEY_PREFIX.sub("", key_tail(key))), norm_alnum(text)
        if nx and nt.endswith(nx) and (re.search(r"\d", text) or any(not vocab.known(t) for t in tokens(text))):
            return False
    if re.fullmatch(r"[A-Za-z0-9_#:{}\-]+", text) and not re.fullmatch(r"[A-Z][a-z]+|[A-Z]{2,5}", text):
        return False
    return not is_bad_label(text, key, vocab, "field") and not re.fullmatch(r"(?i)item\s*\d+[a-z]?|page \d+ item \d+.*", text)


# --------------------------------------------------------------------------
# kinds

BINDING_KIND = {"amount": {"money", "pct"}, "whole": {"int", "sel", "date", "money"}, "text": {"text", "email", "phone", "date", "sel", "int"},
                "choice": {"radio", "chk", "sel"}}
MONTHS = ["01 - January", "02 - February", "03 - March", "04 - April", "05 - May", "06 - June", "07 - July",
          "08 - August", "09 - September", "10 - October", "11 - November", "12 - December"]
INTERNAL = re.compile(r"(?i)^(ebirOnline\w*|txtEnroll|txtFinalFlag|h[A-Z]\w*TableSize|driveSelect\w*)$")
JUNK_OPT = re.compile(r'["+]|\.description|official select choices|see fixtures|runtime function|^\(select ', re.I)


def schema_fields(form):
    """Yield (part_idx, part_name, item_idx, item, field_or_None, col_idx_or_None, key)."""
    for pi, p in enumerate(form["parts"]):
        for ii, it in enumerate(p["items"]):
            if "f" in it:
                for f in it["f"]:
                    yield pi, p["n"], ii, it, f, None, f["k"]
            else:
                for ci, c in enumerate(it["cols"]):
                    k0 = next((row[ci] for row in it["keys"] if ci < len(row) and row[ci]), None)
                    yield pi, p["n"], ii, it, c, ci, k0


# --------------------------------------------------------------------------
# audit


class Audit:
    def __init__(self, repo):
        self.repo = repo
        self.schema = json.loads((repo / "app/forms/forms-schema.json").read_text())
        self.findings = []
        self.overrides = OrderedDict()
        self.open_questions = []
        self.date_groups = []
        self.coverage = {}
        self.vocab = self._vocab()

    # -- vocabulary from every printed and painted string
    def _vocab(self):
        words = set(BASE_VOCAB)
        for form in self.schema:
            for r in paper_runs(self.repo, form["s"]):
                words.update(tokens(r[3]))
        for p in (self.repo / "crates/bir-desktop/src/views").glob("*.rs"):
            for m in RUST_STR.finditer(p.read_text()):
                s = m.group(1)
                if " " in s and not re.search(r"[{}_]", s):
                    words.update(tokens(s))
        # drop key debris that leaked into prose sources
        return Vocab({w for w in words if not re.search(r"\d", w)})

    def add(self, form, part, item, k, category, subtype, severity, current, proposed, confidence, evidence, note=""):
        self.findings.append(OrderedDict([
            ("form", form["c"]), ("part", part), ("item", item), ("k", k), ("category", category), ("type", subtype),
            ("severity", severity), ("current", current), ("proposed", proposed), ("confidence", confidence),
            ("evidence", evidence), ("note", note)]))

    def override(self, code, key, entry):
        ov = self.overrides.setdefault(code, OrderedDict())
        cur = ov.setdefault(key, OrderedDict())
        ev = cur.get("evidence")
        for k, v in entry.items():
            if k == "evidence":
                continue
            cur[k] = v
        cur["evidence"] = (ev + " | " if ev else "") + entry["evidence"]

    # -- per-form context
    def context(self, form):
        repo = self.repo
        fid = form["id"]
        fj = json.loads((repo / "rules/forms" / fid / "fields.json").read_text())
        fields = {f["field_key"]: f for f in fj["fields"]}
        ap = repo / "crates/bir-core/data/agent-forms" / (fid + ".json")
        agent = json.loads(ap.read_text()) if ap.exists() else {"boxes": [], "bindings": {}, "unmapped": {}}
        boxes = {b["key"]: b for b in agent["boxes"]}
        bindings = agent.get("bindings", {})
        view = repo / "crates/bir-desktop/src/views" / ("form_%s_view.rs" % form["c"].lower().replace("-", ""))
        painted = painted_index(view)
        runs = paper_runs(repo, form["s"])
        item_numbers = {str(f["item_number"]) for f in fj["fields"] if f["item_number"] not in (None, "")}
        item_numbers |= {it["no"] for p in form["parts"] for it in p["items"] if it.get("no")}
        item_numbers |= set(painted["numbered"].keys())
        paper = paper_items(runs, item_numbers)
        writer = {}
        wp = repo / "html-frozen" / form["s"] / "writer-cells.json"
        if wp.exists():
            w = json.loads(wp.read_text())
            for sec in ("joins", "money_joins", "xbox_joins"):
                for j in w.get(sec, []):
                    if j.get("printed"):
                        writer[j["writer_key"]] = j["printed"]
        # painted label per official key, through binding paths
        painted_by_key = {}
        for k, b in bindings.items():
            if "path" not in b:
                continue
            for cand in path_candidates(b["path"]):
                labs = painted["draft_label"].get(cand)
                if labs:
                    painted_by_key[k] = (labs, b["path"], cand)
                    break
        return {"fields": fields, "boxes": boxes, "bindings": bindings, "painted": painted, "paper": paper,
                "writer": writer, "painted_by_key": painted_by_key, "view": view, "unmapped": agent.get("unmapped", {})}

    # -- label candidates for one key
    def candidates(self, ctx, k, no):
        c = []  # (text, source, strength 3=high 2=medium 1=low)
        if not k:
            return c
        base = k.replace("{index}", "{index}")
        pk = ctx["painted_by_key"].get(base)
        if pk:
            labs, path, cand = pk
            for text, how, ln in labs:
                t = strip_item_prefix(text, no)
                c.append((t, "painted %s (%s via binding %s%s)" % (ctx["view"].name, how, path, ", line %d" % ln if ln else ""),
                          3 if how.startswith("local:") else 2))
        b = ctx["boxes"].get(k)
        if b and good_source_label(strip_item_prefix(b.get("label", ""), b.get("item") or no), k, self.vocab):
            c.append((strip_item_prefix(b["label"], b.get("item") or no), "agent box label", 3))
        f = ctx["fields"].get(k)
        if f and f["label"] != f["field_key"] and good_source_label(strip_item_prefix(f["label"], no), k, self.vocab):
            c.append((strip_item_prefix(f["label"], no), "fields.json label", 2))
        if k in ctx["writer"]:
            c.append((strip_item_prefix(ctx["writer"][k]), "writer-cells printed caption", 2))
        return c

    def item_number_hint(self, ctx, k):
        """Item number from fields.json / agent box / painted label."""
        f = ctx["fields"].get(k)
        if f and f["item_number"] not in (None, ""):
            return str(f["item_number"]), "fields.json item_number"
        b = ctx["boxes"].get(k)
        if b and b.get("item"):
            return str(b["item"]), "agent box item"
        pk = ctx["painted_by_key"].get(k)
        if pk:
            for text, how, ln in pk[0]:
                m = NUMBERED.match(text)
                if m:
                    return (m.group(1) or m.group(3)), "painted label"
        return None, None

    # -- main per-form audit
    def audit_form(self, form):
        ctx = self.context(form)
        self._radios = defaultdict(list)
        for pi_, pn_, ii_, it_, f_, ci_, k_ in schema_fields(form):
            if ci_ is None and f_["t"] == "radio" and k_:
                self._radios[radio_family(k_)].append(((pn_, it_.get("no"), id(it_)), k_))
        code = form["c"]
        vocab = self.vocab
        cov = {"fields": 0, "painted": 0, "agent": 0, "fields_label": 0}
        all_nos = [it["no"] for p in form["parts"] for it in p["items"] if it.get("no")]
        no_set = set(all_nos) | set(ctx["painted"]["numbered"].keys()) | {
            str(f["item_number"]) for f in ctx["fields"].values() if f["item_number"] not in (None, "")}

        # ---------------- items
        seen_no = defaultdict(list)
        for pi, p in enumerate(form["parts"]):
            items = p["items"]
            for ii, it in enumerate(items):
                ikey = "item:%d:%d" % (pi, ii)
                no = it.get("no", "")
                if no:
                    seen_no[no].append((pi, ii, p["n"]))
                lab = it["l"]
                first_k = (it["f"][0]["k"] if "f" in it and it["f"] else (it["keys"][0][0] if it.get("keys") else None))
                issues = label_issues(lab, first_k, vocab, "item")
                # merged items: the label carries another item's number and caption
                mm = None
                for m in re.finditer(r"(?<![\w./-])(\d{1,3}[A-Z]?)\s+(?=[A-Z(][A-Za-z])", lab):
                    later = no and natkey(m.group(1)) > natkey(no)
                    if m.start() > 0 and (m.group(1) in no_set or later) and m.group(1) != no and not re.search(r"(?i)(no\.|form|section|sec\.|rr|rule|article|annex|of|items?|row|and|less|plus|sum|to|minus|schedule|sched|part|page|column|line|quarter|or|&|,|\()\s*$", lab[:m.start()]):
                        mm = m
                        break
                proposal, conf, ev = None, None, []
                pl = self.painted_item_label(ctx, no)
                if pl:
                    ev.append("painted %s: %r" % (ctx["view"].name, pl[0]))
                paper = self.paper_label(ctx, no, it.get("p"))
                if paper:
                    ev.append("paper: %r" % paper)
                grp = self.group_box_label(ctx, it)
                if grp:
                    ev.append("agent group box: %r" % grp)
                if mm:
                    head = re.sub(r"[\s/]+$", "", lab[:mm.start()]).strip(" -–—:")
                    if not re.sub(r"(?i)\b(yes|no)\b|[/\s]", "", head):
                        head = (grp or (pl[0] if pl else "") or "").strip()
                    self.add(form, p["n"], no, ikey, "LABEL", "merged", "high", lab, head, "high" if head else "medium",
                             ["schema item label"] + ev, "label absorbed item %s's caption %r" % (mm.group(1), lab[mm.start():]))
                    if head:
                        self.override(code, ikey, {"label": head, "evidence": "item %s label carried item %s's caption; trimmed at %r" % (no, mm.group(1), lab[mm.start():])})
                bad = [s for s in issues if s[1] in ("cryptic", "generic", "letter-spaced", "empty")]
                # radio-only item whose label is a choice word ("Yes") means the question is missing
                if "f" in it and lab.strip().lower() in ("yes", "no") and not any(i[1] == "generic" for i in bad):
                    bad.append(("LABEL", "generic", "item label is a choice word"))
                if bad:
                    cands = []
                    if grp:
                        cands.append((grp, "agent group box", 3))
                    if pl:
                        cands.append((pl[0], "painted", 3 if pl[1] else 2))
                    if paper and not is_bad_label(paper, None, vocab, "item"):
                        cands.append((paper, "paper", 2))
                    proposal, conf, srcs = self.choose(cands)
                    if "f" in it and len(it["f"]) == 1 and not proposal:
                        fk = it["f"][0]["k"]
                        fc = self.candidates(ctx, fk, no)
                        proposal, conf, srcs = self.choose(fc)
                    if not proposal and first_k and not no:
                        proposal, conf, srcs = humanize_key(first_k), "low", ["humanized key"]
                    detail = "; ".join(d for _, _, d in bad) or "label disagrees with painted and printed wording"
                    sev = "high" if any(s[1] in ("cryptic", "letter-spaced", "empty") for s in bad) or lab.lower() in ("yes", "no") else "medium"
                    self.add(form, p["n"], no, ikey, "LABEL", bad[0][1] if bad else "wrong", sev, lab, proposal, conf or "low",
                             ["schema item label"] + ev, detail)
                    if proposal and conf == "high":
                        self.override(code, ikey, {"label": proposal, "evidence": "item %s: %s" % (no or "(unnumbered)", "; ".join(srcs))})
                    elif proposal is None or conf != "high":
                        self.open_item(form, p["n"], no, ikey, lab, cands if (pl or paper or grp) else [])
                for cat, sub, d in issues:
                    if sub == "misspelled":
                        self.add(form, p["n"], no, ikey, cat, sub, "low", lab, None, "low", ["vocabulary of printed + painted text"], d)
                if not no and lab == "Additional details" and "f" in it:
                    hints = {self.item_number_hint(ctx, f["k"])[0] for f in it["f"]} - {None}
                    if hints:
                        self.add(form, p["n"], "", ikey, "LABEL", "unnumbered", "medium", lab, "item(s) %s" % ", ".join(sorted(hints, key=natkey)),
                                 "medium", ["fields.json/agent/painted item numbers"], "fields carry item numbers the schema dropped")
        # parts whose numbering restarts (several numbered sections/pages in one part)
        restarts = {}
        for pi, p in enumerate(form["parts"]):
            vals = [natkey(it["no"])[0] for it in p["items"] if it.get("no")]
            restarts[pi] = sum(1 for x, y in zip(vals, vals[1:]) if y < x - 2 and y <= 3)
            if restarts[pi] >= 2:
                self.add(form, p["n"], "", "part:%d" % pi, "LABEL", "mixed-sections", "medium", "numbering restarts %d times in one part" % restarts[pi],
                         "split the part at each restart (one section per printed page/schedule)", "medium", ["schema item order", "paper Part headings"],
                         "items from several numbered sections were grouped under one heading; duplicate and out-of-order item reports for this part are suppressed")
        # duplicated item numbers
        for no, locs in seen_no.items():
            if len(locs) > 1:
                parts = {x[2] for x in locs}
                if all(restarts.get(a, 0) >= 2 for a, _, _ in locs) and len(parts) == 1:
                    continue
                self.add(form, " / ".join(sorted(parts)), no, ",".join("item:%d:%d" % (a, b) for a, b, _ in locs), "LABEL", "duplicate-item",
                         "low" if len(parts) > 1 else "high", "item %s appears %d times" % (no, len(locs)),
                         "merge into one item" if len(parts) == 1 else "confirm the repeated section (e.g. spouse / schedule) is intended",
                         "medium" if len(parts) == 1 else "low", ["schema"],
                         "same item number in %s" % ("different parts (may be a repeated section)" if len(parts) > 1 else "the same part"))
        # out-of-order items (per part; longest increasing run)
        for pi, p in enumerate(form["parts"]):
            seq = [(ii, natkey(it["no"])) for ii, it in enumerate(p["items"]) if it.get("no")]
            keep = lis([v for _, v in seq])
            keep_idx = {seq[i][0] for i in keep}
            if len(seq) >= 3 and restarts[pi] < 2:
                for ii, v in seq:
                    if ii not in keep_idx:
                        it = p["items"][ii]
                        conf = "high" if len(seq) - len(keep) <= max(1, len(seq) // 6) else "medium"
                        self.add(form, p["n"], it["no"], "item:%d:%d" % (pi, ii), "LABEL", "out-of-order", "medium", "item %s at position %d" % (it["no"], ii),
                                 "move item %s into numeric order" % it["no"], conf, ["schema order"], "")
        # duplicated fields by occurrence
        occ = defaultdict(list)
        for pi, pn, ii, it, f, ci, k in schema_fields(form):
            if k and "#occurrence-" in k:
                occ[k.split("#")[0]].append((pi, ii, k, pn, it.get("no", "")))
        for base, lst in occ.items():
            if len(lst) > 1:
                ks = [x[2] for x in lst]
                self.add(form, lst[0][3], lst[0][4], ",".join(ks), "LABEL", "duplicate-field", "high", "%s shown %d times" % (base, len(lst)),
                         "show %s once; hide %s" % (ks[0], ", ".join(ks[1:])), "high",
                         ["fields.json serialized_occurrence", "html-frozen writer-cells not_joined"], "HTA/DOM duplicate of the same serialized key; one value, two DOM copies")
                self.open_questions.append({"form": code, "k": ",".join(ks), "question": "Duplicate occurrence of %s: hide the later copy in the app, or mirror its value on save?" % base,
                                            "candidates": ["hide %s (mirror value at serialize time)" % ", ".join(ks[1:]), "keep both and label the later one as a copy of the first"]})

        # ---------------- fields
        for pi, pn, ii, it, f, ci, k in schema_fields(form):
            no = it.get("no", "")
            cov["fields"] += 1
            role = "column" if ci is not None else ("radio" if f["t"] == "radio" else "field")
            fkey = k if ci is None else (k or "col:%d:%d:%d" % (pi, ii, ci))
            cands = self.candidates(ctx, k, no) if k else []
            if any(s[1].startswith("painted") for s in cands):
                cov["painted"] += 1
            if any(s[1] == "agent box label" for s in cands):
                cov["agent"] += 1
            if any(s[1] == "fields.json label" for s in cands):
                cov["fields_label"] += 1
            lab = f["l"]
            issues = label_issues(lab, k, self.vocab, role)
            if role == "radio":
                self.audit_radio(form, ctx, pn, no, it, f, k, issues)
            else:
                bad = [s for s in issues if s[1] in ("cryptic", "generic", "letter-spaced", "empty", "key-derived")]
                if bad:
                    if ci is not None:
                        cands = [(column_label(strip_item_prefix(t, no)), s, w) for t, s, w in cands]
                    proposal, conf, srcs = self.choose(cands, exclude=lab)
                    if proposal and ci is None and "f" in it and len(it["f"]) > 1:
                        proposal = self.sub_label(proposal, it["l"])
                    keyonly = all(b[1] == "key-derived" for b in bad)
                    if keyonly and (not proposal or similar(proposal, lab) >= 0.5):
                        pass  # readable key, no better wording anywhere
                    else:
                        if not proposal:
                            proposal, conf, srcs = humanize_key(k or lab), "low", ["humanized key"]
                        sev = "low" if keyonly else ("high" if any(b[1] in ("cryptic", "letter-spaced") for b in bad) else "medium")
                        self.add(form, pn, no, fkey, "LABEL", bad[0][1], sev, lab, proposal, conf,
                                 [s for s in srcs] + ["schema field label"], "; ".join(d for _, _, d in bad))
                        if conf == "high":
                            self.override(code, fkey, {"label": proposal, "evidence": "; ".join(srcs)})
                for cat, sub, d in issues:
                    if sub == "misspelled":
                        self.add(form, pn, no, fkey, cat, sub, "low", lab, None, "low", ["vocabulary"], d)
            if k:
                self.audit_component(form, ctx, pn, no, it, f, k, fkey)
        self.audit_dates(form, ctx)
        # fields the schema shows without an item number although a source numbers them
        unnumbered = [(pn, it, k) for pi, pn, ii, it, f, ci, k in schema_fields(form) if ci is None and k and not it.get("no")]
        hinted = [(pn, k, self.item_number_hint(ctx, k)) for pn, it, k in unnumbered]
        hinted = [x for x in hinted if x[2][0]]
        if len(hinted) >= 5:
            srcs = Counter(h[2][1] for h in hinted)
            self.add(form, "(form)", "", "form", "LABEL", "ungrouped", "high" if len(hinted) >= 15 else "medium",
                     "%d fields sit in unnumbered items" % len(unnumbered),
                     "regroup by item number: " + ", ".join("%s→%s" % (key_tail(k)[:24], h[0]) for _, k, h in hinted[:12]) + (" …" if len(hinted) > 12 else ""),
                     "medium", ["%s (%d)" % x for x in srcs.items()],
                     "%d of them carry an item number in another source; the schema grouping cannot be fixed by a label override" % len(hinted))
        self.coverage[code] = cov

    # -- helpers for items
    def painted_item_label(self, ctx, no):
        if not no:
            return None
        lst = ctx["painted"]["numbered"].get(no) or []
        texts = list(OrderedDict.fromkeys(t for t, _ in lst))
        if not texts:
            return None
        if len(texts) == 1:
            return (texts[0], True)
        # several sub-labels: common head before "—"/":"/","
        heads = {re.split(r"\s*(?::|—|,)\s*", t)[0] for t in texts}
        if len(heads) == 1:
            return (heads.pop(), False)
        return (texts[0], False)

    def paper_label(self, ctx, no, page):
        if not no:
            return None
        if (page, no) in ctx["paper"]:
            return ctx["paper"][(page, no)] or None
        for (pg, n), v in ctx["paper"].items():
            if n == no and v:
                return v
        return None

    def group_box_label(self, ctx, it):
        if "f" not in it:
            return None
        for f in it["f"]:
            b = ctx["boxes"].get(f["k"])
            if b and b.get("group"):
                g = ctx["boxes"].get(b["group"])
                if g and good_source_label(strip_item_prefix(g["label"], g.get("item")), g["key"], self.vocab):
                    return strip_item_prefix(g["label"], g.get("item"))
        return None

    def sub_label(self, proposal, item_label):
        s = proposal
        if item_label and s.lower().startswith(item_label.lower()) and len(s) > len(item_label) + 2:
            s = s[len(item_label):].strip(" ,:-–—")
        m = re.match(r"^(.*?)(?:,\s*|\s+—\s+)(month|day|year|mm|dd|yyyy|number|amount|date|code)$", s, re.I)
        if m and similar(m.group(1), item_label) >= 0.5:
            s = m.group(2)
        return s[:1].upper() + s[1:]

    def choose(self, cands, exclude=None):
        """Pick a proposal; high when a strength-3 source exists or two sources agree."""
        cands = [(t.strip(), s, w) for t, s, w in cands if t and t.strip() and t.strip() != exclude]
        if not cands:
            return None, None, []
        cands.sort(key=lambda x: -x[2])
        top = cands[0]
        agreeing = [c for c in cands[1:] if similar(c[0], top[0]) >= 0.5 or agree(c[0], top[0])]
        conflict = [c for c in cands[1:] if c[2] >= 2 and similar(c[0], top[0]) < 0.2 and not agree(c[0], top[0])]
        if top[2] >= 3 and not conflict:
            conf = "high"
        elif agreeing:
            conf = "high" if top[2] >= 2 and not conflict else "medium"
        else:
            conf = "medium" if top[2] >= 2 else "low"
        return top[0], conf, ["%s: %r" % (s, t) for t, s, _ in [top] + agreeing + conflict]

    def open_item(self, form, part, no, ikey, lab, cands):
        if cands:
            self.open_questions.append({"form": form["c"], "k": ikey, "item": no, "part": part, "current": lab,
                                        "question": "Which wording should item %s use?" % (no or "(unnumbered)"),
                                        "candidates": list(OrderedDict.fromkeys(t for t, _, _ in cands))})

    # -- radios
    def radio_meaning(self, ctx, k):
        out = []
        f = ctx["fields"].get(k)
        if f:
            for v in f.get("enum_values") or []:
                if isinstance(v, dict) and v.get("choice_meaning"):
                    out.append((v["choice_meaning"], "fields.json choice_meaning", 3))
                    break
        b = ctx["boxes"].get(k)
        if b and b.get("group"):
            g = ctx["boxes"].get(b["group"])
            if g and k in (g.get("members") or []):
                i = g["members"].index(k)
                opts = g.get("options") or []
                if i < len(opts) and opts[i].get("label") and good_source_label(opts[i]["label"], k, self.vocab) or (i < len(opts) and opts[i].get("label") in ("Yes", "No")):
                    out.append((opts[i]["label"], "agent group option", 3))
            gl = strip_item_prefix(g["label"], g.get("item")) if g else ""
            ml = strip_item_prefix(b.get("label", ""), b.get("item"))
            if gl and ml.lower().startswith(gl.lower()) and len(ml) > len(gl):
                out.append((ml[len(gl):].strip(" ?:-"), "agent box label minus group question", 3))
            elif ml and good_source_label(ml, k, self.vocab) and not (gl and similar(ml, gl) > 0.7):
                out.append((ml, "agent box label", 2))
        bd = ctx["bindings"].get(k)
        if bd and bd.get("type") == "choice":
            for o in bd.get("options", []):
                if o.get("value") == "true" and o.get("set") and len(o["set"]) == 1:
                    (path, val), = o["set"].items()
                    out.append((humanize_value(val), "binding %s=%r when selected" % (path, val), 2 if isinstance(val, bool) else 2))
        return [(t, s, w) for t, s, w in out if t and len(t) > 1 and not re.fullmatch(r"[_\d\s]+|[A-Za-z]?_\d+", t) and (
            t in ("Yes", "No") or not is_bad_label(t, None, self.vocab, "field"))]

    def audit_radio(self, form, ctx, pn, no, it, f, k, issues):
        lab = f["l"]
        meanings = self.radio_meaning(ctx, k)
        bad = [s for s in issues if s[1] in ("cryptic", "generic", "letter-spaced", "empty", "key-derived")]
        proposal, conf, srcs = self.choose(meanings)
        mismatch = proposal and not agree(lab, proposal) and norm_alnum(proposal) not in norm_alnum(lab)
        if lab.lower() in ("yes", "no") and proposal and proposal.lower() in ("yes", "no") and lab.lower() != proposal.lower():
            mismatch = True
        if not bad and not mismatch:
            return
        if not proposal:
            proposal, conf, srcs = humanize_key(k), "low", ["humanized key"]
        sev = "high" if bad and any(b[1] in ("cryptic", "letter-spaced") for b in bad) or (mismatch and proposal.lower() in ("yes", "no")) else "medium"
        detail = "; ".join(d for _, _, d in bad) if bad else "option label does not match the choice it records"
        self.add(form, pn, no, k, "LABEL", "radio-option", sev, lab, proposal, conf, srcs + ["schema radio label"], detail)
        strong = [m for m in meanings if m[2] >= 2]
        if any(not agree(m[0], strong[0][0]) for m in strong[1:]):
            self.open_questions.append({"form": form["c"], "k": k, "item": no, "current": lab,
                                        "question": "Sources disagree on what option %s records (label vs bir-core binding). Which is right?" % key_tail(k),
                                        "candidates": ["%s (%s)" % (t, s_) for t, s_, _ in strong]})
        if conf == "high":
            self.override(form["c"], k, {"label": proposal, "evidence": "; ".join(srcs)})

    # -- components
    def audit_component(self, form, ctx, pn, no, it, f, k, fkey):
        code = form["c"]
        if INTERNAL.search(key_tail(k)):
            self.add(form, pn, no, fkey, "COMPONENT", "internal-field", "high", "%s shown as a %s" % (f["l"], f["t"]), "hidden", "high",
                     ["key %s" % k, "fields.json control_kind=%s" % (ctx["fields"].get(k) or {}).get("control_kind")],
                     "eBIRForms workflow/credential/serializer state, not a printed box; must never be user-editable on the form")
            self.override(code, fkey, {"kind": "hidden", "evidence": "internal workflow/credential key %s (not a printed box)" % key_tail(k)})
            return
        t = f["t"]
        fj = ctx["fields"].get(k)
        bd = ctx["bindings"].get(k)
        box = ctx["boxes"].get(k)
        bt = bd.get("type") if bd else None
        lt = fj["logical_type"] if fj else None
        ck = fj["control_kind"] if fj else None
        ev = ["schema t=%s" % t] + (["fields.json control_kind=%s logical_type=%s" % (ck, lt)] if fj else ["no fields.json entry"]) + \
             (["binding %s %s" % (bt, bd.get("path", ""))] if bd else ["no binding (%s)" % ctx["unmapped"].get(k, "unbound")]) + \
             (["agent box kind=%s" % box["kind"]] if box else [])
        # selects
        if t == "sel":
            opts = f.get("o") or []
            junk = [o for o in opts if JUNK_OPT.search(o)]
            real = [o for o in opts if not JUNK_OPT.search(o) and o not in ("0", "00", "000", "")]
            src = self.option_source(ctx, k, f, bd, box)
            if junk:
                self.add(form, pn, no, fkey, "COMPONENT", "select-junk-options", "high", opts, src[0] if src else real, "high",
                         ev, "template/placeholder text leaked into options: %r" % junk)
            if not real:
                if bt == "text" and not src:
                    self.add(form, pn, no, fkey, "COMPONENT", "select-bound-to-text", "high", "sel with options %r" % opts, "text", "medium",
                             ev, "binding stores free text and no option list exists anywhere")
                    self.open_questions.append({"form": code, "k": k, "question": "%s (%s) is a select in the official HTA but bir-core binds it as free text and no option list is recorded. Text box or select?" % (k, f["l"]),
                                                "candidates": ["text (matches the binding)", "select with the official options"]})
                elif src:
                    opts_new, how, conf = src
                    if bt == "text" and "agent box" not in how and "catalog" not in how:
                        self.open_questions.append({"form": code, "k": k, "current": f["l"],
                            "question": "%s is a select in the official form (control_kind %s) but bir-core binds %s as free text and records no option list; the app shows the stored draft value (e.g. '0') as the only choice. Free-text box, or a select with the candidate options?" % (k, ck, bd.get("path")),
                            "candidates": ["text (matches the binding)", "sel: %s (%s)" % (opts_new if isinstance(opts_new, str) else ", ".join(opts_new), how)]})
                    self.add(form, pn, no, fkey, "COMPONENT", "select-empty-options", "high", opts, opts_new if isinstance(opts_new, list) else how, conf,
                             ev + [how], "select has no usable options")
                    if conf == "high" and isinstance(opts_new, list):
                        self.override(code, fkey, {"kind": "sel", "options": opts_new, "evidence": how})
                else:
                    self.add(form, pn, no, fkey, "COMPONENT", "select-empty-options", "high", opts, None, "low", ev,
                             "select has no usable options and no source lists them")
            elif junk and src and isinstance(src[0], list) and src[2] == "high":
                self.override(code, fkey, {"options": src[0], "evidence": src[1]})
            elif junk:
                self.override(code, fkey, {"options": real, "evidence": "dropped template junk %r" % junk})
            if bt == "text" and src and "binding" not in src[1] and code == "2551Q":
                pass
        # binding type vs kind
        if bt and t not in BINDING_KIND.get(bt, {t}):
            want = {"amount": "money", "whole": "int", "text": "text", "choice": "radio"}[bt]
            if bt == "whole" and re.search(r"(?i)month|day|year", (bd or {}).get("path", "")):
                want = "int"
            sev = "high" if (t == "money" and bt == "text") or (t in ("text", "int") and bt == "amount") else "medium"
            agree = fj and kind_of(fj) == want
            conf = "high" if agree else "medium"
            if t == "money" and bt == "text" and re.search(r"(?i)name|address|specify|desc|tin|zip|code|email|phone|tel", f["l"] + " " + key_tail(k)):
                conf = "high"
            if bt == "whole" and re.search(r"(?i)/(month|day|year)$|_(month|day|year)$|sheets", bd.get("path", "")):
                conf = "high"
            sub = "%s-bound-as-%s" % (t, bt)
            self.add(form, pn, no, fkey, "COMPONENT", sub, sev, t, want, conf, ev,
                     "schema kind %s cannot hold what the binding stores (%s)" % (t, bt))
            if conf == "high" and t != "sel":
                self.override(code, fkey, {"kind": want, "evidence": "binding type %s; fields.json %s/%s" % (bt, ck, lt)})
        # text that the binding stores as a whole number
        if t == "text" and bt == "whole" and not re.search(r"(?i)month|day|year", bd.get("path", "")):
            pass  # already reported above (text is allowed for whole? no — BINDING_KIND whole excludes text)
        # money shown for non-money storage per fields.json
        if fj and t == "money" and lt in ("string", "code", "text") and bt != "amount":
            self.add(form, pn, no, fkey, "COMPONENT", "money-for-text", "medium", t, "text", "medium", ev, "fields.json stores a string")
        # chk vs radio
        if t == "radio":
            siblings = [x for x in it.get("f", []) if x["t"] == "radio"]
            grouped = box and box.get("group")
            fam = radio_family(k)
            family_elsewhere = [x for x in self._radios.get(fam, []) if x[0] != (pn, it.get("no"), id(it))]
            if box and box["kind"] in ("checkbox", "runtime-checkbox"):
                self.add(form, pn, no, fkey, "COMPONENT", "radio-vs-checkbox", "medium", "radio", "chk", "medium", ev, "agent box says checkbox")
            elif family_elsewhere and len([x for x in siblings if radio_family(x["k"]) == fam]) < len(self._radios.get(fam, [])):
                if k == self._radios[fam][0][1]:
                    others = [x[1] for x in self._radios[fam]]
                    self.add(form, pn, no, ",".join(others), "COMPONENT", "radio-group-split", "high", "%d options of one radio group spread over %d items" % (
                        len(others), len({x[0] for x in self._radios[fam]})), "put %s in one item" % ", ".join(key_tail(o) for o in others), "high",
                        ev + ["agent group %s" % (box or {}).get("group", fam)], "each option renders as its own item, so the choice reads as unrelated radios")
            elif len(siblings) == 1 and not grouped:
                self.add(form, pn, no, fkey, "COMPONENT", "lone-radio", "medium", "radio", "chk", "medium", ev,
                         "single radio with no group cannot be cleared once set; a checkbox (or Yes/No pair) is what it records")
        if t == "chk" and ck and ck == "checkbox/radio":
            self.add(form, pn, no, fkey, "COMPONENT", "checkbox-or-radio", "low", "chk", None, "low", ev, "fields.json control_kind is ambiguous (checkbox/radio)")
        # pct vs money
        if t == "money" and lt in ("percentage", "decimal-rate"):
            self.add(form, pn, no, fkey, "COMPONENT", "money-for-rate", "medium", "money", "pct", "high", ev, "fields.json stores a rate")
            self.override(code, fkey, {"kind": "pct", "evidence": "fields.json logical_type %s" % lt})
        # single field carrying a whole date
        if t in ("text",) and ((bd or {}).get("date") or (lt or "").startswith("date-string")):
            self.add(form, pn, no, fkey, "COMPONENT", "date-as-text", "medium", t, "date", "high", ev, "binding/fields.json store an MM/DD/YYYY date")
            self.override(code, fkey, {"kind": "date", "evidence": "binding date=%s; fields.json logical_type=%s" % ((bd or {}).get("date"), lt)})
        # date kind on a date component (2-digit month etc.)
        if t == "date" and ((lt or "").startswith("date-component") or (f.get("m") in (2, 4) and re.search(r"(?i)(month|day|year|mm|dd|yyyy)$", key_tail(k)))):
            self.add(form, pn, no, fkey, "COMPONENT", "date-kind-on-component", "medium", "date (m=%s)" % f.get("m"), "int (part of a date triple)", "high", ev,
                     "a single month/day/year box rendered as a full date input")
            self.override(code, fkey, {"kind": "int", "evidence": "fields.json logical_type %s, max_length %s — one component of a split date" % (lt, f.get("m"))})

    def option_source(self, ctx, k, f, bd, box):
        """(options|description, how, confidence) for a select."""
        path = (bd or {}).get("path", "")
        lab = (f["l"] + " " + key_tail(k)).lower()
        if box and box.get("options"):
            labs = [o.get("label") or o.get("value") for o in box["options"]]
            good = [x for x in labs if x and not JUNK_OPT.search(x) and x not in ("0", "00", "000")]
            if good and len(good) >= 2:
                return good, "agent box options", "high"
        if bd and bd.get("type") == "choice":
            opts = []
            for o in bd.get("options", []):
                vals = list(o.get("set", {}).values())
                opts.append("%s - %s" % (o.get("value"), "; ".join(humanize_value(v) for v in vals)) if o.get("value") not in ("", None) else "(none) - %s" % "; ".join(humanize_value(v) for v in vals))
            if opts:
                return opts, "binding choice options (values with the draft state they set)", "medium"
        if re.search(r"month", path + " " + lab) and (bd or {}).get("type") in ("whole", None):
            return MONTHS, "month select (binding %s, whole number 1-12)" % path, "high"
        if "rdo" in lab or path.endswith("rdo_code") or "rdo" in path:
            return "catalog crates/bir-core/data/rdo.json", "RDO catalog (runtime list)", "medium"
        if "atc" in lab or "atc" in path:
            return "catalog crates/bir-core/data/atcCodes.json", "ATC catalog (runtime list)", "medium"
        if re.search(r"region|province|city", lab + path):
            return "catalog crates/bir-core/data/geo/", "geographic catalog (cascading region/province/city)", "medium"
        if ("treaty" in lab or "relief" in lab or "special" in lab) and "code" not in lab:
            painted = [s for s in ctx["painted"]["strings"] if re.search(r"(?i)special rate|international tax treaty|special law", s)]
            if painted:
                return ["Special Rate", "International Tax Treaty"], "painted view wording %r" % painted[:2], "medium"
        return None

    # -- date triples
    def audit_dates(self, form, ctx):
        flat = [(pi, pn, ii, it, f, k) for pi, pn, ii, it, f, ci, k in schema_fields(form) if ci is None and k]
        comp = []
        for row in flat:
            f, k = row[4], row[5]
            fj = ctx["fields"].get(k)
            lt = (fj or {}).get("logical_type", "")
            tail = key_tail(k)
            m = re.search(r"(?i)(month|mon|mm|day|dd|date|year|yyyy|yr)(\d*|_\d+)?$", tail)
            lab = f["l"].lower()
            part = None
            if m or lt.startswith("date-component") or re.search(r"\b(month|day|year)\b", lab):
                w = (m.group(1).lower() if m else "") + " " + lab
                if re.search(r"month|\bmon\b|\bmm\b", w):
                    part = "M"
                elif re.search(r"day|\bdd\b|date", w) and f.get("m", 2) <= 2:
                    part = "D"
                elif re.search(r"year|yyyy|\byr\b", w):
                    part = "Y"
            comp.append(part)
        i = 0
        while i < len(flat):
            if comp[i] == "M":
                j = i + 1
                seq = ["M"]
                while j < len(flat) and comp[j] in ("D", "Y") and comp[j] not in seq and flat[j][2] == flat[i][2] and flat[j][0] == flat[i][0]:
                    seq.append(comp[j])
                    j += 1
                if seq in (["M", "D", "Y"], ["M", "Y"]):
                    keys = [flat[x][5] for x in range(i, j)]
                    pi, pn, ii, it = flat[i][0], flat[i][1], flat[i][2], flat[i][3]
                    kind = "date-triple" if len(seq) == 3 else "month-year-pair"
                    paths = [(ctx["bindings"].get(x) or {}).get("path") for x in keys]
                    self.date_groups.append({"form": form["c"], "part": pn, "item": it.get("no", ""), "item_label": it["l"], "kind": kind,
                                             "keys": keys, "kinds": [flat[x][4]["t"] for x in range(i, j)], "binding_paths": paths})
                    self.add(form, pn, it.get("no", ""), ",".join(keys), "COMPONENT", kind, "low", " / ".join(flat[x][4]["l"] for x in range(i, j)),
                             "date picker over the triple" if kind == "date-triple" else "month + year picker", "high",
                             ["schema field order", "key suffixes Month/Day/Year", "bindings %s" % paths], "split date stored as separate components")
                    i = j
                    continue
            i += 1

    # -- run
    def run(self):
        for form in self.schema:
            self.audit_form(form)


def kind_of(f):
    ck, lt = f["control_kind"], f["logical_type"]
    if "select" in ck:
        return "sel"
    if "checkbox" in ck and "radio" not in ck:
        return "chk"
    if "radio" in ck:
        return "radio"
    if lt in ("decimal-amount", "whole-peso-amount", "money", "decimal-money", "decimal"):
        return "money"
    if lt in ("percentage", "decimal-rate"):
        return "pct"
    if lt.startswith("date"):
        return "date"
    if lt in ("integer", "integer-year", "year", "year-string", "integer-string", "digit-string", "two-digit-tax-year"):
        return "int"
    return "text"


def radio_family(k):
    return re.sub(r"(?:[:_]?_?(?:\d+|[A-Z]|Y|N|Yes|No|Mon|Qtr|Semi|Other))$", "", k.split("#")[0])


def natkey(no):
    m = re.match(r"(\d+)([A-Z]?)", no or "")
    return (int(m.group(1)), m.group(2)) if m else (9999, no)


def lis(vals):
    """Indices of one longest non-decreasing subsequence."""
    n = len(vals)
    if not n:
        return []
    best = [1] * n
    prev = [-1] * n
    for i in range(n):
        for j in range(i):
            if vals[j] <= vals[i] and best[j] + 1 > best[i]:
                best[i], prev[i] = best[j] + 1, j
    i = max(range(n), key=lambda x: best[x])
    out = []
    while i != -1:
        out.append(i)
        i = prev[i]
    return out[::-1]


# --------------------------------------------------------------------------
# reporting

FIX_CLASSES = [
    ("C1 key-fragment labels from chopped HTA ids", lambda f: f["category"] == "LABEL" and f["type"] in ("cryptic",) and "chopped" in f["note"] + " ".join(f["evidence"]) or "truncated" in f["note"],
     "fields.json / agent-box labels are HTA ids with leading letters cut off (tDateMonth, tegoryNonResident_2). App override fixes display; the corpus labels need correcting at the source."),
    ("C2 camelCase / key-syntax labels", lambda f: f["category"] == "LABEL" and f["type"] == "cryptic" and "truncated" not in f["note"] and "chopped" not in f["note"],
     "Label is a re-spaced or raw official key (ATCCode, Pg1I1Month, Sched1Oct). Replace with painted or printed wording."),
    ("C3 letter-spaced labels", lambda f: f["type"] == "letter-spaced", "Acronym split into letters by the extractor's key splitter (A T C, N A 1)."),
    ("C4 radio options that do not name the choice", lambda f: f["type"] == "radio-option", "Use choice_meaning / agent group option / binding value."),
    ("C5 generic item labels (Yes, Item N, Additional details)", lambda f: f["type"] in ("generic", "wrong", "unnumbered") and f["category"] == "LABEL",
     "The extractor fell back to a choice word or a placeholder; the question text lives in the agent group box or painted view."),
    ("C6 merged items", lambda f: f["type"] == "merged", "Paper caption run swallowed the next item's number and caption."),
    ("C7 duplicate items / fields", lambda f: f["type"] in ("duplicate-item", "duplicate-field"), "Same number or same serialized key shown more than once."),
    ("C8 out-of-order items", lambda f: f["type"] == "out-of-order", "Grouping follows fields.json DOM order instead of item number."),
    ("C9 selects without usable options", lambda f: f["type"] in ("select-empty-options", "select-junk-options", "select-bound-to-text"),
     "Runtime-filled selects (RDO, ATC, geography, months, treaty) were captured empty or with template code."),
    ("C10 kind disagrees with binding", lambda f: f["category"] == "COMPONENT" and "-bound-as-" in f["type"] or f["type"] in ("money-for-text", "money-for-rate", "date-as-text"),
     "Schema kind derived from fields.json logical_type disagrees with what bir-core stores."),
    ("C11 split dates", lambda f: f["type"] in ("date-triple", "month-year-pair", "date-kind-on-component"), "Month/Day/Year boxes; the app will add a date picker."),
    ("C12 radio vs checkbox", lambda f: f["type"] in ("lone-radio", "radio-vs-checkbox", "checkbox-or-radio"), "Control type disagrees with grouping."),
    ("C15 internal workflow/credential fields shown", lambda f: f["type"] == "internal-field",
     "eBIR Online username/secret, enrollment, final-copy flags and serializer counters rendered as editable inputs; hide them."),
    ("C16 structure: ungrouped items, mixed sections, split radio groups", lambda f: f["type"] in ("ungrouped", "mixed-sections", "radio-group-split"),
     "The schema's item grouping is wrong (fields without item numbers, several numbered sections under one heading, one radio group spread over items). Needs the extractor, not a label override."),
    ("C13 misspellings", lambda f: f["type"] == "misspelled", "Token one edit away from a printed/painted word."),
    ("C14 key-derived but readable", lambda f: f["type"] == "key-derived", "Readable but not the printed wording."),
]


def classify(f):
    for name, pred, _ in FIX_CLASSES:
        try:
            if pred(f):
                return name
        except Exception:
            pass
    return "other"


def fmt(v):
    if v is None:
        return "—"
    if isinstance(v, list):
        s = ", ".join(str(x) for x in v[:6]) + (" …(+%d)" % (len(v) - 6) if len(v) > 6 else "")
        return "[" + s + "]"
    return str(v).replace("|", "\\|").replace("\n", " ")


def write_reports(audit, out):
    out.mkdir(parents=True, exist_ok=True)
    for f in audit.findings:
        f["fix_class"] = classify(f)
    sev_rank = {"high": 3, "medium": 2, "low": 1}
    per_form = defaultdict(Counter)
    score = Counter()
    for f in audit.findings:
        per_form[f["form"]][f["category"]] += 1
        per_form[f["form"]][f["severity"]] += 1
        if f["type"] not in ("date-triple", "month-year-pair", "key-derived", "misspelled"):
            score[f["form"]] += sev_rank[f["severity"]]
    totals = Counter((f["category"], f["type"]) for f in audit.findings)
    payload = OrderedDict([
        ("tool", "tools/audit_screen_forms.py"), ("schema", "app/forms/forms-schema.json"),
        ("totals", OrderedDict(("%s/%s" % k, v) for k, v in sorted(totals.items()))),
        ("by_confidence", dict(Counter(f["confidence"] for f in audit.findings))),
        ("forms_ranked", [OrderedDict([("form", c), ("score", s), ("counts", dict(per_form[c]))]) for c, s in score.most_common()]),
        ("painted_coverage", audit.coverage),
        ("date_groups", audit.date_groups),
        ("open_questions", audit.open_questions),
        ("findings", audit.findings),
    ])
    (out / "static.json").write_text(json.dumps(payload, indent=1, ensure_ascii=False))
    ov = OrderedDict([("_comment", "PROPOSED, not applied. Keys: field key k; item:<partIndex>:<itemIndex> for item labels (0-based into forms-schema.json parts/items); table columns are keyed by the column's row-0 key. High-confidence only.")])
    for code in sorted(audit.overrides):
        ov[code] = audit.overrides[code]
    (out / "schema-overrides.proposed.json").write_text(json.dumps(ov, indent=1, ensure_ascii=False))

    L = []
    L.append("# Static audit of the on-screen form schema\n")
    L.append("Generated by `python3 -I tools/audit_screen_forms.py`. Sources: forms-schema.json, rules/forms/*/fields.json, "
             "agent-forms boxes/bindings, painted views (bir-desktop), html-frozen text layer and writer-cells. Nothing is applied.\n")
    L.append("## Totals\n")
    L.append("| category | type | count |\n|---|---|---|")
    for (c, t), n in sorted(totals.items(), key=lambda x: (x[0][0], -x[1])):
        L.append("| %s | %s | %d |" % (c, t, n))
    L.append("\nConfidence: %s. Overrides proposed: %d entries across %d forms.\n" % (
        dict(Counter(f["confidence"] for f in audit.findings)), sum(len(v) for v in audit.overrides.values()), len(audit.overrides)))
    L.append("## Per-form summary (ranked by weighted severity, excluding date groups / key-derived / spelling)\n")
    L.append("| form | score | LABEL | COMPONENT | high | medium | low | painted labels matched / fields |\n|---|---|---|---|---|---|---|---|")
    for form in sorted(audit.schema, key=lambda f: -score[f["c"]]):
        c = form["c"]
        pf = per_form[c]
        cov = audit.coverage.get(c, {})
        L.append("| %s | %d | %d | %d | %d | %d | %d | %d / %d |" % (c, score[c], pf["LABEL"], pf["COMPONENT"], pf["high"], pf["medium"], pf["low"],
                                                                 cov.get("painted", 0), cov.get("fields", 0)))
    L.append("\n## Fix classes (cross-form)\n")
    byclass = defaultdict(list)
    for f in audit.findings:
        byclass[f["fix_class"]].append(f)
    for name, _, desc in FIX_CLASSES + [("other", None, "Unclassified.")]:
        lst = byclass.get(name)
        if not lst:
            continue
        forms = Counter(f["form"] for f in lst)
        L.append("### %s — %d findings in %d forms\n" % (name, len(lst), len(forms)))
        L.append(desc + "\n")
        L.append("Forms: " + ", ".join("%s (%d)" % x for x in forms.most_common()) + "\n")
        for f in lst[:4]:
            L.append("- e.g. %s item %s `%s`: %s → %s" % (f["form"], f["item"] or "-", fmt(f["k"])[:60], fmt(f["current"])[:70], fmt(f["proposed"])[:70]))
        L.append("")
    L.append("\n## Split dates (every Month/Day/Year triple and Month/Year pair)\n")
    L.append("| form | item | kind | keys | binding paths |\n|---|---|---|---|---|")
    for g in audit.date_groups:
        L.append("| %s | %s %s | %s | %s | %s |" % (g["form"], g["item"], fmt(g["item_label"])[:40], g["kind"], ", ".join("`%s`" % key_tail(k) for k in g["keys"]),
                                                  ", ".join(str(p) for p in g["binding_paths"])))
    L.append("\n## Needs the maintainer's call\n")
    for q in audit.open_questions:
        L.append("- **%s** `%s`%s — %s Candidates: %s" % (q["form"], fmt(q.get("k"))[:80], (" (current %r)" % q["current"]) if q.get("current") else "", q["question"],
                                                    "; ".join("%r" % c for c in q["candidates"][:5])))
    L.append("\n## Findings by form\n")
    byform = defaultdict(list)
    for f in audit.findings:
        byform[f["form"]].append(f)
    for form in audit.schema:
        lst = byform.get(form["c"], [])
        if not lst:
            continue
        L.append("### %s — %s (%d findings)\n" % (form["c"], form["t"], len(lst)))
        L.append("| sev | conf | category/type | part | item | key | current | proposed | why |\n|---|---|---|---|---|---|---|---|---|")
        for f in sorted(lst, key=lambda x: (-sev_rank[x["severity"]], x["category"], x["type"])):
            L.append("| %s | %s | %s/%s | %s | %s | `%s` | %s | %s | %s |" % (
                f["severity"], f["confidence"], f["category"], f["type"], fmt(f["part"])[:28], f["item"], fmt(f["k"])[:48], fmt(f["current"])[:60],
                fmt(f["proposed"])[:70], fmt(f["note"])[:90]))
        L.append("")
    (out / "static.md").write_text("\n".join(L) + "\n")


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    here = Path(__file__).resolve().parent.parent
    ap.add_argument("--repo", default=str(here))
    ap.add_argument("--out", default=None)
    a = ap.parse_args()
    repo = Path(a.repo)
    out = Path(a.out) if a.out else repo / "tmp/screen-form-audit"
    audit = Audit(repo)
    audit.run()
    write_reports(audit, out)
    t = Counter(f["category"] for f in audit.findings)
    print("forms=%d findings=%d %s overrides=%d open=%d -> %s" % (
        len(audit.schema), len(audit.findings), dict(t), sum(len(v) for v in audit.overrides.values()), len(audit.open_questions), out))


if __name__ == "__main__":
    main()
