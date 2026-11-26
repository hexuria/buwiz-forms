# merge.py OUT.svg : the Buwiz Forms logo as one SVG, made from the eight part files beside this script.
# The parts are stacked back to front with no outlines between them. "Buwiz" keeps its 17 px white border (a white
# stroke of its own outline, under it). The banner and its white letters sit 10 px lower than in the original artwork;
# that drop is drawn into 8-forms-banner.svg itself, so nothing here moves it.
# `just logo` runs this into assets/brand/buwiz-forms.svg; then `just icons` makes every app icon from that.
# The viewBox is pinned (VIEWBOX below), so the output is the same on every machine: measuring it from a rendering
# depends on the librsvg version by a pixel (the fading speed-line tips set the left edge). After a change that moves the
# logo's outer edge, run `just logo --measure` and paste the viewBox it prints into VIEWBOX.
# The logo is never shown without the words Buwiz Forms: there is no emblem-only version, and none is to be made.
import re, sys, subprocess, tempfile, os
import numpy as np
from PIL import Image
S = os.path.dirname(os.path.abspath(__file__)) + "/"
OUT = sys.argv[1]
MEASURE = "--measure" in sys.argv[2:]
VIEWBOX = (122, 13, 1043, 1134)  # measured 2026-11-26 from the eight parts as committed: 8 px around what is drawn
STACK = [  # (prefix, file, group to keep or None, options), back to front
    ("sun", "1-sun.svg", None, {}),
    ("rb", "5-red-swoosh.svg", "red-back", {}),  # the red swoosh's dark fold, behind the tablet
    ("tab", "3-form.svg", None, {}),
    ("eb", "6-ebirforms-text.svg", None, {}),
    ("bl", "4-blue-swoosh.svg", None, {}),
    ("rf", "5-red-swoosh.svg", "red-front", {}),  # the red swoosh's bright face
    ("ea", "2-eagle.svg", None, {}),
    ("bw", "7-buwiz-forms-text.svg", "buwiz", {"border": 17}),
    ("bn", "8-forms-banner.svg", None, {}),  # the banner with its speed lines and the white Forms letters
]
def group_inner(s, gid):
    m = re.search(r'<g id="%s"[^>]*>' % re.escape(gid), s)
    depth = 0
    for t in re.finditer(r'<g\b[^>]*>|</g>', s[m.start():]):
        depth += 1 if t.group(0).startswith("<g") else -1
        if depth == 0: return s[m.start():m.start() + t.end()]
    raise ValueError(gid)
defs_all, body_all = [], []
for pre, fn, grp, opt in STACK:
    s = open(S + fn).read()
    defs = "".join(re.findall(r"<defs>(.*?)</defs>", s, re.S))
    body = s[s.index(">", s.index("<svg")) + 1:s.rindex("</svg>")]
    body = re.sub(r"<defs>.*?</defs>", "", body, flags=re.S)
    body = re.sub(r"<title>.*?</title>", "", body, flags=re.S)
    if grp: body = group_inner(body, grp)
    ids = set(re.findall(r'\bid="([^"]+)"', defs + body))
    for i in sorted(ids, key=len, reverse=True):
        for a, b in (('id="%s"' % i, 'id="%s-%s"' % (pre, i)), ("url(#%s)" % i, "url(#%s-%s)" % (pre, i)),
                     ('href="#%s"' % i, 'href="#%s-%s"' % (pre, i))):
            defs = defs.replace(a, b); body = body.replace(a, b)
    if opt.get("border"):
        w = 2 * opt["border"]
        paths = re.findall(r"<path [^>]*/>", body)
        # every subpath filled on its own (so letter counters are white, as in the original), plus the border stroke
        under = []
        for p in paths:
            d = re.search(r'\bd="([^"]+)"', p).group(1)
            for sp in re.findall(r"M[^M]*", d):
                under.append('<path fill="#fff" stroke="#fff" stroke-width="%d" stroke-linejoin="round" stroke-linecap="round" d="%s"/>' % (w, sp.strip()))
        under = "".join(under)
        body = '<g id="%s-border">%s</g>' % (pre, under) + body
    if opt.get("dy"): body = '<g transform="translate(0 %g)">%s</g>' % (opt["dy"], body)
    defs_all.append(defs); body_all.append('<g id="layer-%s">%s</g>' % (pre, body))
def doc(vb):
    return ('<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="%s">'
            '<title>%s</title><defs>%s</defs>%s</svg>') % (vb, os.environ.get("TITLE", "Buwiz Forms"), "".join(defs_all), "".join(body_all))
# measure what is drawn (8 px margin): used with --measure, otherwise only to warn when the pinned viewBox looks stale
tmp = tempfile.mkdtemp(); open(tmp + "/a.svg", "w").write(doc("0 0 1254 1300"))
subprocess.run(["rsvg-convert", "-w", "1254", "-h", "1300", tmp + "/a.svg", "-o", tmp + "/a.png"], check=True)
al = np.asarray(Image.open(tmp + "/a.png").convert("RGBA"))[..., 3]
ys, xs = np.nonzero(al > 8)
x0, y0, x1, y1 = int(xs.min()) - 8, int(ys.min()) - 8, int(xs.max()) + 9, int(ys.max()) + 9
measured = (x0, y0, x1 - x0, y1 - y0)
vb = measured if MEASURE else VIEWBOX
if not MEASURE and max(abs(a - b) for a, b in zip(measured, VIEWBOX)) > 2:
    print("warning: what is drawn measures", measured, "but VIEWBOX is", VIEWBOX,
          "- if a part's outer edge moved, run `just logo --measure` and paste the new viewBox into VIEWBOX", file=sys.stderr)
open(OUT, "w").write(doc("%d %d %d %d" % vb))
print("logo", OUT, "viewBox", vb, "(measured)" if MEASURE else "(pinned)", "bytes", os.path.getsize(OUT))
