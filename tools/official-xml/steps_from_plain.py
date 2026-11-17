"""Build official runtime input steps from one of our plaintext samples.

Typed controls get their value (amounts without commas, so the official
round()/formatCurrency() runs); radios that are true get clicked; disabled
numeric controls are left alone because the official page computes them.

Usage: python3 steps_from_plain.py <BIR-FormXXXX.hta> <ours.plain.xml> <out.steps.json>
"""
import json, os, re, subprocess, sys
hta, plain_path, out_path = sys.argv[1:4]
tool = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'list_inputs.js')
info = json.loads(subprocess.run(['node', tool, hta], capture_output=True, text=True, check=True).stdout)
kinds = {x['id']: x for x in info}
def unesc(v):
    v = re.sub(r'%u([0-9A-Fa-f]{4})', lambda m: chr(int(m.group(1), 16)), v)
    return re.sub(r'%([0-9A-Fa-f]{2})', lambda m: chr(int(m.group(1), 16)), v)
text = open(plain_path).read()
steps, seen = [], set()
for key, body in re.findall(r'<div>([^=<]+)=(.*?)\1=</div>', text, re.S):
    if key in seen:
        continue
    seen.add(key)
    value = unesc(body)
    if value in ('true', 'false'):
        if value == 'true':
            steps.append({'click': key})
        continue
    k = kinds.get(key)
    if k and k['numeric'] and k['disabled']:
        continue  # computed by the official page
    if k and k['numeric']:
        value = value.replace(',', '')
    steps.append({'set': key, 'value': value})
json.dump(steps, open(out_path, 'w'), indent=1)
print(len(steps), 'steps')
