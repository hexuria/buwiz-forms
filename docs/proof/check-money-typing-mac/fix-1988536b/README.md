# Money-box real-key check — fix/money-box-no-live-format @ 1988536b

Mac real-key rig: hexuria/buwiz-forms exactly `1988536bfd41b22ee77d6f1e39ef29a61d4bbc87` (confirm `git rev-parse HEAD`),
kit sibling ../nsh at pin `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf` (`nsh 0.6.1 (8c78cfd)`), cargo 1.100.0 (3acc79c0c,
the toolchain's own), RUSTC_WRAPPER empty, build folder `/Users/devin/moneyfix/target`. App run only with
BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1, scratch db, demo taxpayer, invented values,
never Queue. Keys are real OS events (CGEvent key typed, 120 ms cadence unless marked burst). Box text read live from
the a11y tree after every key; the SAVED value read from the scratch draft db (`form_drafts.data_json`,
`$.schedule_1[0].taxable_amount`) after Done each time. 2551Q Schedule 1 row 1 taxable amount.

## Verdict line

The saved value equals what the box shows after leaving in EVERY case, typing and clicks alike.
What was typed lands exactly in every end-typing case — EXCEPT typing after a keyboard Tab-in (case d),
which inserts the digits inside the existing number instead of replacing it (details below).

## (a)–(d), keys 15000.50

| case | keys land as (box states, per key) | after leaving | saved | right? |
|---|---|---|---|---|
| (a) box 0.00, click, type | 1 → 15 → 150 → 1500 → 15000 → 15000. → 15000.5 → 15000.50 (plain, no separators) | 15,000.50 | 15000.5 | right |
| (b) box 1,234.56, click, Cmd+A, type over | (selection) 1 → 15 → … → 15000.50 | 15,000.50 | 15000.5 | right |
| (c) box 1,234.56, click, Cmd+A, Delete, type | (empty) 1 → 15 → … → 15000.50 | 15,000.50 | 15000.5 | right |
| (d) box 1,234.56, fresh drawer, keyboard-only Tab-in, type | 1234.56 → 1234.516 → 1234.5156 → … → 1234.515000506 (digits insert before the last digit) | 1,234.52 | 1234.52 | shown==saved, but NOT what was typed |

Case (d) note: the Tab-in does not leave the text selected-whole for the typist; the caret sits before the final
digit and every key inserts there. The person sees the digits going in, and what the box shows is what is saved —
but it is not the typed 15000.50. Verified twice: once with keyboard-only focus (fresh drawer, 5 Tabs to reach the
box), once after click+shift-tab+tab; same insertion both times.

## From a box holding 0.00

| keys | box after typing | after leaving | saved | right? |
|---|---|---|---|---|
| 99999.99 @120ms | 99999.99 (plain) | 99,999.99 | 99999.99 | right |
| 1000000.005 @120ms | 1000000.005 (plain) | 1,000,000.01 (rounds half up on leaving, as expected) | 1000000.01 | right |
| 1234567890 then Left Left Backspace | … → 1234567890, after LL+BS 123456790 | 123,456,790.00 | 123456790.0 | right |
| 99999999.99 run1 @120ms | 99999999.99 | 99,999,999.99 | 99999999.99 | right |
| 99999999.99 run2 @120ms | 99999999.99 | 99,999,999.99 | 99999999.99 | right |
| 99999999.99 run3 @120ms | 99999999.99 | 99,999,999.99 | 99999999.99 | right |
| 15000.50 as one burst (~8ms/key) | 15000.50 | 15,000.50 | 15000.5 | right |
| paste of 15000.50 | 15000.50 | 15,000.50 | 15000.5 | right |

## Clicks on a box holding 1,234.56, then typing 9

| click | where the digit lands | after leaving | saved | shown==saved? |
|---|---|---|---|---|
| between the 2 and the 3 (pixel-measured on the glyphs) | AT THE FRONT: 91234.56 | 91,234.56 | 91234.56 | yes |
| in the empty left part of the box | AT THE FRONT: 91234.56 | 91,234.56 | 91234.56 | yes |

A person would see the digit appear at the front of the number. Measurement detail: the first click on an unfocused
box puts the caret at position 0 no matter where in the box it lands (verified: clicks directly on digit 5, between
4 and the dot, and just left of the 1 all behave identically — 9 lands at the front each time). A SECOND click while
the box already has the keys lands where clicked (9 typed after clicking digit 5 gave 12394.56). So "the platform
puts the caret where clicked" holds only from the second click; the focusing click always lands at the front.

## No-regression

- TIN box: the 2551Q form's TIN is locked display from the tax profile (not typable). The editable TIN cell control
  was exercised with real keys during this rig's own setup: New taxpayer TIN cells typed 000123453 key by key, and the
  created profile shows exactly 000-123-453-00000. Shown and saved as typed.
- Date box (Business start date ask-field): typed 01152000 → box shows 01/15/2000 live and January 15, 2000 after
  leaving — exactly as typed. Honest gap: the write of this profile-ask field did not appear in the profile row
  within the rig's timebox (null); whether that is a commit-path the rig did not trigger (no error showed) or a real
  miss is open. The money fix's own shared code path is the live-typing path, which is exact.

## Recording

`fix-money-rig.mp4` — the app's own window only. Run 1: case (a), keys `1 5 0 0 0 . 5 0` at 120 ms per key into the
0.00 box: plain digits appear as typed, on leaving it groups to 15,000.50, draft saves 15000.5. Run 2: keys
`9 9 9 9 9 9 9 9 . 9 9` at 120 ms: box shows 99999999.99 plain, leaving shows 99,999,999.99, saved 99999999.99.

Transcripts: transcript-abcd.txt, transcript-main.txt, transcript-clicks.txt (timestamps, per-key box states, verdicts).
