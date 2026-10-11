import re, subprocess, sys
p='crates/bir-rules-codegen/src/status.rs'; s=open(p).read()
ROW=re.compile(r'\("([^"]+)", "([^"]+)", "([^"]+)", (\d+), (true|false)\),')
i=s.index('const EXPECTED: [(&str, &str, &str, u16, bool); '); j=s.index('];',i)
block=s[i:j]; rows={m.group(1):m.group(0) for m in ROW.finditer(block)}
for ref in sys.argv[1:]:  # extra refs to borrow missing rows from
    t=subprocess.run(['git','show',f'{ref}:{p}'],capture_output=True,text=True).stdout
    if 'const EXPECTED: [' in t:
        a=t.index('const EXPECTED: ['); b=t.index('];',a)
        for m in ROW.finditer(t[a:b]): rows.setdefault(m.group(1), m.group(0))
reg=open('crates/bir-core/src/forms/support_level.rs').read()
order=[c for c in re.findall(r'FormCapabilityRecord \{\s*code: "([^"]+)"',reg) if c in rows]
missing=[c for c in re.findall(r'FormCapabilityRecord \{\s*code: "([^"]+)"',reg) if c not in rows and c!='TEST']
if missing: print('NO ROW FOR', missing)
head=block[:block.index('= [')+3]
head=re.sub(r'bool\); \d+\]', f'bool); {len(order)}]', head)
s=s[:i]+head+'\n'+''.join(f'        {rows[c]}\n' for c in order)+'    '+s[j:]
open(p,'w').write(s); print(len(order),'rows')
