# RDO pickers — Android proof (e5f7a504)

Between e5f7a504 and main 219da036, none of the files behind these pickers changed (only FormsTab.tsx moved), so no main build was needed. All measurements on the e5f7a504 packaged build.

## 1. Profile RDO picker — fully populated, search works, picking works

- The job-19 taxpayer's Profile held **RDO empty ("Not set")** before the test.
- The profile-path RDO picker (same `store.rdoOptions()` → `bir.list_rdos()` source) renders the **full reference list** — rows "003 San Fernando La Union", "004 Calasiao Pangasinan", "005 Alaminos", "006 Urdaneta", "007 …" and on. Search works: typing "050" filtered to exactly "050 — South Makati" (rdo-search-050.png). Tapping the row picked it; the picker value read "050 - South Makati".
- In the profile itself, RDO editing lives in the **Registered activity** collapsible section (head office) → Edit → sheet "Registered activity" with Branch code / Branch name / RDO. 050 was picked and **saved**: the profile then reads "RDO 050" (profile-rdo-saved.png).
- In the New-branch form the RDO picker is disabled while its "Same as head office" switch is on (greyed "Choose…"); switching it off enables the picker, which is the same fully populated searchable list.
- The ZIP picker is likewise fully populated with working search (zip-picker.png). Android rendering of SearchPick and the door list calls are fine; the 1601C's two-row sheet ("Choose…", "000") is the schema-placeholder behavior exactly as diagnosed — not an Android fault.

## 2. Reopening the 1601C draft after saving profile RDO 050 — THE BOX DOES NOT INHERIT

Direct visual evidence (rdo-group-open.png, rdo-part1c.png): Part I · Background Information, inside the "From tax profile · 2 filled boxes · 1 needs attention" collapsible group:

- Item 7 **"RDO Code *"** box shows **"000"** — NOT 050 — even though the profile now holds 050.
- The box is **LOCKED**: padlock icon in the box, note under it reads **"Comes from the tax profile."** There is an "Edit" button at the row's right (its behavior unverified — tapping it in the harness collapsed the section without opening anything conclusive).
- The group header reads "1 needs attention", and the bottom to-do bar reads "1 to do · Part I — Background Infor…". The RDO to-do did **NOT** clear.

So for a draft created while the profile RDO was empty, reopening it after the profile gains a valid RDO does not refresh the box: it keeps the schema placeholder default 000, locked to the profile, invalid, and the user cannot fix it in the form. Whether a freshly created draft (after the profile has 050) inherits correctly is untested — flagged honestly. The return cannot be queued in this state, so relay-drop (b)/(c) remain blocked.

## 3. New taxpayer with empty RDO — BLOCKED BY DESIGN in this build

The "New taxpayer" row does not exist in the UI for this session. The Tax profile panel rows end at "Add branch / Settings / Theme / Lock app / Log out". The code gate: `canAddTaxpayer = () => !store.taxpayers().length || (isAdmin() && isHelpdesk())` (nsh/shell/app.ts:204). With one taxpayer present, a second can be added only by an admin+helpdesk session; the demo sign-in is neither. The first-launch New taxpayer page (no chrome) only shows when no taxpayer exists. So item 2 cannot be exercised through the UI in this build/session without a helpdesk sign-in or a database reset. Reported, not worked around.

## 4. How the job-19 taxpayer was created

Created by hand through the app's own UI by me (this session's harness) during job 19: first-launch New taxpayer page, demo values only (Demo Taxpayer, TIN 123-456-789-00000, made-up address/contact). No seed, no test hook. No RDO was given at creation — the RDO stayed empty until the 050 edit above.
