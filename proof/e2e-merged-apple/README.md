# e2e-merged-apple: the merged smoke suite on macOS and the iPhone Simulator

PR hexuria/buwiz-forms#452 head `7d3b77fe6aeea20ebd5b6fecd5d956058ec80e77` (branch
fix/e2e-recipes-all-platforms), checked out exactly and confirmed with `git rev-parse HEAD`.
gpui-agent-cli **0.4.1 from crates.io** (`cargo install gpui-agent-cli --version 0.4.1 --force`)
ran every suite and every probe. Safety on every run: `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1
BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`, a scratch database per run, `--allow-send` inside the suite
only, nothing ever signed in with a PIN or password. The stale-token workaround (delete the
simulator container's `Library/Caches/gpui-agent-token` before launch) was used for iOS; the cause
is filed as hexuria/buwiz-forms#449. No app source changed: every fix is in `agent-recipes/e2e/`
only; `git diff` over `nsh/` and `crates/` is empty.

## Step 7 — does paced typing of 500.25 into a money box now land whole, after #403?

Probe: the suite chain setup-start → sign-up → tax-year → open-2551q, then a money recipe typing
the REAL amounts with `key_delay_ms 80` and reading the value back. Three runs per platform per
variant (probe recipes `mbprobe*.json` beside the suite, raw evidence quoted here):

| platform | typing WITHOUT clicking the box first | typing WITH click-first |
|---|---|---|
| macOS | `00.25`, (`txt16` `500.25` ok, then txt15 `234.56`), `00.25` — the FIRST keystroke is eaten when the box is not focused, the rest land in order | **`500.25` whole 3/3** — full chain passes including the formatted `1,234.56` read after leaving the box |
| iOS | `.52005`, `.52050`, `520.05` — digit PERMUTATIONS, never whole | `.52500`, `.52005`, `520.05` — still permutations, click-first changes nothing |

**Answer for hexuria/nsh-gpui-agent#15:** on macOS YES — post-#403, paced typing of 500.25 lands
whole (3/3) when the box is clicked first, and the pre-#403 `0.25` failure is gone; the remaining
macOS failure mode without a click is a focus-transition artifact (the first keystroke is eaten),
not a money-box corruption, and the fix belongs in the recipe (click first — what a person does),
which this branch does. On iOS NO — injected paced key events arrive REORDERED (digit permutations
in 6/6 probe runs, with or without click-first); that is gpui-agent-cannot on the iOS adapter, and
a real verdict needs the software-keyboard host feature (nsh-gpui-agent#11), which this build does
not have. #15 is therefore settled for the desktop adapter and still open for iOS injection.

## What changed in the recipes, and why

1. `money-boxes-paced.json` (desktop): insert `click <box>` before each paced `type`. Post-#403 the
   box keeps typed text while focused, but the strict adapter's bare `type` does not focus the box
   and the first keystroke is eaten (`500.25` → `00.25`). Clicking first — what a person does —
   makes the recipe pass 3/3 in probes and in the final macOS suite.
2. `money-boxes-phone.json` (phone/linux fallback): same click-first insertion for the same reason.
3. `smoke.json` money-boxes: dropped the stale macOS `known_gap` (the recipe passes now); rewrote
   the phone gap text with the post-#403 permutation evidence above.
4. `queue.wants`: the return validation correctly stops Queue on a profile missing the business
   start date and the annual income-tax election (the merged suite's own `precondition` note). The
   recipe now records both prerequisites through the banners' own Set buttons — business start date
   typed into the branch editor, election chosen as Graduated — each editor returning to the form on
   save, then queues for real. New suite param `bstart: "01/01/2020"`.
5. `smoke.json` sign-out: added the macOS `known_gap` — under the mandatory `BIR_AUTH_BYPASS=1`
   there is no lock gate, so `locked-screen` can never appear after Log out in this environment.
   (The `nb-menu-logout` id fix in the merged branch is confirmed working: the press lands.)

## Final tables

See `macos/report.md` and `ios/report.md` (plus `report.json` each). One screenshot per failure is
in the per-recipe folders.

## Sorted failures

- (a) recipe wrong — FIXED here: money-boxes click-first (both platforms), queue prerequisites,
  sign-out macOS metadata.
- (b) gpui-agent cannot: iOS money-box digit permutations (gap 3, above); phone sheet presses
  refused offscreen (gap 1 — birth-date, sign-out cascade on iOS); delete-row `$table` resolution —
  no `fp-table-*-add` node exists in the tree at recipe start even with Part II expanded, and
  resolve runs once with no chance to scroll, so the recipe cannot be written in recipe space as
  the runner stands (both platforms, `setup` outcome).
- (c) the app misbehaves: NONE proven. Queue's original stop was correct validation behaviour a
  person would also see (screenshot in `macos/07-queue/` of the earlier run).
- Unresolved, honestly: queue's last step — after the confirm press the wait for `fp-cancel-queue`
  times out though `queue-continue` was clickable and pressed (macOS s18). Whether the queue
  actually landed against the mock, and what a queued return exposes in the tree, needs product
  knowledge or one more probe round than the budget allowed.
- Flake observed once: macOS sign-up s19 `nav-home` timeout in one probe run; passed in every
  full-suite run (transient).
