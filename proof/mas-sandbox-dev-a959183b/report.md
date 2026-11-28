# A3b report — print / save-PDF / import picker inside the Mac App Store sandbox (dev build)

M = `a959183b9bfff8988fd778a725bf0eed35357794` (main, includes #501). nsh at pin 8c78cfd6. gpui-agent-cli 0.4.1.
Driver for every UI step: gpui-agent only. Never pressed Queue. Nothing uploaded to Apple; ad-hoc signing only.

## Entitlements (what the dev app was signed with)

`/tmp/store.entitlements` produced by `python3 scripts/store_gmail.py entitlements entitlements.plist /tmp/store.entitlements --agent`
with the two identity keys deleted (PlistBuddy), then `codesign --force --options runtime --entitlements ... --sign -`:

- com.apple.security.app-sandbox — true
- com.apple.security.files.user-selected.read-write — true
- com.apple.security.network.client — true
- com.apple.security.network.server — true (kept by --agent for gpui-agent's loopback socket)
- com.apple.security.print — true

`codesign -d --entitlements -` shows app-sandbox true; verify --strict clean. The container
~/Library/Containers/dev.goldcoders.bir existed from an earlier probe and was deleted first (as instructed to say).
Proof of sandbox reality: the app's cwd is the container's Data dir; its database is
`Data/tmp/ebirforms-door/<pid>-0/bir_data.db` inside the container (test-env databases are per-process, so a
relaunch starts fresh — expected); gpui-agent screenshots land in the container's tmp.

Forced deviation, honestly recorded: the dev macOS host has no .app (nsh launches it in place, loading its bundle
from the dev server). A bare signed binary dies at launch under the sandbox; inside a minimal hand-made .app
wrapper (Info.plist + the binary) it runs sandboxed, launched with nsh run's own arguments. The dev server
(`nsh dev --server-only`, pinned CLI) ran outside the sandbox, as on any developer machine.

## Per step

| step | result |
|---|---|
| open 2551Q | **P** — runner --no-launch inside the sandbox: setup-start, sign-up, tax-year, navigate, open-2551q all pass (report-recipes.md/json) |
| print preview | **P** — fp-print opens the preview window; pictures/02, 04 |
| PDF save | **known_gap** — press print-save-pdf ok; "Preparing the PDF…" persists = the NSSavePanel is open and waiting. No error anywhere; the panel is an AppKit/powerbox window gpui-agent has no verb for. The rust test-path hook is not wired to any UI input, so the write can't be exercised without touching the panel; no workaround used. |
| print panel | **known_gap** — press print-print ok; same waiting state, no failure text (no "No printer is available"). The PDFKit print panel is equally outside the control plane. |
| import picker | **known_gap** — press settings-ebir-choose ok; the button disables waiting on the open panel; no error. Same panel class. |
| the Google row | **P** — tree shows "Google sign-in is not available in this version. Use an app password."; `settings-google-connect` ABSENT (present ids: settings-google, settings-google-gmail, settings-google-tracking); pictures/06 |

Operational note: an open system panel modal-blocks its window and cannot be dismissed through gpui-agent;
the app was relaunched for the settings steps. The presses themselves never error — panels open as unsandboxed.

## Sandbox denials

Exact command output in denials.log. **Zero denials caused by the app.** The only deny lines are
`siriknowledged` (a system process) denied reading the app bundle — Siri indexing noise, not an entitlement gap.
(ContextStoreAgent lines are `allow`, also a system process.) No missing or wrong entitlement was hit by print,
the save/open panels, the loopback dev-server fetch, the agent socket, or the container database.

## 5b — the STORE_GMAIL switch on the real packaging

(i) `BUILD_NUMBER=9999 just _package-mac-appstore`, GOOGLE vars unset — full log in 5b-i-no-gmail.log, entitlements in 5b-i-entitlements.xml:

```
Store build without Gmail Connect (STORE_GMAIL=1 to include it after Google's verification).
OK No Google client id or secret in the build: Gmail Connect is off.
```
Packaged-app entitlements: app-sandbox, files.user-selected.read-write, network.client, print —
**network.server NOT listed** ✓. EXIT:0. (Build stamp: app a959183b — before the proof commit.)

(ii) positive control, STORE_GMAIL=1 with the two made-up values — full log in 5b-ii-with-gmail.log, entitlements in 5b-ii-entitlements.xml:

```
Store build WITH Gmail Connect (STORE_GMAIL=1): the Google client id is compiled in.
com.apple.security.network.server: kept (Gmail Connect)
OK STORE_GMAIL=1: the Google client id and secret are in the build (Contents/MacOS/eBIRForms).
```
Packaged-app entitlements: app-sandbox, files.user-selected.read-write, network.client, **network.server LISTED**, print ✓.
EXIT:0. Never launched. Disclosure: this second build stamped app 50df78e6 because the proof commit existed by
then — proof files only, no code difference; the gmail-switch logic is identical.

Both packages were deleted after the checks; target/nsh-store (the build folder) was deleted too.
The made-up secret value never appears in either log (verified).
