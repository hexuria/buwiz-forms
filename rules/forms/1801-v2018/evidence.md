# Evidence

- January 2018 HTA SHA-256: d19c4ebe40ed109094ef43687ed5486a003bd11a67a251bdd5545a41c2f46d8d; APPLICATIONNAME 1801v2018; printed January 2018 ENCS.
- Revision-matched help SHA-256: 21fc91fffadb99b78f9495cca3c25e8ba14e76a4467a181adbd73ef5607c68f5; identifies January 2018, the 6% rate, one-year deadline, and documentary requirements.
- Official form PDF SHA-256: ec49207aab9b035d1913d41091b677d9df690e01b391ed2c2f4c34cf43a524c6; guidelines PDF SHA-256: 06cbd878536d2960ef556fbfb29a23e9a58896f1ae4e43623a6f389c916f7e0a; valid PDF magic.
- Live DOM inventory: 157 static controls; 137 static serializable controls; one runtime-injected RDO select.
- Runtime projection: 138 concrete definitions plus 37 indexed families; two new-form rows per family produce 212 baseline serializer entries.
- Legacy plaintext SHA-256: cd1ceb8cfb2e1daac21f0e948c25b0ba62e7d4cdf8a0e4a73710daaf96ac7001; 111 unique keys, 104 with frm1801 prefix; inventory c82bc9762711506ac134187ca823581528fd4c163efbe31ddabd73ee01dde9ef. Legacy encrypted SHA-256: 6ab3445921227b0537d9370bd08fdf92672fbada6edf781fcac86f74684ea603. Both are excluded.
- The malformed Item 27 control ID frm1801v2018taxkNum is source-proven at line 1153 and is serialized literally.
- No existing typed 1801 model was found under crates/bir-core/src/forms.