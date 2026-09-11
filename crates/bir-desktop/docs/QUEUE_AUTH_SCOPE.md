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
`confirm=true`. There is no blanket permission. `confirm=true` is a
**caller assertion**: the host records it as the authorization source but
cannot verify that a human was consulted. An unattended agent that sets it
causes a real PUT once cron runs, so operators must gate the agent token, not
rely on this flag.

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

Live 1601-C: Submitted → Confirmed when IMAP matches TIN / form / period
(`#email#` stripped on the receipt, present on the submitted IAF) and the
receipt belongs to this queued generation. Floor is
`queue_authorization.authorized_at`, not `submitted_at`. BIR stamps the file
when PUT lands; the app writes Submitted after PUT returns. 2551Q still
compares the receipt to `submitted_at`. The poller's `job_queue.name` is
`Waiting for {form} {Mon YYYY} confirmation for {TIN} (email)`; `command`
remains `bir_poll_email {email}`. See [AGENT.md](AGENT.md) “Background Tasks
titles”.

Live proof (PR #41 / `68f773c5`, dummy TIN `00000000000000`, **not** re-run
here): 1601-C period **102026** (October). Queued ~3:09:54 PM local 11
September 2026; BIR email `00000000000000-1601Cv2018-102026.xml` at 3:13 PM
from `ebirforms-noreply@bir.gov.ph`; row Submitted ~3:17:25 PM. Transport
PUT is empirically PASS. Automated tests still use **092026** (September
2026) zero-tax.

Dry-run stops at Submitted. IMAP skip is taken from the **session source**,
not from process-global `BIR_SFTP_DRY_RUN` (parallel tests must not leak).

One Gmail inbox shared across profiles is one OAuth grant
(`inbox_oauth_tokens`, copied onto every matching profile). Reconnect must
replace the refresh token the poller uses; the poller must not keep the first
`list_profiles()` row’s dead token. Receipt matching stays TIN / form / period
for every Submitted sibling under that inbox. See
[AGENT.md](AGENT.md) “Email Settings: shared inbox OAuth”.

## Secrets

`BIR_SFTP_*` env only. Never commit, print, or log passwords. Production
dispatcher requires `BIR_SFTP_LIVE=1`. Lab hosts need a SHA-256 pin or
`BIR_SFTP_ACCEPT_ANY_HOST_KEY=1`.
