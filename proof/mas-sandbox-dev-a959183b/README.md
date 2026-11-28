driver: gpui-agent-cli 0.4.1

# A3b — print, save-PDF and the import folder picker inside the Mac App Store sandbox (dev build)

M = buwiz-forms main `a959183b9bfff8988fd778a725bf0eed35357794` (includes #501, the STORE_GMAIL switch).
nsh beside it at M's NSH_PINNED_REF `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`. gpui-agent-cli 0.4.1 (crates.io).

## What was done

The dev test build's macOS host (built by the e2e flow, agent plug at the app's own pin b559e40d — this job changes
no plug and no recipe) was ad-hoc signed with the AGENT store entitlements and run inside the sandbox:

- Entitlements: `python3 scripts/store_gmail.py entitlements entitlements.plist /tmp/store.entitlements --agent`,
  then the two identity keys (com.apple.application-identifier, com.apple.developer.team-identifier) deleted with
  PlistBuddy, exactly as `_package-mac-appstore` does with no identity. Final keys, all true:
  com.apple.security.app-sandbox, com.apple.security.files.user-selected.read-write, com.apple.security.network.client,
  com.apple.security.network.server (kept by --agent for gpui-agent's loopback socket), com.apple.security.print.
- Signed: `codesign --force --options runtime --entitlements /tmp/store.entitlements --sign - eBIRForms.app`
  (identifier dev.goldcoders.bir; verify --strict clean; `codesign -d --entitlements -` shows app-sandbox true).
- DEVIATION forced by the dev host's shape, documented honestly: the dev macOS host has no .app — `nsh run` launches
  the host binary in place and it loads its bundle from the nsh dev server (`--dev-server`). A bare binary dies at
  launch under the sandbox (no bundle containerization); the same binary inside a minimal hand-made .app wrapper
  (Info.plist + the binary, nothing else) runs sandboxed. It was launched with the same arguments `nsh run` passes
  (`--dev-server ws://127.0.0.1:8790/ws --name eBIRForms --app-id dev.goldcoders.bir -ApplePersistenceIgnoreState YES`,
  NSH_DEV_APP_DIR set), safety environment BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1,
  GOOGLE vars unset, NO external database path. The dev server (`nsh dev --server-only`, pinned CLI) ran OUTSIDE the
  sandbox, as it would on any developer machine; the app fetched its bundle over loopback (network.client).
- Container: ~/Library/Containers/dev.goldcoders.bir was deleted before the run (it existed from an earlier probe —
  said here as instructed). Proof the sandbox container is the scratch database: the running app's open files are
  `Data/tmp/ebirforms-door/<pid>-0/bir_data.db*` inside the container, its cwd is the container's Data dir, and
  gpui-agent's own screenshots land in the container's tmp (see pictures/ paths). In the test environment the
  database folder is per-process, so a relaunch starts fresh — expected, not a sandbox effect.
- gpui-agent hello inside the sandbox: ready **true** — the window comes up and the full semantic tree answers.

## Per-step results

| step | result | evidence |
|---|---|---|
| setup-start + sign-up (smoke, runner --no-launch) | PASS | report-recipes.md/json: 5 pass (setup-start, sign-up, tax-year, navigate, open-2551q) |
| open 2551Q | PASS | same; pictures/01-form-2551q.png |
| print preview (fp-print) | PASS | preview window opens in the sandbox; tree shows print-save-pdf / print-print; pictures/02, 04 |
| print to PDF through the print panel | known_gap | press print-print ok, preview shows "Preparing the PDF…" (the PDFKit print operation's panel is up); the panel is an AppKit window outside gpui-agent's control plane — no agent verb reaches it, and there is no error to quote: the press succeeds, the panel simply cannot be driven. No failure text in the tree (no "No printer is available", no "could not open"). |
| Save as PDF | known_gap | same shape: press print-save-pdf ok, "Preparing the PDF…" persists — the NSSavePanel (powerbox) is open and waiting; gpui-agent cannot drive it. The rust test-path hook (print.rs save_target) is not wired to any UI/agent input, so the write itself could not be exercised without touching the panel — no workaround used, per the rules. |
| Settings > Import from BIR eBIRForms folder picker | known_gap | press settings-ebir-choose ok; the button disables (waiting on the open panel); no error text. The NSOpenPanel is equally outside the control plane. pictures/05-import-screen.png |
| Settings > Account > Google row | PASS | tree shows "Google sign-in is not available in this version. Use an app password."; testID settings-google-connect ABSENT (ids present: settings-google, settings-google-gmail, settings-google-tracking); pictures/06-account-google-row.png |

One operational note: once a system panel is open it modal-blocks its window and gpui-agent cannot dismiss it
(no Cancel verb reaches an AppKit panel); the app was relaunched for the settings steps. The print/import presses
themselves never error — the panels open exactly as unsandboxed.

## Sandbox denials (the job's exact command)

`log show --last 30m --predicate 'sender == "Sandbox"' --style compact | grep -i ebir` — full output in denials.log
(18 lines over the whole run, widened to 90m for the final collection).

Every line is one of:
- `Sandbox: siriknowledged(733) deny(1) file-read-data /Users/devin/a3b-app/eBIRForms.app[...]` (2 deny lines, plus
  duplicate-report lines) — the SYSTEM's siriknowledged process denied reading the app bundle, not the app itself;
  unrelated to any entitlement of the app (Siri knowledge indexing every new app).
- `Sandbox: ContextStoreAgent(629) allow file-read-data ...` — allow, not deny, and again a system process.

**Denials caused by the app itself: ZERO.** No missing or wrong entitlement was hit by print, save-PDF panel,
the import open panel, the loopback dev-server fetch, the agent socket, or the container database.

## Step 5b — the STORE_GMAIL switch on the real packaging

See report.md (outputs of both runs, pasted in full). Both packages were deleted after the checks.

## Files

- recipes/ — the suite recipe copies used (setup-start, sign-up, tax-year, open-2551q; desktop navigate has no
  recipe file of its own — the runner composes it)
- report-recipes.md/json — the runner's own report for the five-recipe run inside the sandbox
- pictures/ — gpui-agent's own screenshots only
- denials.log — the sandbox-violation log grep
- report.md — the full report including 5b outputs
