# Job 36 — money box key-by-key typing on Android (2551Q Schedule 1, Taxable Amount)

## Header

- App: hexuria/buwiz-forms main at `47ab502574b2d5f47d2eae5bbf7fa62df8a2edfc` (the main tip when built; `47ab5025` "print: An Android print draws t…"), folder `~/BuwizNsh/target-47ab5025`.
- Sibling nsh, tool and both runtimes: `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf` (the pin in that commit's `.github/workflows/nsh.yml`), real copies in `~/BuwizNsh/tools`.
- Runtime classes.dex sha256 (dev runtime the build used): `73b3126db70bdca7e03f0ee3acb39c63437efc1f606c7232d0324445a1788e17`.
- Build: `nsh run --target android` debug dev host, the usual debug test build. Safety variables `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_SFTP_DRY_RUN=1 BIR_AUTH_BYPASS=1` baked in. Demo taxpayer Juana Santos, invented values, never Queue.
- Device: emulator-5554, API 35, 1080x2400, density 420, gesture navigation, "Show taps" on.
- Path: Forms → New return → 2551Q → Schedule 1 – Computation of Tax → row → Taxable Amount box. All digits typed with the app's on-screen keypad only, one key per real tap, 150 ms hold-taps (`input swipe x y x y 150`). No `input text`, no agent typing.

## VERDICT: YES — every saved value differs from what was typed.

| # | typed | shown after leaving the box | saved (reopened draft) |
|---|-------|------------------------------|------------------------|
| 1 | `99999999.99` | `99,999,999,990.00` | `99,999,999,990.00` |
| 2 | `1000000.005` | `10,000,000,050.00` | `10,000,000,050.00` |
| 3 | `12345678.90` | `12,345,678,900.00` | `12,345,678,900.00` |

## Caret behavior while typing (per-key values, case 1 — cases 2 and 3 identical pattern)

```
key 9     → 90.00
key 9     → 990.00
key 9     → 9,990.00
key 9     → 99,990.00
key 9     → 999,990.00
key 9     → 9,999,990.00
key 9     → 99,999,990.00
key 9     → 999,999,990.00
key .     → 999,999,990.00      (no change; the decimal point never engages)
key 9     → 9,999,999,990.00
key 9     → 99,999,999,990.00
```

Reading: the field starts at `0.00`. The first digit lands BEFORE the existing zero
(`9` → `90.00`, not `9.00`), and every further digit is inserted before the last
digit of the integer part — the caret sits one place early at all times, commas or
no commas. The comma formatting itself is correct for the (wrong) digits. The `.`
key does nothing visible, and digits typed after it still land in the integer part,
again before the trailing zero — the decimal part can never be entered. One extra
observation: the keypad's backspace key never deletes anything in this state
(repeated taps left `99,999,999,990.00` byte-identical; consistent with the caret
being pinned at/one-after the start so there is nothing before it). To clear the
field between cases the whole row had to be deleted and re-added.

Per-key full screenshots after every keypress are in `caseN/key-NN-k.png`;
`caseN/after-leave.png` is the shown value after tapping Done; `caseN/saved-reopen.png`
is the value read back after saving and reopening the draft. `case1/case1-typing.mp4`
records the first entry key by key with taps shown.

No step failed or needed a re-run. Harness notes: (1) clearing a mistyped value is
impossible with backspace, so cases 2 and 3 used fresh rows; (2) the row's ATC was
left unset in cases 2 and 3 — the measurement is the money box itself, and the tax
rate does not affect typing.
