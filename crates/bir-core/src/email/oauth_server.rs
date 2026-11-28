//! Lightweight local HTTP server for receiving OAuth2 callbacks.
//!
//! Listens on 127.0.0.1 at a random port for Google's redirect, for at most [`CALLBACK_TIMEOUT`]. The first request
//! that carries the expected `state` decides: its `code`, or Google's `error` (the person pressed Cancel). Requests
//! that are not a callback (a browser's `/favicon.ico`) get a 404 and the wait goes on; a callback with another
//! state is refused. The receiver always gets exactly one answer, so the sign-in never waits forever.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use tracing::info;

/// How long the sign-in waits for Google's redirect.
pub const CALLBACK_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// What the person did in the browser, when the wait ended without a code.
pub const CALLBACK_TIMED_OUT: &str =
    "Google sign-in was not finished in the browser within 5 minutes";
pub const CALLBACK_CANCELLED: &str = "Google sign-in was cancelled in the browser";
pub const CALLBACK_MISMATCH: &str = "OAuth callback state was missing or did not match";

/// Start a single-shot HTTP server on a random port.
///
/// Returns `(port, receiver)` where `receiver` yields the authorization code after Google redirects with the
/// expected OAuth state, or an error: cancelled, a state that does not match, or the 5-minute timeout.
pub fn start_callback_server(
    expected_state: String,
) -> Result<(u16, mpsc::Receiver<Result<String, String>>), anyhow::Error> {
    start_callback_server_until(expected_state, CALLBACK_TIMEOUT)
}

fn start_callback_server_until(
    expected_state: String,
    timeout: Duration,
) -> Result<(u16, mpsc::Receiver<Result<String, String>>), anyhow::Error> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    listener.set_nonblocking(true)?;
    let port = listener.local_addr()?.port();
    let (tx, rx) = mpsc::channel::<Result<String, String>>();

    info!("OAuth callback server listening on 127.0.0.1:{}", port);

    std::thread::spawn(move || {
        let outcome = wait_for_callback(&listener, &expected_state, Instant::now() + timeout);
        let _ = tx.send(outcome);
        // Listener drops here, freeing the port
    });

    Ok((port, rx))
}

fn wait_for_callback(
    listener: &TcpListener,
    expected_state: &str,
    deadline: Instant,
) -> Result<String, String> {
    while Instant::now() < deadline {
        match listener.accept() {
            Ok((stream, _addr)) => {
                if let Some(outcome) = answer(stream, expected_state) {
                    return outcome;
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(error) => return Err(format!("OAuth callback server failed: {error}")),
        }
    }
    Err(CALLBACK_TIMED_OUT.to_string())
}

/// Answers one connection. `None` when it was not a callback, so the wait goes on.
fn answer(mut stream: TcpStream, expected_state: &str) -> Option<Result<String, String>> {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut buf = vec![0u8; 4096];
    let n = stream.read(&mut buf).unwrap_or(0);
    let request = String::from_utf8_lossy(&buf[..n]);
    match classify(&request, expected_state) {
        Callback::Code(code) => {
            respond(
                &mut stream,
                "200 OK",
                "text/html; charset=utf-8",
                SUCCESS_HTML,
            );
            Some(Ok(code))
        }
        Callback::Cancelled => {
            respond(
                &mut stream,
                "200 OK",
                "text/plain; charset=utf-8",
                "Google sign-in was cancelled. You can close this tab and return to eBIRForms.",
            );
            Some(Err(CALLBACK_CANCELLED.to_string()))
        }
        Callback::Mismatch => {
            respond(
                &mut stream,
                "400 Bad Request",
                "text/plain; charset=utf-8",
                "Invalid OAuth callback. Return to eBIRForms and try again.",
            );
            Some(Err(CALLBACK_MISMATCH.to_string()))
        }
        Callback::Other => {
            respond(
                &mut stream,
                "404 Not Found",
                "text/plain; charset=utf-8",
                "Not found",
            );
            None
        }
    }
}

fn respond(stream: &mut TcpStream, status: &str, content_type: &str, body: &str) {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

#[derive(Debug, PartialEq, Eq)]
enum Callback {
    Code(String),
    Cancelled,
    Mismatch,
    /// Not a callback (a browser's /favicon.ico): answered 404, and the wait goes on.
    Other,
}

/// What one request is: Google's redirect with our state (a code, or Cancel), one with another state, or something
/// else on the port.
fn classify(request: &str, expected_state: &str) -> Callback {
    let params = query_params(request);
    let state = params
        .iter()
        .find(|(key, _)| key == "state")
        .map(|(_, value)| value);
    let code = params
        .iter()
        .find(|(key, _)| key == "code")
        .map(|(_, value)| value);
    let error = params.iter().any(|(key, _)| key == "error");
    if state.is_none() && code.is_none() && !error {
        return Callback::Other;
    }
    if state.map(String::as_str) != Some(expected_state) {
        return Callback::Mismatch;
    }
    match code {
        Some(code) if !error && !code.is_empty() => Callback::Code(code.clone()),
        _ if error => Callback::Cancelled,
        _ => Callback::Mismatch,
    }
}

/// The decoded query parameters of an HTTP GET request line; a parameter that does not decode is dropped.
fn query_params(request: &str) -> Vec<(String, String)> {
    let Some(path) = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
    else {
        return Vec::new();
    };
    let Some((_, query)) = path.split_once('?') else {
        return Vec::new();
    };
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .filter_map(|(key, value)| {
            Some((
                key.to_string(),
                urlencoding::decode(value).ok()?.into_owned(),
            ))
        })
        .collect()
}

const SUCCESS_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Authorization Successful</title>
  <style>
    :root {
      --bg-color: #f8fafc;
      --card-bg: #ffffff;
      --text-main: #0f172a;
      --text-muted: #64748b;
      --success-color: #10b981;
      --ring-color: rgba(16, 185, 129, 0.15);
    }
    @media (prefers-color-scheme: dark) {
      :root {
        --bg-color: #0f172a;
        --card-bg: #1e293b;
        --text-main: #f8fafc;
        --text-muted: #94a3b8;
        --success-color: #34d399;
        --ring-color: rgba(52, 211, 153, 0.15);
      }
    }
    body {
      font-family: 'Inter', system-ui, -apple-system, sans-serif;
      background-color: var(--bg-color);
      color: var(--text-main);
      display: flex;
      justify-content: center;
      align-items: center;
      height: 100vh;
      margin: 0;
      overflow: hidden;
    }
    .container {
      background-color: var(--card-bg);
      padding: 3rem 4rem;
      border-radius: 24px;
      box-shadow: 0 20px 25px -5px rgba(0, 0, 0, 0.1), 0 8px 10px -6px rgba(0, 0, 0, 0.1);
      text-align: center;
      animation: slideUp 0.6s cubic-bezier(0.16, 1, 0.3, 1) forwards;
      opacity: 0;
      transform: translateY(20px);
      max-width: 400px;
      width: 90%;
      border: 1px solid rgba(255,255,255,0.05);
    }
    .icon-wrapper {
      width: 80px;
      height: 80px;
      background: var(--ring-color);
      border-radius: 50%;
      display: flex;
      justify-content: center;
      align-items: center;
      margin: 0 auto 1.5rem;
      animation: popIn 0.6s cubic-bezier(0.16, 1, 0.3, 1) 0.2s forwards;
      opacity: 0;
      transform: scale(0.8);
    }
    .icon-wrapper svg {
      width: 40px;
      height: 40px;
      color: var(--success-color);
    }
    h2 {
      font-size: 1.5rem;
      font-weight: 600;
      margin: 0 0 0.75rem 0;
      letter-spacing: -0.025em;
    }
    p {
      color: var(--text-muted);
      font-size: 1rem;
      line-height: 1.5;
      margin: 0;
    }
    @keyframes slideUp {
      to { opacity: 1; transform: translateY(0); }
    }
    @keyframes popIn {
      to { opacity: 1; transform: scale(1); }
    }
  </style>
</head>
<body>
  <div class="container">
    <div class="icon-wrapper">
      <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5">
        <path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7" />
      </svg>
    </div>
    <h2>Authentication Successful</h2>
    <p>You have successfully connected your account. You can now close this tab safely and return to the BIR application.</p>
  </div>
</body>
</html>"#;

#[cfg(test)]
mod tests {
    use super::*;

    const STATE: &str = "expected state";

    #[test]
    fn a_callback_with_our_state_gives_the_decoded_code() {
        let request = "GET /?code=a%2Fb&state=expected%20state&scope=x HTTP/1.1\r\n";
        assert_eq!(classify(request, STATE), Callback::Code("a/b".into()));
    }

    #[test]
    fn a_missing_or_other_state_is_refused() {
        assert_eq!(
            classify("GET /?code=authorization-code HTTP/1.1\r\n", STATE),
            Callback::Mismatch
        );
        assert_eq!(
            classify("GET /?code=c&state=other HTTP/1.1\r\n", STATE),
            Callback::Mismatch
        );
        assert_eq!(
            classify("GET /?error=access_denied&state=other HTTP/1.1\r\n", STATE),
            Callback::Mismatch
        );
    }

    #[test]
    fn cancel_in_the_browser_is_its_own_answer() {
        let request = "GET /?error=access_denied&state=expected%20state HTTP/1.1\r\n";
        assert_eq!(classify(request, STATE), Callback::Cancelled);
    }

    #[test]
    fn other_requests_are_not_callbacks() {
        assert_eq!(
            classify("GET /favicon.ico HTTP/1.1\r\n", STATE),
            Callback::Other
        );
        assert_eq!(
            classify("GET /?foo=bar HTTP/1.1\r\n", STATE),
            Callback::Other
        );
        assert_eq!(classify("", STATE), Callback::Other);
    }

    fn get(port: u16, path: &str) -> String {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(stream, "GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").unwrap();
        let mut response = String::new();
        let _ = stream.read_to_string(&mut response);
        response
    }

    #[test]
    fn the_server_waits_past_a_favicon_and_returns_the_code() {
        let (port, rx) = start_callback_server(STATE.into()).unwrap();
        assert!(get(port, "/favicon.ico").starts_with("HTTP/1.1 404"));
        assert!(get(port, "/?code=the-code&state=expected%20state").starts_with("HTTP/1.1 200"));
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(10)).unwrap(),
            Ok("the-code".into())
        );
    }

    #[test]
    fn the_server_reports_cancel() {
        let (port, rx) = start_callback_server(STATE.into()).unwrap();
        assert!(
            get(port, "/?error=access_denied&state=expected%20state").starts_with("HTTP/1.1 200")
        );
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(10)).unwrap(),
            Err(CALLBACK_CANCELLED.into())
        );
    }

    #[test]
    fn the_server_gives_up_after_its_timeout() {
        let (_port, rx) =
            start_callback_server_until(STATE.into(), Duration::from_millis(300)).unwrap();
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(10)).unwrap(),
            Err(CALLBACK_TIMED_OUT.into())
        );
    }
}
