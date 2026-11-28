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

## `--accounts mock`

A plain run keeps the old development bypass: the account gate is open and no account exists. `--accounts mock` is for
recipes that must prove sign-in and per-account behavior in the real app. It starts `just mock-account --auto-approve`
(the same loopback in-memory server the Android development scripts use, pinned to `127.0.0.1:3008`), stops it at the
end, points both `BUWIZ_SERVER_URL` and `BUWIZ_ACCOUNT_URL` at `http://127.0.0.1:3008`, and sets `BIR_AUTH_BYPASS`
to `0`: the gate opens only for the exact value `1`, the app's dotenv does not replace a variable that is already set,
and PowerShell does not treat `0` as an unset variable the way it treats an empty one. The run sets both variables:
`BIR_DEVICE_DIR` is its own absolute `~/BuwizNsh/agents/e2e-<target>-<time>-<id>/` folder, and `BIR_DATABASE_PATH`
is `<that folder>/bir_data.db`. An app built with the seam lets `BIR_DEVICE_DIR` win and runs in rooms mode
(`device.db`, `rooms/<id>/bir_data.db`, and the sign-in files inside that folder); an app built before it ignores the
device variable but stays in single-room mode inside the same per-run folder, never in the machine user's real data
folder, and is refused by the launch proof below. The saved and pending sign-in files are pinned inside the same
per-run folder, removed before launch, and
go with that folder afterwards, so no earlier sign-in or half-finished sign-in can carry into the run. A plain run
keeps one scratch database file directly in `~/BuwizNsh/agents/`; it still shares the device store and identity there,
and this change leaves it alone. Only the mock run's pinned sign-in files are removed: scratch cleanup never touches a sign-in file in the app's
shared device folder, and a plain bypass-mode run touches no shared sign-in file there. After launch the runner waits a bounded time for the app's real sign-in-gate
node (`gate-root`) before the first recipe; `setup-root`, `home` or `sign-in` appearing first proves the run is already
past the gate and refuses at once.

`--accounts mock` on the `android` target is refused before the mock is started, anything is written, or the app is
launched: `BIR_DEVICE_DIR` cannot reach the app process on the device, and a host path does not exist there.
Rooms-mode Android mock sign-in needs a device-side seam and is not supported yet; the older single-room Android mock
behavior is not part of this mode. A plain fresh Android run still removes only the emulator's guarded test database.
A bypass-mode Android run first reads the host app's private `dev.env` and refuses one carrying stale mock settings,
because dev.env wins over the next launch's baked settings; its refusal includes the exact adb `rm -f` command,
quoted for PowerShell on Windows (the shell this page tells Windows users to paste into) and for a POSIX shell
elsewhere.

The mode refuses `--no-launch`, refuses when either account URL in the environment is not loopback (`127.0.0.1`,
`[::1]` or `localhost`), and refuses when port 3008 is already taken: the mock has no health or version route by
which another listener can be identified. The dry-run release and database guards are the same as any other run.
Recipes can use the runner-supplied `$account_a` and `$account_b` (both invented
`example.com` addresses) and `$code_1` through `$code_8` (`246810`, then counting up as the mock does). In mock mode
only, a `type` step may put exactly one of those runner-supplied `$code_N` fixtures into the sign-in gate's code box;
the literal code, any other placeholder or value, any other secret box, and every other input operation or mode stay
refused. The exception is pinned to the gpui-agent `type` schema (`op`, `id`, `target`, `text`, `key_delay_ms`, plus
the runner's own `needs`, `screenshot`, `line` and `words` metadata, and gpui-agent's `delivery` /
`expect_fingerprint`): any extra digit-carrying field is read as the secret it could carry. Whether a typed digit then lands in the real page is the sign-in page fix of #608, not part of this runner
change.

The rooms-mode proof replaces the old scratch-database proof in mock mode. Before launch the per-run folder must be
fresh: an existing folder, with or without a marker, is refused. A seam app atomically writes
`<device_dir>/.ebirforms-test-device.json` (a temporary file in the same folder, then a rename) as the first thing it
does, before the account gate locks even the info planes, and the marker must exist by the time the window's control
plane answers `hello`: `{"device_dir": "<plain absolute path>", "rooms": true, "pid": <pid>, "seam": 1}`. The path is
never Windows' extended `\\?\` or `\\?\UNC\` spelling, although the runner normalises those spellings before its own
comparisons. Because the build inside the launcher can take most of `--launch-timeout`, the runner waits for that
`hello` first and only then polls the marker about every 250 ms for up to 10 seconds. A partial marker, unreadable
JSON, or a transient operating-system read error is retried until that deadline and the last error is named; a marker
whose complete JSON says the wrong thing is refused at once. The marker must say `seam: 1` and `rooms: true`, its
canonical `device_dir` must be exactly the per-run folder, and `pid` must be a positive integer. The pid is not
required to be the launcher's pid (the launcher is a shell or PowerShell wrapper); on POSIX a dead pid is only
logged. A launcher that exits early is reported from the app's own launcher output, not as a marker timeout. No
marker in the bound is exactly: `this app build has no BIR_DEVICE_DIR seam: mock sign-in runs need an app with it
(see docs/proof/E2E.md)`.

With `BIR_DEVICE_DIR` set, single-room mode is off: `BIR_DATABASE_PATH` and `set_database_path_override` are ignored
for room choice, and the device and data directories are the named device folder. The runner still passes the
contained fallback `BIR_DATABASE_PATH` so a pre-seam app stays inside the per-run folder and fails the marker proof.
After the sign-in recipes and before the first recipe that can write, the runner retries `app.info` for up to 30
seconds and refuses unless it reports rooms mode in the per-run folder and an existing open database whose
`database_path` is exactly `<device_dir>/rooms/<one non-empty room id>/bir_data.db` by path components. Any other
shape, including the fallback `<device_dir>/bir_data.db`, is refused as single-room. Mock mode therefore needs an app
built with the `BIR_DEVICE_DIR` seam; plain bypass mode is unchanged (single-room, `BIR_DATABASE_PATH`,
`"database": "test"`).

The account gate locks the data plane as well as the window: even `hello` is refused there. Recipes marked
`"sign_in": true` are the boundary that may run first (they write account sign-in state, not the filing database).
Such a recipe is allow-listed: only `click`, `press`, `type`, `key`, `assert`, `wait_until` or `scroll`, and only on
`gate-*`, `sign-in-*`, or selected first-run setup ids. The ids that exist in `nsh/screens/auth/Setup.tsx` are
`setup-root`, `setup-card`, `setup-solo`, `setup-helpdesk`, `setup-pin-card`, `setup-admin-pin`, `setup-back`,
`setup-pin-submit`, `setup-key-card`, `setup-key`, `setup-key-done` and `setup-taxpayer`; there are no
`setup-solo-card` or `setup-helpdesk-card` ids. The containers `setup-root`, `setup-card`, `setup-pin-card`,
`setup-key-card` and `setup-taxpayer` may be targets only of `assert` and `wait_until`: they hold writing children,
and gpui-agent's `press <container> --within X,Y` or virtual delivery can hit one. `--within` and `--delivery` are
refused in every sign_in recipe. The non-writing leaf ids `setup-helpdesk`, `setup-admin-pin`, `setup-back` and
`setup-key` may be acted on. `setup-solo` writes `setup_solo` at once when a taxpayer exists, and `setup-pin-submit`
writes `setup_helpdesk`; `setup-key-done` itself does not write (`finishHelpdeskSetup` only signs the admin in), but
it crosses the post-setup boundary, so all three stay outside sign_in. Settings' `setup-change-*` ids are outside
the first-run page and are refused too. A backslash, non-ASCII whitespace, or an assertion flag the runner does not
model for this boundary (`--screenshot`, `--value`, `--role` and the state flags) is refused rather than reparsed
differently from gpui-agent-recipe 0.5.0. A parameter is checked again after `resolve` binds its real node id. The rule is
allow-list based because the runner's op names and the app's stable ids are not recipe-chosen: a recipe can choose
`fp-queue`, but cannot make that filing node one of the allowed ids. In a plain run the `"database": "test"` proof
still happens before the first recipe, as before.

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

**Cancellation model.** The runner owns only the processes it started: the mock-account server, the app launcher, and
any emulator reverse it added. Ctrl+C, SIGTERM and SIGHUP (where the platform has them) are one boundary at the
runner: `finally` stops the app process group and the mock in reverse ownership order, closes the device-side app
and forwards, redacts and writes the evidence, and only then removes the mock run's per-run folder (or the plain
run's scratch database). A second Ctrl+C can interrupt that
`finally`: what remains is the per-run folder (with any room database, device database and pinned sign-in files), and
possibly the mock or app processes the first interrupt had not yet stopped. Verification boundary: `scripts/tests/test_e2e.py`
proves the SIGTERM/SIGHUP conversion and handler restoration in Python and the cleanup seams with fakes; it does not
prove OS signal delivery or process-group behavior inside every platform terminal, only the runner's ownership and
ordering once the signal or interrupt reaches Python.

On Windows a CI cancel can kill the runner before Python gets that boundary, and because Windows does not keep a
killable tree for grandchildren after their launcher exits, the app or mock can survive even when the launcher did not.
The mock-account process, the app, or the per-run folder can then be left behind. Recover with this command after
confirming the run is dead and using the process id from the run's own window or process list (never an image-name
kill, which could hit another mock). The command is PowerShell, the shell the Windows setup uses; a full adb path the
runner prints for a stale Android dev.env is quoted for this same shell and starts with PowerShell's call operator
`&` when it needs quotes:

```powershell
taskkill /PID <pid> /T /F
```

If an older or interrupted Android mock attempt left its own artifacts behind, remove exactly those too:

```powershell
adb -s <serial> shell run-as com.longbridge.nsh.host rm -f files/nsh/dev.goldcoders.bir/dev.env
adb -s <serial> reverse --remove tcp:3008
```

1. Checks the suite (below), and refuses to start if anything already listens on the agent's ports.
2. Starts the app with `just nsh-run-agent <target> <db>`. That is a debug build in dry-run mode
   (`EBIR_TEST_ENV=1 BIR_SFTP_DRY_RUN=1 BUWIZ_NO_KEYCHAIN=1`, never `BIR_SFTP_LIVE`); a plain run adds
   `BIR_AUTH_BYPASS=1`, while `--accounts mock` sets it to `0`.
   - A plain run's database is new, under `~/BuwizNsh/agents/`; mock mode gives the app both its per-run rooms-mode
     device folder there and `BIR_DATABASE_PATH` inside it.
   - On Android, the emulator's guarded test database is removed first, so the run starts at first-run setup.
   - The token is `GPUI_AGENT_TOKEN` if set, else one made for the run. It is never printed, and the run log masks it.
3. Waits for the window's control plane (`GPUI_AGENT_ADDR`, default `127.0.0.1:17421`); the first build can take a
   long time and `--launch-timeout` is an hour by default. In mock mode the seam marker is expected by that `hello`,
   so the runner then gives it only the short bounded retry described above. A plain run also proves the data plane
   on the next port now; a mock run proves the open room database from `app.info` before the first recipe that can
   write, as above.
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
     recording.

Results:
- `pass`;
- `fail`;
- `known gap`: failed where the suite already names a gap;
- `needs`: skipped until a gpui-agent release or a host build carries what it needs;
- `refused`: a send step without the safety conditions;
- `skipped`: no variant for this form factor;
- `setup`: a generated id was not found, before the recipe or at its `after` step;
- `not run`: a required recipe before it failed.

The exit status is 0 only when every recipe passed or was skipped.

## Safety, in code (`scripts/e2e/runner.py`)

The checks read every word of a step (flag values included) and every string of a JSON step, split camelCase ids
(`fpQueue` is fp, queue), and run again on each id resolved from the tree, with the node's id, name and role.

- **Send steps.** A recipe that presses Queue, Send, Submit or File (or invokes a send) must be marked `"send": true`
  in its suite, or the suite does not run. It runs only with `--allow-send`, in the dry-run test environment
  (`EBIR_TEST_ENV=1`, `BIR_SFTP_DRY_RUN=1`, no `BIR_SFTP_LIVE`: the launch environment, or the runner's own with
  `--no-launch`), and only after the scratch proof for its mode: `"database": "test"` in a plain run, or the open
  room database inside the per-run folder in a mock run. Otherwise it is `refused`.
- **`--no-launch`** drives an app that is already running only when it answers `app.info` with `"database":
  "test"`; otherwise nothing runs.
- **Secret boxes.** No step types, sets, pastes or keys into a box that says password, PIN, passcode, secret,
  token, recovery, OTP or TOTP, also inside a word (`newpin`, `confirmPin`, `adminotp`, `newcode`), or a code box
  (`…-code-boxes`, `…-code-cells`, a node named "6-digit code", "Authenticator code" or "Verification code"), and no
  step presses the software keyboard's keys (`system-keyboard…`). Nested selectors (`{"target": {"id": …}}`) are
  read, and a resolved id is checked by its node's name and role wherever it appears in a selector. The one exception
  is the mock-account mode's exact runner-generated `$code_N` in a `type` step into exactly `gate-code-boxes`:
  invented loopback fixture data only, never a stored or typed secret. The store review
  account's fixed code never goes in a file.
- **Sign-in.** Every press, key or typing on a sign-in screen (`gate-*`, `auth-*`: Enter in the email box sends the
  code), and Continue, Verify or Resend there, only in a recipe marked `"sign_in": true`, in an app this run launched,
  against an account server on this machine. `assert` and `wait_until` there are fine.
- **The token** stays in the environment of the processes that need it, and is masked in `run.log`, the receipts,
  the failure trees and `app.log`.
- **Afterwards** the app is stopped, on the Simulator or emulator too (and the emulator's forwarded ports removed);
  then the plain run's scratch database is deleted or the mock run's whole per-run rooms folder is deleted. Shared
  default-location sign-in files are never deleted. The evidence folder stays.

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
   "android": "sign-up-android.wants",
   "resolve": {"tin_field": {"role": ["textbox", "textfield"], "name_regex": "^TIN, 9 digits"}}, "required": true},
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
- Params come from the suite's `params`, the entry's `set` and its `resolve`; under `--accounts mock` the runner's
  sign-in fixtures win over same-named suite or entry values. Only those a file uses are passed.

**Resolving generated ids.** A `resolve` spec picks the first node of a snapshot that matches every field given:
`id`, `id_regex` (with `group`), `role` (one or a list), `name`, `name_regex`, `visible`.

A spec may add `"after": "<step id>"` when the node can exist only after the recipe has run for a while. JSON recipes
use their own step ids; a `.wants` recipe's steps are `s1`, `s2`, ... in file order. The runner writes the slice files
into the run's evidence folder (never the repository), runs the steps through that step, takes a fresh snapshot and
resolves, then runs the rest in the same app session. `.wants` slicing assumes one step per non-comment line, the same
shape gpui-agent parses; that is what maps a slice line number back to the original step id. The report and the merged
receipt are still one recipe with the original step ids. A node not found there is `setup` at that step, with the
screenshot and tree kept. A parameter used at or before its `after` step is refused by `just e2e check <suite>`.

The delete-row case: 2551Q's Schedule 1 is the collapsed part 3 (`fp-part-3-toggle`), and its Add row can be culled
until that part is expanded or scrolled to. If `delete-row.wants` first takes a step that makes the table visible (that
step must not use `$table`) and only then clicks `$table-add`, the entry's spec becomes:

```json
{"id_regex": "^(fp-table-.+)-add$", "group": 1, "after": "s1"}
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
- every param it uses is supplied;
- a param resolved `after` a step is not used at or before that step;
- in a JSON recipe, the cut after that step does not separate a step from a step it `needs`;
- send steps are marked;
- no secret box is typed into (apart from the one mock-fixture exception above);
- `key_delay_ms` appears only on `type`, and only in entries with `needs_cli`;
- then `gpui-agent recipe validate` on each file the installed client can read.

## The smoke suite (`agent-recipes/e2e/smoke.json`)

On a fresh database, in this order (the three recipes that open sheets run last: on a phone a sheet that cannot be
closed swallows every later press). The `queue` and `queue-send` recipes need a fresh database. Files are relative to
`agent-recipes/e2e/`; "desktop" is macOS and Windows unless a Windows file is named.

| # | Recipe | What it proves | Desktop | Linux (narrow) | iOS / Android |
| --- | --- | --- | --- | --- | --- |
| 1 | `setup-start` | first-run setup opens; "Just me" | `setup-start.wants` | same | same |
| 2 | `sign-up` | the first taxpayer: TIN, name, address, the ZIP and RDO pickers | `sign-up.wants` | `sign-up-linux.wants` | `sign-up-phone.json` / `sign-up-android.wants` (no pickers: gap 1) |
| 3 | `tax-year` | the year and 2551Q in Edit forms (New return is refused without them) | `tax-year.wants` | `tax-year-narrow.wants` | `tax-year.wants` / `tax-year-narrow.wants` |
| 4 | `navigate` | every screen | `../ui/ui-navigate-app.wants` | `navigate-phone.wants` | `navigate-phone.wants` |
| 5 | `open-2551q` | New return > 2551Q | `open-2551q.wants`; Windows `open-2551q-windows.wants` (also the facts 2551Q validates) | `open-2551q-phone.wants` | `open-2551q-phone.wants` |
| 6 | `money-boxes` | Amended Yes, Item 16 typed and read back; Item 15 typed, read back, formatted when left, unchanged when focused again; Amended No | `money-boxes-paced.json`, fallback `money-boxes.wants` | `money-boxes-phone.json` | `money-boxes-paced-phone.json` (awaits the software keyboard), fallback `money-boxes-phone.json` |
| 7 | `queue` | Queue in the dry-run test build (send) | `queue.wants`; Windows `queue-windows.wants` (answers Part I) | `queue.wants` | `queue.wants` |
| 8 | `delete-row` | a schedule row added, filled and deleted | `delete-row.wants` | same | same |
| 9 | `birth-date` | the birth date, typed in the date box / the date sheet, read back, saved | `birth-date.wants` | `birth-date-phone.json` | `birth-date-phone.json` |
| 10 | `sign-out` | Log out; the lock screen | `sign-out.wants` | `sign-out-phone.wants` | `sign-out-phone.wants` |

Why the recipes look as they do (each from a machine's run on the real app, `proof/e2e-fixes-*/README.md`):
- After Create the app is on the taxpayer's own page (Forms tab, Add year band open), not Home. Home tiles are
  `home-tp-<9-digit TIN root>` (`$tin_root`) and are not pressable, so the narrow recipes reach the taxpayer through
  the header's Search (`nb-search`; the narrow palette searches tax profiles only, never forms).
- A fresh taxpayer has no tax year: `tax-year` adds one and ticks 2551Q. On Android the year picker is a sheet, so the
  narrow file takes the year the band offers.
- The form is an accordion on every platform: Items 15 and 16 are not in the tree until Part II
  (`fp-part-2-toggle`) is expanded; the JSON money-box recipes scroll `form-scroll` first (the floating pill can cover
  a toggle). The money boxes type the amounts as written (`500.25`): since #403 a money box keeps its text while
  focused and formats only on leaving, so the box reads `$amount` while focused and `$amount_shown` after it.
- On the macOS strict adapter a bare `type` does not move focus out of the segmented TIN control: the desktop
  recipes click a box before typing into it (sign-up's name, the birth date, delete-row's amount).
- 2551Q's Schedule 1 pre-holds row 1, and Delete row is disabled on an empty row: delete-row fills the new row's
  amount, deletes it and checks the row-2 line is gone (`$table-none` never shows).
- Log out on a phone is `nb-menu-logout` in the header's account menu (`nsh/shell/Narrow.tsx`).
- On Android every press inside a bottom sheet is refused (gap 1), so Android's sign-up leaves the ZIP and RDO empty
  (valid) instead of failing the required recipe; iOS and every desktop keep the pickers.

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
  step, but delete-row still needs a leading step that brings Schedule 1 into the tree before `$table` can be found
  below the fold.
- **macOS focus** (hexuria/nsh-gpui-agent#14): bare `type` does not move focus out of the segmented TIN control;
  focus flags read false everywhere; every step reports `delivered_unverified`, landed or not.
- **Money boxes under injected text** (hexuria/nsh-gpui-agent#15): paced `500.25` lands as `0.25` on macOS and as
  scrambled digits on iOS; the same recipe passed on Windows. Re-run on main (#403) before acting.
- **The software keyboard in the tree** (hexuria/nsh-gpui-agent#11): until it ships, phones run the money-box fallback,
  which is not a person's keyed input (labelled `known gap` on phones).

In the app and this repo:
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
