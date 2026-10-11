"""3-way merge of one file: hunks where base is empty (both sides added) become
ours+theirs; other conflicts are left marked. Usage: merge3.py ours base theirs > out"""
import re, subprocess, sys
o, b, t = sys.argv[1:4]
r = subprocess.run(['git', 'merge-file', '-p', '--diff3', o, b, t], capture_output=True, text=True)
s = r.stdout
H = re.compile(r'<<<<<<< [^\n]*\n(.*?)\|\|\|\|\|\|\| [^\n]*\n(.*?)=======\n(.*?)>>>>>>> [^\n]*\n', re.S)
left = 0
def pick(m):
    global left
    ours, base, theirs = m.groups()
    if base.strip() == '':
        return ours + theirs
    left += 1
    return m.group(0)
s = H.sub(pick, s)
sys.stdout.write(s)
sys.stderr.write(f'unresolved hunks: {left}\n')
