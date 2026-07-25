# Evidence

## Revision binding

- Installed HTA: `C:\Users\uriah\AppData\Local\Temp\{68379049-6E6C-4478-A2C5-EF7B77ED7E38}\forms\BIR-Form1702Qv2018C.hta`, 674,724 bytes, SHA-256 `7184de87a76401da98da3df38dab9e29f848acf6150b425e36574f0a2443ab01`.
- Installed help: `Help1702Qv2018.hta`, 27,102 bytes, SHA-256 `74e101d8db53526799bcdc59c8777c6872e31a9420335b325b9a94fa018c8606`.
- Official PDF: `1702Q 2018ENCS final2.pdf`, 1,218,152 bytes, SHA-256 `589e22190b9211571cb8a0ba14c97c17dff250f3b8ee9f9e8f6cc3b37b1b1be4`.
- Read-only representative save: `00000000000000-1702Qv2018C-2025Q1.xml`, 7,852 bytes, SHA-256 `d97b1e9ba64dab51ab43c56a1e7058a80468730aa0aa8c61ab0e3b7c8c2d71ce`.

The HTA title, help title, printed header, PDF, and serialized `formTyp` agree on January 2018. The trailing `C` is a runtime/file identity detail. Package metadata reports 7.9.6.0; the HTA's internal constant reports `7.9.3.b`, and both are retained rather than normalized away.

## Extraction

`extract-1702q-controls.ps1` inventories the `frmMain` controls in DOM order. `extract-1702q-fields.ps1` assigns stable occurrence-aware keys. The representative save contains the same 113-element order, with ordered serialized-key SHA-256 `a5e56fa2a7e6fb528e8f5dbe03735c0508fac57f4c724d92a63ea52f578334e4`.

The sole duplicate serialized key is `frm1702q:txtTelNum`: the printed Item 10 contact control and a hidden profile contact control. `fields.json` represents these as `#occurrence-1` and `#occurrence-2` while preserving the shared serialized key.

Validation rules were transcribed from `validate`, `initialValidateBeforeSave`, field event handlers, and date/year helpers. Calculations were independently organized from the schedule and Part II functions. Workflow states come from `saveXML`, `saveXMLsubmit`, `openAlertEmail`, and `sendEmail`.

## Instructions

The installed help states that the return, with or without payment, is filed within 60 days after the close of each of the first three quarters. It lists BIR Forms 2304 and 2307, approved tax-debit memo, tax-treaty-relief certificate, SAWT, and proof of other payments as applicable attachments. Online filing deadlines remain subject to applicable issuances.

## Runtime probe

Windows Script Host JScript confirmed that this legacy engine ignores modern `toLocaleString` currency options: `1234.5` formats as `1,234.50`, not with a currency symbol. No online submission, Final Copy confirmation, encrypted-copy creation, or mutation of the representative save was performed.
