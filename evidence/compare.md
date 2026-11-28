# Comparison tables (status-bar strip masked)

| screen | device | result |
|---|---|---|
| setup-start 001-s1 | iphone | identical |
| setup-cards 001-s1 | iphone | identical |
| sign-up 011-s11 | iphone | identical |
| sign-up 018-s18 | iphone | identical |
| sign-up 020-s20 | iphone | identical |
| sign-up 021-s21 | iphone | identical |
| tax-year 016-s16 | iphone | identical |
| home 002-s2 | iphone | identical |
| navigation 002-s2 | iphone | identical |
| navigation 004-s4 | iphone | identical |
| navigation 006-s6 | iphone | identical |
| navigation 008-s8 | iphone | identical |
| new-return 002-s2 | iphone | identical |
| new-return 005-s5 | iphone | identical |
| form-top 001-s1 | iphone | identical |
| form-middle 002-s2 | iphone | identical |
| form-end 004-s4 | iphone | identical |
| schedule-rows 018-s18 | iphone | identical |
| sheet-pick 006-s6 | iphone | identical |
| sheet-pick 009-s9 | iphone | identical |
| sheet-calendar 004-s4 | iphone | identical |

| screen | device | result |
|---|---|---|
| setup-start 001-s1 | ipad | identical |
| setup-cards 001-s1 | ipad | identical |
| sign-up 011-s11 | ipad | identical |
| sign-up 018-s18 | ipad | identical |
| sign-up 020-s20 | ipad | differs count=1834289 box=[0, 210, 1206, 2483] — noise — mid-flow timing: BEFORE caught the sign-up card, AFTER the finished Forms tab |
| sign-up 021-s21 | ipad | identical |
| tax-year 016-s16 | ipad | differs count=84512 box=[27, 2103, 1179, 2331] — noise — the same toast mid-animation at different positions |
| home 002-s2 | ipad | identical |
| navigation 002-s2 | ipad | identical |
| navigation 004-s4 | ipad | identical |
| navigation 006-s6 | ipad | differs count=228912 box=[0, 2145, 1206, 2373] — noise — a toast ("2026 forms updated") overlapping in AFTER only |
| navigation 008-s8 | ipad | differs count=229349 box=[0, 2145, 1206, 2373] — noise — same toast |
| new-return 002-s2 | ipad | identical |
| new-return 005-s5 | ipad | differs count=922539 box=[0, 576, 1206, 2346] — noise — BEFORE caught "Opening 2551Q..." mid-load, AFTER the loaded form |
| form-top 001-s1 | ipad | identical |
| form-middle 002-s2 | ipad | identical |
| form-end 004-s4 | ipad | identical |
| schedule-rows 018-s18 | ipad | identical |
| sheet-pick 006-s6 | ipad | differs count=324 box=[144, 571, 150, 625] — noise — search-box caret blink (324 px in a 6x54 box) |
| sheet-pick 009-s9 | ipad | differs count=324 box=[144, 571, 150, 625] — noise — same caret blink |
| sheet-calendar 004-s4 | ipad | identical |
