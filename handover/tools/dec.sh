#!/bin/bash
S=/tmp/claude-1000/-home-agent-workspace/75a3a4d5-f426-4ca2-af38-3fdce8d073d7/scratchpad
export GHIDRA_INSTALL_DIR=$S/ghidra/ghidra_12.1.4_PUBLIC LC_ALL=C.UTF-8
mkdir -p $S/chkt/dec; cd $S/rea
for a in "$@"; do
  timeout 300 npx -y rea-agents decompile ../chkt/chkt.exe $a --snapshot ../chkt/snap --format json --full-output > $S/chkt/dec/$a.json 2>&1
  python3 -I -c "
import sys,json;d=json.load(open(sys.argv[1]))
best=''
def walk(x):
  global best
  if isinstance(x,dict): [walk(v) for v in x.values()]
  elif isinstance(x,list): [walk(v) for v in x]
  elif isinstance(x,str) and '{' in x and len(x)>len(best): best=x
walk(d); open(sys.argv[2],'w').write(best)" $S/chkt/dec/$a.json $S/chkt/dec/$a.c
  echo "=== $a ($(wc -l < $S/chkt/dec/$a.c) lines)"
done
