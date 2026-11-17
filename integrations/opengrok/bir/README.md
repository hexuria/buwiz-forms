# /file-tax for OpenGrok

Lets OpenGrok prepare a **draft** of any of BIR's 43 forms in the BIR desktop
app while the user watches the form: fill every user box by its official key,
validate with the page's own rules, preview the filled sheet, show open
questions as a chat form card, and hand a clean draft to the user's own Submit
click (`bir_form_review`). Three pieces:

- `bir-mcp` (`crates/bir-mcp`): an MCP server that lists every running BIR
  app through gpui-agent's discovery records, works only on the one the user
  picks (`bir_list_apps` -> `bir_connect`), and exposes draft-only tools. It
  has no submit, queue, file or payment tool, and its tool-to-host-op map is a
  closed allow-list (`crates/bir-mcp/src/tools.rs`). It speaks stdio by
  default and MCP Streamable HTTP with `--http 127.0.0.1:PORT`; OpenGrok only
  dials HTTP, so it uses the second. Both modes share one dispatcher and one
  allow-list.
- This folder is one OpenGrok plugin named `bir` (Agent Plugins format,
  `https://agent-plugins.org/schemas/1.0.0/`):
  - `plugin.json` + `mcp.json`: its one MCP server, `bir-mcp` over
    `streamable-http` on loopback;
  - `skills/file-tax/SKILL.md`: the `/file-tax <FORM>` skill that tells the
    model how to use those tools (ground every value, state its source, never
    guess an election, never overwrite a box the user typed, never submit).

## Build

From the repository root:

```sh
rtk cargo build --locked --release -p bir-mcp
```

BIR itself must run with the agent control plane enabled and a token. Give
each instance a name with `GPUI_AGENT_CONNECT_ID`, so the user can tell them
apart and pick one (otherwise it is listed as `pid:<pid>`):

```sh
rtk cargo build --locked --release -p bir-shell --features agent
GPUI_AGENT=1 GPUI_AGENT_TOKEN='<agent token>' GPUI_AGENT_CONNECT_ID=bir target/release/bir
```

A demo next to it, on a test database (`EBIR_TEST_ENV=1` and/or a
`BIR_DATABASE_PATH` outside the BIR data directory both make `app.info`
report `database: "test"`). When the default port `127.0.0.1:17421` is taken
and `GPUI_AGENT_ADDR` is not set, the second app binds an ephemeral loopback
port and publishes it in its discovery record:

```sh
EBIR_TEST_ENV=1 BIR_DATABASE_PATH=/tmp/bir-demo/bir_data.db \
GPUI_AGENT=1 GPUI_AGENT_TOKEN='<demo agent token>' GPUI_AGENT_CONNECT_ID=bir-demo \
  target/debug/bir-shell
```

## Agent tokens: `BIR_MCP_TOKENS`

`bir-mcp` needs the agent token of each app it may talk to. Put them in a
JSON file that only you can read, keyed by connect id (or by pid, as `"12345"`
or `"pid:12345"`):

```sh
umask 077
cat > ~/.config/bir-mcp-tokens.json <<'JSON'
{ "bir-demo": "<demo agent token>" }
JSON
chmod 600 ~/.config/bir-mcp-tokens.json
export BIR_MCP_TOKENS=~/.config/bir-mcp-tokens.json
```

- On unix the file is refused unless no group/other permission bit is set
  (`chmod 600`).
- It is re-read on every call: add a token without restarting `bir-mcp`.
- `GPUI_AGENT_TOKEN`, if set, is the fallback token for any app without an
  entry. Leave it unset if `bir-mcp` should not be able to reach every app.
- An app whose token is not known is still listed (`"token": "missing"`) but
  cannot be selected.
- Tokens are never logged, never returned by a tool, never written to the
  discovery records, and never part of an error message.

## Run bir-mcp over HTTP

```sh
export BIR_MCP_TOKENS=~/.config/bir-mcp-tokens.json  # see above
target/release/bir-mcp --http 127.0.0.1:8765
# bir-mcp: serving MCP at http://127.0.0.1:8765/mcp
# bir-mcp: OAuth metadata at http://127.0.0.1:8765/.well-known/oauth-protected-resource/mcp
```

- Only loopback addresses are accepted (`127.0.0.0/8`, `[::1]`); anything
  else is refused at startup.
- `--http` is an OAuth 2.1 protected resource. There is no shared bearer
  secret. A request with no token, or a token buwiz-server minted for a
  different resource, gets `401` and
  `WWW-Authenticate: Bearer resource_metadata="…"`. The client registers
  with buwiz-server, signs in in the browser, and sends the access token
  whose audience is this MCP URL.
- A request whose `Origin` header is present and not a loopback origin gets
  `403` (the MCP spec's DNS-rebinding guard).
- `POST /mcp` only: one JSON-RPC message per request, `Content-Type:
  application/json`. Requests are answered with `application/json`;
  notifications get `202 Accepted`. `GET` and `DELETE` get `405` (no SSE
  stream, no sessions; the server is stateless).

All these tokens are local secrets you choose. None is written to the
discovery records, none is logged, and none may be committed.

## How bir-mcp finds BIR, and which one it uses

Discovery is gpui-agent's own (`gpui_agent::discovery`). Every BIR host, the
desktop window and `bir-headless serve` alike, writes
`<runtime dir>/hosts/<pid>-<port>.json` (`addr`, `pid`, `app: "bir-shell"`,
`executable`, and `connect_id` from `GPUI_AGENT_CONNECT_ID`; never a token)
and removes it on exit. The runtime dir is `GPUI_AGENT_RUNTIME_DIR` if set,
else `$XDG_RUNTIME_DIR/gpui-agent`, else `$TMPDIR/gpui-agent-<uid>`; BIR
hosts and `bir-mcp` must agree on it.

`bir-mcp` never picks an app by itself, not even when only one is running,
and it ignores `GPUI_AGENT_ADDR`:

1. `bir_list_apps` lists every live BIR record: `id` (the connect id, else
   `pid:<pid>`), `pid`, `addr`, `mode` (desktop/headless), `token`
   (`known`/`missing`) and, when the token is known, the app's own read-only
   `app.info`: `database` (`test` or `real`) and its `taxpayers`.
2. The model shows that list and asks the user which app to use.
3. `bir_connect {id}` selects it for this `bir-mcp` process (one selection
   per process, shared by every HTTP client of it). An app on the `real`
   database is refused unless the call carries `confirm_real: true`, which
   the skill sends only after the user confirmed in chat. A failed
   `bir_connect` leaves no app selected.
4. Until then every profile, dues and form tool answers `no BIR app selected
   — call bir_list_apps, ask the user which one, then bir_connect`. After it,
   every result names the app: `{"app": {"id", "mode", "database"},
   "result": ...}` (errors end with `[BIR app: <id> (<mode>, <database>
   database)]`). If the app exits, the selection is dropped.

`bir-headless status` / `shutdown` find the daemon the same way: an explicit
`GPUI_AGENT_ADDR`, else the instance named by `GPUI_AGENT_CONNECT_ID`, else
the one running `bir-headless`, else `127.0.0.1:17421`.

## Load it in OpenGrok

OpenGrok does **not** launch stdio MCP servers (a plugin's `stdio` entry is
reported as an unsupported transport) and does **not** read
`~/.cursor/skills`, `.claude/skills` or a `~/.cursor/mcp.json`. A plugin an
account installs from the marketplace must name a public `https` server, so it
cannot reach a loopback `bir-mcp`. Install `bir` as an **operator plugin**
instead: a folder under the directory named by `OG_PLUGINS_DIR` on the
OpenGrok server.

```sh
PLUGIN="$OG_PLUGINS_DIR/bir"
mkdir -p "$PLUGIN"
cp -R integrations/opengrok/bir/plugin.json integrations/opengrok/bir/mcp.json \
  integrations/opengrok/bir/skills "$PLUGIN/"
chmod 600 "$PLUGIN/mcp.json"
```

Then edit the **copy** at `$PLUGIN/mcp.json` and set the port in `url` to
the one `bir-mcp --http` listens on.

The server does not take a shared secret in that file. The client has to
complete buwiz-server's authorization-code + PKCE flow and send the access
token it was given for this MCP URL. A token pasted into `headers` works
only until it expires, and a token minted for another resource is refused.

Restart OpenGrok (operator plugins are read once at boot). The tools appear as
`bir.bir-mcp.<tool>`. Turn the `bir` plugin on for the Bot (its tool ceiling;
operator plugins start off and @mentions don't enable them). Each tool can be
set to always, ask or never on its own.

The skill ships in the plugin's `skills/` folder. An OpenGrok server that
loads skills from operator plugins offers `/file-tax` wherever the `bir`
plugin is on. An older server reads only `plugin.json` and `mcp.json`; there,
upload the skill through `POST /skills` with the whole
`skills/file-tax/SKILL.md` (frontmatter included) as `body` and
`source: "uploaded"`.

## Verify

```sh
cargo test --locked -p bir-mcp              # protocol, allow-list, selection, tokens, discovery, HTTP tests
scripts/check_file_tax_goal.sh              # full done-condition incl. end-to-end smoke
```

A manual check of the HTTP transport (with `GPUI_AGENT_RUNTIME_DIR` pointing
at an empty directory, no BIR app is listed, so nothing is reached):

```sh
GPUI_AGENT_RUNTIME_DIR="$(mktemp -d)" \
  target/release/bir-mcp --http 127.0.0.1:8765 &
# Discover the authorization server, then send the access token it issued
# for http://127.0.0.1:8765/mcp. A request without that token is 401.
curl -s http://127.0.0.1:8765/.well-known/oauth-protected-resource/mcp
H=(-H 'Authorization: Bearer ACCESS_TOKEN' -H 'Content-Type: application/json'
   -H 'Accept: application/json, text/event-stream')
curl -s "${H[@]}" http://127.0.0.1:8765/mcp \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"manual","version":"0"}}}'
curl -s -o /dev/null -w '%{http_code}\n' "${H[@]}" http://127.0.0.1:8765/mcp \
  -d '{"jsonrpc":"2.0","method":"notifications/initialized"}'      # 202
curl -s "${H[@]}" http://127.0.0.1:8765/mcp \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/list"}'
curl -s "${H[@]}" http://127.0.0.1:8765/mcp \
  -d '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"bir_list_apps","arguments":{}}}'
curl -s "${H[@]}" http://127.0.0.1:8765/mcp \
  -d '{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"bir_profiles_list","arguments":{}}}'
```

`tools/list` must show only the thirteen `bir_*` tools; `bir_list_apps` must
list no app, and `bir_profiles_list` must answer `no BIR app selected`. With a
BIR app running (and the same runtime dir), `bir_list_apps` lists it and
`bir_connect` selects it.
The stdio mode is checked the same way by piping those JSON-RPC lines into
`bir-mcp` with no arguments. Test with a temporary database
(`BIR_DATABASE_PATH` + `EBIR_TEST_ENV=1`) and the test TIN, never with real
taxpayer data.
