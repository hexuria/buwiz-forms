# Plan: BIR_SFTP_* env override on top of dispatcher

> **Superseded 2026-09-11:** the `TEST_SFTP_*` compatibility path this plan
> preserved was removed. `BIR_SFTP_*` is the only override; references to
> `TEST_SFTP_*` below are historical.

Branch to implement on: `codex/sftp-on-pr39`
Worktree: C:\Users\uriah\Code\buwiz-forms-wt-sftp-on-pr39
Do not open a PR unless asked.

## Goal

Keep the official dispatcher as the default production path. Add a `.env` / process-env override so an operator can point SFTP at a different host, port, user, password, or folder without a rebuild.

This is the OSS pattern we want:

    BIR_SFTP_HOST=...
    BIR_SFTP_PORT=22
    BIR_SFTP_USERNAME=...
    BIR_SFTP_PASSWORD=***
    BIR_SFTP_FOLDER=1601Cv2018   # optional

Default port is **22**, matching live BIR (`ebf2.bir.gov.ph:22`). Do not copy their default of 23.

If these vars are unset, behavior stays as today: fetch `tinDispatcherSFTP.php`, unwrap ciphertext, PUT.

## Non-goals

- Do not embed secrets in the binary.
- Do not replace dispatcher with env-only production as the only path.
- Do not add WinSCP.exe / OpenSSH / sshpass fallbacks.
- Do not add SFTP fsync.
- Do not commit real host/user/password values.
- Do not change IAF payload crypto.
- Do not change queue claim-before-PUT semantics.

## Current code (what we change)

`crates/bir-core/src/transport.rs`

- `SftpEndpoint::from_env()` only reads `TEST_SFTP_*`.
- `open_iaf_session()` uses env if `TEST_SFTP_HOST` is set, else dispatcher.

`crates/bir-desktop/src/gui.rs` already calls `dotenvy::dotenv().ok()`.
`bir-headless` and `sftp_harness` do **not** load `.env` today.

`.env.example` has Google/IMAP/signing keys only. `.gitignore` already ignores `.env`, `.env.local`, `.env.*.local`.

## Resolution order

When opening an SFTP session for form `form_type` and TIN `tin`:

1. If `BIR_SFTP_HOST` is set and non-empty, build the endpoint from env:
   - host = `BIR_SFTP_HOST` (required)
   - port = `BIR_SFTP_PORT` or `22`
   - username = `BIR_SFTP_USERNAME` (required)
   - password = `BIR_SFTP_PASSWORD` (required)
   - folder = `BIR_SFTP_FOLDER` if set, else `form_type`
2. Else if `TEST_SFTP_HOST` is set (legacy harness), keep current `TEST_SFTP_*` behavior so existing commands do not break. Document `TEST_SFTP_*` as deprecated aliases for `BIR_SFTP_*`.
3. Else fetch dispatcher with the 9-digit TIN prefix and unwrap.

Log only: source (`env` / `test-env` / `dispatcher`), host, port, folder, username_len, password_len. Never log password or ciphertext.

Missing username/password when host is set is a hard error before connect. Do not fall through to dispatcher in that case; that would hide a misconfigured override.

## Implementation steps

### 1. Endpoint config helper

In `transport.rs`:

- Add `SftpEndpointSource { Env, TestEnv, Dispatcher }`.
- Replace `from_env()` with something like:
  - `from_bir_sftp_env() -> Option<Result<SftpEndpoint, TransportError>>`
  - keep `from_test_sftp_env()` for `TEST_SFTP_*`
- Add `resolve_sftp_endpoint(form_type, tin) -> Result<(SftpEndpoint, SftpEndpointSource), TransportError>` implementing the order above.
- `open_iaf_session` calls `resolve_sftp_endpoint` instead of the current `if TEST_SFTP_HOST` branch.

Accept both `BIR_SFTP_USERNAME` and `BIR_SFTP_USER` if we want a one-line alias; prefer `USERNAME` to match OSS. Map our old `TEST_SFTP_USER` only on the test path.

Trim values. Empty string counts as unset.

### 2. Load `.env` on every process that can submit

- Desktop GUI: already loads dotenv. Keep it first thing in `run_gui()`.
- `bir-headless`: call `dotenvy::dotenv().ok()` at the start of `run()` / `serve()` so cron sees `.env`.
- `sftp_harness`: load dotenv at the start of `main`.

Do not fail if `.env` is missing.

Prefer `dotenvy::dotenv()` from cwd, same as GUI. Document that operators should put `.env` in the repo root or launch cwd.

### 3. `.env.example`

Add a blank block, no real values:

    # Optional SFTP override. If BIR_SFTP_HOST is set, dispatcher is skipped.
    # Default production path is tinDispatcherSFTP.php (official client).
    # BIR_SFTP_HOST=
    # BIR_SFTP_PORT=22
    # BIR_SFTP_USERNAME=
    # BIR_SFTP_PASSWORD=
    # BIR_SFTP_FOLDER=

Keep existing Google/IMAP keys. Never put a live host in this file.

### 4. Tests

Unit tests with `temp-env` (already a bir-core dev-dep):

- No env -> resolve would call dispatcher (mock or do not hit network; test the branch selector with a small pure function if we split parse from HTTP).
- `BIR_SFTP_HOST` + user + pass + default port 22 + folder falls back to form_type.
- `BIR_SFTP_PORT=2222` and `BIR_SFTP_FOLDER=lab` are honored.
- Host set but username missing -> error, not dispatcher.
- `TEST_SFTP_*` still works when `BIR_SFTP_HOST` is unset.
- `BIR_SFTP_HOST` wins over `TEST_SFTP_HOST`.
- Empty `BIR_SFTP_HOST=` is treated as unset.

Keep existing wrap/unwrap, remote_path, and 14-digit filename tests.

Do not add a live BIR test in CI.

### 5. Harness

`sftp_harness`:

- `--live-connect` / `--live-put` should go through `resolve_sftp_endpoint` so an operator `.env` override is visible.
- Print `source=env|dispatcher` plus host/port/folder lengths as today.
- `--dry-run` stays offline.

Manual check after code lands:

    # dispatcher path (no BIR_SFTP_HOST)
    cargo run -p bir-core --bin sftp_harness -- --live-connect

    # env override against a local SFTP, never commit these
    # BIR_SFTP_HOST=127.0.0.1 BIR_SFTP_PORT=2222 ...
    cargo run -p bir-core --bin sftp_harness -- --live-connect

### 6. Docs

Update:

- `evidence/sftp-submission/STUDY_WALKTHROUGH.md` — env override section
- `evidence/sftp-submission/README.md` — mention `.env.example`
- `crates/bir-desktop/docs/AGENT.md` claimed-queue paragraph — dispatcher default, `BIR_SFTP_*` override
- `sftp_harness.rs` module docs

State clearly: live BIR email proof used dispatcher, not env. Env is for labs and emergency host changes.

### 7. Desktop and headless

No UI for SFTP settings in this pass. Cron on painted `bir` and `bir-headless` both call `open_iaf_session`, so they pick up env automatically once dotenv is loaded.

Verify by reading code paths:

- GUI `start_cron_jobs` -> `open_iaf_session`
- headless thread `start_cron_jobs` -> `open_iaf_session`

### 8. Follow-ups (same theme, separate commits if we do them)

Do these after the override works. Each is optional.

A. Dry-run gate: `BIR_SFTP_LIVE=0` makes `open_iaf_session` / `store` refuse network and return a clear error, so a queued form cannot hit BIR. Default remains live (dispatcher or env).

B. Ciphertext SHA-256 on the queued row / log line after encrypt, before PUT. Helps "did this exact file already go?". Do not block retries solely on hash until we define product behavior.

C. Rename log/error category `unknown-outcome` to `uncertain` in cron messages only if we touch those strings anyway.

D. Check official amended IAF `Vn` suffix vs our `V1` for 1601C. Evidence-only first; change naming only if official client emits V2+.

## Verify done

- [ ] Dispatcher still used when no `BIR_SFTP_HOST`
- [ ] Override used when `BIR_SFTP_*` complete
- [ ] Incomplete override errors before connect
- [ ] `TEST_SFTP_*` still works
- [ ] `.env.example` has blanks only
- [ ] headless and harness load dotenv
- [ ] unit tests pass without network
- [ ] no secrets in git
- [ ] docs describe order: env override, else dispatcher

## Suggested commit

    Allow BIR_SFTP_* env to override dispatcher SFTP.

    Default remains tinDispatcherSFTP.php. If BIR_SFTP_HOST is set,
    use process env / .env for host, port, user, and password. Load
    dotenv in headless and the SFTP harness. Do not embed secrets.
