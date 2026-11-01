# Changelog — Offline eBIRForms (local observations)

Notes on official Offline eBIRForms package behavior that this checkout
had to work around for dummy-profile Save discovery. These are **not**
print-parity defects and **not** catalog remints.

Local patches, if any, apply only to the extracted HTA/JS tree under
`%TEMP%\{GUID}\`. Packed `C:\eBIRForms\BIRForms.exe` stays untouched.
Reload/re-extract after a package restart; GUID paths are ephemeral.

---

## 7.9.6.1 — dummy TIN prefixes force-failed in frontend (2026-08-20)

Observed on this PC:

- `C:\eBIRForms\BIRForms.exe` product **7.9.6.1**, 58,411,008 bytes,
  sha256 `a43a4599f95158e6ba0e7a1c4b88c4e2cf215ac86e53c24259cc69d1b664829c`
- Form manifests still pin 7.9.6.0 / `de8ef081…`. HTA hashes still matched
  the 7.9.6.0 pins; the dummy-TIN change is in extracted JS, not those HTAs.

### What changed

Fill-up / profile Save does **not** accept any 3-3-3-5 string. Path:

1. Length: `tin1`/`tin2`/`tin3` = 3 digits each; `tin4` (branch) = 3, 4, or 5.
2. `999-999-999` dummy exception: branch must be `00000`, `00123`, `123`,
   `000`, or `99999` (`BIRForms.hta` `checkValidDummy` / `newForm`).
3. Check-digit: `getTinChkCode(tin1,tin2,tin3)` concatenates **only the first
   nine digits** and calls VBS `ValidateTinWChkDgt` → `chkt.exe`. Exit 0 = ok.
   `tin4` is not in the checksum. Random prefixes such as `123123123` fail
   here (`You have entered an incorrect TIN`).

After `chkt.exe`, extracted `js/string-util.js` `getTinChkCode` used to
**force-fail** three prefixes (comment was `DELETE FOR TESTING ONLY`):

```javascript
if (tinNumber === '999999999' || tinNumber === '222222222' || tinNumber === '000000000') {
    return 1; // invalid
}
```

That is why dummy `000-000-000-00000` started failing even though branch
`00000` is allowed. Packed `BIRForms.exe` does **not** contain these ASCII
strings; the gate lives in the extracted JS.

### Local patch (this PC only)

File: `%TEMP%\{0B33C1CE-21A8-44A1-8D91-28A10444A6A3}\js\string-util.js`

Changed the dummy-prefix override from `return 1` to `return 0` so
`000-000-000-00000` can Save locally. Packed exe not modified. Reload
Offline eBIRForms after extract so the HTA loads the patched JS.

Do **not** use a personal TIN. Do not expect `123-123-123-12345` to work;
`chkt.exe` still rejects prefixes that fail the check-digit.
---

## 7.9.6.1 — 2200A loadBGData gated on profile formType (2026-08-20)

`forms/BIR-Form2200Av2020.hta` `loadBGData()` hydrates TIN / name / RDO / address **only** when the dummy profile's `formType` equals `2200Av2020`. A profile saved from another form (for example `formType=2000v2018`) still opens 2200A, but Item 4 TIN and Item 6 name stay empty and locked.

Local workaround for dummy Save discovery: set `C:\eBIRForms\profile\00000000000000.xml` `formType=2200Av2020`, then F5 the open 2200A HTA. Packed exe untouched.

After a 2200A local Save, the form rewrote that same dummy profile's `formType` to `2200Tv2020`. Later Fill-up of 2550M / 1600WP should not assume the profile still says `2200Av2020`.

Also observed on 2200A: occ2/occ3 TIN boxes use `name=tinA_2` / `tinA_3` but **share** `id=frm2200Av2020:tinA`. `saveXML` serializes `elem[i].id`, so dummy Save emitted `frm2200Av2020:tinA` three times and never `tinA_2`.
