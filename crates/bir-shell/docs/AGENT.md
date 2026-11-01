# gpui-agent integration

`bir-shell` owns the BIR agent control plane. The `bir-headless` binary runs
the same DB-backed `agent::session::Session` without opening a window; UI
commands are discarded and screenshots are unavailable. Discovery records use
app `bir-shell`; the headless executable is `bir-headless`.

## Build and install

`bir-shell` is a standalone Cargo workspace with its own lockfile and target
directory:

```sh
cargo build --locked --manifest-path crates/bir-shell/Cargo.toml \
  --features agent --bin bir-headless
cargo install --path crates/bir-shell --features agent \
  --bin bir-headless --locked
```

The development binary is
`crates/bir-shell/target/debug/bir-headless`.

## Start a headless host

Set a non-empty token and opt into the agent before starting the daemon:

```sh
GPUI_AGENT=1 \
GPUI_AGENT_TOKEN='<local-development-token>' \
GPUI_AGENT_ADDR=127.0.0.1:17421 \
crates/bir-shell/target/debug/bir-headless serve
```

`GPUI_AGENT_ADDR` defaults to loopback port `17421`. The discovery record
advertises the actual bound address, including a dynamically assigned port.
`bir-mcp` selects records by app and executable; it identifies
`bir-headless` as headless mode.

Use `serve --wait` when another BIR host owns the database lock, or
`serve --detach` to run in the background. `status`, `logs`, and `shutdown`
operate on the headless daemon. A host may use `BIR_DATABASE_PATH` to select a
database; automated tests use ephemeral databases and set `EBIR_TEST_ENV=1`
and `BIR_SFTP_DRY_RUN=1`.

The headless host has no painted window, screenshot backend, or UI queue.
Validation and queue transitions remain guarded by the same core capabilities
as the shell UI. `GPUI_AGENT_INSECURE_NO_TOKEN=1` is only for local demos and
must not be used for remote binds.
