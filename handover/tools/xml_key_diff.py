#!/usr/bin/env python3
"""Compare a BIR payload's keys with the controls official saveXML serializes.

Usage: python3 xml_key_diff.py <BIR-FormXXXX.hta> <payload.xml> [<payload2.xml> ...]

saveXML walks frmMain.elements in DOM order and writes every text,
select-one, radio and checkbox control keyed by id (buttons, hidden and
password inputs are skipped). getRdo() injects <prefix>:txtRDOCode into
td#rdoSelect at load.
"""
import re
import sys
from html.parser import HTMLParser

SERIALIZED = {'text', 'select-one', 'radio', 'checkbox'}


class Controls(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.in_form, self.raw, self.out = False, None, []

    def handle_starttag(self, tag, attrs):
        a = {k.lower(): (v or '') for k, v in attrs}
        if self.raw:
            return
        if tag in ('script', 'style', 'xmp', 'textarea'):
            self.raw = tag
            return
        if tag == 'form' and a.get('id') == 'frmMain':
            self.in_form = True
        if not self.in_form:
            return
        if tag == 'td' and a.get('id') == 'rdoSelect':
            self.out.append(('select-one', '@RDO'))
        elif tag == 'input':
            self.out.append(((a.get('type') or 'text').lower(), a.get('id', '')))
        elif tag == 'select':
            self.out.append(('select-one', a.get('id', '')))

    handle_startendtag = handle_starttag

    def handle_endtag(self, tag):
        if self.raw and tag == self.raw:
            self.raw = None
        elif tag == 'form':
            self.in_form = False


def official_keys(hta_path):
    p = Controls()
    p.feed(open(hta_path, encoding='utf-8-sig', errors='replace').read())
    keys = [i for t, i in p.out if t in SERIALIZED]
    prefix = next((k.split(':')[0] for k in keys if ':' in k), '')
    return [prefix + ':txtRDOCode' if k == '@RDO' else k for k in keys]


def payload_keys(xml_path):
    text = open(xml_path, encoding='utf-8', errors='replace').read()
    return re.findall(r'<div>([^=<]+)=', text)


if __name__ == '__main__':
    official = official_keys(sys.argv[1])
    print(f'official: {len(official)} keys from {sys.argv[1]}')
    for path in sys.argv[2:]:
        ours = payload_keys(path)
        print(f'\n== {path}: {len(ours)} keys')
        missing = [k for k in official if k not in set(ours)]
        extra = [k for k in ours if k not in set(official)]
        dups = sorted({k for k in ours if ours.count(k) > 1})
        same_order = [k for k in ours if k in set(official)] == [k for k in official if k in set(ours)]
        print('  missing (official writes, payload lacks):', missing or 'none')
        print('  extra (payload has, official form lacks):', extra or 'none')
        print('  duplicates:', dups or 'none')
        print('  shared keys in official DOM order:', same_order)
