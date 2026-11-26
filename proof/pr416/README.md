# PR #416 — 2551Q Item 16 gate (6c4ce772), Windows real window, OS keys
Build: hexuria/buwiz-forms fix/2551q-item16-gate @ 6c4ce77278ee7f7c632ecede9dd191653c515999, own checkout + own build folder `C:\Users\Administrator\370pr416\buwiz-forms\target\nsh-store`. 2551Q Q4 2026 draft (Santos). Saved values read from the draft db.

| case | result | evidence |
|---|---|---|
| a. original return, Amended UNANSWERED | **FAULT** — Item 16 EDITABLE: click+type 42 entered, showed 42.00 after leaving, Item 18 recomputed including it (10,049.00); no sentence under the box. The box is focusable and takes select-all+replace like any editable amount. | s23/s24/s25 png |
| a2. Amended=No answered | Item 16 SHUT: greyed + lock icon, 0.00, static sentence "Item 16 applies only to an amended return." | s34-casec.png |
| b. Amended=Yes | Item 16 open; typed 42 → 42.00 shown; draft tax_paid_previous=42.0; Item 18 includes it (10,049.00 with item15=7, item17=10,000) | s42-printb region; db |
| c. Yes→No (something to clear) | Item 16 greyed+lock 0.00; once-sentence VERBATIM: **"Item 16 was cleared: it applies only to an amended return."**; draft holds 0.0; Item 18 drops it (10,007.00) | s44-cleartrans.png |
| d. Yes again | Item 16 open, shows 0.00, nothing comes back; db 0.0 | s36-cased.png |
| e. 8% option in Item 13 | NOT AVAILABLE in this return: banner "Set the tax profile's business start date to determine whether Item 13 applies to this quarter." — case not applicable here | s42-printb2 full shot top banners |
| f. close+reopen after c | Item 16 still 0.00 shut; only the static sentence "Item 16 applies only to an amended return." — the once-sentence does not repeat | s39-casef.png |
| g1. print preview after b (42 in item16, Yes) | Item 16 PRINTS 42.00; Item 4 Yes X-marked | s43-printb2.png |
| g2. print preview after c | see s45-printc.png | s45-printc.png |

Notes: the custom Yes/No and quarter pills do not respond to OS mouse clicks in this build — keyboard (Tab into group, arrows/Enter) works. Text inputs accept clicks fine. This made every radio interaction keyboard-driven; flagging in case it is not intentional.
