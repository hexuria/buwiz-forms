# /file-tax for OpenGrok

Lets OpenGrok prepare a **draft** 2551Q or 1601C in the BIR desktop app while
the user watches the form. Three pieces:

- `bir-mcp` (`crates/bir-mcp`): an MCP server that finds the running BIR app
  through its gpui-agent discovery records and exposes draft-only tools. It
  has no submit, queue, file or payment tool, and its tool-to-host-op map is a
  closed allow-list (`crates/bir-mcp/src/tools.rs`). It speaks stdio by
  default and MCP Streamable HTTP with `--http 127.0.0.1:PORT`; OpenGrok only
  dials HTTP, so it uses the second. Both modes share one dispatcher and one
  allow-list.
- `SKILL.md`: the `/file-tax <FORM>` skill that tells the model how to use
  those tools (ground every value, state its source, never guess an election,
  never overwrite a box the user typed, never submit).
- `plugin.json` + `mcp.json`: an OpenGrok operator plugin (Agent Plugins
  format, `https://agent-plugins.org/schemas/1.0.0/`) whose one MCP server is
  `bir-mcp` over `streamable-http` on loopback.

## Build

From the repository root:

```sh
rtk cargo build --locked --release -p bir-mcp
```

BIR itself must run with the agent control plane enabled and a token:

```sh
rtk cargo build --locked --release -p bir-desktop --features agent
GPUI_AGENT=1 GPUI_AGENT_TOKEN='<your local agent token>' target/release/bir
```

## Run bir-mcp over HTTP

```sh
export GPUI_AGENT_TOKEN='<your local agent token>'   # same value BIR was started with
export BIR_MCP_HTTP_TOKEN="$(openssl rand -hex 32)"  # bearer token OpenGrok will send
target/release/bir-mcp --http 127.0.0.1:8765
# bir-mcp: serving MCP at http://127.0.0.1:8765/mcp
```

- Only loopback addresses are accepted (`127.0.0.0/8`, `[::1]`); anything
  else is refused at startup.
- `BIR_MCP_HTTP_TOKEN` is required in `--http` mode; it refuses to start
  without it. Every request must carry `Authorization: Bearer <token>`
  (compared in constant time), otherwise `401`.
- A request whose `Origin` header is present and not a loopback origin gets
  `403` (the MCP spec's DNS-rebinding guard).
- `POST /mcp` only: one JSON-RPC message per request, `Content-Type:
  application/json`. Requests are answered with `application/json`;
  notifications get `202 Accepted`. `GET` and `DELETE` get `405` (no SSE
  stream, no sessions; the server is stateless).

Both tokens are local secrets you choose. Neither is written to the discovery
records, neither is logged, and neither may be committed.

## How bir-mcp finds BIR

Each BIR host writes `<registry>/bir-desktop-<pid>.json` (`app`, `pid`, `addr`,
`mode`, `protocol`). The registry is `GPUI_AGENT_REGISTRY` if set, otherwise
`agent-instances/` under the BIR data directory. `bir-mcp` picks, in order:

1. `GPUI_AGENT_ADDR` if set,
2. a live `desktop` record (the window the user is watching),
3. a live `headless` record (`bir-headless serve`),
4. `127.0.0.1:17421`.

It re-resolves on every tool call, so starting BIR after `bir-mcp` is fine.

## Load it in OpenGrok

OpenGrok does **not** launch stdio MCP servers (a plugin's `stdio` entry is
reported as an unsupported transport) and does **not** read
`~/.cursor/skills`, `.claude/skills` or a `~/.cursor/mcp.json`. MCP servers
come from plugins; skills come from its `/skills` API (or from the `skills/`
folder of a plugin installed from a registry, which must be public `https` and
so cannot reach a loopback `bir-mcp`).

### 1. The MCP server: an operator plugin

Install `bir` as an **operator plugin**: a folder under the directory named by
`OG_PLUGINS_DIR` on the OpenGrok server, holding `plugin.json` and `mcp.json`.
Operator plugins may point at a loopback URL; plugins an account installs from
the catalogue may not.

```sh
PLUGIN="$OG_PLUGINS_DIR/bir"
mkdir -p "$PLUGIN"
cp integrations/opengrok/file-tax/plugin.json integrations/opengrok/file-tax/mcp.json "$PLUGIN/"
chmod 600 "$PLUGIN/mcp.json"
```

Then edit the **copy** at `$PLUGIN/mcp.json`:

- set the port in `url` to the one `bir-mcp --http` listens on;
- replace `REPLACE_WITH_BIR_MCP_HTTP_TOKEN` with the value of
  `BIR_MCP_HTTP_TOKEN`.

Write the token literally in that untracked copy. A `${NAME}` placeholder in
an operator plugin's headers is filled only from a connected account's token
(`<CONNECTOR>_TOKEN`); there is none for BIR, so an unresolved placeholder
makes OpenGrok drop the server instead of sending it. Never put the real token
into the tracked `mcp.json`.

Restart OpenGrok (operator plugins are read once at boot). The tools appear as
`bir.bir-mcp.<tool>`, subject to the coworker's tool ceiling allowing the
`bir` plugin.

### 2. The skill: `POST /skills`

The operator plugin loader reads only `plugin.json` and `mcp.json`, so upload
the skill through the server's API: `POST /skills` with the whole `SKILL.md`
(frontmatter included) as `body`. The frontmatter `name` (`file-tax`) and
`description` are used when the request does not give them. An uploaded skill
starts switched off; switch it on and attach it to the coworker that has the
`bir` plugin. Then open a chat and type `/file-tax 2551Q`.

## Verify

```sh
cargo test --locked -p bir-mcp              # protocol, allow-list, discovery, HTTP transport tests
scripts/check_file_tax_goal.sh              # full done-condition incl. end-to-end smoke
```

A manual check of the HTTP transport (point `GPUI_AGENT_ADDR` at an unused
port to exercise it without any BIR running):

```sh
BIR_MCP_HTTP_TOKEN=test target/release/bir-mcp --http 127.0.0.1:8765 &
H=(-H 'Authorization: Bearer test' -H 'Content-Type: application/json'
   -H 'Accept: application/json, text/event-stream')
curl -s "${H[@]}" http://127.0.0.1:8765/mcp \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"manual","version":"0"}}}'
curl -s -o /dev/null -w '%{http_code}\n' "${H[@]}" http://127.0.0.1:8765/mcp \
  -d '{"jsonrpc":"2.0","method":"notifications/initialized"}'      # 202
curl -s "${H[@]}" http://127.0.0.1:8765/mcp \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/list"}'
curl -s "${H[@]}" http://127.0.0.1:8765/mcp \
  -d '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"bir_status","arguments":{}}}'
```

`tools/list` must show only the eleven `bir_*` tools; with BIR running,
`bir_status` must report `"connected": true` and which instance it reached.
The stdio mode is checked the same way by piping those JSON-RPC lines into
`bir-mcp` with no arguments. Test with a temporary database
(`BIR_DATABASE_PATH` + `EBIR_TEST_ENV=1`) and the test TIN, never with real
taxpayer data.
