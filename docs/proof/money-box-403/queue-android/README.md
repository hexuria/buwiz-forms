# Job 47 — money-box "press Queue" cases at 542e4844, Android, local mock relay only

- Build: hexuria/buwiz-forms #403 at exactly `542e484401494623a7ccbae9e25832985739a3a8`, the SAME debug APK as job 46 (sha256 `cd7627635ca76e1b4bfed3f28d681936a755fa001e1467cceba26e19cfeb5fb7`, verified before reuse).
- Safety (#388 guards): EBIR_TEST_ENV=1, BUWIZ_NO_KEYCHAIN=1, BIR_SFTP_DRY_RUN=1 (never BIR_SFTP_LIVE). BIR_RELAY_URL baked to `http://127.0.0.1:8787` = the LOCAL mock relay through `adb reverse`; mock account server on reverse 3008. Network cut proven before the first Queue (`ping 8.8.8.8` → Network is unreachable). Nothing non-local in either mock log. Invented values only (DEMO, TIN 123-456-788/00000, RDO 050).
- Form: 1601C monthly, amount box = item 14 "Total amount of compensation". Two returns: October 2026 (case 1), September 2026 (case 2).

## Case 1 — PASS

Type 15000.50 WITHOUT leaving the box, press Queue.

1. Typed `15000.50` into item 14, keypad taps, never left the box (`j47-c1-retyped.png` — unformatted while focused, as designed at this head).
2. **First Queue press**: the typed value settled first — the form immediately showed committed `15,000.00`→`15,000.50` formatted — and Queue **stopped ONCE** surfacing the note: banner "1 item needs attention. [Show item]" (`j47-c1-queueA.png`). The stop was validation, not a crash: item 25 (Total taxes withheld) was still 0.
3. **Second press** opened the note's folded part as a dialog: "Queue this return? Line of business is not set in the tax profile. This return can go without it. [Continue] [Cancel]" (`j47-c1-queueB.png`, `j47-c1-dialog.png`).
4. Continue → the worker sent it. App flipped to **"Submitted · read-only"** (`j47-c1-queued.png`).
5. **Mock relay log: exactly ONE new line** — `HTTP 202 (1009 bytes)` at 22:47:01 — and exactly one inbox file for the period: `…102026#test@example.com#.xml`.
6. **Payload** (decrypted with the repo's `dump_xml` dev-passphrase tool; amounts-only excerpt in `case1-payload-amounts.txt`): **`txtTax14=15,000.50` — equal to what the box showed.** PASS.

Note: the first queue attempt surfaced a hard profile-validation banner ("EOPT tier is not set in the tax profile … Answer it at item 32, then queue" with an "Open tax profile" action, `j47-c1-queue1.png`). Setting Micro + "Save for 2026" in the profile cleared it; only then did the fold-stop flow above apply. The EOPT banner is validation working as intended, not a money-box issue.

## Case 2 — PASS with one platform note

Type an amount, leave, come back and type "1500x", then Queue.

1. Typed `1500` into item 14, left the box → committed `1,500.00` (`j47-c2-committed.png`).
2. **"1500x" is not producible on Android**: the money box rejects letters at input. `KEYCODE_X` keyevent — nothing enters; `input text x` — nothing enters; the on-screen keypad has no letter keys; `cmd clipboard set-text` has no shell implementation on API 35 so paste was unavailable. The box never left the valid `1500` state (`j47-c2-typedx.png`, `j47-c2-x2.png`). The invalid-typed state the case describes is unreachable through Android keyboard input — stated honestly rather than simulated. Nothing was queued while typing, trivially, because no invalid state ever existed.
3. Pressed Queue with `1500` still uncommitted in the box: the value settled first — the form showed committed `1,500.00` in the same frame as the stop banner (`j47-c2-queue1.png`). Queue stopped once with the same "1 item needs attention" fold (item 25 again — this return's cascade did not fill it; it is an editable money box and was set to 97.50, an invented positive value, to satisfy validation).
4. Second press → same line-of-business dialog (`j47-c2-dialog.png`) → Continue → sent. App "Submitted · read-only".
5. **Mock relay: exactly one** inbox file `…092026#test@example.com#.xml`; the 202-count over both cases went 2→3→4, one per return, no duplicates.
6. **Payload** (`case2-payload-amounts.txt`): **`txtTax14=1,500.00` — equal to what the box showed.** PASS.

## Verdicts

| case | verdict | payload vs box | relay log lines per return |
|---|---|---|---|
| 1 — type-then-queue without leaving | PASS | 15,000.50 = 15,000.50 | exactly 1 |
| 2 — invalid input / queue properly | PASS (letters rejected at input; invalid state unreachable on Android) | 1,500.00 = 1,500.00 | exactly 1 |

Shared behaviors proven in both cases: an uncommitted typed value settles to the committed formatted amount before Queue evaluates anything; Queue stops once with the note's fold when the return has a note, and a second press proceeds; nothing is ever sent twice.
