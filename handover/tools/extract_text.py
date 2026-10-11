import sys
from html.parser import HTMLParser

SKIP = {"style", "script", "svg", "head", "title", "noscript"}
BLOCK = {"p", "div", "br", "tr", "li", "h1", "h2", "h3", "h4", "h5", "h6", "table", "section", "td", "th", "span", "label", "b", "strong", "em", "i", "small"}

class P(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.skip = 0
        self.buf = []
        self.out = []
        self.stack = []
    def handle_starttag(self, tag, attrs):
        if tag in SKIP:
            self.skip += 1
        if tag in BLOCK or tag in ("br",):
            self.flush()
        cls = dict(attrs).get("class", "")
        self.stack.append((tag, cls))
    def handle_endtag(self, tag):
        if tag in SKIP and self.skip:
            self.skip -= 1
        if tag in BLOCK:
            self.flush()
        for i in range(len(self.stack) - 1, -1, -1):
            if self.stack[i][0] == tag:
                del self.stack[i:]
                break
    def handle_data(self, data):
        if self.skip:
            return
        t = " ".join(data.split())
        if t:
            self.buf.append(t)
    def flush(self):
        if self.buf:
            self.out.append(" ".join(self.buf))
            self.buf = []

p = P()
with open(sys.argv[1], encoding="utf-8", errors="replace") as f:
    p.feed(f.read())
p.flush()
for line in p.out:
    print(line)
