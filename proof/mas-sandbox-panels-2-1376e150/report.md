# A3d report — 1376e150, 2026-11-27/28

## Entitlements (entitlements.txt)
app-sandbox, files.user-selected.read-write, network.client, network.server, print — all true; identity keys deleted; ad hoc.

## Per step
- 2551Q: **P** — smoke chain setup-start/sign-up/tax-year/navigate/open-2551q 5/5 pass in-sandbox.
- preview: **P** (pictures/print-preview.png).
- PDF saved: **P** — print panel driven by cua-driver only (PDF menu ▸ Save as PDF…; save panel filename set, Save pressed; Where = Documents as offered). **~/Documents/a3d-2551q.pdf — 110445 bytes, %PDF-1.3, 2 pages**; the app/panel count is 2 — match.
- import scan: **P** — open panel driven by cua-driver only (go-to sheet: foreground cmd+shift+G through the parent window, clipboard paste, Return; folder sel:true; Scan). Review shows **"~/Desktop/a3d-legacy-ebirforms — Taxpayers 1, Filed returns 3, Saved drafts (not imported) 1"** (+1 not-read garbage.xml, fixture noise) — pictures/import-scan.png. **Import NOT pressed.**

## Denials — real run
`log show --start "2026-11-27 19:32:00" --predicate 'sender == "Sandbox"' --style compact` filtered to the app's process name eBIRForms: **ZERO lines** over the entire run (launch 19:32:58 through the import scan ~19:41).

## Positive control — the probe (this is the job's key question)
Probe: 3-line C program opening ~/Desktop/a3d-legacy-ebirforms/profile/123456788000.xml, compiled with clang, in a bundle named eBIRForms.app with the process name eBIRForms, ad hoc signed with ONLY com.apple.security.app-sandbox=true. Unsigned build reads the file fine (exit 0); the sandboxed probe prints **"open: Operation not permitted", exit 1** — the deny is real and attributed to the sandbox, not the file. secinitd logs "eBIRForms[pid]: AppSandbox request successful".

**But no `Sandbox: eBIRForms … deny` line exists for it.** Not in `log stream` (capturing live, non-root, while the probe ran), not in `log show --last` with the job's predicate, not with the subsystem predicate, for either the file read or a CFPreferences write probe. Meanwhile the SAME filter shows system-process denies in the same minutes (biomesyncd user-preference-read, WeatherWidget file-read-data, parsecd mach-lookup). Repeatable across A3c and A3d. Conclusion on this macOS build: **denies of containerized user apps do not reach the kernel violation stream that `sender == "Sandbox"` surfaces — only system daemons' denies do.** Practical meaning: the real run's zero lines are consistent with "no denials" but the filter CANNOT prove a negative for file-access denies of app-sandboxed processes; the strong evidence is instead positive — the PDF was written through powerbox and the fixture was scanned, which requires files.user-selected.read-write to be working exactly as entitled, and the app is fully functional inside the sandbox. Probe deleted after.

## Recording disclosure
Screen recorder used (started before the print step, stopped after the scan): stalled again — a3d-run.mkv is the concatenated raw capture, **80 seconds**, covering print preview + print panel opening; it does not reach the save or import panels. Fallback per the job: the screenshots at each step (print-preview, save-panel mid-drive, import-scan result) plus the PDF artifact itself. Frame count equivalents: 3 step screenshots + the 80 s capture.
