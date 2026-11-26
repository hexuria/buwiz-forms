# PR #403 money box: press-Queue cases in a real Mac window against the local mock relay

- App commit: 542e484401494623a7ccbae9e25832985739a3a8 (detached worktree, DEBUG build)
- `nsh --version`: nsh 0.6.1 (8c78cfd); ../nsh 8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf
- cargo 1.99.0 (5f94df478 2026-08-27), rustup; build folder ~/BuwizNsh/target-q403 (job folder, deleted after)
- Run: justfile `nsh-run` recipe, scratch database under the job folder, `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`, `BIR_RELAY_URL=http://127.0.0.1:8787`. Never `BIR_SFTP_LIVE`.
- Mock relay: `just mock-relay` shape (`cargo run -p bir-core --features mock-relay --bin mock_relay`), confirmed listening on 127.0.0.1:8787 before the first Queue (`/v1/health` returned `"mock":true,"ok":true`).
- Taxpayer: invented — Juan Dela Cruz, TIN 123-456-788 branch 00000. Form: 1601C monthly.
- Input: money boxes reject synthesized input, so every amount was typed with real keyboard input into the real window, as a person types. Buttons (Queue, radios, dialog) were pressed through the gpui-agent control plane, the same press the headless tests use.
- Non-loopback check: the app process's only TCP connections were loopback (dev-server websocket 127.0.0.1:8790, agent listeners 127.0.0.1:17501/17502); no UDP flows; the app log names no non-loopback host. Nothing reached the BIR or api.buwiz.com.
- Payloads: the relay stores the encrypted IAF byte for byte. Excerpts here are the decrypted form fields (the encryption is the BIR-mandated protocol constant, not a secret); no tokens exist in them.

## Case 1 — Queue with a typed amount still pending: PASS

1601C for October 2026 filled to pass Check except item 14 (Total amount of compensation). Clicked the box, typed `15000.50` with real keys, did not leave the box (case1-1-typed-pending.png), pressed Queue.

- The typed value settled first: no note, no stop — Queue went straight on, because the amount was valid.
- The confirm dialog "Queue this return?" opened (case1-2-queue-confirm.png, warning that line of business is not set in the profile); Continue queued it.
- The return is queued: toast "Queued for background submission", then "Submitted · read-only — Sent to the mock relay" (case1-3-submitted.png).
- The mock relay's payload for the return: **frm1601c:txtTax14=15,000.50** — the queued amount equals what the box showed (case1-payload-excerpt.txt).

## Case 2 — Queue while the box shows an unreadable text: PASS

Fresh 1601C for September 2026, item 14 typed as 8000 and settled (box showed 8,000.00). Went back into the box, selected all, typed `1500x` — the box showed "1500x", still focused (case2-1-typed-1500x.png). Pressed Queue.

- The box went back to the return's amount 8,000.00 and showed its sentence: toast and box message "Item 14: 1500x is not an amount. The box kept 8,000.00." (case2-2-stopped-note.png).
- Queue stopped once: no confirm dialog opened on that press.
- NO return was queued while the box showed "1500x": the relay inbox held only the Case 1 file.
- Second press of Queue went on to "Queue this return?" (case2-3-second-press.png); Continue queued it (case2-4-submitted.png).
- The payload of what was then queued: **frm1601c:txtTax14=8,000.00** — exactly what the box showed; the string "1500x" appears nowhere in it (case2-payload-excerpt.txt).
