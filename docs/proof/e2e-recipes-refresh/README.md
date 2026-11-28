driver: gpui-agent-cli 0.4.1

Recipes: the `smoke` suite on macOS and iOS, plus the macOS `rdo-row-press` suite.

Evidence:
- `macos/report.md` and `macos/report.json`: fresh-database macOS smoke run at app commit `d0025c42`.
- `macos/sign-out-failure.png`: the bypassed sign-out gap, issue #450.
- `macos/queue-check-no-problems.png`: the macOS queue recipe stopped after validation and before Queue.
- `macos-rdo/report.md` and `macos-rdo/report.json`: macOS `rdo-row-press` run; its app header is merge commit `ca657bea` because it ran before the recipe-only queue-stop commit.
- `ios/report.md` and `ios/report.json`: fresh-database iPhone smoke run at app commit `d0025c42`.
- `ios/sign-up-failure.png` and `ios/sign-up-failure-tree.json`: the current iPhone SearchPick stop.

## open: needs the plug pin of #584

The iPhone run starts from a fresh scratch database. `setup-start` passes, then `sign-up` stops at `s9a` because the phone ZIP SearchPick sheet does not open. Four gpui-agent-only attempts were made against the same fresh-database flow; each ended at step `s9a` with exactly:

```text
wait_until timed out: node `new-tp-zip_code-search` not found
```

Attempts:
1. `press` on `new-tp-zip_code-trigger` — wait_until timed out: node `new-tp-zip_code-search` not found
2. `click` on `new-tp-zip_code-trigger` — wait_until timed out: node `new-tp-zip_code-search` not found
3. `click` on the containing `new-tp-zip_code` control — wait_until timed out: node `new-tp-zip_code-search` not found
4. `click` with `delivery: "virtual"` on `new-tp-zip_code-trigger` — wait_until timed out: node `new-tp-zip_code-search` not found

The recipe is left at the physical `press` steps. This is the covered-control/press-then-keys case expected to be fixed when the app picks up the plug pin from pull request #584.

## Sign-out without the bypass

`sign-out-mock.md` describes the separate no-bypass mock-account sign-out run. It is outside the default suite because the suite launcher starts the app with `BIR_AUTH_BYPASS=1`.
