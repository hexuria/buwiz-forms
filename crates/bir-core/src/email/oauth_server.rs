//! Lightweight local HTTP server for receiving OAuth2 callbacks.
//!
//! Starts on a random available port, waits for a single request from
//! Google's redirect, extracts the `code` parameter, and shuts down.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use tracing::info;

/// Start a single-shot HTTP server on a random port.
///
/// Returns `(port, receiver)` where `receiver` will yield the authorization code
/// once Google redirects the browser back.
pub fn start_callback_server() -> Result<(u16, mpsc::Receiver<String>), anyhow::Error> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    let (tx, rx) = mpsc::channel::<String>();

    info!("OAuth callback server listening on 127.0.0.1:{}", port);

    // Spawn a thread that handles exactly one request then exits
    std::thread::spawn(move || {
        // Accept one connection (with a 5-minute timeout)
        listener
            .set_nonblocking(false)
            .expect("set_nonblocking failed");

        if let Ok((mut stream, _addr)) = listener.accept() {
            let mut buf = vec![0u8; 4096];
            let n = stream.read(&mut buf).unwrap_or(0);
            let request = String::from_utf8_lossy(&buf[..n]);

            // Extract the `code` query parameter from GET /?code=...&scope=...
            if let Some(code) = extract_code(&request) {
                // Send a friendly HTML response
                let html = r#"<!DOCTYPE html>
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
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    html.len(),
                    html
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
                let _ = tx.send(code);
            } else {
                let body = "Missing authorization code.";
                let response = format!(
                    "HTTP/1.1 400 Bad Request\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes());
            }
        }

        // Listener drops here, freeing the port
    });

    Ok((port, rx))
}

/// Parse the `code` query parameter from an HTTP GET request line.
fn extract_code(request: &str) -> Option<String> {
    // GET /?code=XXXX&scope=...  HTTP/1.1
    let first_line = request.lines().next()?;
    let path = first_line.split_whitespace().nth(1)?;
    let query = path.split('?').nth(1)?;

    for pair in query.split('&') {
        let mut kv = pair.splitn(2, '=');
        if kv.next()? == "code" {
            return kv
                .next()
                .map(|v| urlencoding::decode(v).unwrap_or_default().into_owned());
        }
    }
    None
}
