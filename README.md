driver: gpui-agent-cli 0.5.0 + nsh-gpui-agent b0c1a5f2e0a23d9b37a8c3059fac52c0c9feae0b

Apple proof for hexuria/buwiz-forms#584 at head b3bddd4d252d0daac05b322f18ce6e45e6c1fa18 (macOS desktop, smoke suite via scripts/e2e/machines/macos.sh, dev test build, scratch database, dry-run send).

`gpui-agent --version`: gpui-agent-cli 0.5.0 (prebuilt route). Note: report.json's cli_version reads 0.4.1 — the runner parses cargo install metadata, which is stale on a prebuilt install; the binary is 0.5.0.

Evidence layout: runN/smoke = the smoke suite report; runN/rdo-row-press = the second suite the machine script runs. No absolute local paths here: ~ means the builder's home.
