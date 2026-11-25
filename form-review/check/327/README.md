# Live check of pull request #327 at c7fd0dad

Independent verification of the row-drawer claim on the withholding returns, run
live in the NSH app on a Linux desktop (Ubuntu 24.04 sidecar image, X11 display),
demo taxpayer Juana Santos, invented values, scratch database, the four safety
variables set throughout. Nothing here was produced by the PR's author for this
check; the app was driven through its real window with real input.

- **After** = `c7fd0dada098a67e393f23f0e0833bf3b8f1835d` (the pull request's head;
  the branch had not moved when checked).
- **Before** = `6ddcac3fa7cd3aca7801da893e3225a998bd0141` (origin/main at the time).

## Layout

- `after/` — screenshots at the pull request's commit. `vt-*` is form 1600VT,
  `pt-*` 1600PT, `fq-*` 1601FQ, `s3-* 2553`. For each way out there is a
  `<way>-before.png` (the refused state in the open drawer before the attempt)
  and `<way>-after.png` (immediately after the attempt); `vt-<way>-reopen.png`
  is the table after closing and reopening the return. Ways out: `done`, `x`,
  `escape`, `outside` (click outside the drawer), `otherrow` (opening a
  different row), `partnav` (navigating to another part), `back` (the app's
  back control). `vt2-*` is the second refusal type (ATC picked while Item 10
  is unanswered); `vt3-*` is the non-refusal validation message (over-long
  payee name reported by Check). `vt-outside-preview-not-open.png` shows the
  state after pressing print preview following an outside-click escape: no
  preview opened.
- `before/` — the same file names at origin/main, 1600VT only, plus
  `vt-<way>-preview-not-open.png` where a print preview was attempted.
  File names match `after/` one-to-one so pairs can be compared directly.
- `recordings/` — one app-only video per form of the core flow: open the form,
  add a row, type a tax base with no tax code, the refusal shows, Done is
  unavailable, then delete the row and the page settles on "✓ Saved".
- `results/` — machine-readable matrices the driving harness wrote per scenario
  (what it observed after each action). `results.json` is the 1600VT ways-out
  matrix at the PR commit, `before-results.json` the same at main,
  `wi2-results.json`/`wi3-results.json` the two variant checks,
  `wi3-cleanrow-control.json` the clean-row control, `formways-results.json`
  the Done/X/Escape checks on 1600PT/1601FQ/2553.
- `test-results/summary.txt` — the pull request's own screen tests run at its
  commit: every `nsh/screens/form/*.test.tsx` file, pass/fail per file.

## What the evidence does NOT show — read before trusting anything here

1. **The videos are not literal desktop captures.** On this Linux desktop the
   NSH/GPUI window is invisible to X11 screen capture (compositing: a recorder
   sees only the wallpaper). The PNGs are genuine — they are captured from the
   app surface through gpui-agent's internal screenshot — and the MP4s are
   assembled from those same internal frames at 1 frame per second. They show
   the app window only, each under two minutes, but motion in them is stepwise,
   not continuous. Nothing outside the app window exists in any capture.
2. **X and Escape prove nothing about the refusal logic on this platform.**
   In a clean-row control (valid row, no message, page already saved), neither
   the X control nor Escape closed the drawer — on main and on the PR build
   alike. Their non-closure in the refused state therefore cannot be credited
   to the PR. On the maximized Linux window the drawn X sits at the extreme
   top-right adjacent to the native window controls, so hit reliability is
   also questionable. X/Escape rows in the matrix are platform data, not
   claim data.
3. **Print preview was attempted once per way out at most.** On the PR build a
   separate earlier attempt to close a preview window crashed the Linux host
   with an X `BadDrawable` error, so the matrix does not retry preview closing.
   "Preview did not open" entries mean exactly that: one press of the enabled
   print control, no preview window, no toast — not a crash.
4. **The 2553 refusal detection in `results/formways-results.json` is wording-bound.**
   The harness looked for 1600VT's sentence; 2553's actual refusal reads
   "Choose the ATC in 18B first." and is visible in the `s3-*.png` screenshots.
   Trust the screenshots over the JSON flags there.
5. **No Windows or macOS capture.** Everything is the Linux build. X/Escape and
   window-control behavior may differ on the platforms the app ships on.
6. The screenshots show the states before and after each attempt and the
   reopen state; they do not by themselves prove how long the save line took
   to settle — where the matrix says "did not settle" it means the line still
   read "Unsaved changes" after the harness's settle wait (tens of seconds),
   not that it would never settle.
