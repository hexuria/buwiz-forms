# PR406 AFTER notes — ipad-portrait

App `a9c9fd7c0a50a3639435dcb3e83c71fdb6f6bdea`; nsh `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`; CLI `nsh 0.6.1 (8c78cfd)`; target `/Users/devin/BuwizNsh/target-a9c9fd7c`. Scratch DBs; safety env; real input only. Connect Hardware Keyboard OFF for iOS evidence.

Device: iPad Pro 13-inch (M5), PORTRAIT. Production `App.tsx`; fresh scratch `newtp.db`; on-screen keyboard used.

## New tax profile RDO/ZIP — RDO PASS, ZIP city FAIL (`pickers.mp4`, edited timestamps)

- 6.0 FAIL/open observation: RDO initial open had sheet/search top clipped offscreen while letters keyboard was up (`rdo-open-clipped.png`).
- 26.6 PASS after typing `0`, sheet settled; search pinned visible over scrolling list.
- 32.6 PASS owner end: query `0`, search pinned, last match `120 — RDO 120` visible above keyboard (`rdo-owner-end.png`); tapped/picked.
- 37.9 PASS ZIP opened focused with digits-first keyboard (`zip-digits-open.png`).
- 192.1 PASS query `12` scrolled and numeric row pick succeeded (`zip-numeric-picked.png`).
- 193.3 FAIL correct lowercase letters query `makati` showed `Nothing matches`, so no city row could be picked (`zip-makati-nothing.png`). ABC/letters state itself was visible.

## 1701Q item “RDO Code” — PASS (`1701q-rdo.mp4`, edited timestamps)

Fixture use disclosed: `TaxpayerDemo.tsx` and host-side sqlite-seeded 1701Q fixture DB.

- 13.9 PASS precondition: Part I item 6 RDO Code visible with existing `039 — South Quezon City`.
- 24.7 PASS typed code `050`; search pinned/focused with letters keyboard and exact `050 — South Makati` filtered.
- 39.0 PASS cleared then typed place `makati`; filtered rows 047/048/049/050 visible, end reachable with keyboard up and field pinned (`1701q-rdo-makati.png`).
- 42.2 PASS picked `050 — South Makati`; sheet closed and form box showed code+name.
- 47.2 PASS reopened after leaving edit mode; current `050 — South Makati` checked/in view (`1701q-rdo-current.png`).
