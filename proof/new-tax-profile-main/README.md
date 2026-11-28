driver: gpui-agent-cli 0.5.0
- evidence/8f38ceda-macos-sidepanel-entry-visible.png — 8f38cedacd21d05708e7c6e576421965e224517e — macOS side panel, owner with one tax profile: New tax profile entry visible.
- evidence/8f38ceda-macos-sidepanel-opened-new-profile.png — 8f38cedacd21d05708e7c6e576421965e224517e — macOS side panel: pressing the entry opened the New tax profile page.
- evidence/8f38ceda-macos-picker-addrow-visible.png — 8f38cedacd21d05708e7c6e576421965e224517e — macOS narrow picker, owner with one tax profile: add row visible.
- evidence/8f38ceda-macos-picker-opened-new-profile.png — 8f38cedacd21d05708e7c6e576421965e224517e — macOS narrow picker: pressing the add row opened the New tax profile page.
- evidence/8f38ceda-macos-palette-row-visible.png — 8f38cedacd21d05708e7c6e576421965e224517e — macOS command palette, owner with one tax profile: New tax profile row visible.
- evidence/8f38ceda-macos-palette-opened-new-profile.png — 8f38cedacd21d05708e7c6e576421965e224517e — macOS command palette: pressing the row opened the New tax profile page.
- evidence/8f38ceda-ios-picker-addrow-visible.png — 8f38cedacd21d05708e7c6e576421965e224517e — iPhone picker, owner with one tax profile: add row visible.
- evidence/8f38ceda-ios-picker-opened-new-profile.png — 8f38cedacd21d05708e7c6e576421965e224517e — iPhone picker: pressing the add row opened the New tax profile page.
- evidence/8f38ceda-ios-homeempty-new-profile-button.png — 8f38cedacd21d05708e7c6e576421965e224517e — iPhone Home empty state: New tax profile button visible.
- evidence/8f38ceda-ios-homeempty-opened-new-profile.png — 8f38cedacd21d05708e7c6e576421965e224517e — iPhone Home empty state: pressing the button opened the New tax profile page.
- evidence/af4348cf-macos-sidepanel-entry-absent.png — af4348cfbaf95ac00aeec84b274359706a5d7ba8 — macOS side panel, owner with one tax profile: no add entry.
- evidence/af4348cf-macos-picker-addrow-absent.png — af4348cfbaf95ac00aeec84b274359706a5d7ba8 — macOS narrow picker, owner with one tax profile: no add row.
- evidence/af4348cf-macos-palette-row-absent.png — af4348cfbaf95ac00aeec84b274359706a5d7ba8 — macOS command palette, owner with one tax profile: New tax profile query has no matching row.
- evidence/af4348cf-ios-picker-addrow-absent.png — af4348cfbaf95ac00aeec84b274359706a5d7ba8 — iPhone picker, owner with one tax profile: no add row.
- evidence/af4348cf-ios-homeempty-add-first-taxpayer-button.png — af4348cfbaf95ac00aeec84b274359706a5d7ba8 — iPhone Home empty state: old Add your first taxpayer button visible.
- evidence/af4348cf-ios-homeempty-opened-new-taxpayer.png — af4348cfbaf95ac00aeec84b274359706a5d7ba8 — iPhone Home empty state: pressing the button opened the old New taxpayer page.

Recording: evidence/8f38ceda-macos-desktop-entry-proof.mp4 — 8f38cedacd21d05708e7c6e576421965e224517e — one 18.3 s macOS desktop AFTER recording.

known_gap: the native macOS application menu is outside gpui-agent's semantic tree and has no GPUI keybinding; closest tracked gpui-agent protocol gap is hexuria/gpui-agent#120 (native menus in the tree). Exact BEFORE/AFTER errors are in evidence/*native-menu-gpui-attempts.txt: `node run:File > New tax profile… not found` and `unknown binding menu.File/New tax profile…`. OS input was forbidden, so the menu item itself is not runtime-proven.

Caveat: Home empty-state evidence used the reachable helpdesk-admin/no-profiles scratch session, not literal solo-owner/no-profiles; the report explains why the literal state was unreachable through the real UI. Visual BEFORE/AFTER wording and navigation evidence is valid for that state.

Build provenance: AFTER macOS run header `build: app 8f38ceda, nsh 8c78cfd6 beside it, tool nsh 0.6.1 (8c78cfd), cargo 1.99.0 (5f94df478 2026-08-27) ... folder /Users/devin/repos/newtaxproof492/target`; BEFORE macOS and BEFORE iPhone headers use `app af4348cf` with the same NSH pin/toolchain. Full headers and commands are in report.md and run-logs/.

Safety: scratch databases only; `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`; never `BIR_SFTP_LIVE`; no Queue/Send/Submit; UI input only through gpui-agent-cli 0.5.0.

Recipes reused/kept as proof copies in recipes/. Full capture index: CAPTURES.tsv. Full report: report.md.
