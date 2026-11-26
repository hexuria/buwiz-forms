# macOS BEFORE evidence for PR406

Captured 2026-11-26 before PR406.

- App source: `/Users/devin/searchpick-repro/buwiz-forms`, exactly `44ccfd8080f3028528077cd9a7fa64733846f309`.
- NSH source/pin: `/Users/devin/searchpick-repro/nsh`, exactly `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf` (matches workflow `NSH_PINNED_REF`).
- CLI: `/Users/devin/BuwizNsh/tools-main-8c78cfd/bin/nsh`, `nsh 0.6.1 (8c78cfd)`.
- Build target reused, not cleaned/rebuilt from scratch: `CARGO_TARGET_DIR=/Users/devin/BuwizNsh/target-44ccfd80`.
- macOS dev host was already being built in this target for this evidence run; build log is `run.log` / `profile-run.log`.
- Window: macOS development host, width requested/set about 980 pt, therefore wider than 600 pt and using the wide pre-PR406 popover/edit layout.
- Scratch DBs only: `before.db` for new taxpayer RDO, `profile.db` for the profile fixture.
- Safety env: `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`; `BIR_SFTP_LIVE`, Google client env unset; no Queue/real service.
- Input: real mouse clicks and real keyboard typing.
- New-taxpayer RDO evidence used production `App.tsx` on a fresh scratch DB. Birth-date profile evidence used the repository's `TaxpayerDemo.tsx` fixture in the same exact build to seed Juana Santos; this is fixture setup only, not input synthesis.

## RDO picker before PR406

- `rdo-before.mp4` — short annotated recording.
- `rdo-open-before.png` — old wide-layout RDO picker opens as a tall popover over the form, search at top.
- `rdo-search-before.png` — typing `05 makati` filters to `050 South Makati`; popover shrinks/repositions below the trigger.
- `rdo-picked-before.png` — row picked; trigger shows `050 - South Makati`.

## Birth date before PR406

- `birthdate-before.mp4` — short annotated recording.
- `birthdate-edit-before.png` — old Individual profile Taxpayer edit UI: Birth date is a plain `YYYY-MM-DD` text field; no calendar/date sheet.
- `birthdate-typed-before.png` — real typed value `1990-05-17` in the plain text field.
- `birthdate-saved-before.png` — saved profile row shows `May 17, 1990` with `Changes saved` toast.
