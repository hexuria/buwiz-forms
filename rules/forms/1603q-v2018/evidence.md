# Evidence

- HTA: `BIR-Form1603Qv2018.hta`, 252,351 bytes, SHA-256 `823e4f7f06efc12326062da2565241d9c685db5fd82d8c04343e6dba301fa697`.
- Help: `Help1603Q.hta`, 28,254 bytes, SHA-256 `59a0d921ef0a7e7daffa459e4048fde895da63aa1e9cb75196c02008a59f8961`. Its HTML title says 1603, while the visible heading binds 1603Q January 2018 ENCS.
- Official PDF: `1603Q Jan 2018 final.pdf`, 875,639 bytes, SHA-256 `e18b6bfd755bc02c8be1d2413cbad30a355b652e871f6dee720e5f76975abbae`.
- Dummy representative save: `00000000000000-1603Qv2018-2025Q1.xml`, 5,546 bytes, SHA-256 `b2b1d8c7aea0bf052150ba91c58875bcbdec756c7e47ff9dd3aa24193a7f4d54`.

The 76 XML keys are unique and ordered-key SHA-256 is `6a2659a7294da950807df4c8a6a91eedf296ef30915a0942dd35f29eea4c6e78`. The HTA has 125 static controls; the injected RDO select gives a maximum of 126.

The Schedule 1 zero-total defect is proven across three pinned sources: `computeTotalSchedule` passes a literal ID string, `NumWithComma` applies `parseFloat`, and `formatCurrency` converts NaN to zero.
