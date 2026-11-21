//! In-memory account server for local development; never expose it publicly.

use bir_core::mock_account::MockAccount;
use std::net::{IpAddr, SocketAddr};
use std::process::ExitCode;

const USAGE: &str = "mock_account: local development accounts (no passwords, no persistence)
  --bind IP        listening IP (MOCK_ACCOUNT_HOST, default 127.0.0.1; allows 0.0.0.0)
  --port N         listening port (MOCK_ACCOUNT_PORT, default 3008)
  --auto-approve   approve device codes immediately as dev@buwiz.local
  -h, --help       this text
Never expose this server publicly or use real credentials.";

fn args() -> Result<Option<(SocketAddr, bool)>, String> {
    let mut host = std::env::var("MOCK_ACCOUNT_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let mut port = std::env::var("MOCK_ACCOUNT_PORT").unwrap_or_else(|_| "3008".into());
    let mut auto = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bind" => host = args.next().ok_or("--bind needs an IP")?,
            "--port" => port = args.next().ok_or("--port needs a number")?,
            "--auto-approve" => auto = true,
            "-h" | "--help" => return Ok(None),
            _ => return Err(format!("unknown option {arg}")),
        }
    }
    let host: IpAddr = host.parse().map_err(|_| "invalid listening IP")?;
    let port: u16 = port.parse().map_err(|_| "invalid listening port")?;
    Ok(Some((SocketAddr::new(host, port), auto)))
}

#[tokio::main]
async fn main() -> ExitCode {
    let (addr, auto) = match args() {
        Ok(Some(args)) => args,
        Ok(None) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(error) => {
            eprintln!("{error}\n{USAGE}");
            return ExitCode::FAILURE;
        }
    };
    let server = match MockAccount::start(addr, auto).await {
        Ok(server) => server,
        Err(error) => {
            eprintln!("mock_account: {error}");
            return ExitCode::FAILURE;
        }
    };
    println!(
        "[mock-account] listening on {} (auto-approve: {auto})",
        server.url()
    );
    println!("[mock-account] DEVELOPMENT ONLY: no passwords; all state disappears on exit");
    let _ = tokio::signal::ctrl_c().await;
    ExitCode::SUCCESS
}
