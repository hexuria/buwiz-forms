driver: gpui-agent-cli 0.4.0

# D2 proof: TIN status on screen

Pictures and a recording of the chips, the Activate flow and the Queue sheet, driven only through gpui-agent-cli
recipes (`just e2e <target> tin-status --allow-send --record`, recipes in `agent-recipes/e2e/`) against the local
mock account server (`just mock-account`, crates/bir-core/src/mock_account.rs). Nothing but the mock is contacted;
dry-run SFTP, no keychain, test env. Never a simctl tap, osascript or CGEvent: steps gpui-agent cannot do are
known_gap entries with their issue numbers.

Suite `agent-recipes/e2e/tin-status.json`: setup-start, sign-up (tin-sign-up types the 9-digit TIN root into the
segmented control), buwiz-sign-in (sign_in: true; the code goes as keys to gate-code-page while the code boxes
have focus), tin-chips, tin-queue-sheet (the sheet stops Queue, nothing is queued).

Status of this branch: the macOS run at the first head proved setup-start and sign-up end to end
(01-setup-start/, 02-sign-up/ screenshots; report.json/report-macos.md here); the sign-in recipe then failed on a
recipe-ordering fault (the account card is drawn only after the account settings tab is picked — fixed in
buwiz-sign-in.wants: navigate first, then the card). The full-suite rerun at the final head and the iPhone run
land here as they finish. known_gaps:
1. gpui-agent-cli 0.4.0 cannot key the sign-in code: `key` needs native focus on a target node, the OTP textbox
   ("Sign-in code, 6 digits", role textbox) is not targetable by id, and the boxes wrapper (gate-code-boxes) is a
   non-focusable generic. Every recipe verb combination was tried (click boxes: not pressable; click the textbox
   name: not a target form; key the page: cannot receive native focus). The email step before it passes end to end.
   buwiz-sign-in stops at s15, so tin-chips and tin-queue-sheet cannot run in the suite — they need the signed-in
   account. Issue number: to be filed host-side by the owner (not in this PR).
2. The iPhone simulator proof moves to another machine: the only simulator runtime on the build machine is the
   in-use iOS 27.0 and may not be deleted.
