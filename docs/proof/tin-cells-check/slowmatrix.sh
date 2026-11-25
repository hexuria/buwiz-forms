#!/bin/bash
# usage: slowmatrix.sh <label> <gaps...> — 10 runs per gap at current conditions
cd /Users/devin/BuwizNsh/evidence/tin-cells-check
LABEL=$1; shift
for gap in "$@"; do
  for run in 1 2 3 4 5 6 7 8 9 10; do
    ./keybs 9 >/dev/null 2>&1; sleep 0.25
    ./keyinject $gap 000123453 >/dev/null 2>&1; sleep 0.35
    v=$(python3 /tmp/tinval.py 2>/dev/null)
    echo "$LABEL $gap $run [$v]" | tee -a slow-raw.txt
  done
done
