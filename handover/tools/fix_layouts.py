"""Rebuild official_xml.rs LAYOUTS as the sorted union of ids in the working
file, HEAD and MERGE_HEAD (if merging). Run in repo root."""
import re, subprocess
P = 'crates/bir-core/src/official_xml.rs'
def block_ids(text):
    i = text.find('const LAYOUTS'); 
    if i < 0: return set()
    j = text.find('\n\n', i)
    return set(re.findall(r'"([a-z0-9-]+)"', text[i:j]))
s = open(P).read()
ids = block_ids(s)
for ref in ('HEAD', 'MERGE_HEAD'):
    r = subprocess.run(['git', 'show', f'{ref}:{P}'], capture_output=True, text=True)
    if r.returncode == 0: ids |= block_ids(r.stdout)
i = s.index('const LAYOUTS'); j = s.index('\n\n', i)
s = s[:i] + 'const LAYOUTS: &[(&str, &str)] = layouts![\n' + ''.join(f'    "{x}",\n' for x in sorted(ids)) + '];' + s[j:]
open(P, 'w').write(s); print(len(ids), 'layouts')
