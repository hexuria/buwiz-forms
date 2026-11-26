# Confirming run b4b0fc11 results

Build/commit verified: buwiz-forms `b4b0fc1156842e69041e721f6990288db0c0b1bf`; sibling nsh `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`. Fresh target `target-b4b0fc11`, fresh scratch DB, window 900x700, safety env `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`; no Queue, no live submit, no suites.

- RDO: PASS. On a new tax profile, clicked into the RDO combobox, typed `0`, scrolled the filtered list to its end, pressed Enter (selected `001 — Laoag City`), reopened, scrolled the full list to its end, and clicked the final visible row. The clicked row was correctly committed as `93B — Zamboanga Sibugay`.
- ZIP: PASS. On a new tax profile, clicked into the ZIP combobox, typed `1`, scrolled the filtered list to its final row (`0410 — Radio Bible Class`), pressed Enter (committed the hover-highlighted row `1006 — Binondo`), reopened the full catalog, scrolled it to the final row, and clicked `0560 — Far Eastern Broadcasting`; the clicked row was correctly committed.

No focus loss after picking, misplaced list, dead keys, note overlap, or recycled-row wrong-check behavior was observed in these two confirming flows. The ZIP mp4 is a dead-time-trimmed version of the genuine screen capture because scrolling the very long ZIP catalog with physical wheel input took several minutes; the interaction frames themselves were not altered.
