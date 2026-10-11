"""List the controls official saveXML would serialize, in DOM order.

saveXML walks document.getElementById('frmMain').elements and writes every
element whose type is text/select-one/radio/checkbox (skipping button/hidden),
keyed by element id.
"""
import json, re, sys
from html.parser import HTMLParser

RAW = {'script', 'style', 'xmp', 'textarea'}


class P(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.depth_form = 0
        self.in_form = False
        self.raw = None
        self.out = []

    def handle_starttag(self, tag, attrs):
        a = {k.lower(): (v or '') for k, v in attrs}
        if self.raw:
            return
        if tag in RAW:
            self.raw = tag
            if tag == 'textarea' and self.in_form:
                self.out.append(('textarea', a.get('id', ''), a.get('name', '')))
            return
        if tag == 'form' and a.get('id') == 'frmMain':
            self.in_form = True
        if not self.in_form:
            return
        if tag == 'td' and a.get('id') == 'rdoSelect':
            # getRdo() injects <select id='<prefix>:txtRDOCode'> here at load.
            self.out.append(('select-one', '@RDO', '@RDO'))
        if tag == 'input':
            t = a.get('type', 'text').lower() or 'text'
            self.out.append((t, a.get('id', ''), a.get('name', '')))
        elif tag == 'select':
            self.out.append(('select-multiple' if 'multiple' in a else 'select-one',
                             a.get('id', ''), a.get('name', '')))
        elif tag == 'button':
            self.out.append(('button', a.get('id', ''), a.get('name', '')))

    def handle_startendtag(self, tag, attrs):
        self.handle_starttag(tag, attrs)

    def handle_endtag(self, tag):
        if self.raw and tag == self.raw:
            self.raw = None
        elif tag == 'form' and self.in_form:
            self.in_form = False


def controls(path):
    p = P()
    p.feed(open(path, encoding='utf-8-sig', errors='replace').read())
    return p.out


if __name__ == '__main__':
    for t, i, n in controls(sys.argv[1]):
        print(t, i, n, sep='\t')
