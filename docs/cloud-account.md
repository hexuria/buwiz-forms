# Buwiz account

Settings → Account signs this computer in to buwiz-server. It is separate
from the PIN that unlocks a taxpayer on this computer. Preparing and filing
returns does not wait on the server. With `EBIR_TEST_ENV=1` and
`BIR_SFTP_DRY_RUN=1` the app never contacts BIR.

## Server address

The person using the app never types the server address. Settings → Account
shows the API origin and does not edit it. An older `buwiz_server_url`
setting is deleted on startup and is not read.

The address is an origin, with no path, query, or password. Precedence, for
the API and for the account site separately:

1. The process environment: `BUWIZ_SERVER_URL` for the API,
   `BUWIZ_ACCOUNT_URL` for the browser pages. For a dev or test run.
2. The same names compiled into the binary (`option_env!` at build time).
   Neither value is a secret.
3. The build's own default:
   - Release: API `https://api.buwiz.com`, account pages
     `https://account.buwiz.com`.
   - Debug: both `http://localhost:3008`.

Sign-in opens the account origin (`/login` or `/signup`), then the device
page. Token calls stay on the API origin. A different address from a saved
session drops that session.

## Sign in

The app uses the server's existing device flow for the public client
`buwiz-desktop`. It does not ask for the account password.

1. `POST /auth/device/start` with `{"client_id":"buwiz-desktop"}`.
2. The system browser opens the account origin at `/login` or `/signup`,
   with `next` set to `/device?user_code=…`. After the password step the
   server continues on `/device`, which the sign-in page accepts as a
   return path. Token requests stay on the API origin.
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

`bir-mcp` reads this same account for its tool gate and refreshes it in
place. `bir-mcp login` runs this device flow and writes this store. It does
not keep a second keychain. `BUWIZ_ACCESS_TOKEN` still selects an agent token
instead of this account.

## Offline

Local taxpayer setup, drafts, and dry-run filing do not call these functions.
After the first successful sign-in, an unreachable server leaves the session
in place and the app stays open. The gate returns only when the person signs
out, or when the server is reachable and says the refresh token is revoked
or invalid.
