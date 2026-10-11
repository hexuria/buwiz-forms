import sys, re
from html.parser import HTMLParser

class P(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.depth_stack = []
        self.items = []
        self.cur = None
        self.skip = 0
    def handle_starttag(self, tag, attrs):
        a = dict(attrs)
        if tag in ("style", "script"):
            self.skip += 1
        cls = a.get("class", "") or ""
        style = a.get("style", "") or ""
        if tag == "div" and re.search(r"(^|\s)t(\s|$)", cls):
            m_top = re.search(r"top:([-\d.]+)pt", style)
            m_left = re.search(r"left:([-\d.]+)pt", style)
            self.cur = {"top": float(m_top.group(1)) if m_top else -1,
                        "left": float(m_left.group(1)) if m_left else -1,
                        "text": "", "id": a.get("id", ""), "page": None}
            self.cur_depth = len(self.depth_stack)
            self.depth_stack.append("t")
        else:
            self.depth_stack.append(tag)
    def handle_endtag(self, tag):
        if tag in ("style", "script") and self.skip:
            self.skip -= 1
        if self.depth_stack:
            popped = self.depth_stack.pop()
        if self.cur is not None and popped == "t" and len(self.depth_stack) == self.cur_depth:
            self.cur["text"] = " ".join(self.cur["text"].split())
            if self.cur["text"]:
                self.items.append(self.cur)
            self.cur = None
    def handle_data(self, data):
        if self.cur is not None and not self.skip:
            self.cur["text"] += data

p = P()
with open(sys.argv[1], encoding="utf-8", errors="replace") as f:
    p.feed(f.read())
for it in p.items:
    print(f"{it['id']:10s} top={it['top']:8.2f} left={it['left']:8.2f}  {it['text']}")
