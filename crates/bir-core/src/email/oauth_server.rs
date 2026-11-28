//! Lightweight local HTTP server for receiving OAuth2 callbacks.
//!
//! Listens on 127.0.0.1 at a random port for Google's redirect, for at most [`CALLBACK_TIMEOUT`]. Only a `GET /`
//! that carries the expected `state` decides: its `code`, Google's Cancel, or another refusal from Google. Anything
//! else on the port (a browser's `/favicon.ico`, another method or path, a callback with some other state) gets an
//! error page and the wait goes on, so a stray or hostile local request cannot end the sign-in. The receiver always
//! gets exactly one answer, so the sign-in never waits forever.

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

/// How much of a request is read: the request line and headers fit well inside it.
const REQUEST_LIMIT: usize = 4096;

/// How long one connection may take to send its request or read the answer.
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(5);

/// Start a single-shot HTTP server on a random port.
///
/// Returns `(port, receiver)` where `receiver` yields the authorization code after Google redirects with the
/// expected OAuth state, or an error: cancelled, refused by Google, or the 5-minute timeout.
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

/// Answers one connection. `None` when it did not decide the sign-in, so the wait goes on.
fn answer(mut stream: TcpStream, expected_state: &str) -> Option<Result<String, String>> {
    // An accepted socket inherits the listener's non-blocking mode on macOS and Windows (not on Linux): it must block
    // with timeouts, or a read before the request arrives gives nothing and the real callback would be refused.
    if let Err(error) = stream
        .set_nonblocking(false)
        .and_then(|()| stream.set_read_timeout(Some(CONNECTION_TIMEOUT)))
        .and_then(|()| stream.set_write_timeout(Some(CONNECTION_TIMEOUT)))
    {
        return Some(Err(format!("OAuth callback server failed: {error}")));
    }
    let request = read_request(&mut stream);
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
        Callback::Refused(error) => {
            respond(
                &mut stream,
                "200 OK",
                "text/plain; charset=utf-8",
                "Google did not allow the sign-in. You can close this tab and return to eBIRForms.",
            );
            Some(Err(format!("Google refused the sign-in ({error})")))
        }
        Callback::WrongState => {
            respond(
                &mut stream,
                "400 Bad Request",
                "text/plain; charset=utf-8",
                "Invalid OAuth callback. Return to eBIRForms and try again.",
            );
            None
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

/// The request's line and headers: reads until the blank line that ends them, the peer closes, the read times out,
/// or [`REQUEST_LIMIT`] bytes, so a request that arrives in several pieces is read whole.
fn read_request(stream: &mut TcpStream) -> String {
    let mut request = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    while request.len() < REQUEST_LIMIT && !request.windows(4).any(|window| window == b"\r\n\r\n") {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => request.extend_from_slice(&chunk[..n.min(REQUEST_LIMIT - request.len())]),
        }
    }
    String::from_utf8_lossy(&request).into_owned()
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
    /// Google's `error=access_denied`: the person pressed Cancel.
    Cancelled,
    /// Any other `error=` from Google (an administrator's policy, an unknown scope): its error code only.
    Refused(String),
    /// A callback whose state is not ours: answered 400, and the wait goes on.
    WrongState,
    /// Not a callback (a browser's /favicon.ico, another method or path): answered 404, and the wait goes on.
    Other,
}

/// What one request is: Google's redirect to `GET /` with our state (a code, Cancel or another refusal), a callback
/// with another state, or something else on the port.
fn classify(request: &str, expected_state: &str) -> Callback {
    let mut request_line = request
        .lines()
        .next()
        .unwrap_or_default()
        .split_whitespace();
    let (Some("GET"), Some(target)) = (request_line.next(), request_line.next()) else {
        return Callback::Other;
    };
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    if path != "/" {
        return Callback::Other;
    }
    let params = query_params(query);
    let value = |name: &str| {
        params
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    };
    let (state, code, error) = (value("state"), value("code"), value("error"));
    if state.is_none() && code.is_none() && error.is_none() {
        return Callback::Other;
    }
    if state != Some(expected_state) {
        return Callback::WrongState;
    }
    match (code, error) {
        (_, Some("access_denied")) => Callback::Cancelled,
        (_, Some(error)) => Callback::Refused(error_code(error)),
        (Some(code), None) if !code.is_empty() => Callback::Code(code.to_string()),
        _ => Callback::WrongState,
    }
}

/// Google's error code as a short, plain word (it names the reason, never anything about the person).
fn error_code(error: &str) -> String {
    let code: String = error
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .take(40)
        .collect();
    if code.is_empty() {
        "unknown".into()
    } else {
        code
    }
}

/// The decoded parameters of a query string; a parameter that does not decode is dropped.
fn query_params(query: &str) -> Vec<(String, String)> {
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
    fn a_missing_or_other_state_is_not_ours() {
        for request in [
            "GET /?code=authorization-code HTTP/1.1\r\n",
            "GET /?code=c&state=other HTTP/1.1\r\n",
            "GET /?error=access_denied&state=other HTTP/1.1\r\n",
            "GET /?state=expected%20state HTTP/1.1\r\n",
        ] {
            assert_eq!(classify(request, STATE), Callback::WrongState, "{request}");
        }
    }

    #[test]
    fn cancel_and_other_refusals_are_told_apart() {
        let cancel = "GET /?error=access_denied&state=expected%20state HTTP/1.1\r\n";
        assert_eq!(classify(cancel, STATE), Callback::Cancelled);
        let policy = "GET /?error=admin_policy_enforced&state=expected%20state HTTP/1.1\r\n";
        assert_eq!(
            classify(policy, STATE),
            Callback::Refused("admin_policy_enforced".into())
        );
        let odd = "GET /?error=%3Cscript%3E%20x&state=expected%20state HTTP/1.1\r\n";
        assert_eq!(classify(odd, STATE), Callback::Refused("scriptx".into()));
    }

    #[test]
    fn other_requests_are_not_callbacks() {
        for request in [
            "GET /favicon.ico HTTP/1.1\r\n",
            "GET /?foo=bar HTTP/1.1\r\n",
            "GET /other?code=c&state=expected%20state HTTP/1.1\r\n",
            "POST /?code=c&state=expected%20state HTTP/1.1\r\n",
            "GET",
            "",
        ] {
            assert_eq!(classify(request, STATE), Callback::Other, "{request:?}");
        }
    }

    /// Sends `pieces` as separate writes, `pause` apart, and returns the answer.
    fn send(port: u16, pieces: &[&str], pause: Duration) -> String {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream.set_nodelay(true).unwrap();
        for (index, piece) in pieces.iter().enumerate() {
            if index > 0 {
                std::thread::sleep(pause);
            }
            stream.write_all(piece.as_bytes()).unwrap();
        }
        let mut response = String::new();
        let _ = stream.read_to_string(&mut response);
        response
    }

    fn get(port: u16, target: &str) -> String {
        let request = format!("GET {target} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n");
        send(port, &[&request], Duration::ZERO)
    }

    fn outcome(rx: &mpsc::Receiver<Result<String, String>>) -> Result<String, String> {
        rx.recv_timeout(Duration::from_secs(10)).unwrap()
    }

    #[test]
    fn the_server_waits_past_a_favicon_and_returns_the_code() {
        let (port, rx) = start_callback_server(STATE.into()).unwrap();
        assert!(get(port, "/favicon.ico").starts_with("HTTP/1.1 404"));
        assert!(get(port, "/?code=the-code&state=expected%20state").starts_with("HTTP/1.1 200"));
        assert_eq!(outcome(&rx), Ok("the-code".into()));
    }

    #[test]
    fn a_request_split_across_writes_is_read_whole() {
        let (port, rx) = start_callback_server(STATE.into()).unwrap();
        let answer = send(
            port,
            &[
                "GET /?code=the-co",
                "de&state=expected%20st",
                "ate HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",
            ],
            Duration::from_millis(300),
        );
        assert!(answer.starts_with("HTTP/1.1 200"), "{answer}");
        assert_eq!(outcome(&rx), Ok("the-code".into()));
    }

    #[test]
    fn a_callback_with_another_state_does_not_end_the_sign_in() {
        let (port, rx) = start_callback_server(STATE.into()).unwrap();
        assert!(get(port, "/?code=stolen&state=guess").starts_with("HTTP/1.1 400"));
        assert!(rx.try_recv().is_err(), "still waiting");
        assert!(get(port, "/?code=the-code&state=expected%20state").starts_with("HTTP/1.1 200"));
        assert_eq!(outcome(&rx), Ok("the-code".into()));
    }

    #[test]
    fn the_server_reports_cancel_and_refusal() {
        let (port, rx) = start_callback_server(STATE.into()).unwrap();
        assert!(
            get(port, "/?error=access_denied&state=expected%20state").starts_with("HTTP/1.1 200")
        );
        assert_eq!(outcome(&rx), Err(CALLBACK_CANCELLED.into()));
        let (port, rx) = start_callback_server(STATE.into()).unwrap();
        assert!(
            get(port, "/?error=org_internal&state=expected%20state").starts_with("HTTP/1.1 200")
        );
        assert_eq!(
            outcome(&rx),
            Err("Google refused the sign-in (org_internal)".into())
        );
    }

    #[test]
    fn the_server_gives_up_after_its_timeout() {
        let (_port, rx) =
            start_callback_server_until(STATE.into(), Duration::from_millis(300)).unwrap();
        assert_eq!(outcome(&rx), Err(CALLBACK_TIMED_OUT.into()));
    }
}
