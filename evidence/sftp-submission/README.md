# SFTP submission restore

Buwiz can submit to BIR again. Official 7.9.6.x moved from FTP to dispatcher-backed SFTP. This folder is the write-up of that restore.

Read in this order:
1. [STUDY_WALKTHROUGH.md](STUDY_WALKTHROUGH.md) - how we got here, step by step, for study later.
2. [HOW_WE_GOT_THE_SFTP_CREDENTIALS.md](HOW_WE_GOT_THE_SFTP_CREDENTIALS.md) - how dispatcher credentials were recovered into local .env. No passwords in git.
3. [EBIRFORMS_SFTP_SUBMISSION_FINDINGS_2026-09-11.md](EBIRFORMS_SFTP_SUBMISSION_FINDINGS_2026-09-11.md) - evidence tables, hashes, IL, protocol diff.
4. [PROTOCOL.md](PROTOCOL.md) - live host `ebf2` vs ftp2, ws2 vs ws1, one PUT path, HTTPS-then-HTTP dispatcher.

Live proof used dummy TIN `00000000000000` / form `1601Cv2018`.

- Automated tests: period **092026** (September 2026) zero-tax.
- Live PUT + BIR receipt (11 September 2026, PR #41 / `68f773c5`): period
  **102026** (October), file `00000000000000-1601Cv2018-102026.xml`.
  Transport is empirically PASS.

Credentials are fetched at runtime from `tinDispatcherSFTP.php` and are not
stored in git.

Harness:

```
cargo run -p bir-core --bin sftp_harness -- --dry-run
cargo run -p bir-core --bin sftp_harness -- --live-connect
cargo run -p bir-core --bin sftp_harness -- --live-put
```

## Env override and dry-run

Production `tinDispatcherSFTP.php` requires `BIR_SFTP_LIVE=1`. Unset refuses
dispatcher. To submit without production BIR credentials:

1. Point `BIR_SFTP_*` at a lab or local SFTP (real protocol, not BIR). Port
   defaults to 22. Lab hosts need `BIR_SFTP_HOST_KEY_SHA256` or
   `BIR_SFTP_ACCEPT_ANY_HOST_KEY=1`.
2. Set `BIR_SFTP_DRY_RUN=1` (or `BIR_SFTP_LIVE=0`) for an in-process fake PUT
   with no TCP. Cron still marks Submitted but skips IMAP from the **session
   source**, not from a process-global env flag.

See `.env.example` and [STUDY_WALKTHROUGH.md](STUDY_WALKTHROUGH.md). Do not
commit real passwords. Dry-run is not a BIR filing.
