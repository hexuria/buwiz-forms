# main afc1c738 baseline (unmodified), Windows Server 2022, real OS keys at 120 ms, 2551Q Sch1 row 1 taxable amount

| probe | entry | result | saved |
|---|---|---|---|
| M1 Tab into 1,234.56, type 15000.50 | amount IS selected whole on Tab (m1-before highlight) | keys replace selection BUT live reformatting corrupts digits while typing | **1500.50** (typed 15000.50) |
| M2 Shift+Tab into 1,234.56, type 15000.50 | same select-all | same corruption | **1500.50** |
| M3 click between 2 and 3, key 9 | click SELECTS WHOLE amount (no caret) | 9 replaces all | 9.00 |
| M4 click just after last digit, key 9 | NO highlight visible; caret lands BEFORE final digit | box becomes 1,234.596 (m4-inbox.png) | **1234.60** (intended append: 1,234.57) |
| M5 second click on focused box, key 9 | highlight persists, no caret | 9 replaces all | 9.00 |

Key answers: on main Tab DOES select the filled amount (so the Tab-selection failure on the fix heads was NOT inherited from main - yet main still corrupts through live reformatting: 15000.50 saves as 1500.50). On main clicks mostly select-all like the candidate, BUT a click at the end can place a misplaced caret that splices digits (1234.596 -> saved 1234.60 instead of 1,234.57).
