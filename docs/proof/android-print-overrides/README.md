# Android print overrides proof - PR #333, fair pair 46a0f44e (before) vs bf9fd1d3 (after)

Phone emulator API 35, dev builds with the allowlisted env (EBIR_TEST_ENV=1 BUWIZ_NO_KEYCHAIN=1 BIR_SFTP_DRY_RUN=1 BIR_AUTH_BYPASS=1), made-up taxpayer MariaClara 123-456-789, long registered address (137 chars), form 0605 print preview. NOTE: the line-of-business cell stayed empty in both runs - see report.

## Mechanical proof (the document the app made)
- `before-sheet.html` / `after-sheet.html` - the prepared print documents pulled from the app's private tmp (adb root).
  - BEFORE: `grep -c 'id="form-override"'` = 0 (no injection)
  - AFTER: `grep -c 'id="form-override"'` = 1, `data-slug="0605-1999"`, rules `#p1c33-i{font-size:6.1px !important}` and `#p1c38-i{font-size:6.7px !important}`
  - `id="p1c33-i"` and `id="p1c38-i"` exist once in BOTH documents - the injected rules do match real elements.
  - p1c38 holds the full 137-char address in both; p1c33 is empty in both.

## Pictures and recordings
- `before-0605-main.png` / `before-0605-parent.png` - print preview on the parent build (large font, clipped at the cell edge)
- `after-0605-print-fix.png` - print preview on the fix build (visibly smaller font in the address cell)
- `before-0605-main.mp4` / `after-0605-print-fix.mp4` - opening the print preview in each build
- `before-0605-main-setup.mp4` - earlier partial run (kept for completeness)
