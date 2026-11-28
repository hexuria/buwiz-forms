driver: gpui-agent-cli 0.5.0
- evidence/49bf61aa-macos-state-left.png — 49bf61aa5b3ea240262d0ef90d2d40736d6cd6b0 — macOS BEFORE step “leave Juana state”: form page, Filings filter, and profile panel state left before account switch.
- evidence/49bf61aa-macos-after-switch-stale.png — 49bf61aa5b3ea240262d0ef90d2d40736d6cd6b0 — macOS BEFORE step “after account switch”: Juana's profile/panel state remains visible; the following absence assertion fails as the bug proof.
- evidence/af85bd19-macos-state-left.png — af85bd1964564693d1c96417892fab62baca8d89 — macOS AFTER step “leave Juana state”: the same positive-control state is visible before account switch.
- evidence/af85bd19-macos-after-switch-fresh.png — af85bd1964564693d1c96417892fab62baca8d89 — macOS AFTER step “after account switch”: fresh Home frame; form page, Filings search, and profile panel are absent.
- evidence/af85bd19-macos-full-recipe-final.png — af85bd1964564693d1c96417892fab62baca8d89 — macOS AFTER full-sign-in recipe final step: Pedro's fresh frame after typing both one-time codes through the generated inner-input ids.
- evidence/49bf61aa-iphone-state-left.png — 49bf61aa5b3ea240262d0ef90d2d40736d6cd6b0 — iPhone BEFORE step “leave Juana state”: form page and Filings search state left before account switch.
- evidence/49bf61aa-iphone-after-switch-stale.png — 49bf61aa5b3ea240262d0ef90d2d40736d6cd6b0 — iPhone BEFORE step “after account switch”: Filings state remains visible; the following absence assertion fails as the bug proof.
- evidence/af85bd19-iphone-state-left.png — af85bd1964564693d1c96417892fab62baca8d89 — iPhone AFTER step “leave Juana state”: the same positive-control state is visible before account switch.
- evidence/af85bd19-iphone-after-switch-fresh.png — af85bd1964564693d1c96417892fab62baca8d89 — iPhone AFTER step “after account switch”: fresh Home frame; form page and Filings search are absent.

Recording: evidence/af85bd19-macos-desktop-after-proof.mp4 — af85bd1964564693d1c96417892fab62baca8d89 — one macOS desktop AFTER recording of the full-sign-in recipe.

known_gap: gpui-agent recipes cannot type a composite input through its stable wrapper test id; the inner OTP textbox has only generated ids (`n3`, `n237`, `n261`). Filed as hexuria/gpui-agent#135. The full AFTER desktop recipe used the generated ids observed in that deterministic run; apples-to-apples BEFORE/AFTER recipes used the fixture's stable test-only `demo-switch-account` seam for the second account switch, leaving the final absence assertions unchanged.

known_gap: BEFORE evidence is the try/a3a production `nsh/shell/app.ts` at 49bf61aa5b3ea240262d0ef90d2d40736d6cd6b0 overlaid onto the A4 proof fixture, because the proof fixture did not exist on try/a3a. The overlay was temporary, restored immediately, and the final worktree was clean. This isolates the production behavior difference being proved.

Phone adaptation: the iPhone UI has no desktop `context-pill`/profile-panel surface, so phone proof leaves form + Filings-search state and asserts `form-page` and `filings-search` absent. A debug fixture readout overlays the top of iPhone screenshots, but the BEFORE Filings state and AFTER Home state remain visible.

Safety: scratch databases only; `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`; never `BIR_SFTP_LIVE`; no Queue/Send/Submit; UI input only through gpui-agent-cli 0.5.0; no OS input.

Receipts and exact stdout/stderr/exit codes: receipts/. Runtime recipes: recipes/. Full report: report.md. Capture index: CAPTURES.tsv.
