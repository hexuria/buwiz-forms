driver: gpui-agent-cli 0.5.0 + nsh-gpui-agent b0c1a5f2e0a23d9b37a8c3059fac52c0c9feae0b (app b3bddd4d)

# job 63 part 5: job-58 recipes on Linux

The wrapper was verified before the run with `app_dir=$HOME/repos/j63/app/nsh`. The app was detached at `b3bddd4d252d0daac05b322f18ce6e45e6c1fa18`; job 58 recipes from `6117e181bc87c7906d128cf84555c84fe204c5b9` were overlaid uncommitted, with `s13p` removed and `may_scroll` `"phone": ["s14"]` added. `just e2e check smoke` passed: `11 recipes, 30 files validated by gpui-agent-cli 0.5.0, 0 problem(s)`.

Linux smoke result: 8 pass, 1 skipped, 1 refused, 1 known gap. No step result showed `scrolled`; no `error_code: scrolled` occurred; no recipe in `report.json` has a `scrolled` field.

Parts 1 and 2 in this proof branch are environment-suspect (the wrapper pointed at another checkout), not evidence.
