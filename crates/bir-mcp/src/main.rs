//! `bir-mcp`: MCP server over stdio for drafting BIR returns.
//!
//! Environment: `GPUI_AGENT_TOKEN` (the BIR app's agent token),
//! `GPUI_AGENT_ADDR` (explicit address, overrides discovery),
//! `GPUI_AGENT_REGISTRY` (discovery directory override).
use bir_mcp::client::AgentBirHost;
use bir_mcp::server::{Server, serve};
use std::process::ExitCode;

const USAGE: &str = "bir-mcp: draft-only MCP server (stdio) for BIR forms 2551Q and 1601C.

Usage: bir-mcp            speak MCP (JSON-RPC 2.0, one message per line) on stdin/stdout
       bir-mcp --version
       bir-mcp --help

Environment:
  GPUI_AGENT_TOKEN     agent token of the running BIR app (required when the app requires one)
  GPUI_AGENT_ADDR      connect to this address instead of using discovery
  GPUI_AGENT_REGISTRY  discovery record directory (default: BIR data dir/agent-instances)";

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        None => {}
        Some("--version" | "-V") => {
            println!("bir-mcp {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        Some("--help" | "-h") => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Some(other) => {
            eprintln!("unknown argument `{other}`\n\n{USAGE}");
            return ExitCode::from(2);
        }
    }
    let mut server = Server::new(AgentBirHost::from_env());
    match serve(
        &mut server,
        std::io::stdin().lock(),
        std::io::stdout().lock(),
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("bir-mcp: {error}");
            ExitCode::from(1)
        }
    }
}
