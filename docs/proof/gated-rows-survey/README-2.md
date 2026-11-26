# Gated rows survey — follow-up reading report

Free disk: 49.6 GB.

Exact main confirmed with `git rev-parse HEAD`: `642bfc78221c0a87b318c7c3378c3f5b0b2c9359`. Pinned kit `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`, `nsh 0.6.1 (8c78cfd)`. No product code changed.

Method: `FormSession` probes typed invented values, blurred, saved, and searched `form_drafts.data_json` for exact unique markers. Page probes opened `nsh/screens/form/FormPageAny.tsx` headlessly with `openParts=all`. The first combined probe shared one scratch database across tests; overlapping forms were rerun in fresh `nsh test` processes. “Editable” means present, `locked=false`, and `off` empty; `off` itself only fades the control in `FieldControl.tsx`.

## 1. Dormant rows — run confirmations

- **1701** `txtPg2IShed1a_1SName`: closed host blank; typing stored at `$.employers[0].employer_name`; after married + spouse-income Yes + joint + Part IV ATC II011 + Spouse owner it came back uppercased. Same for `txtPg2IShed2a_2SName` at `$.employers[1].employer_name`. `txtPg3IShed3_27Desc` was present/editable closed, stored at `$.computations.schedule_3_descriptions.27`, and came back uppercased at ATC II017.
- **1706** `frm1706:j_id391:_7`: present/editable closed; host blank, stored at `$.property_class_other`; came back at property-class Others.
- **1707** `txtPg1I9TransDescSpec`: present/editable closed; host blank, stored at `$.transaction_description`; came back at transaction Others.

Headless page scan: **1601FQ** row ids were not drawn in the simple scan (0/15), so modal/row-drawer inspection is still needed; **2551Q** item 16 drawn once; **1606** all five installment boxes drawn once; **1706** and **1707** targets drawn once. **1701**'s full page scan hit the 30 s headless timeout; in the session the spouse keys are not schema-indexed while closed and item 27 description is present/unlocked.

## 2. The 18 discarded rows — typed-and-left results

| form | editable in gated state | typed and left | sentence under it |
|---|---|---|---|
| 0619E | 1/1 present and editable; all `off` empty/unlocked | typed/left values `0.00`; exact marker in draft 0/1 | none |
| 0619F | 1/1 present and editable; all `off` empty/unlocked | typed/left values `0.00`; exact marker in draft 0/1 | none |
| 1600PT | 2/2 present and editable; all `off` empty/unlocked | typed/left values ``, `0.00`; exact marker in draft 0/2 | none |
| 1600VT | 2/2 present and editable; all `off` empty/unlocked | typed/left values ``, `0.00`; exact marker in draft 0/2 | none |
| 1600WP | 1/1 present and editable; all `off` empty/unlocked | typed/left values `0.00`; exact marker in draft 0/1 | none |
| 1602Q | 1/1 present and editable; all `off` empty/unlocked | typed/left values `0.00`; exact marker in draft 0/1 | none |
| 1603Q | 1/1 present and editable; all `off` empty/unlocked | typed/left values `0.00`; exact marker in draft 0/1 | none |
| 1606 | 1/1 present and editable; all `off` empty/unlocked | typed/left values `0.00`; exact marker in draft 0/1 | none |
| 1701 | 13/13 present and editable; all `off` empty/unlocked | typed/left values ``, `0.00`; exact marker in draft 0/13 | none |
| 1701A | 31/31 present and editable; all `off` empty/unlocked | typed/left values ``, `0.00`; exact marker in draft 0/31 | none |
| 1701MS | 10/10 present and editable; all `off` empty/unlocked | typed/left values ``, `0.00`; exact marker in draft 0/10 | none |
| 1702MX | 21/21 present and editable; all `off` empty/unlocked | typed/left values ``, `1357.89`; exact marker in draft 0/21 | Item 1: type a whole number, for example 3.; Item 27: type a whole number, for example 3.; Item 2: type a whole number, for example 3.; Item 3: type a whole number, for example 3. |
| 1702RT | 20/20 present and editable; all `off` empty/unlocked | typed/left values ``, `1357.89`; exact marker in draft 0/20 | Item 1: type a whole number, for example 3.; Item 2: type a whole number, for example 3.; Item 3: type a whole number, for example 3.; Item 45: type a whole number, for example 3.; Item 51: type a whole number, for example 3. |
| 1706 | 1/1 present and editable; all `off` empty/unlocked | typed/left values `0.00`; exact marker in draft 0/1 | none |
| 1707 | 5/5 present and editable; all `off` empty/unlocked | typed/left values `0`, `0.00`; exact marker in draft 0/5 | none |
| 2000 | 2/2 present and editable; all `off` empty/unlocked | typed/left values ``, `0.00`; exact marker in draft 0/2 | none |
| 2550M | 1/1 present and editable; all `off` empty/unlocked | typed/left values `0.00`; exact marker in draft 0/1 | none |
| 2552 | 1/1 present and editable; all `off` empty/unlocked | typed/left values ``; exact marker in draft 0/1 | none |

## 3. Official toggles recorded in the repository

“Not in the repository” means no close/open mutation rule for that target row is recorded in `validations.json`, not that the official form necessarily lacks one.

| form | official toggle evidence |
|---|---|
| 1601FQ dormant | `1601fq-change-003`, `disableSelTreaty:L3573-L3591`: “Treaty selection and Schedule 1 are cleared and disabled” — close disable+clear; open mutation not recorded. `1601fq-change-004`, `showAddSchedI:L3696-L3730`: “You are not availing of tax relief under Special Law or International Tax Treaty. There is no need to\nfill up schedule 1.” Inventory L1185/L1197/L1198 disabled. |
| 2551Q dormant | `2551q-amended-toggle`, `updateAmended:L4881-L4901`: “Disables and zeros Item 16; amended Yes enables it unless the 8% option is active” — close disable+zero, open enable. `2551q-tax-rate-eight-percent`, `taxRateOption:L3117-L3176`, zeros/disables listed figures. Inventory L1125 `0.00` disabled. |
| 1606 dormant | `enableInstallment:L3230-L3266` is in the function inventory, but close/open behavior is not in the repository. Inventory L1124/L1127/L1130/L1133/L1136 disabled. |
| 1701 dormant | Close/open toggle not in the repository. Related spouse validation only: `validateSpouseFields:L9041-L9132`. Inventory spouse keys L1362/L1411 and Schedule 3 item 27 description L2023 disabled. |
| 1706 dormant | Close/open toggle not in the repository. `1706-validate-018-classification-other-description`, `validate:L3306-L3310` + `control:L930-L946`, is validation only. Inventory L931 disabled. |
| 1707 dormant | Close/open toggle not in the repository. `1707-validate-022-transaction-other`, `validateAll:L3680-L3685`, is validation only. Inventory L843 disabled. |
| 0619E | Close/open toggle not in the repository; inventory `txtTax15` L634 not disabled. |
| 0619F | Close/open toggle not in the repository; inventory `txtTax16` L679 not disabled. |
| 1600PT | `1600pt-change-001`, `isAmended:L3486-L3494`: “Item 19 is reset, disabled, and downstream totals recompute” — close disable+zero; open not separately recorded. `1600pt-change-002`, `availTaxRelief:L3496-L3503`: “Item 12A is cleared and disabled” — close disable+clear; open not separately recorded. Inventory L757/L662 disabled. |
| 1600VT | `1600vt-change-001`, `isAmended:L3302-L3310`: Item 19 reset/disabled/recomputed; `1600vt-change-002`, `availTaxRelief:L3312-L3319`: Item 12A cleared/disabled. Open not separately recorded. Inventory L757/L662 disabled. |
| 1600WP | Close/open toggle not in the repository; inventory `txtTax13` L954 `0.00` disabled. |
| 1602Q | Close/open toggle not in the repository; inventory `txt20` L818 `0.00` not disabled. |
| 1603Q | Close/open toggle not in the repository for item 15; inventory `txtTax15` L691 `0.00` disabled. |
| 1606 discarded | Close/open toggle not in the repository; inventory `txtLess` L1320 `0.00` disabled. |
| 1701 discarded | Close/open toggle not in the repository; foreign tax number L547 disabled/blank, Schedule 5 rows mostly not initially disabled. |
| 1701A | Close/open toggle not in the repository; all 31 target controls found, all initially disabled. |
| 1701MS | Close/open toggle not in the repository; all ten found, six disabled in aggregate. |
| 1702MX | Close/open toggle not in the repository; all 21 found, none initially disabled. |
| 1702RT | Close/open toggle not in the repository; all 20 found, item 51 and Schedule 4 rows disabled, item 45 not. |
| 1706 discarded | Close/open toggle not in the repository; inventory `txtLess` L1328 `0.00` disabled. |
| 1707 discarded | Close/open toggle not in the repository; five target controls found, none initially disabled. |
| 2000 | Close/open toggle not in the repository; `txtOtherName` L634 not disabled, `txtTax15A` L752 `0.00` disabled. |
| 2550M | Close/open toggle not in the repository; inventory `txtTax23D` L1020 `0.00` not disabled. |
| 2552 | Close/open mutation not in the repository. `2552-validate-013-tax-relief-spec`, `validateAll:L4367-L4372`, validates only. Inventory L645 disabled. |

## 4. Where a fix naturally goes (reading only)

| form | natural model/host location |
|---|---|
| Host examples | `crates/bir-app-api/src/screen_form_module/conditions.rs:227-236` inserts donor/estate `off` tags; `conditions.rs:250-267` inserts condition labels for closed withholding/monthly/annual-individual gates. |
| 1601FQ | `form_1601fq.rs:1136-1165` outputs closed Schedule 1 as `-`/`0.00`; `set_tax_relief` is the natural gate-transition clear/preserve point. |
| 2551Q | `form_2551q_xml.rs:171-178` outputs item 16 zero unless amended; the amended setter/recompute path is the clear point. |
| 1606 dormant | `form_1606.rs:520-555` clears installment values on non-installment choices; output blanks at `750-774`. |
| 1701 dormant | `form_1701_official.rs:1587-1601` blanks employer names by owner; `1659-1690` blanks Schedule 3 item 27 unless 8% applies. |
| 1706 dormant | `form_1706.rs:800-805` blanks Others details unless class Others; property-class setters `525-551` are the clear point. |
| 1707 dormant | `form_1707.rs:581-587` blanks description unless transaction Others; transaction setters `315-330` are the clear point. |
| 0619E / 0619F | `form_0619e_official.rs:91-95` and `form_0619f_official.rs:96-105` already zero previous remittance when not amended. |
| 1600PT / 1600VT | `form_1600pt.rs:602-607` and `form_1600vt.rs:523-528` already zero item 19 and clear relief specification. |
| 1600WP / 1602Q / 1603Q | `form_1600wp.rs:357-372`, `form_1602q.rs:432-439`, `form_1603q.rs:268-284` already zero previous payment when not amended. |
| 1606 discarded | `form_1606.rs:562-564` already zeroes item 29 when not amended. |
| 1701 discarded | Foreign tax output `form_1701_official.rs:1367-1371`; Schedule 5 output `1724-1754`; clear in the foreign-credit/schedule applicability transition. |
| 1701A / 1701MS | `form_1701a.rs:612-634,1134-1140` and `form_1701ms.rs:559-582,695`; clear in the joint/rate/deduction/foreign/amended transitions feeding them. |
| 1702MX / 1702RT | Serialization `form_1702mx_xml.rs:182-184`, `form_1702rt_xml.rs:432-468`; clear in regime/MCIT/amended/schedule applicability transitions feeding those rows. |
| 1706 / 1707 discarded | `form_1706.rs:512-565` zeroes previous payment when unamended; 1707 installment output `form_1707.rs:638-660` with installment model `115-377`. |
| 2000 / 2550M / 2552 | Natural points: `form_2000.rs:929-946` party/mode transitions; `form_2550m.rs:699-701` item 23D recompute; `form_2552.rs:910-918` relief specification state. |

Raw machine evidence: `C:\Users\Administrator\gated-survey\focus-*.log`, `probe-full.log`, `focus-objects.json`, `all-probe-objects.json`, and `WORK-NOTES.txt`.
