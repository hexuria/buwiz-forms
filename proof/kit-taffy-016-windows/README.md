# Taffy 0.15 → 0.16 on Windows: before/after layout comparison

The kit moved its layout engine from Taffy 0.15 to 0.16 (hexuria/nsh#62); every screen's layout runs
through it. This evidence answers, on Windows, in the real app window: does any screen look
different after the move, and what does a user get in time.

**Answer: no screen differs.** Of 22 screenshot pairs, 20 are pixel-identical and the 2 remaining
differ only by a toast's fade timing (deterministic per run, unrelated to layout — see below).
Timing: opening the 2551Q form is consistently ~5 % faster after the move; app start and scrolling
are unchanged.

## The two states

- BEFORE: buwiz-forms `main` at `b983b2a4eff06b0c6203fd5829d62f5128db8639`, kit at the pin that
  commit names: `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf` (taffy 0.15).
- AFTER: buwiz-forms `try/kit-taffy-016` at `8f825621dc697f13bea69e4f83a46711d59a42c5`, kit pin
  moved to `bc417406b63479bf137b8327374265a62a2c0f6a` (taffy 0.16 — `taffy = "0.15"` → `"0.16"` in
  the kit's Cargo.toml). The branch is main plus the pin move, the lock resolved, and CI reference
  updates; no app code changes. 18 kit commits sit between the two pins.

## Method

Everything is driven through the project's test driver: gpui-agent recipes run by the e2e runner
(`python scripts/e2e windows <suite>`, which `just e2e` wraps), gpui-agent-cli 0.5.0 — the version
the repository pins (`GPUI_AGENT_CLI_VERSION` in scripts/impeccable.pin). No fixed-coordinate
clicks, no OS keystroke injection anywhere. Screenshots are the kit agent's own window captures:
real 1440×900 PNGs. The window size is the runner's default, identical in both states; display
scale 100 %. Every run uses a fresh scratch database, the dry-run test environment, the demo
taxpayer only. Five runs per state; run1 of each state is the picture set, all five feed the timing.

Method check: across independent runs of one state, 44 of 46 screenshots are pixel-identical — so a
real before/after difference of even one pixel is attributable to the kit change, not noise.

## The tour

`agent-recipes/e2e/tour.json` (7 recipes, `just e2e check tour` clean: 15 files validated) walks,
taking a picture at each:

- the first-run set-up card (as far as the test mode shows sign-in/set-up: authentication is
  bypassed in it, so the set-up card is what exists);
- sign-up with the ZIP and RDO picker sheets open (the picker-sheet coverage);
- Home with the sidebar open; the Library card grid; the Calendar screen; Settings at the top and
  scrolled to its end (at 1440×900 the General page fits without scrolling — the end IS the top, a
  layout fact in itself); the New return sheet; the command Palette;
- the 2551Q form at the top; in the middle with the part title pinned; after eight fixed scroll
  steps with Schedule 1 expanded — see the honesty note below; the form's navigation list and the
  sections popover;
- the print preview at the top and scrolled — see the gap note below.

Coverage gaps, honestly:
1. A Schedule table *with three rows*: the tour shows Schedule 1 expanded with its one default row.
   Filling three rows needs data entry the tour does not do.
2. The form's very end: the eight fixed scrolls (12,800 px of wheel events) reach deep into the
   expanded form but the screenshots show the Schedule 1 area, not provably the last pixel of the
   scroll. Both states sit at the SAME deterministic scroll depth, so the pair still compares
   like with like.
3. THE PRINT PREVIEW WINDOW: the in-app screenshot captures the window the agent drives — the main
   window. The print preview is a separate native window and is not in any capture (the pairs
   `tour-preview-*` show the main window with the "Print preview opened" toast, identical in both
   states, but not the preview itself). Comparing the preview's centring and end padding needs
   either a window-targeted capture or an OS-level observe-only capture, plus the BEFORE binary —
   which was cleaned to make disk room for the AFTER build. This is the one headline case this
   evidence does not answer; a focused follow-up (rebuild BEFORE, capture both previews) closes it.

## Display scale

125 % and 150 % could not be run. The kit's Windows crate reads the scale ONLY from
`GetDpiForWindow` — `crates/nsh-platform-windows/src/window.rs:823-827` (`dpi()`), passed to the
surface as `dpi(hwnd) as f32 / 96.0` in `sync_size`; there is no environment or test override
anywhere in the crate (searched both pins). The only source of a different scale is the OS display
setting, which on Windows Server 2022 requires signing out — and signing out ends the machine
session that produces this evidence. Nothing was resized to fake a scale.

## The comparison

`compare/` pairs every screenshot of BEFORE run1 with its AFTER run1 twin (same recipe, same step):
22 pairs. Per pair: same size, differing-pixel count (any channel delta > 8) and bounding box, a
side-by-side PNG for the differing ones.

| pair | result |
|---|---|
| 20 of 22 pairs | pixel-identical |
| tax-year-015 | a 17-px strip at the bottom (y 810–827): the same strip is non-deterministic across same-state runs — a fading toast, not layout |
| tour-form-008 | the toast "Income-tax rate saved for 2026." visible at AFTER, already faded at BEFORE — the toast fades on wall-clock time and step pacing differs slightly; underneath it the layout is pixel-identical |

The reviewer's expected-to-matter cases, one by one: extra empty space at the END of scroll areas —
no difference anywhere the tour reaches (settings end, library, form at fixed depth). Sideways
wobble — none: identical x positions in every pair. Pinned part titles — identical (tour-form-008/010
show the pinned "Part I · Background Information" header at the same offset). Grids — the Library
card grid and the calendar screen are pixel-identical. Sheets reaching their last row — the
picker sheets of sign-up identical. The print preview's centring and end padding — NOT answered,
see gap 3.

## Timing

See TIMING.md. Headline: opening the 2551Q form is ~490 ms (5 %) faster at AFTER, consistently and
outside the noise; cold start, home-visible and scrolling are unchanged.

## Smoke

See SMOKE.md. `just e2e windows smoke --allow-send` at both states: 10 pass + the same one
pre-existing known gap at each. Nothing fails at AFTER.

## Layout of this evidence

- `before/run1`, `after/run1` — the full runner evidence of the picture runs (screenshots,
  receipts, run.log). `before/run2..5`, `after/run2..5` — receipts and run.log of the timing runs.
- `compare/` — pairs.tsv, results.json, side-by-side PNGs of the differing pairs.
- `TIMING.md`, `SMOKE.md` — the tables.
- the tour itself: `agent-recipes/e2e/tour.json` + `tour-screens.wants`, `tour-form.wants`,
  `tour-preview.wants` at the repository root's agent-recipes, part of this commit.

## What this does not show

A real dialog or printout a person produced; any scale but 100 %; the print preview window's own
pixels (gap 3); a schedule table with three rows; the pixel-exact end of the form's scroll (gap 2 —
same depth both states).

## The three gaps closed: the print preview window itself, the schedule table with three rows, the form's very end

Same two states, same machine, same window sizes. BEFORE: app b983b2a4 + kit 8c78cfd6 (taffy 0.15); AFTER: app 8f825621 + kit bc417406 (taffy 0.16).

### How the pictures were taken

- **The print preview is a second native window and gpui-agent cannot capture it**: `screenshot --target print-sheet` captures the main window, `--mode scrolled` is `screenshot_unavailable` on Windows, and CLI 0.5.0 has no window subcommand reaching the kit's `window_screenshot`. So the preview pictures are **observe-only operating-system captures** of the preview window's screen region (no clicks, no keys through the OS — pixels only), window 956×1139 at the same position in both states. Scrolling inside the preview was driven by gpui-agent (`scroll print-sheet`), and the end capture is provably the end: two consecutive captures after further scrolls are pixel-identical.
- **The schedule table** (in-driver screenshot, wide 1440×900): three rows in both states — the default row plus two rows added through the app's own drawer (add, fill amount 1,000.00 / 2,000.00, done).
- **The form's very end** (in-driver screenshot): `fp-pager-clearance` exists **only in the narrow layout** (`<If when={narrow()}>`, FormPage.tsx — narrow is width under 600 points or touch), so the driver's `viewport` op resized the content area to 500×800 points for this capture in both states. The end is proven: the clearance node's screen position does not move across an extra 1,600-point scroll.

### Comparison table (differing pixels = any channel delta over 8)

| picture | size | result |
|---|---|---|
| print preview, top | 956×1139 | identical outside the masks below |
| print preview, end | 956×1139 | identical outside the masks below |
| schedule table, three rows | 1440×900 | identical outside the masks below |
| form end, narrow 500×800 | 500×800 | pixel-identical, every pixel |

The masks, and why they are not layout: the only differing pixels anywhere were (a) **hover-state fills** — exact theme hover color `#ebebee` on the control the mouse happened to rest over in one state and not the other (the preview's *Save as PDF* button, the preview's zoom control, the expanded *Return details* part-toggle row; proven by hovering the same row at BEFORE, which turns it exactly `#ebebee`), (b) the **OS-drawn window frame edges** of the preview (x<10, x>945 — shadow pixels vary with focus), and (c) the rows of the preview capture beyond the screen's bottom edge or under the taskbar (the window is taller than the screen; same geometry both sides). One apparent content difference — the preview end showing the filled amounts 1,000/2,000 at AFTER but zeros at BEFORE — was a **data-state artifact of capture order**, not a layout difference: the preview updates live, and the BEFORE end capture was first taken before its rows existed; recaptured with the same data present, the pair is pixel-identical outside the masks (`zoom-preview-end-rows.png` shows the before/after of that artifact).

### Measured geometry (identical in both states)

| measurement | BEFORE | AFTER |
|---|---|---|
| preview sheet centring: left margin / sheet width / window width | 24 px / 932 px / 956 px | 24 px / 932 px / 956 px |
| end padding below the last content row of the preview end capture | identical (pixel-identical captures) | identical |
| `fp-pager-clearance` spacer height, narrow | 92 px (60 + 16 + 16) | 92 px |
| empty space between the last form content and the floating pager, narrow, at the end | 144 px | 144 px |
| slack below the clearance spacer to the viewport bottom | 128 px | 128 px |

**Verdict: nothing moved.** The preview's centring, end padding and scroll range, the schedule table with three rows, and the form's very end are the same before and after the taffy 0.16 move; every difference found was a hover or capture artifact and is proven as such above.
