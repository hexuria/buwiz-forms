# Continuation state (work in progress)

Done: counts stable, pushed. Matcher: nrm() numeric-normalizes, matches against ALL printed rows
(incl. derived keys) plus concatenated digit stream. Pipeline: /tmp/pacmp.py (audit2.json ->
per-key.tsv), /tmp/patable.py, /tmp/njclass.py (notjoined.json -> njclass.tsv). Regenerate:
python3 /tmp/pacmp.py && python3 /tmp/patable.py && python3 /tmp/njclass.py. audit2.json is the
73-sample dump (print maps at main 75bd78b8, target-audit).

NEXT: (1) verify 2200P/2200M remaining (a) keys: mostly txtPg1Pt1I10Province/City style digit codes —
trace whether sheet cells print them (grep the key in html-frozen/2200p-2020/index.html and check
per-key.tsv for nearby place keys). (2) worst-three plain words: 2200p, 2200m, 1701a. (3) 1702RT
3 held cells + 1701A 1 held cell with cell ids: held.json has them (1702rt drpPg1I5AtcOther x3,
1701a txt42Desc). (4) case-only grouped list: per-key.tsv verdict='case differs only', group by form,
check fields.json control uppercase forcing. (5) (d) keys absent from fields.json: compare against
official program control inventory (rules/forms/<pkg>/fixtures/runtime-control-inventory-*.json) —
working theory: legacy duplicate serialization keys. User requirement details are in the chat of
session 806656d715244f78973e1c34b5bf121d (message at 01:23). Report classes per form when done.
