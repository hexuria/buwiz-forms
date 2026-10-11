# Forms push handover (2026-10-11)

Session scratchpad from the "all 43 BIR forms" run (session 75a3a4d5-f426-4ca2-af38-3fdce8d073d7), saved before the 24h sandbox cap.
These files are working tools and agent briefs, **not product code**. They are not built or tested, and they are not meant to merge into `main`.

- `handover.html`: the visual handover (also published as an artifact).
- `tools/`: merge helpers (`merge_branch.sh`, `fix_registry.py`, `resolve_merge.py`, …), `verify_final.sh` (the full verify), `check_print.py`, `sync_capabilities.py`, `official_encrypt_sha.py` (Encrypt.exe pin) and `env.sh` (aarch64 sandbox build env: zig cc shim, sysroot, etc.). Paths inside are absolute to the old sandbox; adjust them.
- `vnc/`: run the desktop app under Xvfb + noVNC: `setup_runtime.sh` (no-root GTK/WebKit/Mesa download), `build_app.sh`, `run_review.sh` (isolated :98 / noVNC 6081), `nosve.c` (LD_PRELOAD so llvmpipe doesn't emit SVE), `xdo98`/`shot98` helpers. Agent briefs: `NAV.md` (how to drive the app), `BATCH.md` (render-check steps), `SWEEP.md` (phone-width label fix).
- `audit/`: `AUDIT.md` (schema), `forms/*.json` (295 grounded findings, one file per form), `gen_report.py` (builds the audit artifact).
- `briefs/`: the 2550Q unfreeze brief and the PR descriptions.

See the tracking issue for status, open questions and links.
