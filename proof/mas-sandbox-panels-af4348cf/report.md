# A3c report — af4348cf, 2026-11-27/28

## Entitlements (codesign -d --entitlements -, full output in entitlements.txt)
com.apple.security.app-sandbox, com.apple.security.files.user-selected.read-write, com.apple.security.network.client, com.apple.security.network.server, com.apple.security.print — all true; no application-identifier/team-identifier keys; ad hoc signature.

## Per step
- open 2551Q: **P** — smoke chain setup-start, sign-up, tax-year, navigate, open-2551q = 5/5 pass inside the sandbox (plug override 3e78d303).
- print preview: **P** — fp-print opens the preview window ("BIR Form 2551Q · Q4 2026"); pictures/a3c-01-preview.png.
- PDF saved: **P** — print-print opened the AppKit print panel ("Page 1 of 2"). cua-driver only: pressed the PDF menu button, chose "Save as PDF…", set filename a3c-2551q.pdf in the save panel (Where = Documents, as the panel offered), pressed Save. The panel wrote **/Users/devin/Documents/a3c-2551q.pdf — 110445 bytes, valid %PDF-1.3, 2 pages** (python page count; mdls null only because indexing hadn't caught up). The app/panel page count is 2 — **they match**. Note the panel-mediated write lands in the real ~/Documents, not the container: that is exactly how powerbox + files.user-selected.read-write is designed to work, and no denial occurred.
- import scan shows the fixture: **P** — settings-ebir-choose pressed via gpui-agent; open panel driven with cua-driver only: cmd+shift+G go-to sheet (foreground keys through the panel's parent window), path pasted via clipboard_write + cmd+V (foreground type_text cannot focus sheet windows on this build — documented), Return navigated, folder a3c-legacy-ebirforms selected (sel:true), Scan pressed. The review screen shows **"/Users/devin/Desktop/a3c-legacy-ebirforms — Taxpayers 1, Filed returns 3, Saved drafts (not imported) 1"** + "Files not read 1 (garbage.xml — invalid return filename, expected fixture noise)" — pictures/import-scan.png. Exactly the fixture's shape. **Import itself NOT pressed** — the scratch database was never written.
- Google row: not re-verified in A3c (proven in A3b on the same build shape); out of this job's requested steps.

## cua-driver driving notes (all inside the panels only)
- Print panel: AXPress on the PDF menu button opened the menu; "Save as PDF…" menuitem pressed; the save panel's Save As field set via AX set_value; Save pressed — all accessibility actions.
- Open panel: NSBrowser folder cells implement no working AX action (AXPress/AXPick/AXConfirm all no-op or error) — the job's needed selection was achieved through the panel's own go-to-folder sheet instead. One deviation: keystrokes had to use cua-driver's foreground delivery because the background type path refuses sheet windows; no CGEvent/osascript/XCUITest was used outside cua-driver, and cua-driver never touched the app itself.
- One pixel-level click was attempted on the NSBrowser cell but never delivered (no screenshot frame); the final flow uses only AX actions + keyboard inside the panels.

## Sandbox denials — the whole run
`log show --start "2026-11-27 18:55:00" --predicate 'sender == "Sandbox"' --style compact` filtered to process eBIRForms: **ZERO lines**. During the same window the filter was live and noisy — 181/158/34/29 deny lines per 5-minute block for system processes (parsec, contactsd, logd, WeatherWidget…), so silence for eBIRForms is meaningful, not a dead filter. The unfiltered context noise is in denials-context-noise.txt (all "allow" or other-process lines).

## Positive control (filter validation) — PARTIAL, honestly reported
- /bin/cat copies cannot execute at all on this macOS (Killed: 9 even unsigned — platform-binary validation), so the control binary is a 3-line clang C program that fopen()s the target file, signed ad hoc with ONLY com.apple.security.app-sandbox=true.
- Bare-binary form dies in libsystem_secinit (Trace/BPT trap) — sandboxed non-bundled binaries can't initialize; inside a minimal .app wrapper it runs.
- **The control read FAILS as designed: exit 1, "open: Operation not permitted"** reading ~/Desktop/a3c-legacy-ebirforms/profile/123456788000.xml — the sandbox deny is functionally proven.
- BUT the kernel did NOT emit a `Sandbox: sbcat deny` line in the job's filter within 3 minutes, even though the same filter caught apsd violations at the same second. By that time (19:19+) system-wide violation volume had collapsed (1 deny in 5 minutes vs 29–181 earlier — the duplicate-suppression had engaged), and/or user-home file-read denies of this class don't surface in the violation stream. Consequence for reading this proof: the app's zero-denial evidence is strongest for 19:00–19:18 (the whole active run — panels included) when the pipeline is demonstrably flowing; the control confirms sandbox enforcement is real (Operation not permitted) but does not independently re-verify the log filter for this exact deny class. Control binaries deleted after.

## Video — CAPTURE GAP, honestly reported
a3c-run.mkv is 4 minutes (18:49–18:53), concatenated from the raw capture segments. The harness recorder stopped writing segments at 18:53 (post-processing then failed: "ffmpeg motion pass failed"), so the video covers only the run's setup phase — NOT the panel driving. The panel proof is instead: the PDF artifact itself (in this folder), pictures/import-scan.png (the scan review), pictures/open-panel-goto.png (the open panel mid-drive), and the zero-denial log evidence.
