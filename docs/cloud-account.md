# Buwiz account

Settings → Account signs this computer in to buwiz-server. It is separate
from the PIN that unlocks a taxpayer on this computer. Preparing and filing
returns does not wait on the server. With `EBIR_TEST_ENV=1` and
`BIR_SFTP_DRY_RUN=1` the app never contacts BIR.

## Server address

The address is an origin (`https://api.buwiz.com`), with no path, query, or
password. Precedence:

1. The saved setting `buwiz_server_url` (Settings → Account → Server → Save).
2. The environment variable `BUWIZ_SERVER_URL`.
3. `https://api.buwiz.com`.

Saving a different address drops the saved tokens.

## Sign in

The app uses the server's existing device flow for the public client
`buwiz-desktop`. It does not ask for the account password.

1. `POST /auth/device/start` with `{"client_id":"buwiz-desktop"}`.
2. The system browser opens `verification_uri_complete` (`/login/device`).
   After the password step the server continues on `/device`, which the
   sign-in page accepts as a return path.
3. `POST /auth/device/poll` until the browser approval returns tokens.
4. `GET /me` records the email.
5. Later, `POST /oauth/token` with `grant_type=refresh_token` renews the
   access token. `invalid_grant` signs out. A network failure keeps the
   tokens and shows the account as offline.

The request body is also sent in `x-buwiz-request-body` (`b64`), which is
how buwiz-server reads small posts on the current Spin host.

Sign out calls `POST /api/auth/logout` with the access token, then deletes
the local copy even if the server is unreachable.

## Where the tokens live

The refresh token is not shown in Settings and is not written to the page.

- OS keychain, service `com.buwiz.bir`, account `desktop-refresh`.
- Always also a mode-0600 file `buwiz-cloud-account.json` next to the
  database. `BUWIZ_CLOUD_ACCOUNT_FILE` overrides that path. The file exists
  because a keyring build without a real backend can report success and then
  forget the secret when the process exits.

## Offline

Local taxpayer setup, drafts, and dry-run filing do not call these functions.
A failed refresh leaves the last session in place and marks it offline.
