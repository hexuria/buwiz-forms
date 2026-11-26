//! How long an HTTP request the submission worker makes may take. The worker's calendar sync and the inbox poll's
//! token refresh run while a database handle is held; reqwest's default client waits for ever (no connect, read or
//! total timeout), so a stalled server would keep the owner lock past an engine's end.

use std::time::Duration;

/// Time to open the connection.
pub const CONNECT: Duration = Duration::from_secs(10);
/// Time for the whole request, the body included.
pub const TOTAL: Duration = Duration::from_secs(30);

/// A blocking client that gives up after [`CONNECT`] to connect and [`TOTAL`] in all.
pub fn bounded_blocking_client() -> reqwest::blocking::Client {
    bounded(CONNECT, TOTAL)
}

fn bounded(connect: Duration, total: Duration) -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .connect_timeout(connect)
        .timeout(total)
        .build()
        // The builder fails only where the TLS backend cannot start, which `Client::new` reports the same way.
        .unwrap_or_else(|_| reqwest::blocking::Client::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A server that takes the connection and never answers: the request ends with a timeout error at the total, not
    /// never (the default client's wait).
    #[test]
    fn a_server_that_never_answers_ends_the_request_at_the_total() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let held = std::thread::spawn(move || listener.accept().map(|(stream, _)| stream));
        let started = std::time::Instant::now();
        let error = bounded(Duration::from_secs(1), Duration::from_millis(400))
            .get(format!("http://{address}/"))
            .send()
            .expect_err("no answer");
        assert!(error.is_timeout(), "{error}");
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "ended at the total"
        );
        drop(held.join());
    }
}
