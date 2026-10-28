//! `localhost` over IPv4 first, for the HTTP clients that talk to
//! buwiz-server.
//!
//! A development buwiz-server listens on `127.0.0.1` only, while BIR is
//! configured with `http://localhost:<port>`. On macOS `localhost` resolves
//! to `::1` first, and a refused IPv6 connect was the end of the request
//! ("could not reach buwiz-server ... error sending request"), where curl
//! falls back. Every such client resolves `localhost` itself instead: to
//! `127.0.0.1`, then `::1`, so a server on either loopback answers. The
//! configured and stored URLs keep saying `localhost` (sign-in compares
//! them as written); only name resolution changes.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

/// `localhost`'s addresses in the order they are tried. Port 0: the URL's
/// own port is used.
pub const LOCALHOST: [SocketAddr; 2] = [
    SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
    SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0),
];

/// `builder` with `localhost` resolved to [`LOCALHOST`].
pub fn localhost_ipv4_first(
    builder: reqwest::blocking::ClientBuilder,
) -> reqwest::blocking::ClientBuilder {
    builder.resolve_to_addrs("localhost", &LOCALHOST)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// A server bound to 127.0.0.1 only answers `http://localhost:<port>`.
    #[test]
    fn localhost_reaches_a_server_on_ipv4_loopback_only() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 1024];
            let _ = stream.read(&mut request);
            stream
                .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok")
                .unwrap();
        });
        let client = localhost_ipv4_first(reqwest::blocking::Client::builder())
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap();
        let response = client
            .get(format!("http://localhost:{port}/"))
            .send()
            .expect("localhost reaches 127.0.0.1");
        assert_eq!(response.status(), 200);
        assert_eq!(response.text().unwrap(), "ok");
        server.join().unwrap();
    }
}
