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
     otherwise `desktop`. A `windows` variant wins on Windows when the suite has one.
   - **Generated ids.** Ids the app generates (the TIN box, a schedule's table) are read from a snapshot first
     (`resolve`).
   - **Screenshots.** Steps marked `--screenshot` / `"screenshot": true` are saved in the recipe's folder.
5. Stops the app and writes, under `~/BuwizNsh/evidence/e2e/<target>-<time>/` (or `--out`):
   - `report.md` and `report.json`: per recipe, the result, the time, the failing step with its `.wants` line and
     error, the failure screenshot and the tree at the failure;
   - `run.log`: one line per action (time, verb, target, result, ms), for a proof README to include;
   - `app.log`: the app's own output;
   - one folder per recipe, with `receipt.json` (gpui-agent's receipt), the screenshots and, with `--record`, the
     recording.

Results:
- `pass`;
- `fail`;
- `known gap`: failed where the suite already names a gap;
- `needs`: skipped until a gpui-agent release or a host build carries what it needs;
- `refused`: a send step without the safety conditions;
- `skipped`: no variant for this form factor;
- `setup`: a generated id was not found;
- `not run`: a required recipe before it failed.

The exit status is 0 only when every recipe passed or was skipped.

## Safety, in code (`scripts/e2e/runner.py`)

The checks read every word of a step (flag values included) and every string of a JSON step, split camelCase ids
(`fpQueue` is fp, queue), and run again on each id resolved from the tree, with the node's id, name and role.

- **Send steps.** A recipe that presses Queue, Send, Submit or File (or invokes a send) must be marked `"send": true`
  in its suite, or the suite does not run. It runs only with `--allow-send`, in the dry-run test environment
  (`EBIR_TEST_ENV=1`, `BIR_SFTP_DRY_RUN=1`, no `BIR_SFTP_LIVE`: the launch environment, or the runner's own with
  `--no-launch`), and only when the app's data plane answers `app.info` with `"database": "test"`. Otherwise it is
  `refused`.
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
  {"name": "sign-up", "desktop": "sign-up.wants", "phone": "../ui/ui-sign-up-phone.wants",
   "resolve": {"tin_field": {"role": ["textbox", "textfield"], "name_regex": "^TIN, 9 digits"}},
   "required": true, "known_gap": {"phone": "gap 1: ..."}},
  {"name": "money-boxes", "desktop": "money-boxes-paced.json", "phone": "money-boxes-paced-phone.json",
   "needs_cli": "0.4.1", "needs_feature": {"phone": "software_keyboard"}, "fallback": "money-boxes.wants"},
  {"name": "queue", "file": "queue.wants", "send": true}
 ]
}
```

**Recipe file fields.**
- `file`: the same recipe everywhere.
- `phone` / `desktop` / `windows`: one per form factor.
- Paths are relative to the suite.
- Params come from the suite's `params`, the entry's `set` and its `resolve`. Only those a file uses are passed.

**Resolving generated ids.** A `resolve` spec picks the first node of a snapshot that matches every field given:
`id`, `id_regex` (with `group`), `role` (one or a list), `name`, `name_regex`, `visible`.

**Requirements and fallbacks.**
- `needs_cli` / `needs_feature`: the installed gpui-agent-cli version, or a `hello.features` entry. Either can be a
  map per target or form factor.
- `fallback`: the file to run while those are missing. Without one, the recipe is reported as `needs`.

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
- every param it uses is supplied;
- send steps are marked;
- no secret box is typed into;
- `key_delay_ms` appears only on `type`, and only in entries with `needs_cli`;
- then `gpui-agent recipe validate` on each file the installed client can read.

## The smoke suite (`agent-recipes/e2e/smoke.json`)

On a fresh database:

| # | Recipe | What it proves | Desktop / phone |
| --- | --- | --- | --- |
| 1 | `setup-start` | first-run setup opens; "Just me" | same |
| 2 | `sign-up` | the first taxpayer: TIN, name, address, the ZIP and RDO pickers | `sign-up.wants` / `ui/ui-sign-up-phone.wants` |
| 3 | `navigate` | every screen | `ui/ui-navigate-app.wants` / `navigate-phone.wants` |
| 4 | `birth-date` | the birth date, typed in the date box / the date sheet, saved | `birth-date.wants` / `birth-date-phone.wants` |
| 5 | `open-2551q` | New return > 2551Q | `open-2551q.wants` / `open-2551q-phone.wants` (Search) |
| 6 | `money-boxes` | Amended Yes, Item 16 typed and read back; Item 15 (a zero box) typed, read back, formatted when left, unchanged when focused again (a filled box); Amended No | paced: `money-boxes-paced.json` / `money-boxes-paced-phone.json` (awaits the software keyboard); fallback `money-boxes.wants` |
| 7 | `delete-row` | a schedule row added and deleted | same |
| 8 | `queue` | Queue in the dry-run test build (send) | same |
| 9 | `sign-out` | Log out; the lock screen | `sign-out.wants` / `sign-out-phone.wants` |

## Known gaps and what lifts them

- **Gap 1: presses inside sheets** (bottom sheets, the RDO and ZIP pickers, the date sheet, the row drawer) did not
  land in this week's rounds. No failure text was recorded anywhere. The suite marks those phone steps as
  `known gap`, and a failure keeps its exact error, screenshot and tree, so the first run records the cause.
- **Gap 2: the money box refused agent text on macOS.** The macOS window plane was the plug's older in-process
  listener (`gpui-agent-server`). That listener lists no `press`, `paste` or `scroll`, and presses the box again for
  each `type` or `key`, which moved the caret in a filled box (`try/money-box-agent-input`). This branch moves macOS to
  the strict adapter every other platform uses (`nsh/Cargo.toml`). Until a Mac run proves it, macOS money-box
  failures are labelled `known gap`.
- **A person's pace** needs gpui-agent-cli 0.4.1 (`key_delay_ms`, hexuria/gpui-agent#118). On 0.4.0 the money boxes
  run their unpaced fallback, and the report says so.
- **The software keyboard** needs the app built with nsh-gpui-agent at hexuria/nsh-gpui-agent#11 (the rev in
  `nsh/Cargo.toml` moves once it is merged). Until then, phones run the money-box fallback, and a press under the
  keyboard is not refused.
- **The agent door refuses while the app is locked**, by design. Recipes that need the data plane run before Log out.
