# Durable filing queue and auth scope

Enqueue is already shipped on `codex/sftp-on-pr39` (`7151220a`, `68f773c5`).
This note is the worker/completion layer on top of those routes. It does not
add a second queue verb.

## One task, three surfaces

See [AGENT.md](AGENT.md) “Three ways to queue”. All three write the same `form_drafts`
Queued row:

1. Grok Bot / `bir-headless` with `filing.queue` `confirm=true` (JSON boolean),
   optionally passing `tin` / `code` / `year` / `period`.
2. Painted desktop Submit.
3. Cron executing an **already-queued** row. Cron never invents a queue.

Headless executes authorized work. Interactive approve stays GUI or agent
`confirm=true`. There is no blanket permission.

## Scope

`QueueAuthorization` is form + TIN + period + completion=`Confirmed`. It is
written by the existing `transition_to_queued` path. A worker that sees a
mismatched or missing grant fails closed back to Draft.

Retries of the **same** authorized task do not re-ask. Cancel / revert clears
the grant. Max attempts (5) also clear it.

## Claim lease vs fail-closed

1. Open SFTP (or dry-run session) **before** claim.
2. Claim writes token, `claimed_at`, and a 120s lease. PUT has not started.
3. Persist `submission_put_started_at` **immediately before** PUT.
4. PUT Ok → Submitted. Live source schedules IMAP. Dry-run source skips IMAP.
5. PUT Err or crash after PUT start → fail-closed (human
   `abandoned_no_bir_filing`). Bytes may have reached BIR.
6. Crash **before** PUT start, after lease expiry → recover the same authorized
   task and retry. Legacy claims with no lease stay fail-closed.

Do not blind re-PUT after an unknown mid-transfer.

## Eventual completion

Live 1601-C and 2551Q: Submitted → Confirmed when IMAP matches the exact
filename/TIN/period and the receipt is not older than `submitted_at`. Dry-run
stops at Submitted. IMAP skip is taken from the **session source**, not from
process-global `BIR_SFTP_DRY_RUN` (parallel tests must not leak).

## Secrets

`BIR_SFTP_*` env only. Never commit, print, or log passwords. Production
dispatcher requires `BIR_SFTP_LIVE=1`. Lab hosts need a SHA-256 pin or
`BIR_SFTP_ACCEPT_ANY_HOST_KEY=1`.
