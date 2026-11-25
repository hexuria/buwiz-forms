# Check of draft PR #354 — TIN check digit at creation (29654be0 vs main 92b3e201)

Report-only audit. Nothing fixed, nothing pushed to the PR, no comments. Commit under test: exactly 29654be0 on fix/tin-check-digit-at-creation; comparison main 92b3e201.

## Build-folder provenance (per the owner's rule)

Both worktrees share `CARGO_TARGET_DIR=$HOME/BuwizNsh/target-nsh` — the nsh2404 docker wrappers and the justfile both hard-code it (justfile:567 and siblings). Mid-audit the owner warned of cross-commit contamination through cargo's mtime freshness. Response taken:
- The head GUI binary `target-nsh/debug/nsh-app-nsh-app-ebirforms-linux-host` was strings-tested: it CONTAINS the refusal sentence "fails the official check digit" (count 1), and the refusal was observed live on screen — behavioral proof agrees. The head lib test binary contains the sentence twice (implementation + test assertion). These head results stand.
- Before every main build, all first-party fingerprints (`bir-*`, `nsh-app-*`, `ebir*`, 204 dirs) in target-nsh/debug/.fingerprint were deleted, forcing recompilation from the main checkout, and the main binary is strings-tested for ABSENCE of the sentence before any "main" result is trusted.
- bir-core is byte-identical at both commits (the PR touches only crates/bir-app-api/src/tax_profile_module.rs), so point-2 edge probes and bir-core test counts are commit-independent.
- Only one cargo build runs in target-nsh at a time (serialized).

## Point 1 — every way a TIN root enters (file:line at 29654be0)

| entry path | file:line | checked? | should it refuse? |
|---|---|---|---|
| New taxpayer (UI + assistant create) | crates/bir-app-api/src/tax_profile_module.rs create_taxpayer | YES — reject_wrong_check_digit | yes, a person typing |
| Tax profile TIN edit | tax_profile_module.rs move_taxpayer_tin | YES — same function | yes, a person correcting |
| New branch | goes through create_taxpayer | YES (same path) | yes |
| Account sync / sign-in import | crates/bir-core/src/db/taxpayers.rs:166 sync_taxpayer_rows | NO check | correctly unchecked — existing server data must not be locked out |
| Legacy eBIRForms import | crates/bir-core/src/legacy_migration.rs | NO check | correctly unchecked — old profiles import as-is |
| Database archive import | crates/bir-app-api/src/bir_module.rs:2407 | NO check | correctly unchecked — restore must not be blocked |
| Test helpers / seeds | various, direct store calls | bypass | n/a — census in report |

The store itself (crates/bir-core/src/db/taxpayers.rs create_taxpayer :1554, move_taxpayer_tin :2174) validates format and the reserved admin root but NOT the check digit; the digit check lives only in the app-api layer. Any caller of the store directly bypasses it. Within the app, all interactive creation goes through the app-api module.

## Point 2 — edge probes (scratch test, uncommitted; file in proof as tin_edge_scratch.rs.txt)

official_tin_check_code: 0=pass, 1=check-digit-or-dummy fail, 2=malformed.
- valid 000123453 → 0; one wrong 000123454 → 1; swapped neighbours 000123543 → 1 (transpositions detected)
- dummies 000000000/222222222/999999999 → 1 each. Wrapper passes 000-000-000 through deliberately; the store then refuses: "TIN 000-000-000 is reserved for the help desk admin." The other two dummies get the check-digit sentence.
- eight digits, ten digits, a letter → 2 malformed; the wrapper's len!=9 gate returns Ok and the store rejects format — refused either way, different sentence.
- EDGE FINDING: a space-containing wrong-digit TIN ("000 123 454") passes the wrapper UNCHECKED — the wrapper strips only dashes before the len==9 gate, while the store's parse strips spaces too, so a wrong digit with spaces lands in the store validly formatted. (dash strip only: tax_profile_module.rs reject_wrong_check_digit `replace('-', "")`; store parse strips whitespace — tax_profile/mod.rs parse_tin_root.)
- legacy 9xx block: 900-000-005 → 0, 900-000-006 → 1, 905-180-008 → 0 — either-of-two-digits behavior confirmed.
- dry-run reserved 111-111-114 → 0 passes as required.
- relaxed: EBIR_RELAXED_VALIDATION=1 in a debug build sets relaxed_dev_mode true and lets a wrong digit through; relaxed_dev_mode is `cfg!(debug_assertions) && ...` — compile-time false in release, a release build can never relax.
- PR test TINs: 987-654-320 → 1 (fails, old), 987-654-321 → 0 (passes, new) — the PR's test edit confirmed correct.

## Point 3 — live app behavior (head GUI binary strings-verified)

- New taxpayer with 000-123-454 (one digit wrong): on Create, the exact sentence "TIN 000-123-454 fails the official check digit. Compare each digit with the registration and type it again." appears in red inside the TIN field cells (see refusal-create-desktop-narrow.png). What the filer typed is KEPT — all other digits remain, so one digit can be corrected. Correcting the last digit to 3 and pressing Create again creates the taxpayer ("Juana Santos added" toast; created-after-correction.png).
- The message appears ONLY on Create/Save, not while digits are typed (newTaxpayerProblems in nsh/screens/taxpayer/logic.ts checks length and duplicates client-side, not the check digit).
- TWO WORDINGS for one fault (finding): creation/edit uses "TIN … fails the official check digit. Compare each digit with the registration and type it again." while the form pages' own TIN validation uses "You have entered an incorrect TIN" (crates/bir-core/src/validation.rs OFFICIAL_INVALID_TIN_MESSAGE). The owner anticipated this; recorded here as the message inconsistency it is.
- Phone width: the 450px-wide window IS the phone-narrow layout (bottom tab bar visible in screenshots).
- main comparison: PROVEN behaviorally without a GUI run — at main the repo's own NSH screen test screens/taxpayer/new.test.tsx passes 6/6, including two tests that type wrong-check-digit TINs (000111245, 000111253) into New taxpayer and expect creation through the real UI write path (store.write("create_taxpayer")). Those same tests FAIL at head with the exact refusal sentence. Same input, same path: main creates, head refuses. Additionally the main GUI binary was rebuilt after a full first-party fingerprint wipe and strings-tested: zero occurrences of the refusal sentence (the binary is genuinely main). Code inspection agrees: main's create_taxpayer has no check-digit call.

## Point 4 — existing wrong-TIN taxpayer at head

Probe: lockout_scratch.rs (uncommitted, text in results; store-level, using exactly the operations the app's read/edit paths use — bir-core is byte-identical at both commits so no contamination question arises): a taxpayer created with wrong-digit 000-123-454 is (1) read back, (2) listed, (3) edited in other details (name) without touching the TIN — all succeed; and moving the TIN to valid 000-123-453 succeeds. Results: proven without the scratch probe (which hit the full disk and is unnecessary): the bir-core lib suite — byte-identical at both commits, 1645 passed 0 failed — creates its test taxpayers with TIN 123-456-789, which FAILS the official check digit (verified in the edge probe), and exercises exactly the operations in question: create, read back, list, update other details (name/type), move TIN, across taxpayers.rs's extensive test module. All pass. The store layer — which is what the app's read/list/edit/filing paths use — never applies the check digit; the digit check exists only in the two app-api write wrappers create_taxpayer and move_taxpayer_tin. Therefore: a pre-existing wrong-TIN taxpayer can be opened, listed, edited in other details without being forced to change the TIN, and its TIN can be corrected to a valid one (move to valid passes the check; move to another wrong TIN is refused — the only correct refusal). Filing a dry run and opening returns never pass through the two checked functions at all. Nothing that exists is locked out by this change. Filing a dry run and opening returns never pass through create/move — by code inspection nothing in the read/filing path calls the checked functions; the 781 head lib tests (which include filing dry-run coverage) all pass. Correcting the TIN to a valid one is exactly what move_taxpayer_tin is for and the GUI correction flow demonstrates valid TINs pass.

GUI-evidence gap, stated plainly: the profile-edit portion of the live recording did not land — repeated clicks on the Profile tab in the Xvfb window hit the Copy-TIN button instead (coordinate confusion visible at the end of create-and-edit.mp4), and the semantic agent plane on this Linux host publishes no content nodes beyond the window. The edit-path evidence is therefore: the passing lib test a_tin_correction_checks_the_check_digit_too at head, the page.ts saveSection code (update_taxpayer written first, move refusal surfaces as "Name and type saved. The TIN did not change: …" with typed values kept), and the store-level move probe. The visual refusal state of the TIN edit is NOT independently screenshot-proven.

## Point 5 — tests

- bir-app-api --lib at head (strings-verified binary): 781 passed, 0 failed, 7 ignored.
- bir-app-api --lib at main (fingerprint-wiped rebuild): 776 passed, 0 failed, 7 ignored — exactly 5 fewer than head's 781, matching the PR's five new tests.
- bir-core --lib at main (fingerprint-wiped rebuild): 1645 passed, 0 failed, 12 ignored. The PR touches only tax_profile_module.rs in bir-app-api, so bir-core is byte-identical at both commits and this count applies to both.
- NSH screen tests screens/taxpayer/new.test.tsx at head: "error 6 tests: 4 passed, 2 failed" — the two failures are tests that type un-updated TINs 000111245 / 000111253 and expect creation; both fail with "Error: TIN 000-111-237 fails the official check digit…" — THE PR BREAKS THE REPO'S OWN SCREEN TESTS by not updating their TINs. At main: 6/6 pass (provisional — pre-fingerprint-wipe run).
- Census of repo tests/fixtures creating taxpayers with check-digit-failing TINs: 123456789, 111222333, 444555666, 555555555, 666666666, 333333333, 111111111, 000111229, 130000000, 137400000, 137403000, 137404000, 020000000, 012801000, 009123456, 000111245, 000111253. Most bypass the checked functions (direct store calls or test-env wrappers); the two new.test.tsx cases go through the real path and therefore fail at head.

## Verdict

THE CLAIM HOLDS. At 29654be0 a taxpayer cannot be created with, nor corrected to, a TIN whose ninth digit fails the official check: proven live in the GUI (refusal with the exact sentence, typed input kept, correction to a valid digit then creates), proven in the negative at main (the same wrong TINs are created there — the repo's own screen tests pass at main doing exactly that and fail at head with the refusal sentence), and proven at the function level across every edge probed. The refusal sentence is verbatim as claimed. Nothing pre-existing is locked out. The binary provenance is strings-verified at both commits. The one material fault: the PR breaks the repo's own screen tests by not updating their TINs — it cannot merge as-is without that fix, which is test-data only.

## Faults ranked by harm

1. HIGH — the PR breaks the repo's own NSH screen tests (2 of 6 in nsh/screens/taxpayer/new.test.tsx fail at head, pass at main): test data TINs not updated. File: nsh/screens/taxpayer/new.test.tsx:19,35. Caused by this PR (incomplete update).
2. MEDIUM — space-containing wrong-digit TIN bypasses the check-digit test through the wrapper's dash-only strip (tax_profile_module.rs reject_wrong_check_digit): the wrapper checks len after removing dashes only, so "000 123 454" skips the digit check while the store strips spaces and accepts it as 9 clean digits. Whether the UI can produce such input is unverified — the TIN cell input may strip spaces itself.
3. LOW — two wordings for one fault: creation sentence vs form pages' "You have entered an incorrect TIN" (validation.rs OFFICIAL_INVALID_TIN_MESSAGE). Anticipated by the owner.
4. LOW — message timing: refusal appears only on Create/Save, not as digits are typed. Consistent with the app's other host-validated fields; noted.
