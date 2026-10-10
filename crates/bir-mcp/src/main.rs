//! `bir-mcp`: MCP server for drafting BIR returns, over stdio or (with
//! `--http`) Streamable HTTP on loopback.
//!
//! Environment: `GPUI_AGENT_TOKEN` (the BIR app's agent token),
//! `GPUI_AGENT_ADDR` (explicit address, overrides discovery),
//! `GPUI_AGENT_REGISTRY` (discovery directory override),
//! `BIR_MCP_HTTP_TOKEN` (bearer token, required with `--http`).
use bir_mcp::client::AgentBirHost;
use bir_mcp::http;
use bir_mcp::server::{Server, serve};
use std::process::ExitCode;

const USAGE: &str = "bir-mcp: draft-only MCP server for BIR forms 2551Q and 1601C.

Usage: bir-mcp                       speak MCP (JSON-RPC 2.0, one message per line) on stdin/stdout
       bir-mcp --http 127.0.0.1:PORT serve MCP Streamable HTTP at http://127.0.0.1:PORT/mcp
                                     (loopback addresses only; needs BIR_MCP_HTTP_TOKEN)
       bir-mcp --version
       bir-mcp --help

Environment:
  GPUI_AGENT_TOKEN     agent token of the running BIR app (required when the app requires one)
  GPUI_AGENT_ADDR      connect to this address instead of using discovery
  GPUI_AGENT_REGISTRY  discovery record directory (default: BIR data dir/agent-instances)
  BIR_MCP_HTTP_TOKEN   with --http: every request must send `Authorization: Bearer <this>`";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        [] => serve_stdio(),
        ["--version" | "-V"] => {
            println!("bir-mcp {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        ["--help" | "-h"] => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        ["--http", addr] => serve_http(addr),
        ["--http"] => {
            eprintln!("--http needs an address, e.g. --http 127.0.0.1:8765\n\n{USAGE}");
            ExitCode::from(2)
        }
        [other, ..] => {
            eprintln!("unknown argument `{other}`\n\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn serve_stdio() -> ExitCode {
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

fn serve_http(addr: &str) -> ExitCode {
    let started = http::token_from_env().and_then(|token| Ok((token, http::bind(addr)?)));
    let (token, listener) = match started {
        Ok(started) => started,
        Err(error) => {
            eprintln!("bir-mcp: {error}");
            return ExitCode::from(2);
        }
    };
    let bound = listener
        .server_addr()
        .to_ip()
        .map_or_else(|| addr.to_string(), |a| a.to_string());
    eprintln!("bir-mcp: serving MCP at http://{bound}{}", http::MCP_PATH);
    let mut server = Server::new(AgentBirHost::from_env());
    http::serve_http(&mut server, &listener, &token);
    ExitCode::SUCCESS
}
