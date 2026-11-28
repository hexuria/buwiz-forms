driver: gpui-agent-cli 0.5.0

# Linux real-app amount rollback proof

Run on the Linux desktop app in the dry-run test environment (`BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`), with a scratch database and the demo taxpayer. No Queue, Send, Submit or File step was run.

The 2551Q Schedule 1 row-1 Taxable Amount cell first held `1,234.56`. The recipe cleared it, typed `1234567890123` one key at a time with `key_delay_ms: 450`, then pressed Tab. The slow typing step took 5,784 ms before and 5,785 ms after.

- Before (`358331c7`, a main descendant of `e9f34205`): the cell held the wrong committed prefix `123,456,789,012.00`; the note under it says the return holds that prefix, and Tax Due became `3,703,703,670.36`. Suite result: 5 pass.
- After (`14b653f6`, `try/amount-notes-f-round`): the cell rolled back to `1,234.56`; the note under it says the return holds `1,234.56`, and Tax Due stayed `37.04`. Suite result: 5 pass.

`before.png` and `after.png` are the screenshots gpui-agent wrote at the final semantic assert. `before-note.png` and `after-note.png` crop the amount cell and refusal note from them. The receipts, reports, run logs and exact suite/recipe files are beside them.

known_gap: the row-drawer refusal sentence has no stable gpui-agent target (`fp-boxmsg-txtATCAmt1` is not emitted for table-cell messages), so the recipe semantically asserts the amount value and the refusal sentence is proved by the screenshots rather than by a semantic text assert.
