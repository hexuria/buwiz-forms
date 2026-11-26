# Official upload keys for rows past the paper

Source: `hexuria/buwiz-validation` at `ea8334a`, byte-exact official Offline eBIRForms package as recorded by `try/official-print-past-paper` `1bb90f61`.

- `package/forms/BIR-Form1601Cv2018.hta` SHA-256: `3fb7b4185264e47c9d77b0def4301fa696e7b4424ad30b4e973c7c0b1f759879`
- `package/forms/BIR-Form2200Av2020.hta` SHA-256: `1df302eeb1352eccb88f6aa7a23fdcc185b6fbb4d15435996250f985a0198e2c`

## Serializer rule

2200A `saveXML` iterates `document.getElementById('frmMain').elements` and emits every text/select control it finds (`BIR-Form2200Av2020.hta:4883`, elements loop at `:4902-4905`). 1601C uses the same package serializer pattern (`saveXML` starts at `BIR-Form1601Cv2018.hta:2201`). Therefore the keys are exactly the ids/names created by the row builders; there is no row maximum in these builders.

## 1601C Schedule I — zero-based, continuous

`addSchedule` loops `for(i = 0; i < schedule1ToCommit.length; i++)` at `BIR-Form1601Cv2018.hta:3369` and creates these ids with `+ i +` — no cap:

| suffix | source line | row 4 key (`i=3`) | row 5 (`i=4`) | row 10 (`i=9`) |
|---|---:|---|---|---|
| `txtMonthYear` | 3372 | `frm1601c:sched1:txtMonthYear3` | `frm1601c:sched1:txtMonthYear4` | `frm1601c:sched1:txtMonthYear9` |
| `txtDatePaid` | 3373 | `frm1601c:sched1:txtDatePaid3` | `frm1601c:sched1:txtDatePaid4` | `frm1601c:sched1:txtDatePaid9` |
| `txtBankCode` | 3374 | `frm1601c:sched1:txtBankCode3` | `frm1601c:sched1:txtBankCode4` | `frm1601c:sched1:txtBankCode9` |
| `txtNumber` | 3375 | `frm1601c:sched1:txtNumber3` | `frm1601c:sched1:txtNumber4` | `frm1601c:sched1:txtNumber9` |
| `txtTaxPaid` | 3376 | `frm1601c:sched1:txtTaxPaid3` | `frm1601c:sched1:txtTaxPaid4` | `frm1601c:sched1:txtTaxPaid9` |
| `txtShouldTaxDue` | 3381 | `frm1601c:sched1:txtShouldTaxDue3` | `frm1601c:sched1:txtShouldTaxDue4` | `frm1601c:sched1:txtShouldTaxDue9` |
| `txtAdjustments` | 3382 | `frm1601c:sched1:txtAdjustments3` | `frm1601c:sched1:txtAdjustments4` | `frm1601c:sched1:txtAdjustments9` |

## 2200A Schedule 1 Others — one-based, continuous

`sched1Fields()` appends `sched1Index` and increments it at `BIR-Form2200Av2020.hta:7576-7605`; no cap. Every control has both `id` and `name` with the same key:

| suffix | source line | row 4 key | row 5 key | row 10 key |
|---|---:|---|---|---|
| `Check` | 7582 | `frm2200Av2020:txtSched1_xa_others4_Check` | `frm2200Av2020:txtSched1_xa_others5_Check` | `frm2200Av2020:txtSched1_xa_others10_Check` |
| `Atc` | 7583 | `frm2200Av2020:txtSched1_xa_others4_Atc` | `frm2200Av2020:txtSched1_xa_others5_Atc` | `frm2200Av2020:txtSched1_xa_others10_Atc` |
| `Desc` | 7586 | `frm2200Av2020:txtSched1_xa_others4_Desc` | `frm2200Av2020:txtSched1_xa_others5_Desc` | `frm2200Av2020:txtSched1_xa_others10_Desc` |
| `Bracket` | 7589 | `frm2200Av2020:txtSched1_xa_others4_Bracket` | `frm2200Av2020:txtSched1_xa_others5_Bracket` | `frm2200Av2020:txtSched1_xa_others10_Bracket` |
| `Rate` | 7592 | `frm2200Av2020:txtSched1_xa_others4_Rate` | `frm2200Av2020:txtSched1_xa_others5_Rate` | `frm2200Av2020:txtSched1_xa_others10_Rate` |
| `Export` | 7595 | `frm2200Av2020:txtSched1_xa_others4_Export` | `frm2200Av2020:txtSched1_xa_others5_Export` | `frm2200Av2020:txtSched1_xa_others10_Export` |
| `Taxable` | 7598 | `frm2200Av2020:txtSched1_xa_others4_Taxable` | `frm2200Av2020:txtSched1_xa_others5_Taxable` | `frm2200Av2020:txtSched1_xa_others10_Taxable` |
| `Due` | 7601 | `frm2200Av2020:txtSched1_xa_others4_Due` | `frm2200Av2020:txtSched1_xa_others5_Due` | `frm2200Av2020:txtSched1_xa_others10_Due` |

`Check` is a checkbox and is not serialized as a text/select value by the quoted serializer; it is still part of the official control family and is listed here for completeness.
