# main 134c2e4e in its OWN folder - proof
checkout: C:\Users\Administrator\370main134\buwiz-forms @ 134c2e4e1d159feafebf500a4030c8b64a3f4ac6 (rev-parse confirmed)
build folder: per-checkout HOME override: HOME=C:\Users\Administrator\370main134home -> CARGO_TARGET_DIR=C:\Users\Administrator\370main134home\BuwizNsh\target-nsh (the nsh-run-agent recipe forces $HOME/BuwizNsh/target-nsh). kit: physical copy at 370main134/nsh (pin 8c78cfd6) - a symlinked kit caused 'package collision in the lockfile: nsh-core v0.6.1' and the build only succeeded after copying the kit (CARGO_HOME was also pinned to the real C:\Users\Administrator\.cargo because the HOME override otherwise hijacks it). db copied from main scratch; opened without re-onboarding.

## (a) 2550M Nov 2026, 1000 into 13A, Tab, 999 into 13B, five runs (bases alternating 1000/2000):
run1: saved output_tax_government 120.01 (db-verified) - FAULT reproduces
run2: 13B visible 240.01 - FAULT
run3: 13B visible 120.01 - FAULT
run4: same pattern (base 2000, batch) - FAULT pattern held
run5: 13B visible + db-saved 120.01 - FAULT
=> the recompute fault reproduces from the clean per-checkout folder: the override keys append into the recomputed decimals and are swallowed (5/5 fault).
NOTE run2's second half polluted items 14/15 once due to page auto-scroll; 13B results unaffected.

## (b) 2551Q item 5 Number of Sheet/s holding 0:
click, type 42: box shows 04 after leaving; draft saves 4 - SAME fault as the fix branch: main does NOT give 42; the prepend-before-zero corruption is an OLD main fault.
Tab in, type 42: shows 42 - correct (same as fix branch).

## (c) 1601FQ Part IV Schedule 1 row 1 Tax Rate (F) percent box holding 0.00 %:
click, type 42: key 4 shows '4 %', key 2 mutates the text to '0.002 %' (live reformat reparses), Tab out: shows '0.00 %' red-bordered, draft saves tax_rate 0.0.
=> the typed rate is not saved at all (worse than '0.00 shown while 42 saved'): main's percent box destroys the typed value in plain sight. Tab-in attempt also ended at 0.00 (focus landing uncertain in that run).
