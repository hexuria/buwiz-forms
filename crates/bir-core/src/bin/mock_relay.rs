//! Local mock of the Buwiz relay. `just mock-relay`, or
//! `cargo run -p bir-core --features mock-relay --bin mock_relay -- [options]`.
//! See docs/relay.md. Listens on loopback only; never expose it.

use bir_core::mock_relay::{DEFAULT_PORT, MockBirTarget, MockRelay, RelayConfig, RelayFailure};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "\
mock_relay: a local mock of the Buwiz relay (loopback only)

Options:
  --port N          listening port (default 8787)
  --bind IP         127.0.0.1 (default) or ::1; anything else is refused
  --dir PATH        where inbox/ and receipts/ go (default target/mock-relay)
  --mock-bir FILE   the app.env of a running `just mock-bir` to hand files on
                    to (default target/mock-bir/app.env when it exists)
  --no-forward      keep files in the inbox only
  --fail MODE       none | reject | unavailable
  -h, --help        this text";

fn parse_args() -> Result<(SocketAddr, RelayConfig), String> {
    let mut ip: IpAddr = Ipv4Addr::LOCALHOST.into();
    let mut port = DEFAULT_PORT;
    let mut root = PathBuf::from("target/mock-relay");
    let mut env_file: Option<PathBuf> = None;
    let mut forward = true;
    let mut failure = RelayFailure::None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or_else(|| format!("{arg} needs a value"));
        match arg.as_str() {
            "--port" => {
                port = value()?
                    .parse()
                    .map_err(|_| "--port needs a number".to_string())?
            }
            "--bind" => {
                ip = value()?
                    .parse()
                    .map_err(|_| "--bind needs an IP".to_string())?
            }
            "--dir" => root = PathBuf::from(value()?),
            "--mock-bir" => env_file = Some(PathBuf::from(value()?)),
            "--no-forward" => forward = false,
            "--fail" => {
                failure =
                    RelayFailure::parse(&value()?).ok_or("--fail: none | reject | unavailable")?
            }
            "-h" | "--help" => return Err(USAGE.to_string()),
            other => return Err(format!("unknown option {other}\n\n{USAGE}")),
        }
    }
    if !ip.is_loopback() {
        return Err(format!(
            "--bind {ip} is not loopback; the mock relay never listens elsewhere"
        ));
    }
    let mock_bir = if !forward {
        None
    } else {
        let explicit = env_file.is_some();
        let file = env_file.unwrap_or_else(|| PathBuf::from("target/mock-bir/app.env"));
        if explicit || file.exists() {
            Some(MockBirTarget::from_app_env(&file)?)
        } else {
            None
        }
    };
    Ok((
        SocketAddr::new(ip, port),
        RelayConfig {
            root,
            failure,
            mock_bir,
        },
    ))
}

#[tokio::main]
async fn main() -> ExitCode {
    let (addr, config) = match parse_args() {
        Ok(parsed) => parsed,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::FAILURE;
        }
    };
    let root = config.root.clone();
    let forward = match &config.mock_bir {
        Some(mock) => format!("the mock BIR server at {}:{}", mock.host, mock.port),
        None => "nothing (files stay in the inbox)".to_string(),
    };
    let failure = config.failure;
    let relay = match MockRelay::start(addr, config).await {
        Ok(relay) => relay,
        Err(error) => {
            eprintln!("mock_relay: cannot listen on {addr}: {error}");
            return ExitCode::FAILURE;
        }
    };
    println!("[mock-relay] listening on {}", relay.url());
    println!("  inbox       {}", root.join("inbox").display());
    println!("  receipts    {}", root.join("receipts").display());
    println!("  hands on to {forward}");
    println!("  behaviour   {failure:?}");
    println!();
    println!("Point the app at it with:");
    println!("  BIR_RELAY_URL={}", relay.url());
    println!();
    println!("Waiting for submissions. Ctrl-C stops the relay.");
    let _ = tokio::signal::ctrl_c().await;
    println!("[mock-relay] stopped");
    ExitCode::SUCCESS
}
