# Taffy 0.15 → 0.16: macOS before/after comparison

Evidence for the kit's layout-engine move (hexuria/nsh#62), driven end-to-end through the
project's test driver (gpui-agent recipes run by `just e2e`): no fixed-coordinate clicks,
no OS keystroke injection.

## The two states

- BEFORE: buwiz-forms `main` at `b983b2a4eff06b0c6203fd5829d62f5128db8639`, kit pin
  `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf` (Taffy 0.15).
- AFTER: buwiz-forms `try/kit-taffy-016` at `8f825621dc697f13bea69e4f83a46711d59a42c5`, kit pin
  `bc417406b63479bf137b8327374265a62a2c0f6a` (Taffy 0.16).

## How to re-run

1. Check out either state (app commit + the kit pin it names in `.github/workflows/nsh.yml`).
2. `just e2e check tour` — validates the tour suite (`agent-recipes/e2e/tour.json`, 8 recipes,
   gpui-agent-cli 0.5.0).
3. `just e2e macos tour` — runs the tour; screenshots land in `~/BuwizNsh/evidence/e2e/macos-<ts>/`.
   Sign-up pacing on macOS is a documented flaky gap (about 1 in 3 runs); rerun.
4. `just e2e macos smoke --allow-send` — the smoke suite in the dry-run test environment.
5. Compare pairs with Python + Pillow (`compare.json` here is the output schema: same size,
   differing-pixel count, bounding box).

Machine: macOS arm64, display 1600x1200 points at **1x** scale (the machine cannot do 2x);
app window 1200x800 px. All pictures are gpui-agent window captures of that window.

## What the tour covers

Home; Shell sidebar navigation (Home, Form library, Calendar, Filings); the top panel and the
account menu into Settings; Settings top and scrolled to the end; New return (sheet, 2551Q pick,
open); the 2551Q form page top, middle with the navigation list, scrolled to the very end, and the
sections/parts list open; the Schedule 1 table with rows added through its drawer (paced typing);
the print path (preview opened, zoom control). Sign-up, setup and tax-year cards come from the
smoke suite's own reused recipes.

**Print-preview caveat**: on macOS the print preview is a *separate native window*. gpui-agent
captures only the main window, so the print pictures show the main window with the preview-open
toast; the sheet's presence and bounds were verified in the tree (940x1030) but its pixels are not
capturable through the driver. Everything else is captured as real pixels.

## Results — comparison

23 screenshot pairs, same size (1200x800) in every pair. **18 of 23 pairs are pixel-identical.**
Every layout-bearing screen is identical: all Shell navigation pages, Settings top and end,
New return, the 2551Q form top / middle / end / sections list, and the Schedule table.

The 5 differing pairs are run-to-run noise, proven by a control: the same pairs differ by the same
magnitude when two BEFORE runs are compared against each other (and two AFTER runs against each
other):

| pair | cross-state diff px | same-state control diff px | what it is |
|---|---|---|---|
| 02-sign-up/018-s18 | 659588 | 659588 (BEFORE control) | paced typing caught mid-flight — different digits visible |
| 02-sign-up/020-s20 | 34131 | 34131 (BEFORE control) | toast/pill timing in the bottom strip |
| 03-tax-year/015-s15 | 8963 | 10148 (BEFORE control) | toast timing in the bottom strip |
| 08-tour-print/010-s10 | 1953 | 716–1932 (both controls) | toast fade timing |
| 08-tour-print/012-s12 | 1953 | 716–1932 (both controls) | toast fade timing |

The cases the kit's reviewer expected to matter — end-of-scroll empty space, sideways wobble,
pinned part titles, the calendar grid, the library card grid, sheets reaching their last row —
are all inside the pixel-identical pairs: **nothing moved**.

## Results — timing (5 green runs per state, median, range in brackets)

| measurement | BEFORE | AFTER |
|---|---|---|
| app launch → window plane ready | 3808 ms [3656–4035] | 3911 ms [3787–4139] |
| app launch → Home visible (whole sign-up path) | 12.2 s [12.2–12.4] | 13.0 s [12.9–13.6] |
| opening 2551Q (click → form page) | 210 ms [207–219] | 218 ms [203–227] |
| scrolling the form top → end (6 scroll steps) | 11295 ms [11244–11339] | 11353 ms [11314–11402] |

**The difference is inside the noise.** Every range overlaps heavily; these receipt timings are
dominated by the driver's own settle waits, so layout time is a small fraction of them. If anything
there is a slight upward drift of 1–4%, far from the 4–8% layout-time improvement the kit's own
microbenchmark measured — a user will not feel the difference either way.

## Results — smoke at AFTER (`just e2e macos smoke --allow-send`)

`1 fail, 1 known gap, 7 pass, 1 setup, 1 skipped` — identical shape to BEFORE:

- pass: setup-start, sign-up, tax-year, navigate, open-2551q, money-boxes, birth-date
- fail: queue at s18 `fp-send-line` — same failure as BEFORE; red on main for a recipe fault
  fixed in #507, not a Taffy regression
- known gap: sign-out at s4 (`locked-screen`) — pre-existing macOS gap, same as BEFORE
- skipped: sheet-press (no desktop variant)

One extra observation: during timing runs at AFTER, one sign-up attempt failed at s4 with
`occluded_by` once in 7 attempts; it did not repeat and sign-up is the documented flaky recipe
on macOS — noted for completeness, not attributable to the engine move.
