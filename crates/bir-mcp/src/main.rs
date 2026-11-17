//! `bir-mcp`: MCP server for drafting BIR returns, over stdio or (with
//! `--http`) Streamable HTTP on loopback.
//!
//! Environment: `BIR_MCP_TOKENS` (0600 JSON file of per-app agent tokens),
//! `GPUI_AGENT_TOKEN` (fallback agent token for any app),
//! `GPUI_AGENT_RUNTIME_DIR` (upstream discovery directory override),
//! `BUWIZ_ACCESS_TOKEN` (optional agent token; when set, every stdio tool checks
//! that token instead of the desktop account stored by the Buwiz app).
//! `--http` does not use that account: each request carries an OAuth bearer
//! token issued by buwiz-server for this process's MCP URL.
use bir_mcp::client::AgentBirHost;
use bir_mcp::http;
use bir_mcp::license::License;
use bir_mcp::server::{Server, serve};
use bir_mcp::signin::{self, DesktopStore, LoginHooks};
use std::process::ExitCode;

const USAGE: &str = "bir-mcp: draft-only MCP server for all 43 BIR forms (the user clicks Submit in BIR).

Usage: bir-mcp                       speak MCP (JSON-RPC 2.0, one message per line) on stdin/stdout
       bir-mcp login                 sign in through the Buwiz device page
       bir-mcp logout                sign out and remove the stored session
       bir-mcp status                show whether this computer is signed in
       bir-mcp --http 127.0.0.1:PORT serve MCP Streamable HTTP at http://127.0.0.1:PORT/mcp
                                     (loopback only; OAuth protected resource)
       bir-mcp --version
       bir-mcp --help

Tools work on one BIR app at a time, chosen by the user: bir_list_apps lists every
running BIR app, bir_connect selects one. Nothing is selected at startup.

The Buwiz server address comes from the build, the same way the Buwiz app
resolves it. A debug build uses http://localhost:3008. `bir-mcp login` runs
the app's device flow and writes the app's saved account. Stdio tools run only
after that account is signed in, or when BUWIZ_ACCESS_TOKEN is set to a
buwiz-server agent token.

`--http` is a separate door. It serves OAuth protected-resource metadata and
accepts only a bearer token buwiz-server minted for that MCP URL. A token for
another resource is refused. Tools still run only when that account can see a
taxpayer profile with a working pass.

Environment:
  BIR_MCP_TOKENS          JSON file {\"<connect id or pid>\": \"<agent token>\"}; must be mode 0600
  GPUI_AGENT_TOKEN        fallback agent token for any app without an entry in BIR_MCP_TOKENS
  GPUI_AGENT_RUNTIME_DIR  gpui-agent discovery directory (default: gpui-agent's per-user runtime dir)
  BUWIZ_ACCESS_TOKEN      stdio only. When set, every tool checks that token with
                          buwiz-server instead of the desktop session.
  BUWIZ_MCP_URL           origin for that stdio agent-token check. When unset, the
                          build's server address is used.
  BUWIZ_ACCOUNT_URL       authorization server for --http. Overrides the build.
  BUWIZ_SERVER_URL        API origin. Used for --http only when the account URL
                          was not configured.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        [] => serve_stdio(),
        ["login"] => run_login(),
        ["logout"] => run_logout(),
        ["status"] => run_status(),
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

fn licensed_server() -> Result<Server<AgentBirHost>, String> {
    let license = if let Some(token) = env_nonempty("BUWIZ_ACCESS_TOKEN") {
        let base =
            env_nonempty("BUWIZ_MCP_URL").unwrap_or_else(|| signin::builtin_origin().to_string());
        License::required(base, token)
    } else {
        let (origin, _) = signin::configured_origins()?;
        let Some(session) = signin::ensure_fresh(&origin, &DesktopStore, signin::unix_now())?
        else {
            return Err(bir_mcp::license::UNAUTHENTICATED.to_string());
        };
        License::desktop(origin, session.access_token, session.refresh_token, true)
    };
    Ok(Server::with_license(AgentBirHost::from_env(), license))
}

fn env_nonempty(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn run_login() -> ExitCode {
    let (api, account) = match signin::configured_origins() {
        Ok(origins) => origins,
        Err(error) => {
            eprintln!("bir-mcp: {error}");
            return ExitCode::from(2);
        }
    };
    match signin::login(
        &api,
        &account,
        &DesktopStore,
        LoginHooks {
            open_browser: signin::open_system_browser,
            now: signin::unix_now,
            sleep: std::thread::sleep,
            emit: |line: &str| {
                println!("{line}");
                let _ = std::io::Write::flush(&mut std::io::stdout());
            },
        },
    ) {
        Ok(_lines) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("bir-mcp: {error}");
            ExitCode::from(2)
        }
    }
}

fn run_logout() -> ExitCode {
    let origin = match signin::configured_origins() {
        Ok((api, _)) => api,
        Err(error) => {
            eprintln!("bir-mcp: {error}");
            return ExitCode::from(2);
        }
    };
    match signin::logout(&origin, &DesktopStore) {
        Ok(line) => {
            println!("{line}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("bir-mcp: {error}");
            ExitCode::from(2)
        }
    }
}

fn run_status() -> ExitCode {
    let origin = match signin::configured_origins() {
        Ok((api, _)) => api,
        Err(error) => {
            eprintln!("bir-mcp: {error}");
            return ExitCode::from(2);
        }
    };
    match signin::status(&origin, &DesktopStore, signin::unix_now()) {
        Ok(line) => {
            println!("{line}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("bir-mcp: {error}");
            ExitCode::from(2)
        }
    }
}

fn serve_stdio() -> ExitCode {
    let mut server = match licensed_server() {
        Ok(server) => server,
        Err(error) => {
            eprintln!("bir-mcp: {error}");
            return ExitCode::from(2);
        }
    };
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
    let listener = match http::bind(addr) {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("bir-mcp: {error}");
            return ExitCode::from(2);
        }
    };
    let issuer = match bir_mcp::oauth::configured_authorization_server() {
        Ok(issuer) => issuer,
        Err(error) => {
            eprintln!("bir-mcp: {error}");
            return ExitCode::from(2);
        }
    };
    let Some(address) = listener.server_addr().to_ip() else {
        eprintln!("bir-mcp: the listener has no socket address");
        return ExitCode::from(2);
    };
    let resource = bir_mcp::oauth::Resource::from_bind(address, &issuer);
    eprintln!("bir-mcp: serving MCP at {}", resource.mcp);
    eprintln!("bir-mcp: OAuth metadata at {}", resource.metadata);
    eprintln!(
        "bir-mcp: authorization server {}",
        resource.authorization_server
    );
    // Stdio is the desktop account. HTTP starts closed and admits each
    // request from its own bearer token.
    let mut server = Server::with_license(
        AgentBirHost::from_env(),
        License::settled(Err(bir_mcp::license::UNAUTHENTICATED.to_string())),
    );
    let auth = bir_mcp::oauth::Introspection::new(resource.authorization_server.clone());
    http::serve_http(&mut server, &listener, &auth, &resource);
    ExitCode::SUCCESS
}
