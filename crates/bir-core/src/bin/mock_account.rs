//! In-memory account server for local development; never expose it publicly.

use bir_core::mock_account::{MockAccount, MockOptions};
use std::net::{IpAddr, SocketAddr};
use std::process::ExitCode;
use std::time::Duration;

const USAGE: &str = "mock_account: local development accounts (no passwords, no persistence)
  --bind IP            listening IP (MOCK_ACCOUNT_HOST, default 127.0.0.1; allows 0.0.0.0)
  --port N             listening port (MOCK_ACCOUNT_PORT, default 3008)
  --auto-approve       approve device codes immediately as dev@buwiz.local
  --code DIGITS        the first email sign-in code; each later one (a new request or a Resend) counts up (default 246810)
  --random-codes       a new random code each time instead
  --code-ttl SECONDS   how long a code works (default 300)
  --resend-wait SECONDS  how long until Resend is allowed (default: the code's life)
  --wrong-codes        every code is wrong (to see the wrong-code answer)
  --expired-codes      every code has expired when it is checked
  --no-email           the email sign-in is not offered (404): the app uses the browser
  --ban EMAIL          that address's account is banned (repeatable)
  --html               the email sign-in answers a challenge page (HTML), as a rule in front of a server can
  --html-429           the email sign-in answers the outer rate limit's HTML 429
  --busy-verify        the code check answers 503 busy (the server's waiting places are full)
  --network-starts N   start and resend calls one network may make in an hour (default 40)
  --network-verifies N code checks one network may make in an hour (default 60)
  -h, --help           this text
Never expose this server publicly or use real credentials.";

fn seconds(text: Option<String>, flag: &str) -> Result<Duration, String> {
    text.ok_or(format!("{flag} needs a number of seconds"))?
        .parse::<u64>()
        .map(Duration::from_secs)
        .map_err(|_| format!("{flag} needs a number of seconds"))
}

fn count(text: Option<String>, flag: &str) -> Result<usize, String> {
    text.and_then(|t| t.parse::<usize>().ok())
        .ok_or(format!("{flag} needs a number"))
}

fn args() -> Result<Option<(SocketAddr, MockOptions)>, String> {
    let mut host = std::env::var("MOCK_ACCOUNT_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let mut port = std::env::var("MOCK_ACCOUNT_PORT").unwrap_or_else(|_| "3008".into());
    let mut options = MockOptions::default();
    let mut resend_wait = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bind" => host = args.next().ok_or("--bind needs an IP")?,
            "--port" => port = args.next().ok_or("--port needs a number")?,
            "--auto-approve" => options.auto_approve = true,
            "--code" => {
                let code = args.next().ok_or("--code needs six digits")?;
                if code.len() != 6 || !code.chars().all(|c| c.is_ascii_digit()) {
                    return Err("--code needs six digits".into());
                }
                options.code = Some(code);
            }
            "--random-codes" => options.code = None,
            "--code-ttl" => options.code_ttl = seconds(args.next(), "--code-ttl")?,
            "--resend-wait" => resend_wait = Some(seconds(args.next(), "--resend-wait")?),
            "--wrong-codes" => options.wrong_codes = true,
            "--expired-codes" => options.expired_codes = true,
            "--no-email" => options.no_email = true,
            "--ban" => options
                .banned
                .push(args.next().ok_or("--ban needs an email")?),
            "--html" => options.html = true,
            "--html-429" => options.html_429 = true,
            "--busy-verify" => options.busy_verify = true,
            "--network-starts" => {
                options.start_per_network = count(args.next(), "--network-starts")?
            }
            "--network-verifies" => {
                options.verify_per_network = count(args.next(), "--network-verifies")?
            }
            "-h" | "--help" => return Ok(None),
            _ => return Err(format!("unknown option {arg}")),
        }
    }
    options.resend_wait = resend_wait.unwrap_or(options.code_ttl);
    let host: IpAddr = host.parse().map_err(|_| "invalid listening IP")?;
    let port: u16 = port.parse().map_err(|_| "invalid listening port")?;
    Ok(Some((SocketAddr::new(host, port), options)))
}

#[tokio::main]
async fn main() -> ExitCode {
    let (addr, options) = match args() {
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
    let summary = format!(
        "auto-approve: {}, email sign-in: {}, code: {}, code life: {}s, resend after: {}s",
        options.auto_approve,
        if options.no_email { "off (404)" } else { "on" },
        options
            .code
            .as_deref()
            .map_or("random".to_owned(), |first| format!(
                "{first} and up, one per request"
            )),
        options.code_ttl.as_secs(),
        options.resend_wait.as_secs()
    );
    let server = match MockAccount::start_with(addr, options).await {
        Ok(server) => server,
        Err(error) => {
            eprintln!("mock_account: {error}");
            return ExitCode::FAILURE;
        }
    };
    println!("[mock-account] listening on {} ({summary})", server.url());
    println!(
        "[mock-account] each email sign-in code is printed below as it is sent (\"sign-in code for <email>: <code>\")"
    );
    println!("[mock-account] DEVELOPMENT ONLY: no passwords; all state disappears on exit");
    let _ = tokio::signal::ctrl_c().await;
    ExitCode::SUCCESS
}
