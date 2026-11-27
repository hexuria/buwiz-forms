# e2e-fixes-apple: the smoke suite on iOS and macOS, fixed in the recipes

Branch `try/e2e-fixes-apple`, rebased on `main` (`6bd529d0`, which includes the merged #428).
gpui-agent-cli **0.4.1** installed from `hexuria/gpui-agent` rev `5867457a364b5c27d208fb729e374983c50cf8fb`
ran every suite and probe here (the report header of each run names it).
Safety on every run: `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`, a scratch
database per run, `--allow-send` for the Queue recipe. No app source (`nsh/`, `crates/`) was changed;
`git diff main -- nsh crates` is empty.

## Final tables

### iOS Simulator (iPhone 17e, iOS 27) — `ios/final/`

| # | recipe | result | ms | failing step |
|---|---|---|---|---|
| 1 | setup-start | pass | 2653 | |
| 2 | sign-up | pass | 22216 | |
| 3 | tax-year | pass | 7909 | |
| 4 | navigate | pass | 3669 | |
| 5 | open-2551q | pass | 3689 | |
| 6 | money-boxes | known gap | 9600 | s7 (gap 3, below) |
| 7 | queue | fail | 15939 | s2 `wait_until queue-continue` |
| 8 | delete-row | setup | 45 | `$table`: no node matches |
| 9 | birth-date | known gap | 1182 | s1 (gap 1; cascade, the app is not on the taxpayer page) |
| 10 | sign-out | known gap | 6814 | s2 (gap 1) |

### macOS — `macos/final/`

| # | recipe | result | ms | failing step |
|---|---|---|---|---|
| 1 | setup-start | pass | 737 | |
| 2 | sign-up | pass | 7440 | |
| 3 | tax-year | pass | 6265 | |
| 4 | navigate | pass | 8983 | |
| 5 | open-2551q | pass | 1977 | |
| 6 | money-boxes | known gap | 8014 | s6: txt16 held `0.25` for `500.25` even paced (gap 2 confirmed on the strict adapter) |
| 7 | queue | fail | 14176 | s2 `wait_until queue-continue` (validation prerequisites, below) |
| 8 | delete-row | setup | 194 | `$table`: no node matches |
| 9 | birth-date | pass | 4353 | |
| 10 | sign-out | fail | 12165 | s4: no `locked-screen` exists under BIR_AUTH_BYPASS |

## (b) gpui-agent / adapter cannot do the step

1. **Gap 1, phone sheets (mechanism now pinned).** The semantic tree parks the sheet container offscreen
   (`tp-edit-sheet` at y 2517, h 0) while its contents report on-screen positions; the Done button
   (`y 754, h 44`, fully visible in the screenshot) is refused as "offscreen" because it falls outside its
   zero-height ancestors' clip chain. `set_value` into the sheet's input works (no hit test). Evidence:
   `ios/final/09-birth-date/failure-tree.json` + `failure.png`. Birth date, the row drawer and the profile
   sheet all hit this on a phone; the recipes are marked `known_gap` per the suite's own labels.
2. **Gap 3, money boxes — now confirmed on the macOS strict adapter too.** The paced desktop variant runs with cli 0.4.1, expands Part II, and the type still lands wrong (`0.25` for `500.25`): gap 2's 'not yet proven on a Mac' is now answered — the strict adapter does not fix money-box typing, because the scrambling comes from injected text meeting the box's auto-reformatting, not from the old listener. On the phone the same class of fault (`0.01`/`520.00`/`250.01`/`520.01` across pacings) needs the software keyboard for a real verdict. Injected text cannot type into an auto-formatting money box:
   the same input lands as `0.01`, `520.00`, `250.01`, `520.01` depending on pacing and form — digits are
   scrambled by the reformatting between key events. The owner warned a fault seen only under synthetic
   typing is not a finding; a person's keyed input needs the `software_keyboard` host feature
   (nsh-gpui-agent#11), which this build does not have (`hello.features` lacks it), so the paced phone
   variant can never run here. Evidence: `ios/final/06-money-boxes/`.
3. **macOS strict adapter: bare `type` does not move keyboard focus out of the segmented TIN control.**
   Deterministic probe matrix (`probes/`, each probe is setup + the New taxpayer form + the step sequence
   + an assert that the name box holds the typed text):
   - probe-a (type TIN, then bare `type` into the name box — the suite's original sequence): **FAIL 4/4**,
     the name stays `""` while the TIN lands fine (`probes/probe-a*/`).
   - probe-b (click the name box first, then type): **PASS 3/3**.
   - probe-c (`set_value`): **PASS**. probe-d (type the name *before* the TIN): **PASS**.
   Focus flags read `false` on every node in the tree, so focus cannot be observed directly.
   Every `type`/`click` on macOS reports `delivery_status: delivered_unverified` even when it lands —
   that status is noise on this adapter, not a failure signal.
   This is the dry run's empty-name failure: it was not flaky, it is deterministic. The recipe fix
   (what a person does): click the box, then type. Applied to `sign-up.wants` and `birth-date.wants`.

## (c) app misbehaves

Nothing proven. Two observations short of a finding:
- the floating to-do pill (`fp-pill`) overlaps accordion toggles at some scroll positions and a press
  there is occluded — a person taps elsewhere or scrolls; the recipes now scroll first.
- under `BIR_AUTH_BYPASS=1` there is no lock screen after Log out (there was never a gate), so
  sign-out's `locked-screen` wait cannot pass in this environment; whether the suite should sign in
  through the mock account server instead is a suite design question.

## (a) recipes wrong, fixed

- `sign-up-phone.json` (new): the picker sheets' search box is clipped above the screen top on a phone
  (y −17, `visible: false`) — a person taps the visible rows directly, so the recipe does, scrolling the
  RDO list in 950 pt sweeps (~52 pt/row) instead of typing in the search. Lands on the taxpayer page,
  which the phone actually shows — the old recipe asserted Home, and `wait_until tab-home` passed
  trivially because the tab bar is everywhere.
- `tax-year.wants` (new prerequisite): a fresh taxpayer has no tax year and New return is refused
  ("Head office has no tax year yet") — the suite could never open 2551Q without it. Adds year 2026 and
  selects 2551Q in Edit forms.
- Home tiles are `home-tp-<9-digit TIN root>`, and the row has no `onPress` (not pressable at all):
  navigate/birth-date fixed to use `$tin_root` and to start from the taxpayer page.
- `open-2551q-phone.wants`: the narrow bar has a direct New return button (`nb-new`); the Search palette
  detour was wrong.
- The form is an accordion on every platform: Items 15/16 do not exist in the tree until Part II is
  expanded (`fp-part-2-toggle`); the money-boxes recipes now expand it and scroll `form-scroll`.
- navigate desktop asserted `tp-template-1701-create`, but the suite's taxpayer files 2551Q only —
  fixed to `tp-template-2551Q-create`.
- Suite order: the three sheet-opening recipes (delete-row, birth-date, sign-out) run last, because on a
  phone a sheet that cannot be closed (gap 1) swallows every later press.
- birth-date desktop typed into the date box and never checked: it passed vacuously (no edit → no save
  button → "save absent" trivially true) while the profile still showed "Not set". Now clicks first,
  types, asserts the value — which also makes it a real test of gap-2-class typing.

## What remains unsolved (not recipe-wrong, not proven app faults)

1. **queue** — validation correctly stops the return: the smoke flow never sets the taxpayer's business
   start date nor the annual income-tax election (both prerequisites visible in `fp-profile-prerequisite-*`
   banners, and "Needed by 2551Q" on the profile), and the money boxes never hold correct values on the
   phone. Fixing means a new profile-editing flow — on a phone it is gap-1 sheets again. The recipe's own
   precondition note predicted this.
2. **delete-row** — no `fp-table-*-add` node exists in the tree on either platform even with Part II
   expanded; 2551Q's Schedule 1 either renders further down than virtualization keeps alive (Part II
   shows only items 2:0–2:2 of 14–24 at once) or appears only when applicable. The suite-level `resolve`
   runs once at recipe start with no chance to scroll first — unsolvable in recipe space as the runner
   stands, short of a suite change.
3. **sign-out on macOS** — see (c): no lock screen exists in the bypass environment.

## The stale agent-token deadlock (one paragraph, for a proper fix in nsh/)

`nsh/agent/ios-handoff.sh token` writes `$GPUI_AGENT_TOKEN` into the simulator data container at
`Library/Caches/gpui-agent-token` **only `if [ ! -f "$file" ]`** — it never overwrites. By design the
window plane reads that file once per surface open and deletes it, and the writer loop keeps rewriting it
while the launcher lives so later surface opens still find one. When a run ends, one copy can be left
unread in the container (the app exited without opening another surface). The next launch generates a
fresh random token, but the stale file already exists, so the writer never installs the new one; the host
reads the stale token at the first surface open while the CLI authenticates with the new one, the tokens
differ, and the runner waits for the plane forever (observed: a 15-minute hang in `wait_for`). Deleting
the container file before every launch is this session's workaround (`scripts` outside the repo);
the proper fix belongs in `nsh/agent/ios-handoff.sh` — overwrite at launch, or key the file to the launch.

## Folders

- `ios/final/` — the final iOS run's full evidence (report.md/json, per-recipe receipts, failure trees,
  screenshots, recordings).
- `macos/final/` — the final macOS run's evidence.
- `probes/` — the macOS name-box probe matrix (probe-a x4, probe-b x3, probe-c, probe-d), each with
  report.md, per-recipe receipts and failure trees/screenshots.

## Left out of the integration PR

`fix/e2e-recipes-all-platforms` carries this folder without the recordings (`recording.mp4`) and the per-step
screenshots (`NNN-sN.png`), 19.6 MB together: they stay on `try/e2e-fixes-apple` at `18d4a188`. The reports,
receipts, failure screenshots, failure trees and probe recipes are here.
