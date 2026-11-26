# head-7eacb1d2 (fix/money-box-keep-text-on-focus) Windows proof
Build: exact commit 7eacb1d2195b8dc1e1a1b65ad97728c15b50bc2f, no local changes. Real OS keys, saved from draft DB. 2551Q Sch1 row1 taxable.

## W-series results (allRIGHT unless noted)
W1 Tab into 1,234.56 type 15000.50 -> saved 15000.5 RIGHT (separators kept on focus, whole amount selected)
W2 Shift+Tab same RIGHT 15000.5
W3 click ctrl+a type RIGHT 15000.5
W4 click ctrl+a BS type RIGHT 15000.5
W5 one click type RIGHT 15000.5 (click selects whole)
W6 Tab into 12,345,678.90 type RIGHT 15000.5
W7 click 0.00 type RIGHT 15000.5 (zero box empty on focus)
W8 Tab into 0.00 type RIGHT 15000.5
W9 99999.99 RIGHT 99999.99
W10 1000000.005 RIGHT 1000000.01 (half up)
W11 1234567890+Left Left BS RIGHT 123456790.0
W12 99999999.99 x3 RIGHT all three (99999999.99)
W13 burst 15000.50 no delay RIGHT 15000.5
W14 paste 15000.50 RIGHT 15000.5
W15 type 1234 alt-tab roundtrip type 5 RIGHT 12345.0 (text intact on return)
W21 clear+leave RIGHT 0.0, box 0.00, no message
W22a typed 15,000.50 (comma as separator) RIGHT saved 15000.5
W22b typed 15000,50 (comma as decimal): comma VISIBLE while typing (15000,50) but dropped at parse -> shows 1,500,050.00 after leaving, saved 1500050.0 - see hunt H6
W23 pastes into 0.00: `₱ 15,000.50` -> 15000.5 taken; ` 15000.50 ` spaces -> 15000.5 taken; `15,000.50` -> 15000.5 taken; `PHP 15000.50` -> 15000.5 taken; `15000.50.25` -> REFUSED cleanly (box 0.00, saved 0, no wrong number)
W24 paste 15,000.50 over Tab-selected filled amount -> replaced, saved 15000.5 RIGHT
No "box shows/draft stored" mismatch message in ANY case.
W16-W20 skipped per orchestrator (head will move).
