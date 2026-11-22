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

`form.dismiss` refuses to close a form with unsaved edits by default. To
close without writing the agent's copy of the return, invoke `form.dismiss`
with `{"discard": true}`; the result includes `"discarded": true`. The NSH
window saves what the filer typed as it leaves. In the classic GPUI shell,
`enterView` switches views without saving, so unsaved typing there is not
stored.

`form.fields` lists boxes the agent can fill. Pass `{"locked": true}` to
include locked boxes as read-only, marked with their lock name; this does not
make them fillable, and `form.fill` still refuses locked boxes.

An invoke error beginning with `approval required:` means the window is
holding the change for the filer. It is not an ordinary failure to retry;
after the filer answers, read the return again with `form.fields`.

## Where the code lives

The session (every `invoke`), the UI-command queue, the opt-in rules, the batch drain and the headless daemon with its CLI are UI-free and
live in `crates/bir-app-api/src/agent/` (cargo features `agent`, and `headless` for the daemon): one copy, used by the gpui shell, by
`bir-headless` and by the NSH app. `bir-shell` keeps only the gpui glue (`src/agent/mod.rs`: the drain timer, which waits `bir_app_api::agent::drain_interval`: 1 ms while a request arrived in the last 500 ms and 32 ms when idle, and the semantic tree of the
script view). Their tests run with `cargo test -p bir-app-api --features headless --lib agent::` (the four safety variables).

## NSH app

Build with `cargo ... --manifest-path nsh/Cargo.toml --features agent` (off by default). Same opt-in as above, with two differences: the NSH
app listens on loopback only (any other `GPUI_AGENT_ADDR` is refused, token or not), and its discovery record names the executable `bir`, so
`bir-mcp` lists it (the real executable is in `hello`). `nsh/src/agent.rs` is the host: `Agent.start()` (the door boots first, so the send
guard and the database guard have run) and `Agent.drain()` (TSX calls it every 32 ms, on the thread that owns the database). `nsh/shell/agent`
polls `agent_take` every 50 ms, only while the listener runs, and applies `refresh`, `go`, `open`, `choose`, `review` and `quit` to the
frame as `app/main.js` did. An agent's queue request is the host function `agent_send_draft`: the same validate, standing OK and Queue as the
UI; the door's send guard has forced a dry run (`nsh/tests/agent_send.rs`).

The window itself has a second control plane, NSH's own (`nsh-platform-macos` feature `gpui-agent-server`, switched on by `agent` on macOS):
the semantic tree, press, type, key, scroll and a PNG of the window. It listens at `GPUI_AGENT_ADDR` (the data plane then takes another port;
read it from the discovery record) and is not in `bir-mcp`: use the `gpui-agent` CLI. The `nsh` CLI builds its hosts without app features, so
`just nsh-agent-check` sets `nsh/agent/cargo-with-agent.sh` as `$CARGO`, which adds the feature to the generated host's build.
`nsh/tests/window_plane.rs` drives a real window with both planes.
