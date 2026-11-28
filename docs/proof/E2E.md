# End-to-end runs: `just e2e`

One shared way for a test machine (a Devin VM, a CI runner) to drive the real app as a person does, on every
platform, so that no machine writes a driver of its own. A run starts the app, runs a suite of
[gpui-agent recipes](https://github.com/hexuria/gpui-agent/blob/main/docs/RECIPES.md), and writes one report.

```sh
just e2e check smoke                                                   # no app: is every recipe of the suite sound?
just e2e macos smoke --allow-send                                      # the whole smoke suite in a real window
just e2e android smoke --only sign-up --only navigate                  # a part of it (no Queue: no --allow-send)
```

`target` is `macos`, `linux`, `windows`, `ios` (the Simulator) or `android` (the emulator). The command is
`python3 scripts/e2e <target> [suite] [options]` under the hood; `--help` lists the options. `just` passes the
options on as words (this justfile supports `just` releases without per-argument quoting): an option value with a
space in it (an `--out` folder) goes to `python3 scripts/e2e` directly.

The run does not depend on `.env`: the runner launches the app in the dry-run test environment itself, and removes
the live SFTP and relay variables (`BIR_SFTP_HOST`, `_PORT`, `_USERNAME`, `_PASSWORD`, `_LIVE`,
`_ACCEPT_ANY_HOST_KEY`, `BIR_RELAY_URL`, `BUWIZ_RELAY_SEND`) that `just` reads from `.env`; `nsh-run-agent` removes
them again after its own read of `.env`. An account server that is not on this machine (`BUWIZ_SERVER_URL`,
`BUWIZ_ACCOUNT_URL`) is removed too, by both, so a debug build talks to the local mock.

## The rule

- **Never fixed-coordinate taps, never OS keystroke injection, never burst typing for proof.** Every press goes
  through gpui-agent to an element the app's own tree names. Before it acts, the host proves with a native hit test
  that the element is on screen, enabled and not covered (the software keyboard included, from
  hexuria/nsh-gpui-agent#11). Typing goes through a recipe `type` step with `key_delay_ms`: one key at a time, at a
  person's pace (gpui-agent-cli 0.4.1, hexuria/gpui-agent#118).
- **A fault seen only under synthetic burst input, or only from a driver of your own, is not a finding.** Re-run it
  through `just e2e` before reporting it.
- **A new flow is a new recipe** in a suite (`agent-recipes/e2e/`), checked with `just e2e check <suite>`.

## What a run does

1. Checks the suite (below), and refuses to start if anything already listens on the agent's ports.
2. Starts the app with `just nsh-run-agent <target> <db>`. That is a debug build in dry-run mode
   (`EBIR_TEST_ENV=1 BIR_SFTP_DRY_RUN=1 BIR_AUTH_BYPASS=1 BUWIZ_NO_KEYCHAIN=1`, never `BIR_SFTP_LIVE`).
   - The database is new, under `~/BuwizNsh/agents/`.
   - On Android, the emulator's guarded test database is removed first, so the run starts at first-run setup.
   - The token is `GPUI_AGENT_TOKEN` if set, else one made for the run. It is never printed, and the run log masks it.
3. Waits for the window's control plane (`GPUI_AGENT_ADDR`, default `127.0.0.1:17421`) and the data plane on the next
   port. The first build can take a long time: `--launch-timeout` is an hour by default.
4. Runs each recipe with `gpui-agent recipe run`, in the variant for what the host reports:
   - **Variant.** `phone` when `hello.platform` is mobile (iPhone, iPad and Android tablets use the phone layout),
     otherwise `desktop`. A variant named after the target (`windows`, `linux`, `android`, ...) wins on that target
     when the suite has one. Linux needs its own: its window opens 420 px wide (although `nsh.json` asks 1440x900),
     so the app shows the narrow layout on a desktop host.
   - **Generated ids.** Ids the app generates (the TIN box, a schedule's table) are read from a snapshot before the
     recipe, or after a named step when the `resolve` spec carries `after`.
   - **Screenshots.** Steps marked `--screenshot` / `"screenshot": true` are saved in the recipe's folder.
5. Stops the app and writes, under `~/BuwizNsh/evidence/e2e/<target>-<time>/` (or `--out`):
   - `report.md` and `report.json`: per recipe, the result, the time, the failing step with its `.wants` line and
     error, the failure screenshot and the tree at the failure;
   - `run.log`: one line per action (time, verb, target, result, ms), for a proof README to include;
   - `app.log`: the app's own output;
   - one folder per recipe, with `receipt.json` (gpui-agent's receipt), the screenshots and, with `--record`, the
     recording. `--record` records only iOS and Android; a desktop video is the machine recorder's, not the runner's.

Results:
- `pass`;
- `fail`;
- `known gap`: failed where the suite already names a gap;
- `needs`: skipped until a gpui-agent release or a host build carries what it needs;
- `refused`: a send step without the safety conditions;
- `skipped`: no variant for this form factor;
- `skipped (needs --allow-send)`: a send recipe run without `--allow-send` (never a failure);
- `setup`: a generated id was not found, before the recipe or at its `after` step;
- `not run`: a required recipe before it failed.

The exit status is 0 only when every recipe passed or has one of the skipped results.

## Safety, in code (`scripts/e2e/runner.py`)

The checks read every word of a step (flag values included) and every string of a JSON step, split camelCase ids
(`fpQueue` is fp, queue), and run again on each id resolved from the tree, with the node's id, name and role.

- **Send steps.** A recipe that presses Queue, Send, Submit or File (or invokes a send) must be marked `"send": true`
  in its suite, or the suite does not run. It runs only with `--allow-send`, in the dry-run test environment
  (`EBIR_TEST_ENV=1`, `BIR_SFTP_DRY_RUN=1`, no `BIR_SFTP_LIVE`: the launch environment, or the runner's own with
  `--no-launch`), and only when the app's data plane answers `app.info` with `"database": "test"`. Without
  `--allow-send` it is `skipped (needs --allow-send)`; with it, a missing safety condition is `refused`.
- **`--no-launch`** drives an app that is already running only when it answers `app.info` with `"database":
  "test"`; otherwise nothing runs.
- **Secret boxes.** No step types, sets, pastes or keys into a box that says password, PIN, passcode, secret,
  token, recovery, OTP or TOTP, also inside a word (`newpin`, `confirmPin`, `adminotp`, `newcode`), or a code box
  (`…-code-boxes`, `…-code-cells`, a node named "6-digit code", "Authenticator code" or "Verification code"), and no
  step presses the software keyboard's keys (`system-keyboard…`). Nested selectors (`{"target": {"id": …}}`) are
  read, and a resolved id is checked by its node's name and role wherever it appears in a selector. The store review
  account's fixed code never goes in a file.
- **Sign-in.** Every press, key or typing on a sign-in screen (`gate-*`, `auth-*`: Enter in the email box sends the
  code), and Continue, Verify or Resend there, only in a recipe marked `"sign_in": true`, in an app this run launched,
  against an account server on this machine. `assert` and `wait_until` there are fine.
- **The token** stays in the environment of the processes that need it, and is masked in `run.log`, the receipts,
  the failure trees and `app.log`.
- **Afterwards** the app is stopped, on the Simulator or emulator too (and the emulator's forwarded ports removed),
  and the run's scratch database is deleted; the evidence folder stays.

## Per machine type

Every machine needs the setup script for its platform. That script installs `just` and `gpui-agent-cli` at the
version `Cargo.lock` pins. Each machine then runs one script from `scripts/e2e/machines/`. Each script checks the
tools, runs `just e2e check smoke`, then the smoke suite with `--allow-send` in the dry-run environment.

| Machine | Script | Needs | Screenshots / recording |
| --- | --- | --- | --- |
| macOS (desktop) | `scripts/e2e/machines/macos.sh` | Xcode command line tools; Screen Recording granted to the app's host binary | gpui-agent window PNGs; record with the machine's own recorder |
| Linux (desktop) | `scripts/e2e/machines/linux.sh` | `scripts/setup-linux-nsh.sh`; a display, or `xvfb-run` (the script uses it when there is none) | gpui-agent has none on Linux (`screenshot_unavailable`); record with the machine's recorder |
| Windows (desktop) | `scripts/e2e/machines/windows.ps1` | `scripts/setup-windows.ps1`, PowerShell 7 | gpui-agent has none on Windows; record with the machine's recorder |
| macOS + iOS Simulator | `scripts/e2e/machines/ios.sh` | one booted Simulator (`xcrun simctl boot <udid>`) | Simulator PNGs; `--record`: `xcrun simctl io booted recordVideo`, one file per recipe |
| Linux + Android emulator | `scripts/e2e/machines/android.sh` | `scripts/setup-linux-nsh.sh` (the Android runtime at the NSH pin), exactly one online emulator, `ANDROID_HOME` | emulator PNGs; `--record`: `adb shell screenrecord`, one file per recipe (180 s at most each) |

gpui-agent records no video by design ([RECORDING.md](https://github.com/hexuria/gpui-agent/blob/main/docs/RECORDING.md)).
The only recordings are the platforms' own plain recorders above. The pictures-and-a-recording rule in
`orchestrator/ops/DEVIN.md` still applies to UI work.

## Suites

A suite is `agent-recipes/e2e/<name>.json`:

```json
{
 "v": 1,
 "params": {"tin": "12345678800000"},
 "recipes": [
  {"name": "sign-up", "desktop": "sign-up.wants", "linux": "sign-up-linux.wants", "phone": "sign-up-phone.json",
   "android": "sign-up-android.json",
   "resolve": {"tin_field": {"role": ["textbox", "textfield"], "name_regex": "^TIN, 9 digits", "before_first_use": true}}, "required": true},
  {"name": "money-boxes", "desktop": "money-boxes-paced.json", "phone": "money-boxes-paced-phone.json",
   "needs_cli": "0.4.1", "needs_feature": {"phone": "software_keyboard"},
   "fallback": {"phone": "money-boxes-phone.json", "file": "money-boxes.wants"}, "known_gap": {"macos": "gap 2: ..."}},
  {"name": "queue", "file": "queue.wants", "send": true}
 ]
}
```

**Recipe file fields.**
- `file`: the same recipe everywhere.
- `phone` / `desktop`: one per form factor.
- `macos` / `linux` / `windows` / `ios` / `android`: one for that target, ahead of the form factor's.
- Paths are relative to the suite.
- Params come from the suite's `params`, the entry's `set` and its `resolve`. Only those a file uses are passed.

**Resolving generated ids.** A `resolve` spec picks the first node of a snapshot that matches every field given:
`id`, `id_regex` (with `group`), `role` (one or a list), `name`, `name_regex`, `visible`.

A spec may add `"after": "<step id>"` when the node can exist only after the recipe has run for a while. JSON recipes
use their own step ids; a `.wants` recipe's steps are `s1`, `s2`, ... in file order. The runner writes the slice files
into the run's evidence folder (never the repository), runs the steps through that step, takes a fresh snapshot and
resolves, then runs the rest in the same app session. `.wants` slicing assumes one step per non-comment line, the same
shape gpui-agent parses; that is what maps a slice line number back to the original step id. The report and the merged
receipt are still one recipe with the original step ids. A node not found there is `setup` at that step, with the
screenshot and tree kept; the run's summary calls it `setup failure`, and the exit status is nonzero. A parameter used at or before its `after` step is refused by `just e2e check <suite>`.

A spec may say `"before_first_use": true` in place of `after`: the node is then looked up right before the first step
that uses the parameter, whatever that step's id is in the variant being run. Use it when one suite entry maps
several variants of a flow whose steps are numbered differently: the `.wants` sign-up first uses `$tin_field` at
`s4`, the JSON ones at `s6`, after two more steps that choose the start option, so one fixed `after` id is too
early in some of them. A spec that names both is refused.

The delete-row case: 2551Q's Schedule 1 is the collapsed part 4 (`fp-part-4-toggle`), and its Add row can be culled
until that part is expanded or scrolled to. `delete-row*.json` first opens its own fresh Q1 return, scrolls to and opens
that part without using `$table`, and only then clicks `$table-add`; the entry's spec is:

```json
{"id_regex": "^(fp-table-.+)-add$", "group": 1, "after": "s3"}
```

**Requirements and fallbacks.**
- `needs_cli` / `needs_feature`: the installed gpui-agent-cli version, or a `hello.features` entry. Either can be a
  map per target or form factor.
- `fallback`: the file to run while those are missing, or a map per form factor (`phone`, then `file`). Without one,
  the recipe is reported as `needs`.

**Labels.**
- `known_gap`: a failure there is reported as `known gap` with that text. It can be a map per target or form factor.
- `precondition`: said alongside a failure.

**Flow.**
- `required`: a failure stops the suite.
- `send`, `sign_in`: see Safety.
- `plane`: `"data"` for data-plane invokes.
- `timeout_s`: the recipe's time limit (900 s by default).

`just e2e check <suite>` checks a suite with no app:
- every file exists and parses;
- every param it uses is supplied, and every placeholder a JSON recipe uses is declared in that recipe's own
  `params` (gpui-agent-cli 0.5.0 refuses it otherwise);
- a param resolved `after` a step is not used at or before that step;
- in a JSON recipe, the cut after that step does not separate a step from a step it `needs`;
- send steps are marked;
- no secret box is typed into;
- `key_delay_ms` appears only on `type`, and only in entries with `needs_cli`;
- a step whose op needs a `hello.features` entry is not in the variant a desktop target (`macos`, `linux` or
  `windows`) would run when that host does not list the feature, unless the entry's own `needs_feature` sends that
  host to a fallback or `needs` first (mobile hosts differ, so check does not infer them);
- then `gpui-agent recipe validate` on each file the installed client can read.

## The smoke suite (`agent-recipes/e2e/smoke.json`)

On a fresh database, in this order (the three recipes that open sheets run last: on a phone a sheet that cannot be
closed swallows every later press). Files are relative to `agent-recipes/e2e/`; "desktop" is macOS and Windows unless
a Windows file is named.

| # | Recipe | What it proves | Desktop | Linux (narrow) | iOS / Android |
| --- | --- | --- | --- | --- | --- |
| 1 | `setup-start` | first-run setup opens; "Just me" | `setup-start.wants` | same | same |
| 2 | `sign-up` | the first taxpayer: TIN, name, address, the ZIP and RDO pickers | `sign-up.wants` | `sign-up-linux.wants` | `sign-up-phone.json` / `sign-up-android.json` (no pickers: gap 1) |
| 3 | `tax-year` | the year and 2551Q in Edit forms (New return is refused without them) | `tax-year.wants` | `tax-year-narrow.wants` | `tax-year.wants` / `tax-year-narrow.wants`; Android `tax-year-android.json` |
| 4 | `navigate` | every screen | `../ui/ui-navigate-app.wants` | `navigate-phone.wants` | `navigate-phone.wants` |
| 5 | `open-2551q` | New return > 2551Q | `open-2551q.wants`; Windows `open-2551q-windows.wants` (also the facts 2551Q validates) | `open-2551q-phone.wants` | `open-2551q-phone.wants` |
| 6 | `money-boxes` | Amended Yes, Item 16 typed and read back; Item 15 typed, read back, formatted when left, unchanged when focused again; Amended No | `money-boxes-paced.json`, fallback `money-boxes.wants` | `money-boxes-phone.json` | `money-boxes-paced-phone.json` (awaits the software keyboard), fallback `money-boxes-phone.json` |
| 7 | `delete-row` | in its own fresh Q1 return, a schedule row added, filled and deleted | `delete-row-desktop.json` | `delete-row.json` | `delete-row.json` |
| 8 | `queue` | a fresh Q2 return validated; Queue visible but not pressed | `queue-desktop.json`; Windows `queue-windows.wants` | `queue-linux.json` | `queue-phone.json` |
| 9 | `queue-send` | a fresh Q3 return validated, then the explicit dry-run Queue/Continue rehearsal | `queue-send-desktop.json`; Windows `queue-send-windows.wants` | `queue-send-linux.json` | `queue-send-phone.json` |
| 10 | `birth-date` | the birth date, typed in the date box / the date sheet, read back, saved | `birth-date.wants` | `birth-date-phone.json` | `birth-date-phone.json` |
| 11 | `sign-out` | Log out; the lock screen | `sign-out.wants` | `sign-out-phone.wants` | `sign-out-phone.wants` |

Why the recipes look as they do (each from a machine's run on the real app, `proof/e2e-fixes-*/README.md`):
- After Create the app is on the taxpayer's own page (Forms tab, Add year band open), not Home. Home tiles are
  `home-tp-<9-digit TIN root>` (`$tin_root`) and are not pressable, so the narrow recipes reach the taxpayer through
  the header's Search (`nb-search`; the narrow palette searches tax profiles only, never forms).
- A fresh taxpayer has no tax year: `tax-year` adds one and ticks 2551Q. On Android the year picker is a sheet, so the
  narrow file takes the year the band offers.
- The form is an accordion on every platform: Items 15 and 16 are not in the tree until Part II
  (`fp-part-2-toggle`) is expanded; the JSON money-box recipes scroll `form-scroll` first (the floating pill can cover
  a toggle). The money boxes type the amounts as written (`500.25`): since #403 a money box keeps its text while
  focused and formats only on leaving, so the box reads `$amount` while focused and `$amount_shown` after it. They do
  not promise to leave Items 15 and 16 at zero; the queue recipes never reuse that return.
- On the macOS strict adapter a bare `type` does not move focus out of the segmented TIN control: the desktop
  recipes click a box before typing into it (sign-up's name, the birth date, delete-row's amount).
- 2551Q's Schedule 1 pre-holds row 1, and Delete row is disabled on an empty row: delete-row fills the new row's
  amount, deletes it and checks the row-2 line is gone (`$table-none` never shows).
- Log out on a phone is `nb-menu-logout` in the header's account menu (`nsh/shell/Narrow.tsx`).
- On Android every press inside a bottom sheet is refused (gap 1), so Android's sign-up leaves the ZIP and RDO empty
  (valid) instead of failing the required recipe; iOS and every desktop keep the pickers. The phone-shaped JSON recipes
  hide the software keyboard after `set_value`: the next low control is otherwise keyboard-occluded. Linux's window is
  narrow enough to need the same layout, but its desktop host has no system keyboard actions, so the Linux queue recipes
  are the phone recipes without the hide step.

## Results on the real apps (2026-11-27, gpui-agent-cli 0.4.1)

Each machine ran the suite on its own branch (`try/e2e-fixes-apple` 18d4a188, `try/e2e-fixes-windows` c8ce9008,
`try/e2e-fixes-linux-android` 53e241c5, all on main 6bd529d0), before the three were merged into this suite and
before #403. The merged suite has not been run as a whole. Evidence: `proof/e2e-fixes-apple/{ios,macos}/final/`,
`proof/e2e-fixes-windows/`, `proof/e2e-fixes-linux-android/{android,linux}-final/`.

| Recipe | Windows | macOS | iOS Simulator | Android emulator | Linux (420 px) |
| --- | --- | --- | --- | --- | --- |
| setup-start | pass | pass | pass | pass | pass |
| sign-up | pass | pass | pass | pass (no pickers) | pass |
| tax-year | (in open-2551q) | pass | pass | (in open-2551q) | (in open-2551q) |
| navigate | pass | pass | pass | pass | pass |
| open-2551q | pass | pass | pass | pass | pass |
| money-boxes | pass (paced) | known gap: s6 `500.25` read `0.25` | known gap: s7 read `520.01` | fail: s2 `target_gone` txt16 (Part II not expanded) | fail: same |
| queue | fail: s9 `fp-cancel-queue` (does not validate) | fail: s2 `queue-continue` | fail: s2 | fail: s2 | fail: s2 |
| delete-row | setup: no `fp-table-*-add` (known gap marked for Windows) | setup | setup | setup | setup |
| birth-date | pass | pass | known gap: s1 (cascade: not on the taxpayer page) | known gap: s14 sheet Done refused offscreen | fail: s14 the same (now marked gap 1 for Linux) |
| sign-out | known gap: Log out needs an admin PIN | fail: s4 no `locked-screen` | known gap: s2 `nb-menu-profile` (id fixed since) | known gap: s1 occluded by the date sheet | fail: s1 the same (now marked gap 1 for Linux) |

## Gaps to fix in gpui-agent, the plug and the app

In gpui-agent and the plug (nsh-gpui-agent):
- **Gap 1, sheets** (hexuria/nsh-gpui-agent#13): a sheet's nodes report `visible: false` or a zero-height container
  though painted, so every press inside one is refused as offscreen: the date sheet (Linux, Android, iOS), the row
  drawer (Windows), Android's picker sheets. `set_value` into a sheet's box lands. Modal dialogs hit-test fine.
- **Scrolling** (hexuria/gpui-agent#121): `.wants` has no scroll verb (JSON recipes have `op: scroll`), and the tree
  culls nodes outside the viewport with no "scroll until it exists" step. The runner can now resolve after a recipe
  step; delete-row uses that: it opens a fresh return, scrolls to Schedule 1 and resolves `$table` only after step s3.
- **macOS focus** (hexuria/nsh-gpui-agent#14): bare `type` does not move focus out of the segmented TIN control;
  focus flags read false everywhere; every step reports `delivered_unverified`, landed or not.
- **Money boxes under injected text** (hexuria/nsh-gpui-agent#15): paced `500.25` lands as `0.25` on macOS and as
  scrambled digits on iOS; the same recipe passed on Windows. Re-run on main (#403) before acting.
- **The software keyboard in the tree** (hexuria/nsh-gpui-agent#11): until it ships, phones run the money-box fallback,
  which is not a person's keyed input (labelled `known gap` on phones).

In the app and this repo:
- **`ios-handoff.sh` keeps a stale agent token** (#449): the token file is written only if absent, so the next launch's
  token never replaces a leftover one and the runner waits forever. Workaround: delete the container file first.
- **sign-out under `BIR_AUTH_BYPASS=1`** (#450): no lock screen exists after Log out, and on Windows Log out needs an
  admin PIN the suite may not type. A suite-design question.
- **queue** (#451): needs a taxpayer whose profile validates for 2551Q (business start date, income-tax election, Part I
  answers, schedule amounts). Windows sets the facts in `open-2551q-windows.wants` and `queue-windows.wants`; carry
  them to the other desktops once proven there.
- **The Linux window opens 420 px wide** although `nsh.json` asks 1440x900, so the Linux desktop runs the narrow layout.

Seen only under injected input, not chased: on Linux, machine-speed typed digits came out of order in the TIN cells
(`12345678800000` read `124-567-880`). The Linux sign-up fills its boxes with `set_value`; no app finding is made of it.

Also known:
- **A person's pace** needs gpui-agent-cli 0.4.1 (`key_delay_ms`, hexuria/gpui-agent#118). On 0.4.0 the money boxes
  run their unpaced fallback, and the report says so. `just e2e check` with an older client (0.1.0) also refuses
  `sign-up-phone.json`'s `scroll` steps.
- **The agent door refuses while the app is locked**, by design. Recipes that need the data plane run before Log out.
