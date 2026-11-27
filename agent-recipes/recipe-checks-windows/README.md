driver: gpui-agent-cli 0.4.1
recipes: setup-taxpayer-ui.wants, setup-years-ui.wants, open-2551q-ui.wants, r1-2551q-schedule1-invalid-ui.wants, reset-form-ui.wants; the exact run-specific recipes are r1-attempt.wants, r2-data-setup.wants, r2-open-penalties.wants, and r2-type.wants under each runs/<case>-<run>/ directory

# Windows gpui-agent recipe checks

App: `hexuria/buwiz-forms` main `95b412777e23d695a15a7eb9d5098f06515bd700`, Windows test build. Scratch database: `C:\Users\Administrator\recipe-checks\scratch.db`. Demo taxpayer: Juana Santos, invented TIN `000-123-453-00000`; all values are invented. Safety variables were `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`; Send/Queue was never used and `--allow-send` was never passed.

All state-changing input was issued by `gpui-agent-cli 0.4.1` `.wants` recipes. No real keyboard/mouse input, UI Automation, or separate input rig was used. Read-only snapshots and scratch-database reads were used only to record outcomes.

Endpoint note: this build exposes the Native Shell surface control plane at `127.0.0.1:17421` and the app data-plane agent at `127.0.0.1:17422`. The surface endpoint rejects app invokes such as `form.open`; the data-plane endpoint has only a stub window tree. The R2 setup therefore used invoke-only recipes on the data-plane endpoint, while every measured UI interaction used surface recipes on the surface endpoint. Both were executed through the pinned `gpui-agent-cli 0.4.1`; no non-recipe input was used. The build also rejects `source=user` for `form.fill` (“reserved for boxes the user typed”), so the R2 setup recipes used `source=document`.

Recording: `recipe-checks-windows.mp4`.

## Results

| case | run | period | verdict | recorded result |
|---|---:|---|---|---|
| R1 Schedule 1 invalid amount | 1 | 2021 Q1 | known_gap: `hexuria/nsh-gpui-agent#13 sheet press` | stopped at `click fp-drpATC2-trigger` with ``error: `node 31078` is offscreen``; exact sentence untested (`null`); no `2551Q 2021 Q1` draft row existed at the stop point, so `row_exists_in_draft=false` |
| R1 Schedule 1 invalid amount | 2-5 | — | stopped | per instruction, the case stopped at the first hard gpui-agent gap; no fallback was used |
| R2 recomputed Item 20 surcharge | 1 | 2021 Q2 | passed | before `5.00`; after typed `9` at 120 ms: box `9.00`, saved `surcharge=9.0`; first key replaced the selection |
| R2 recomputed Item 20 surcharge | 2 | 2022 Q2 | passed | before `5.00`; after typed `9` at 120 ms: box `9.00`, saved `surcharge=9.0`; first key replaced the selection |
| R2 recomputed Item 20 surcharge | 3 | 2023 Q2 | passed | before `5.00`; after typed `9` at 120 ms: box `9.00`, saved `surcharge=9.0`; first key replaced the selection |
| R2 recomputed Item 20 surcharge | 4 | 2024 Q2 | passed | before `6.00`; after typed `9` at 120 ms: box `9.00`, saved `surcharge=9.0`; first key replaced the selection |
| R2 recomputed Item 20 surcharge | 5 | 2025 Q2 | passed | before `6.00`; after typed `9` at 120 ms: box `9.00`, saved `surcharge=9.0`; first key replaced the selection |

R2 conclusion: in all five runs the first paced key replaced the existing selected money-box text; it did not append.

## Evidence layout

- `run_strict_validation.py`: exact runner used for the recorded evidence.
- `runs/strict-validation-summary.json`: machine-readable combined summary.
- `runs/r1-2551q-schedule1-invalid-run1/`: exact R1 recipe, receipt, stderr/stdout, post-recipe snapshot, `report.json`, and `report.md`.
- `runs/r2-2551q-recomputed-money-type-run{1..5}/`: exact setup/open/type recipes, receipts, stderr/stdout, before/after snapshots, `report.json`, and `report.md`.
- The top-level mixed invoke+surface recipes `r1-2551q-schedule1-invalid.wants`, `r2-2551q-recomputed-money-type.wants`, and `reset-form.wants` are reference drafts only; they were not the executed evidence because this Windows build exposes the data plane and surface control plane as separate agent endpoints.
