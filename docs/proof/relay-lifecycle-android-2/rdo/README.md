# RDO pickers — Android proof (e5f7a504)

Between e5f7a504 and main 219da036, none of the files behind these pickers changed (only FormsTab.tsx moved), so no main build was needed. All measurements on the e5f7a504 packaged build.

## 1. Profile RDO picker — fully populated, search works, picking works

- The job-19 taxpayer's Profile held **RDO empty ("Not set")** before the test.
- The profile-path RDO picker (same `store.rdoOptions()` → `bir.list_rdos()` source) renders the **full reference list** — rows "003 San Fernando La Union", "004 Calasiao Pangasinan", "005 Alaminos", "006 Urdaneta", "007 …" and on. Search works: typing "050" filtered to exactly "050 — South Makati" (rdo-search-050.png). Tapping the row picked it; the picker value read "050 - South Makati".
- In the profile itself, RDO editing lives in the **Registered activity** collapsible section (head office) → Edit → sheet "Registered activity" with Branch code / Branch name / RDO. 050 was picked and **saved**: the profile then reads "RDO 050" (profile-rdo-saved.png).
- In the New-branch form the RDO picker is disabled while its "Same as head office" switch is on (greyed "Choose…"); switching it off enables the picker, which is the same fully populated searchable list.
- The ZIP picker is likewise fully populated with working search (zip-picker.png). Android rendering of SearchPick and the door list calls are fine; the 1601C's two-row sheet ("Choose…", "000") is the schema-placeholder behavior exactly as diagnosed — not an Android fault.

## 2. Inheritance into the 1601C draft — RDO to-do cleared; one non-RDO to-do remains

Before the profile edit the draft's single open validation item was the RDO (per relay-drop measurements). After saving profile RDO 050 and reopening the draft:

- The form overview reads "1 to do · Page 1 — **Return details** — 1" (rdo-draft-state.png). The to-do is now in the **Return details** section (month/ammended/taxes-withheld/sheets/ATC — all visibly filled, ATC WW010); the RDO/Part-I complaint is gone. Inference: setting the profile RDO cleared the RDO validation item, i.e. the form does pick up the profile RDO. Not directly confirmed: I did not manage to open the Part I RDO box itself in the harness within the session's budget, so the exact box rendering (050 shown, locked vs editable, lock-note wording) is unverified — flagged honestly.
- The return still does not pass validation (1 to-do left in Return details, cause not yet identified — possibly an unanswered radio), so relay-drop (b)/(c) could not run. Queueing remains blocked, now by a different, smaller item.

## 3. How the job-19 taxpayer was created

Created by hand through the app's own UI by me (this session's harness) during job 19: New taxpayer in the on-device app, demo values only (Demo Taxpayer, TIN 123-456-789-00000, made-up address/contact). No seed, no test hook. No RDO was given at creation — the RDO stayed empty until the 050 edit above.

## 4. New taxpayer with empty RDO — NOT RUN

Not measured: harness budget was consumed by the RDO picker/inheritance navigation. Open item.
