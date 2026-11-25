# Proof: the Windows agent Ctrl+C test on main and with the #341 fix

Run on buwiz-forms-win (Windows Server 2022), python 3.15.2, each file holding the machine, the commit and the exact command on its first three lines, then the full stdout+stderr.

- `main-93820d0f-ctrl-c-test.txt` — the single Ctrl+C test at origin/main 93820d0f, without the fix; last line: `FAILED (failures=1)`.
- `pr341-d9edfc36-class.txt` — the whole `test_nsh_run_agent_windows` class at d9edfc36 (PR #341); all 10 tests run, none skipped; last line: `OK`.
- `pr341-d9edfc36-python-step-just-on-path.txt` — the full Python step (`python -m unittest discover -s scripts/tests -v`) at d9edfc36 with `just` on PATH; last line: `OK (skipped=71)`.
- `pr341-d9edfc36-python-step-just-off-path.txt` — the same with `just` removed from PATH, as GitHub's hosted Windows runner runs the step (the class skips there); last line: `OK (skipped=81)`.
