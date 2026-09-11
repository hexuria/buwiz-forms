# SFTP submission restore

Buwiz can submit to BIR again. Official 7.9.6.x moved from FTP to dispatcher-backed SFTP. This folder is the write-up of that restore.

Read in this order:

1. [STUDY_WALKTHROUGH.md](STUDY_WALKTHROUGH.md) — how we got here, step by step, for study later.
2. [EBIRFORMS_SFTP_SUBMISSION_FINDINGS_2026-09-11.md](EBIRFORMS_SFTP_SUBMISSION_FINDINGS_2026-09-11.md) — evidence tables, hashes, IL, protocol diff.

Live proof used dummy TIN `00000000000000` / form `1601Cv2018`. BIR later emailed a confirmation. Credentials are fetched at runtime from `tinDispatcherSFTP.php` and are not stored in git.

Harness:

```
cargo run -p bir-core --bin sftp_harness -- --dry-run
cargo run -p bir-core --bin sftp_harness -- --live-connect
cargo run -p bir-core --bin sftp_harness -- --live-put
```

## Env override and dry-run

Default is still `tinDispatcherSFTP.php`. To submit without production BIR
credentials:

1. Point `BIR_SFTP_*` at a lab or local SFTP (real protocol, not BIR). Port
   defaults to 22. This is the mock-server path; we do not ship a first-party
   mock SFTP daemon here.
2. Set `BIR_SFTP_DRY_RUN=1` (or `BIR_SFTP_LIVE=0`) for an in-process fake PUT
   with no TCP. Cron still marks Submitted but skips IMAP.

See `.env.example` and [STUDY_WALKTHROUGH.md](STUDY_WALKTHROUGH.md). Do not
commit real passwords. Dry-run is not a BIR filing.
