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
   with `next` set to `/device?user_code=…`. After email-code sign-in the
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

## Local mock account server

Run `just mock-account` in a separate terminal before the NSH app. Debug builds
already use `http://localhost:3008`; no URL override is needed. For an
installed release build, set both `BUWIZ_SERVER_URL` and `BUWIZ_ACCOUNT_URL`
to `http://localhost:3008` in the app's environment before launching it.
Setting only `BUWIZ_ACCOUNT_URL` points the browser at the mock, but the app
still creates the device code against the production API.

All accounts, device codes and tokens live in memory and disappear when the
server stops. Use only development emails. This is not the real account
server and must never be exposed publicly.

Choose Sign in or Create account in the app, continue with the prefilled
`dev@buwiz.local` in the browser (no password or code), then Approve the device.
Deny exercises the refusal path. The server logs requests and user codes to
stdout, not tokens. `just mock-account --auto-approve` skips browser approval
for headless runs. Refresh tokens rotate and sign-out revokes the grant.

`--bind IP` / `MOCK_ACCOUNT_HOST` and `--port N` / `MOCK_ACCOUNT_PORT` change
the default `127.0.0.1:3008`. Flags override the environment. `0.0.0.0` is
allowed only for isolated development networks; it permits unauthenticated
development sign-in from other machines. Prefer loopback and port forwarding.

- Android: `adb reverse tcp:3008 tcp:3008` before launch (also for the browser).
- iOS Simulator: shares the host's localhost; no forwarding is needed.
- Linux NSH Docker sidecar: use `--network host` so localhost reaches the host.

The real `buwiz-com` server uses passwordless **email-code** sign-in and requires
PostgreSQL, `DATABASE_URL` and `DOCUMENT_KEY`; see its `docs/development.md`.
It has no in-memory mock mode. The local mock does not implement TIN claims,
licensing, agent filing pickup or the submission relay; keep using `mock-bir`
and `mock-relay` for filing tests.

## Where the tokens live

The refresh token is not shown in Settings and is not written to the page.

- OS keychain, service `com.buwiz.bir`, account `desktop-refresh`.
- Always also a mode-0600 file `buwiz-cloud-account.json` next to the
  database. `BUWIZ_CLOUD_ACCOUNT_FILE` overrides that path. The file exists
  because a keyring build without a real backend can report success and then
  forget the secret when the process exits.

`bir-mcp` on stdio reads this same account for its tool gate and refreshes
it in place. `bir-mcp login` runs this device flow and writes this store. It
does not keep a second keychain. `BUWIZ_ACCESS_TOKEN` still selects an agent
token instead of this account.

`bir-mcp --http` does not read this account. It is an OAuth protected
resource. `GET /.well-known/oauth-protected-resource` names the account
origin as the authorization server (`https://account.buwiz.com` in a release
build, `http://localhost:3008` in a debug build, with the same
`BUWIZ_ACCOUNT_URL` / `BUWIZ_SERVER_URL` / `option_env!` precedence). A
request without a bearer token for that MCP URL gets 401 and
`WWW-Authenticate: Bearer resource_metadata=…`. The token is checked at the
authorization server's introspection endpoint, and a token minted for
another resource is refused. A tool still runs only when that account can
see a taxpayer profile with a working pass.

## Offline

Local taxpayer setup, drafts, and dry-run filing do not call these functions.
After the first successful sign-in, an unreachable server leaves the session
in place and the app stays open. The gate returns only when the person signs
out, or when the server is reachable and says the refresh token is revoked
or invalid.
