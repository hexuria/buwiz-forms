//! Local mock of the BIR SFTP endpoint. `just mock-bir`, or
//! `cargo run -p bir-core --features mock-bir --bin mock_bir -- [options]`.
//! See docs/mock-bir.md. Listens on loopback only; never expose it.

use bir_core::mock_bir::{
    Behaviour, CONTROL_FILE, DEFAULT_PASSWORD, DEFAULT_PORT, DEFAULT_USERNAME, DelayAt, Failure,
    MockConfig, MockServer,
};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

const USAGE: &str = "\
mock_bir: a local mock of the BIR SFTP endpoint (loopback only)

Options:
  --port N          listening port (default 2222)
  --bind IP         127.0.0.1 (default) or ::1; anything else is refused
  --dir PATH        where uploads, receipts and the host key go
                    (default target/mock-bir)
  --user NAME       accepted username (default mock-filer)
  --password TEXT   accepted password (default mock-password-not-a-bir-secret)
  --fail MODE       none | refuse-login | fail-before-store | fail-after-store
  --delay SECS      be slow for SECS seconds ...
  --delay-at WHERE  ... at login (default) | open | close
  -h, --help        this text

A file named mock-bir.control in --dir is read at every new connection and
overrides --fail / --delay / --delay-at, for example:
  fail=fail-after-store
  delay=12
  delay_at=close";

fn parse_args() -> Result<MockConfig, String> {
    let mut config = MockConfig::new("target/mock-bir");
    let mut ip: IpAddr = Ipv4Addr::LOCALHOST.into();
    let mut port = DEFAULT_PORT;
    let mut behaviour = Behaviour::default();
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        if flag == "-h" || flag == "--help" {
            return Err(String::new());
        }
        let value = args.next().ok_or_else(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--port" => port = value.parse().map_err(|_| format!("bad --port {value}"))?,
            "--bind" => ip = value.parse().map_err(|_| format!("bad --bind {value}"))?,
            "--dir" => config.root = PathBuf::from(value),
            "--user" => config.username = value,
            "--password" => config.password = value,
            "--fail" => {
                behaviour.failure =
                    Failure::parse(&value).ok_or_else(|| format!("unknown --fail {value}"))?
            }
            "--delay" => {
                behaviour.delay = Duration::from_secs(
                    value
                        .parse()
                        .map_err(|_| format!("bad --delay {value} (whole seconds)"))?,
                )
            }
            "--delay-at" => {
                behaviour.delay_at =
                    DelayAt::parse(&value).ok_or_else(|| format!("unknown --delay-at {value}"))?
            }
            other => return Err(format!("unknown option {other}")),
        }
    }
    if !ip.is_loopback() {
        return Err(format!(
            "refusing --bind {ip}: the mock listens on loopback only"
        ));
    }
    config.bind = SocketAddr::new(ip, port);
    config.behaviour = behaviour;
    config.echo = true;
    Ok(config)
}

#[tokio::main]
async fn main() -> ExitCode {
    let config = match parse_args() {
        Ok(config) => config,
        Err(error) if error.is_empty() => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(error) => {
            eprintln!("mock_bir: {error}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let username = config.username.clone();
    let password = config.password.clone();
    let behaviour = config.behaviour;
    let server = match MockServer::start(config).await {
        Ok(server) => server,
        Err(error) => {
            eprintln!("mock_bir: could not start: {error}");
            return ExitCode::from(1);
        }
    };
    let root = std::fs::canonicalize(server.root()).unwrap_or_else(|_| server.root().into());
    let addr = server.addr();
    println!("Mock BIR SFTP server (nothing here reaches the BIR)");
    println!("  listening   {addr} (loopback only; never expose it)");
    println!("  uploads     {}", root.join("uploads").display());
    println!("  receipts    {}", root.join("receipts").display());
    println!("  control     {}", root.join(CONTROL_FILE).display());
    println!("  behaviour   {}", behaviour.describe());
    println!();
    // `just mock-bir-app` reads these; the values are the mock's fakes.
    let env = [
        format!("BIR_SFTP_HOST={}", addr.ip()),
        format!("BIR_SFTP_PORT={}", addr.port()),
        format!("BIR_SFTP_USERNAME={username}"),
        format!("BIR_SFTP_PASSWORD={password}"),
        format!("BIR_SFTP_HOST_KEY_SHA256={}", server.host_key_sha256()),
    ];
    let env_file = root.join("app.env");
    if let Err(error) = std::fs::write(&env_file, env.join("\n") + "\n") {
        eprintln!("mock_bir: could not write {}: {error}", env_file.display());
    }
    println!("Point the app at it with (also in {}):", env_file.display());
    for line in &env {
        println!("  {line}");
    }
    if username != DEFAULT_USERNAME || password != DEFAULT_PASSWORD {
        println!("  (custom credentials)");
    }
    println!();
    println!("Waiting for connections. Ctrl-C stops the server.");
    match tokio::signal::ctrl_c().await {
        Ok(()) => {
            let _ = std::fs::remove_file(&env_file);
            println!("[mock-bir] stopped");
        }
        Err(error) => eprintln!("[mock-bir] signal handler failed: {error}"),
    }
    ExitCode::SUCCESS
}
