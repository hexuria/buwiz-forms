# /file-tax for OpenGrok

Lets OpenGrok prepare a **draft** 2551Q or 1601C in the BIR desktop app while
the user watches the form. Two pieces:

- `bir-mcp` (`crates/bir-mcp`): an MCP server over stdio that finds the running
  BIR app through its gpui-agent discovery records and exposes draft-only
  tools. It has no submit, queue, file or payment tool, and its tool-to-host-op
  map is a closed allow-list (`crates/bir-mcp/src/tools.rs`).
- `SKILL.md`: the `/file-tax <FORM>` skill that tells the model how to use
  those tools (ground every value, state its source, never guess an election,
  never overwrite a box the user typed, never submit).

`mcp.json` is the MCP server entry OpenGrok loads.

## Build and install

From the repository root:

```sh
rtk cargo build --locked --release -p bir-mcp
# put it on PATH (or use the absolute path in mcp.json, see below)
rtk cargo install --locked --path crates/bir-mcp
```

BIR itself must run with the agent control plane enabled and a token:

```sh
rtk cargo build --locked --release -p bir-desktop --features agent
GPUI_AGENT=1 GPUI_AGENT_TOKEN='<your local token>' target/release/bir
```

The token is a local secret you choose. Give the same value to `bir-mcp`
through its environment; it is never written to the discovery records and must
never be committed.

## How bir-mcp finds BIR

Each BIR host writes `<registry>/bir-desktop-<pid>.json` (`app`, `pid`, `addr`,
`mode`, `protocol`). The registry is `GPUI_AGENT_REGISTRY` if set, otherwise
`agent-instances/` under the BIR data directory. `bir-mcp` picks, in order:

1. `GPUI_AGENT_ADDR` if set,
2. a live `desktop` record (the window the user is watching),
3. a live `headless` record (`bir-headless serve`),
4. `127.0.0.1:17421`.

It re-resolves on every tool call, so starting BIR after OpenGrok is fine.

## Load it in OpenGrok

OpenGrok loads skills from `.cursor/skills/<name>/SKILL.md` (also
`.claude/skills/` and `.agents/skills/`), in the project or in your home
directory, and MCP servers from an `mcpServers` config.

1. Skill: link this directory as a skill named `file-tax`:

   ```sh
   mkdir -p ~/.cursor/skills
   ln -s "$PWD/integrations/opengrok/file-tax" ~/.cursor/skills/file-tax
   ```

2. MCP server: add the `bir` entry from `mcp.json` to your MCP config (for
   example `~/.cursor/mcp.json`). `${GPUI_AGENT_TOKEN}` is a placeholder, not
   a token: when the config ships inside an OpenGrok plugin it becomes a
   write-only (secret) plugin variable the user fills in; in a plain
   `mcp.json` use `${env:GPUI_AGENT_TOKEN}` and export the variable before
   launching OpenGrok. Never paste a real token into a tracked file.
   GUI apps often do not see `~/.cargo/bin` on `PATH`; if the server fails to
   start, set `"command"` to the absolute path of `bir-mcp`.

3. Restart OpenGrok, open a chat and type `/file-tax 2551Q`.

## Verify

```sh
rtk cargo test --locked -p bir-mcp          # protocol, allow-list, discovery tests
scripts/check_file_tax_goal.sh              # full done-condition incl. end-to-end smoke
```

A manual check against a running BIR:

```sh
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"manual","version":"0"}}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' \
  '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"bir_status","arguments":{}}}' \
  | GPUI_AGENT_TOKEN='<your local token>' bir-mcp
```

`tools/list` must show only the eleven `bir_*` tools; `bir_status` must report
`"connected": true` and which instance it reached. Test with a temporary
database (`BIR_DATABASE_PATH` + `EBIR_TEST_ENV=1`) and the test TIN, never with
real taxpayer data.
