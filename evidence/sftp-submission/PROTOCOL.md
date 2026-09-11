# SFTP protocol notes (consolidation)

These notes sit on PR #41’s transport. They pull verified facts from PR #42’s
investigation without adopting that PR’s dual-trait module, `ws1` backup, or
“ftp2 is production” assumption.

## Live host vs DEV/UAT vector

- Live dummy-TIN dispatcher `server` unwraps to **`ebf2.bir.gov.ph`** (SSH/SFTP :22).
- `ftp2.birgovph.com` is a DEV/UAT wrap vector, not the production PUT host.
- Backup dispatcher in this tree is **`ws2.birgovph.com`**. `ws1` reconstructed
  from JS returns HTTP 404 today; do not switch production wrap to ws1.

## One production PUT path

`submit_iaf` → `resolve_sftp_endpoint` → russh PUT `/{formType}/{basename}`.

Resolve order:

1. `BIR_SFTP_DRY_RUN=1` or `BIR_SFTP_LIVE=0` → in-process fake PUT.
2. Complete `BIR_SFTP_*` (incomplete = config error, no fallthrough). Lab
   host-key must be pinned or `BIR_SFTP_ACCEPT_ANY_HOST_KEY=1`.
3. Deprecated `TEST_SFTP_*`.
4. Production `tinDispatcherSFTP.php` **only** with `BIR_SFTP_LIVE=1`.
   HTTPS is tried first; official ebfSFTP is HTTP, so HTTP is the fallback.
   Official host-key policy remains accept-any.

Loopback / Static / Mock are **injected** at tests (`submit_iaf_with_endpoint`,
cron `SubmissionTransport`). They are not a second production trait.

## Filename / remote path

IAF basename splits on `/` and `\`, so a Windows absolute dummy path still
yields the file name on Linux CI. `#email#` is stripped before period parse.

## Unwrap

Dispatcher field unwrap keeps the official UTF-8 **BOM** (PR #41). Do not
merge PR #42’s “no BOM” wrap as the production path.

## Loopback test

In-crate `sftp_loopback` (adapted from PR #42’s evidence harness) PUTs through
`submit_iaf_with_endpoint` against an in-process russh server. It is not a
standalone evidence crate.
