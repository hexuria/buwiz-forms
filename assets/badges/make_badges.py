# The README's badges, drawn as our own SVG files so GitHub shows them straight from the repository. (GitHub sends
# an image from another site, such as shields.io, through its camo proxy.) They look like shields.io "flat" badges.
#
#   just badges            rewrite every badge from the code
#   just badges --check    exit 1 when a committed badge is not what the code gives
#
# Where each value comes from:
# - forms: every form the app opens, counted from the code (the two hand-built forms in AgentFormKind::all plus
#   QueueableKind::ALL), each form code counted once.
# - eBIRForms: the newest eBIRForms package version a ported form names in its source (`eBIRForms 7.9.6.2.1` in the
#   header of crates/bir-core/src/forms/*.rs). Porting forms from a newer package moves the badge.
# - version: the app's release version, parked at APP_VERSION until the first release (`--app-version` overrides).
# - platforms: the five the app ships on.
# Text widths come from the Verdana 11 px table below, so every machine writes the same bytes; no font or package
# is needed.
import pathlib, re, sys

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parents[1]
APP_VERSION = "1.0.0"
FORMS_DIR = ROOT / "crates/bir-core/src/forms"
PLATFORMS = [  # file, name, colour, text colour
    ("ios.svg", "iOS", "#000000", "#fff"),
    ("android.svg", "Android", "#3DDC84", "#1b1b1b"),
    ("linux.svg", "Linux", "#FCC624", "#1b1b1b"),
    ("macos.svg", "macOS", "#A2AAAD", "#1b1b1b"),
    ("windows.svg", "Windows", "#0078D4", "#fff"),
]
VERDANA_11 = {
    ' ': 3.9, '!': 4.3, '"': 5.0, '#': 9.0, '$': 7.0, '%': 11.8, '&': 8.0, "'": 3.0, '(': 5.0, ')': 5.0, '*': 7.0, '+': 9.0,
    ',': 4.0, '-': 5.0, '.': 4.0, '/': 5.0, '0': 7.0, '1': 7.0, '2': 7.0, '3': 7.0, '4': 7.0, '5': 7.0, '6': 7.0, '7': 7.0,
    '8': 7.0, '9': 7.0, ':': 5.0, ';': 5.0, '<': 9.0, '=': 9.0, '>': 9.0, '?': 6.0, '@': 11.0, 'A': 7.5, 'B': 7.5, 'C': 7.7,
    'D': 8.5, 'E': 7.0, 'F': 6.3, 'G': 8.5, 'H': 8.3, 'I': 4.6, 'J': 5.0, 'K': 7.6, 'L': 6.1, 'M': 9.3, 'N': 8.2, 'O': 8.7,
    'P': 6.6, 'Q': 8.7, 'R': 7.6, 'S': 7.5, 'T': 6.8, 'U': 8.1, 'V': 7.5, 'W': 10.9, 'X': 7.5, 'Y': 6.8, 'Z': 7.5, '[': 5.0,
    '\\': 5.0, ']': 5.0, '^': 9.0, '_': 7.0, '`': 7.0, 'a': 6.6, 'b': 6.9, 'c': 5.7, 'd': 6.9, 'e': 6.6, 'f': 3.9, 'g': 6.9,
    'h': 7.0, 'i': 3.0, 'j': 3.8, 'k': 6.5, 'l': 3.0, 'm': 10.7, 'n': 7.0, 'o': 6.7, 'p': 6.9, 'q': 6.9, 'r': 4.7, 's': 5.7,
    't': 4.3, 'u': 7.0, 'v': 6.5, 'w': 9.0, 'x': 6.5, 'y': 6.5, 'z': 5.8, '{': 7.0, '|': 5.0, '}': 7.0, '~': 9.0,
}


def form_codes():
    """Form codes of every form the app opens, each once (from the code, not a hand-kept number)."""
    agent = (FORMS_DIR / "agent/mod.rs").read_text()
    body = re.search(r"pub fn all\(\) -> Vec<Self> \{(.*?)\n    \}", agent, re.S)
    hand_built = re.findall(r"Self::Form(\w+)", body.group(1)) if body else []
    queue = (FORMS_DIR / "queueable.rs").read_text()
    listing = re.search(r"pub const ALL: &'static \[QueueableKind\] = &\[(.*?)\];", queue, re.S)
    if not listing or not hand_built:
        sys.exit("make_badges: could not find AgentFormKind::all or QueueableKind::ALL; update form_codes()")
    queued = re.findall(r"QueueableKind::Form(\w+)", listing.group(1))
    return sorted(set(c.upper() for c in hand_built + queued))


def ebirforms_version():
    """The newest eBIRForms package version a ported form names in its source."""
    found = {}
    for f in sorted(FORMS_DIR.rglob("*.rs")):
        for v in re.findall(r"eBIRForms (\d+(?:\.\d+){1,4})", f.read_text()):
            found[v] = found.get(v, 0) + 1
    if not found:
        sys.exit("make_badges: no 'eBIRForms <version>' found in crates/bir-core/src/forms")
    return max(found, key=lambda v: tuple(int(p) for p in v.split("."))), found


def width(text):
    return sum(VERDANA_11.get(c, 7.0) for c in text)


def badge(label, value, colour, ink):
    lw, vw = round(width(label)) + 10, round(width(value)) + 10
    w = lw + vw
    shadow = "#010101" if ink == "#fff" else "#ccc"
    def text(s, x0, span, fill):
        x, tl = (x0 + span / 2) * 10, (span - 10) * 10
        return (f'<text aria-hidden="true" x="{x:g}" y="150" fill="{shadow}" fill-opacity=".3" transform="scale(.1)" textLength="{tl:g}">{s}</text>'
                f'<text x="{x:g}" y="140" transform="scale(.1)" fill="{fill}" textLength="{tl:g}">{s}</text>')
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="20" role="img" aria-label="{label}: {value}">'
            f'<title>{label}: {value}</title>'
            '<linearGradient id="s" x2="0" y2="100%"><stop offset="0" stop-color="#bbb" stop-opacity=".1"/><stop offset="1" stop-opacity=".1"/></linearGradient>'
            f'<clipPath id="r"><rect width="{w}" height="20" rx="3" fill="#fff"/></clipPath>'
            f'<g clip-path="url(#r)"><rect width="{lw}" height="20" fill="#555"/><rect x="{lw}" width="{vw}" height="20" fill="{colour}"/>'
            f'<rect width="{w}" height="20" fill="url(#s)"/></g>'
            '<g fill="#fff" text-anchor="middle" font-family="Verdana,Geneva,DejaVu Sans,sans-serif" text-rendering="geometricPrecision" font-size="110">'
            + text(label, 0, lw, "#fff") + text(value, lw, vw, ink) + '</g></svg>\n')


def main(argv):
    check = "--check" in argv
    app_version = APP_VERSION
    if "--app-version" in argv:
        app_version = argv[argv.index("--app-version") + 1].lstrip("v")
    codes = form_codes()
    version, seen = ebirforms_version()
    badges = [
        ("version.svg", "version", f"v{app_version}", "#0B35B0", "#fff"),
        ("ebirforms.svg", "eBIRForms", version, "#C8102E", "#fff"),
        ("forms.svg", "forms", str(len(codes)), "#F4B400", "#1b1b1b"),
    ] + [(f, "platform", name, c, ink) for f, name, c, ink in PLATFORMS]
    print(f"forms: {len(codes)} ({', '.join(codes)})")
    print("eBIRForms versions named in the forms' sources: " + ", ".join(f"{v} x{n}" for v, n in sorted(seen.items())) + f" -> {version}")
    stale = []
    for name, label, value, colour, ink in badges:
        path, svg = HERE / name, badge(label, value, colour, ink)
        if check:
            if not path.exists() or path.read_text() != svg:
                stale.append(name)
        else:
            path.write_text(svg)
            print(f"wrote assets/badges/{name} ({label}: {value})")
    if check:
        if stale:
            sys.exit("stale badges: " + ", ".join(stale) + " (run `just badges`)")
        print("badges are up to date")


if __name__ == "__main__":
    main(sys.argv[1:])
