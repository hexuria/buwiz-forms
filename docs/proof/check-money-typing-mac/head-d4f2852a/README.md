# check-money-typing-mac at d4f2852a (B1 fix head) — macOS real-key proof

App: hexuria/buwiz-forms fix/money-box-keep-text-on-focus at exactly
`d4f2852a28c323e3f6da85eae4f61d970d84ff0e` (rev-parse confirmed), fresh clone,
`CARGO_TARGET_DIR` inside the checkout, kit sibling at pin 8c78cfd6,
cargo 1.100.0 (`RUSTUP_TOOLCHAIN`), `RUSTC_WRAPPER` empty, the four safety vars
(`BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`),
never `BIR_SFTP_LIVE`, Queue never pressed. Demo taxpayer juana santos
(000-123-453), invented values. Real window, real hardware-key events.

## B1 — delete row with typing pending: PASS (7/7)

Driver `b1fix2.py` (transcript `caseB1.log`, data `caseB1.json`, per-run shots
`shot-b1-*.png`). Rows PT010 1,000 / PT040 2,000 / PT010 3,000 rebuilt before
each run; the amount box showed `500` uncommitted before every delete.

Every run of (a) mid row x3, (b) last row x3 and (c) the Tab-first control:
the page afterwards shows only the two remaining rows with their own amounts
(1,000.00 + 3,000.00 mid, 1,000.00 + 2,000.00 last); the draft's deleted slot
holds NO amount — every key: `atc ""`, `atc_description ""`,
`taxable_amount 0.0`, `tax_rate 0.0`, `tax_due 0.0`; the uncommitted 500 lands
nowhere; totals right (120.0 mid, 90.0 last). The slot remains in the draft as
a blanked row (the page hides blank rows) — no amount, no ATC.

## B2 — commit paths of a focused box: PASS

2550M item 13A holding 1,234.56; clicked in, Cmd+A, typed `1500,50`
(box verified `1500,50`), no Tab, no click elsewhere.

- PRINT: clicked the page's Print without leaving — `shot-b2-print3.png`:
  the preview's item 13A reads **1,500.50** (13B 180.06, 16A 1,500.50);
  the draft holds `sales_to_government` **1500.5** (`caseB2-print.json`).
- SAVE: this build has no manual Save button — only the automatic draft save
  (the "✓ Saved" text is a status indicator). The automatic save does not fire
  while the box keeps focus: after 8 idle seconds the draft still held
  1234.56 with `1500,50` in the box (`caseB2-wait.json`); it fires the instant
  the box is left — Tab away → draft **1500.5**.

## A — entry into an editable zero: every amount saved as typed — YES

`caseAB.json` / `caseAB.log`. 2551Q Schedule 1 row 1 amount.
click/Tab/Shift+Tab `15000.50` → 15,000.50 saved exact; `1000000.005` →
1,000,000.01 (half-up); bursts right; click with the first key at ~0 ms:
**wrong 0 of 10** (every run `15000.50` → saved 15000.5).
`abshot-A1ph-placeholder.png` / `abshot-A1ph-typing.png`: the untouched zero is
an empty box with the grey 0.00 placeholder, black digits while typing.

## B — filled 1,234.56

Tab then 7 → 7.00 saved exact; Tab, Right, 9 → `1,234.569` while typing,
1,234.57 saved; click between the 2 and the 3 then 9 → `1,2934.56` while
typing, **12,934.56** saved (frac 0.91 of the box width, measured by the scan
in `b3scan.json` — the first pass at frac 0.35 was a rig aim miss in the empty
left area and is not an app result); Cmd+A, Backspace, Tab away → empty box,
grey placeholder (`abshot-b4-after.png`), 0.0 in the draft, no sentence;
caret end, 4xBackspace → `1,23` → 123.00 saved. No sentence in any case.

## Rig disclosure

This round exposed a rig fault, not an app fault: the rig's Cmd+A chord events
left the Command modifier logically held at OS level, so a following comma
acted as Cmd+comma and opened the app's Settings page. Confirmed by isolation
(a lone comma after Cmd+A opened Settings; after a modifier reset it did not)
and fixed in the rig (the chord now presses and releases the physical Command
key). Every reported result is from clean runs after the fix.
