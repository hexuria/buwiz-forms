driver: gpui-agent-cli 0.5.0 + nsh-gpui-agent 4c009c716a86df1c1b11a939baa47dc63c432abb

# Plug #28 scroll-clear proof

App `f030fbfd63a47cf2b322ffec6598122ad9fd6936`, sibling NSH `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`, cargo `1.99.0`, Android dev test build on `emulator-5554`. The job-58 recipe overlay and the five direct plug dependency rev edits in `nsh/Cargo.toml` were local and uncommitted; cargo wrote the lock locally. Safety env was `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`; no `--allow-send`.

## Results

- (a) normal scroll-clear, `money-boxes-paced-phone` without job 58 step `s13p`: pass. Exact step: `s14` click `fp-frm2551Qv2018:amendedRtn_2` passed with result `{"scrolled": 1}`. Before `normal/money-boxes-plug28/018-s13.png`, after `normal/money-boxes-plug28/022-s14.png`, recording `normal/money-boxes-plug28/recording.mp4`, receipt beside them.
- (b) real modal-sheet cover: the birth-date sheet stayed open and fixed chrome target `nb-search` behind its modal backdrop refused. Exact step: `s13` click `nb-search` failed `occluded_by: n395`, error code `host_error`, delivery `delivery_uncertain`; no `after n scroll(s)` wording because it did not scroll. Before `normal/modal-cover-plug28/011-s11.png`, after `normal/modal-cover-plug28/013-s13.png`, receipt beside them.
- (c) strict opt-out `GPUI_AGENT_PRESS_STRICT=1` compile/runtime environment: rerun of (a) refused exactly as before. Exact step: `s14` click `fp-frm2551Qv2018:amendedRtn_2` failed `occluded_by: fp-pill-list (its n4120)`. Receipt/report in `strict/`.

The opt-out is `GPUI_AGENT_PRESS_STRICT=1`, not `as_written`: gpui-agent 0.5.0's `as_written` is client-side legacy-protocol negotiation and is not visible to the plug per request.
