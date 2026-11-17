# Evidence

- HTA: `BIR-Form1600PTv2018.hta`, 368,605 bytes, SHA-256 `2f0f2d7e58763f0a57b4b62fbe830a39656e92e03916cb8e058e0805c981af22`.
- Official form: `1600-PT January 2018 ENCS final.pdf`, 572,536 bytes, SHA-256 `a7341728d072a70aafbea3291eec07573b111d058f0bc838ae0f58b80c41db84`.
- Official guide: `BIR Form No. 1600-PT 2018 Guidelines.pdf`, 151,443 bytes, SHA-256 `f3e9d796eb59afaffa9003bd06e7bcc0ff481907a1ac05b783326098c2d1da67`.
- Dummy representative save: `00000000000000-1600PTv2018-012025.xml`, 11,331 bytes, SHA-256 `da4301f5add3e4d3a870f5c28d7ed9cd851aed2e88e50375a7dfeb4e550beb77`.
- ATC catalog: 31 records from the pinned package catalog—6 Private slots and 31 Government slots because six `PG` records appear in both lists.

The observed save has 133 unique elements. The maximum ATC union adds selectors 7-31 and computation rows 6-31 for 262 unique field keys; ordered-key SHA-256 is `e9d2bb254555f57e5e851160cb7d3308221a4d23036802ed729487ea848cf743`. The HTA has 160 static controls and 316 maximum runtime controls.

`Help1600.hta` (SHA-256 `72425790b620970d00bd35ea9b22477d37742214dd2e5eb2198b1ca0e347d37d`) identifies legacy Form 1600, not 1600-PT, and is recorded only as a package mismatch.
