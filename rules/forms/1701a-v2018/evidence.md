# Evidence

- Exact HTA SHA-256: c85f8f1e13e4752549cb2886bc598a0be4cf8b67a709d9a913d09d22f087de1b; APPLICATIONNAME 1701A, version 4.7, printed January 2018 (ENCS).
- Two plaintext saves independently contain the same 194-key set; hashes are pinned in the manifest.
- Encrypted companion SHA-256 2f77216933ad96fce43bd2da82e782e15e74d987c4159ff50f1dc60a5e74bb57 replays in memory to SHA-256 0bd7b97fcac70faee523c8a2841b8e95591d64c2d917f117b8728743ccdfa9b9, 195 unique keys, version 11112018, adding frm1701A:txtAddress2. No values are emitted.
- Official PDF SHA-256: 8d492eabc6da2088cf9a55084488b192def5cc415048f607142c8bce1b72bfb8 and valid PDF magic.
- Shared js/lib/1701.js is hashed and inventoried; it contributes population/transport mapping rather than additional local validation alerts.
- js/gserializer.js is referenced but absent from the extracted package.
